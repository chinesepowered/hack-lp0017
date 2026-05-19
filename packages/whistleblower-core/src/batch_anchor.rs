use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::adapters::{AnchorAdapter, AnchorEntry, DeliveryAdapter, SubscriptionHandle};
use crate::envelope::{validate, DocumentEnvelope};
use crate::hash::metadata_hash;

#[derive(Clone, Debug)]
pub struct BatchAnchorConfig {
    pub topic: String,
    pub min_batch: usize,
    pub max_batch: usize,
    /// Maximum buffer-fill latency before flushing a partial batch.
    pub max_buffer: Duration,
}

impl Default for BatchAnchorConfig {
    fn default() -> Self {
        Self {
            topic: crate::DEFAULT_DELIVERY_TOPIC.into(),
            min_batch: 10,
            max_batch: 50,
            max_buffer: Duration::from_secs(15),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchAnchorEvent {
    pub tx: String,
    pub cids: Vec<String>,
    pub newly_anchored: u32,
    pub compute_units: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct BatchAnchorStatus {
    pub pending_cids: usize,
    pub total_anchored: u64,
    pub last_batch_tx: Option<String>,
}

struct Inner {
    pending: HashMap<String, AnchorEntry>,
    seen: HashSet<String>,
    total_anchored: u64,
    last_batch_tx: Option<String>,
}

/// Permissionless, restartable batch anchor.
///
/// `seen` holds the union of (a) CIDs we've already anchored and (b) CIDs
/// observed as already-anchored on-chain. Persist `snapshot()` to disk
/// every time `on_batch_anchored` fires so a restart resumes from there.
pub struct BatchAnchor {
    delivery: Arc<dyn DeliveryAdapter>,
    anchor: Arc<dyn AnchorAdapter>,
    cfg: BatchAnchorConfig,
    inner: Arc<Mutex<Inner>>,
    handle: Option<SubscriptionHandle>,
    on_batch: Option<Arc<dyn Fn(BatchAnchorEvent) + Send + Sync>>,
    /// Notified after a handler inserts into pending OR after `flush()` is
    /// called, so the deadline-task can react without polling.
    notify: Arc<tokio::sync::Notify>,
    /// Serialises flushes — `flush()` and the deadline task share this so
    /// they can't race and drain the same batch twice.
    flush_lock: Arc<Mutex<()>>,
}

impl BatchAnchor {
    pub fn new(
        delivery: Arc<dyn DeliveryAdapter>,
        anchor: Arc<dyn AnchorAdapter>,
        cfg: BatchAnchorConfig,
    ) -> Self {
        Self {
            delivery,
            anchor,
            cfg,
            inner: Arc::new(Mutex::new(Inner {
                pending: HashMap::new(),
                seen: HashSet::new(),
                total_anchored: 0,
                last_batch_tx: None,
            })),
            handle: None,
            on_batch: None,
            notify: Arc::new(tokio::sync::Notify::new()),
            flush_lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn set_initial_seen(&self, cids: impl IntoIterator<Item = String>) {
        let mut g = self.inner.lock().await;
        g.seen.extend(cids);
    }

    pub fn on_batch_anchored<F: Fn(BatchAnchorEvent) + Send + Sync + 'static>(&mut self, f: F) {
        self.on_batch = Some(Arc::new(f));
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        let inner = self.inner.clone();
        let anchor = self.anchor.clone();
        let cfg = self.cfg.clone();
        let on_batch = self.on_batch.clone();
        let flush_lock = self.flush_lock.clone();
        let notify = self.notify.clone();

        let handler: crate::adapters::EnvelopeHandler = Arc::new(move |env: DocumentEnvelope| {
            let inner = inner.clone();
            let anchor = anchor.clone();
            let cfg = cfg.clone();
            let on_batch = on_batch.clone();
            let flush_lock = flush_lock.clone();
            let notify = notify.clone();
            Box::pin(async move {
                // Always notify on exit so `wait_for_envelope` returns even
                // when an envelope is rejected (validation failure, already
                // anchored, dedup). Use a guard so early-return paths fire.
                struct NotifyOnDrop(Arc<tokio::sync::Notify>);
                impl Drop for NotifyOnDrop {
                    fn drop(&mut self) {
                        self.0.notify_one();
                    }
                }
                let _notify_guard = NotifyOnDrop(notify);

                if validate(&env).is_err() {
                    return;
                }

                // Compute hash before locking — we hold the lock only for state mutation.
                let h = match metadata_hash(&env) {
                    Ok(h) => h,
                    Err(_) => return,
                };

                // Pre-check via the registry to avoid burning CU on duplicates.
                if anchor.is_anchored(&env.cid).await.unwrap_or(false) {
                    let mut g = inner.lock().await;
                    g.seen.insert(env.cid.clone());
                    return;
                }

                let should_flush = {
                    let mut g = inner.lock().await;
                    if g.seen.contains(&env.cid) || g.pending.contains_key(&env.cid) {
                        false
                    } else {
                        g.pending.insert(
                            env.cid.clone(),
                            AnchorEntry {
                                cid: env.cid.clone(),
                                metadata_hash: h,
                            },
                        );
                        g.pending.len() >= cfg.max_batch
                    }
                };

                if should_flush {
                    flush_now(&inner, &anchor, on_batch.as_ref(), &flush_lock).await;
                }
            })
        });

        // Spawn a deadline-based flusher so partial batches don't stall
        // forever. It only fires when pending size is at least min_batch
        // — explicit `flush()` is the path that drains partial-below-min
        // batches.
        let inner_d = self.inner.clone();
        let anchor_d = self.anchor.clone();
        let cfg_d = self.cfg.clone();
        let on_batch_d = self.on_batch.clone();
        let flush_lock_d = self.flush_lock.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(cfg_d.max_buffer).await;
                let pending = inner_d.lock().await.pending.len();
                if pending >= cfg_d.min_batch {
                    flush_now(&inner_d, &anchor_d, on_batch_d.as_ref(), &flush_lock_d).await;
                }
            }
        });

        let handle = self.delivery.subscribe(&self.cfg.topic, handler).await?;
        self.handle = Some(handle);
        Ok(())
    }

    /// Wait until at least one envelope has been processed by the handler,
    /// or until `timeout` elapses. Useful for tests and the demo driver —
    /// avoids racing against the deadline flusher.
    pub async fn wait_for_envelope(&self, timeout: Duration) -> bool {
        tokio::time::timeout(timeout, self.notify.notified())
            .await
            .is_ok()
    }

    pub async fn stop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.close().await;
        }
    }

    /// Force-flush any pending entries. Returns Some(event) or None if nothing pending.
    pub async fn flush(&self) -> anyhow::Result<Option<BatchAnchorEvent>> {
        Ok(flush_now(
            &self.inner,
            &self.anchor,
            self.on_batch.as_ref(),
            &self.flush_lock,
        )
        .await)
    }

    pub async fn status(&self) -> BatchAnchorStatus {
        let g = self.inner.lock().await;
        BatchAnchorStatus {
            pending_cids: g.pending.len(),
            total_anchored: g.total_anchored,
            last_batch_tx: g.last_batch_tx.clone(),
        }
    }

    pub async fn snapshot(&self) -> Vec<String> {
        self.inner.lock().await.seen.iter().cloned().collect()
    }
}

async fn flush_now(
    inner: &Arc<Mutex<Inner>>,
    anchor: &Arc<dyn AnchorAdapter>,
    on_batch: Option<&Arc<dyn Fn(BatchAnchorEvent) + Send + Sync>>,
    flush_lock: &Arc<Mutex<()>>,
) -> Option<BatchAnchorEvent> {
    // Serialise concurrent flushes — `flush()` and the deadline task
    // share `flush_lock` so they can't drain the same batch twice or
    // submit overlapping batches to the registry.
    let _guard = flush_lock.lock().await;

    let entries = {
        let mut g = inner.lock().await;
        if g.pending.is_empty() {
            return None;
        }
        g.pending.drain().map(|(_, v)| v).collect::<Vec<_>>()
    };

    match anchor.anchor_batch(entries.clone()).await {
        Ok(receipt) => {
            let cids = entries.iter().map(|e| e.cid.clone()).collect::<Vec<_>>();
            {
                let mut g = inner.lock().await;
                g.total_anchored = g
                    .total_anchored
                    .saturating_add(receipt.newly_anchored as u64);
                g.last_batch_tx = Some(receipt.tx.clone());
                for c in &cids {
                    g.seen.insert(c.clone());
                }
            }
            let ev = BatchAnchorEvent {
                tx: receipt.tx,
                cids,
                newly_anchored: receipt.newly_anchored,
                compute_units: receipt.compute_units,
            };
            if let Some(cb) = on_batch {
                cb(ev.clone());
            }
            Some(ev)
        }
        Err(e) => {
            // Put the entries back so a retry can pick them up.
            let mut g = inner.lock().await;
            for e in entries {
                g.pending.entry(e.cid.clone()).or_insert(e);
            }
            tracing::warn!(error = %e, "batch anchor failed, entries re-queued");
            None
        }
    }
}
