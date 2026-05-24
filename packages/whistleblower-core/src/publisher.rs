use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::adapters::{AnchorAdapter, AnchorEntry, DeliveryAdapter, StorageAdapter};
use crate::envelope::{validate, DocumentEnvelope, ENVELOPE_SCHEMA};
use crate::hash::metadata_hash;
use crate::retry::{retry, RetryConfig};

#[derive(Clone)]
pub struct PublisherConfig {
    pub topic: String,
    pub retry: RetryConfig,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            topic: crate::DEFAULT_DELIVERY_TOPIC.into(),
            retry: RetryConfig::default(),
        }
    }
}

pub struct Publisher {
    storage: Arc<dyn StorageAdapter>,
    delivery: Arc<dyn DeliveryAdapter>,
    anchor: Option<Arc<dyn AnchorAdapter>>,
    cfg: PublisherConfig,
}

#[derive(Debug, Clone)]
pub struct PublishResult {
    pub envelope: DocumentEnvelope,
    pub metadata_hash: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct PublishMeta {
    pub title: String,
    pub description: String,
    pub content_type: String,
    pub tags: Vec<String>,
}

impl Publisher {
    pub fn new(
        storage: Arc<dyn StorageAdapter>,
        delivery: Arc<dyn DeliveryAdapter>,
        anchor: Option<Arc<dyn AnchorAdapter>>,
        cfg: PublisherConfig,
    ) -> Self {
        Self {
            storage,
            delivery,
            anchor,
            cfg,
        }
    }

    pub async fn publish(&self, bytes: &[u8], meta: PublishMeta) -> anyhow::Result<PublishResult> {
        let storage = self.storage.clone();
        let bytes_owned = bytes.to_vec();
        let ct = meta.content_type.clone();
        let upload = retry(
            self.cfg.retry,
            // Treat anyhow errors as transient by default. Production code can
            // wrap a typed error and discriminate on `is_transient` more
            // precisely (e.g. don't retry 4xx).
            |_e: &anyhow::Error| true,
            move || {
                let storage = storage.clone();
                let bytes = bytes_owned.clone();
                let ct = ct.clone();
                async move { storage.put(&bytes, Some(&ct)).await }
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("upload failed: {}", e))?;

        let envelope = DocumentEnvelope {
            schema: ENVELOPE_SCHEMA.into(),
            cid: upload.cid,
            title: meta.title,
            description: meta.description,
            content_type: meta.content_type,
            size_bytes: upload.size_bytes,
            timestamp: now_ms(),
            tags: if meta.tags.is_empty() {
                None
            } else {
                Some(meta.tags)
            },
        };
        validate(&envelope)?;

        let hash = metadata_hash(&envelope)?;
        self.delivery.publish(&self.cfg.topic, &envelope).await?;

        Ok(PublishResult {
            envelope,
            metadata_hash: hash,
        })
    }

    /// Publisher-side anchor — the optional "anchor on-chain" action that's
    /// distinct from the basic upload flow.
    pub async fn anchor(&self, result: &PublishResult) -> anyhow::Result<AnchorOutcome> {
        let anchor = self
            .anchor
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("anchor adapter not configured"))?;
        if anchor.is_anchored(&result.envelope.cid).await? {
            return Ok(AnchorOutcome {
                tx: String::new(),
                already_anchored: true,
            });
        }
        let receipt = anchor
            .anchor_single(AnchorEntry {
                cid: result.envelope.cid.clone(),
                metadata_hash: result.metadata_hash,
            })
            .await?;
        Ok(AnchorOutcome {
            tx: receipt.tx,
            already_anchored: receipt.newly_anchored == 0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AnchorOutcome {
    pub tx: String,
    pub already_anchored: bool,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
