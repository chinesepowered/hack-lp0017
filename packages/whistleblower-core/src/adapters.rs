use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::envelope::DocumentEnvelope;

/// Adapter for Logos Storage. Wraps the real `liblogosstorage` client in
/// production; an in-memory mock is provided for tests.
#[async_trait]
pub trait StorageAdapter: Send + Sync {
    /// Upload bytes and return the resulting CID + reported size.
    /// Implementations SHOULD treat already-present content as a no-op.
    async fn put(&self, bytes: &[u8], content_type: Option<&str>) -> anyhow::Result<UploadOutput>;
    /// Optional fetch — used by the demo script to verify round-trips.
    async fn get(&self, _cid: &str) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("get not supported by this storage adapter")
    }
}

#[derive(Debug, Clone)]
pub struct UploadOutput {
    pub cid: String,
    pub size_bytes: u64,
}

/// Adapter for Logos Delivery — gossip/pubsub used to broadcast envelopes.
#[async_trait]
pub trait DeliveryAdapter: Send + Sync {
    /// Publish an envelope to a topic. MUST be idempotent on (topic, cid).
    async fn publish(&self, topic: &str, envelope: &DocumentEnvelope) -> anyhow::Result<()>;

    /// Subscribe and invoke `handler` on each received envelope. Returns
    /// a handle that, when dropped or `unsubscribe`d, stops delivery.
    async fn subscribe(
        &self,
        topic: &str,
        handler: EnvelopeHandler,
    ) -> anyhow::Result<SubscriptionHandle>;
}

pub type EnvelopeHandler =
    Arc<dyn Fn(DocumentEnvelope) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub struct SubscriptionHandle {
    /// Adapter-supplied teardown callback.
    pub unsubscribe: Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send>,
}

impl SubscriptionHandle {
    pub async fn close(self) {
        (self.unsubscribe)().await;
    }
}

/// On-chain registry adapter — either a LEZ program client (default) or
/// a zone-SDK consensus-layer submitter.
#[async_trait]
pub trait AnchorAdapter: Send + Sync {
    async fn anchor_single(&self, entry: AnchorEntry) -> anyhow::Result<AnchorReceipt>;
    /// MUST be idempotent — re-submitting an already-registered CID is required to succeed.
    async fn anchor_batch(&self, entries: Vec<AnchorEntry>) -> anyhow::Result<AnchorReceipt>;
    async fn is_anchored(&self, cid: &str) -> anyhow::Result<bool>;
}

#[derive(Debug, Clone)]
pub struct AnchorEntry {
    pub cid: String,
    pub metadata_hash: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct AnchorReceipt {
    pub tx: String,
    pub newly_anchored: u32,
    /// Reported compute units, if the underlying chain exposes them.
    pub compute_units: Option<u64>,
}

// =========================================================================
// In-memory reference implementations — used in tests and the demo script.
// =========================================================================
pub mod in_memory {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;
    use tokio::sync::broadcast;

    /// In-memory content-addressed storage. CID is `bafy<hex(sha256[:28])>`.
    pub struct InMemoryStorage {
        inner: Mutex<HashMap<String, Vec<u8>>>,
    }
    impl InMemoryStorage {
        pub fn new() -> Self {
            Self {
                inner: Mutex::new(HashMap::new()),
            }
        }
    }
    impl Default for InMemoryStorage {
        fn default() -> Self {
            Self::new()
        }
    }
    #[async_trait]
    impl StorageAdapter for InMemoryStorage {
        async fn put(&self, bytes: &[u8], _ct: Option<&str>) -> anyhow::Result<UploadOutput> {
            let mut h = Sha256::new();
            h.update(bytes);
            let digest = h.finalize();
            let cid = format!("bafy{}", &hex::encode(&digest[..28]));
            self.inner
                .lock()
                .unwrap()
                .entry(cid.clone())
                .or_insert_with(|| bytes.to_vec());
            Ok(UploadOutput {
                cid,
                size_bytes: bytes.len() as u64,
            })
        }
        async fn get(&self, cid: &str) -> anyhow::Result<Vec<u8>> {
            self.inner
                .lock()
                .unwrap()
                .get(cid)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("not found"))
        }
    }

    /// Per-topic state: a broadcast sender plus the set of already-seen CIDs.
    type TopicState = (broadcast::Sender<DocumentEnvelope>, HashSet<String>);

    /// In-memory delivery with topic-keyed broadcast channels. Deduplicates
    /// re-published (topic, cid) tuples to mirror the real Logos Delivery
    /// "no duplicates visible to subscribers" semantics.
    pub struct InMemoryDelivery {
        topics: Mutex<HashMap<String, TopicState>>,
    }
    impl InMemoryDelivery {
        pub fn new() -> Self {
            Self {
                topics: Mutex::new(HashMap::new()),
            }
        }
    }
    impl Default for InMemoryDelivery {
        fn default() -> Self {
            Self::new()
        }
    }
    #[async_trait]
    impl DeliveryAdapter for InMemoryDelivery {
        async fn publish(&self, topic: &str, envelope: &DocumentEnvelope) -> anyhow::Result<()> {
            let mut t = self.topics.lock().unwrap();
            let (tx, seen) = t.entry(topic.to_string()).or_insert_with(|| {
                let (tx, _) = broadcast::channel(1024);
                (tx, HashSet::new())
            });
            if seen.contains(&envelope.cid) {
                return Ok(());
            }
            seen.insert(envelope.cid.clone());
            let _ = tx.send(envelope.clone());
            Ok(())
        }
        async fn subscribe(
            &self,
            topic: &str,
            handler: EnvelopeHandler,
        ) -> anyhow::Result<SubscriptionHandle> {
            let mut rx = {
                let mut t = self.topics.lock().unwrap();
                let (tx, _) = t.entry(topic.to_string()).or_insert_with(|| {
                    let (tx, _) = broadcast::channel(1024);
                    (tx, HashSet::new())
                });
                tx.subscribe()
            };
            let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = &mut stop_rx => break,
                        msg = rx.recv() => match msg {
                            Ok(env) => handler(env).await,
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        }
                    }
                }
            });
            Ok(SubscriptionHandle {
                unsubscribe: Box::new(move || {
                    Box::pin(async move {
                        let _ = stop_tx.send(());
                    })
                }),
            })
        }
    }

    /// In-memory registry. Models CU cost roughly as `5_000 + 2_000*n` to
    /// match the synthetic numbers the benchmark script reports when the
    /// real LEZ devnet isn't available.
    pub struct InMemoryAnchor {
        inner: Mutex<HashMap<String, ([u8; 32], u64)>>,
        next_tx: Mutex<u64>,
    }
    impl InMemoryAnchor {
        pub fn new() -> Self {
            Self {
                inner: Mutex::new(HashMap::new()),
                next_tx: Mutex::new(1),
            }
        }
    }
    impl Default for InMemoryAnchor {
        fn default() -> Self {
            Self::new()
        }
    }
    #[async_trait]
    impl AnchorAdapter for InMemoryAnchor {
        async fn anchor_single(&self, entry: AnchorEntry) -> anyhow::Result<AnchorReceipt> {
            self.anchor_batch(vec![entry]).await
        }
        async fn anchor_batch(&self, entries: Vec<AnchorEntry>) -> anyhow::Result<AnchorReceipt> {
            let mut newly = 0u32;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            let mut g = self.inner.lock().unwrap();
            for e in &entries {
                if !g.contains_key(&e.cid) {
                    g.insert(e.cid.clone(), (e.metadata_hash, now));
                    newly += 1;
                }
            }
            let mut tx = self.next_tx.lock().unwrap();
            let id = *tx;
            *tx += 1;
            Ok(AnchorReceipt {
                tx: format!("tx-{id}"),
                newly_anchored: newly,
                compute_units: Some(5_000 + entries.len() as u64 * 2_000),
            })
        }
        async fn is_anchored(&self, cid: &str) -> anyhow::Result<bool> {
            Ok(self.inner.lock().unwrap().contains_key(cid))
        }
    }
}
