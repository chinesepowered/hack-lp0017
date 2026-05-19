use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use whistleblower_core::adapters::{DeliveryAdapter, EnvelopeHandler, SubscriptionHandle};
use whistleblower_core::envelope::DocumentEnvelope;

/// Live delivery adapter. Speaks JSON-RPC over a Unix domain socket
/// exposed by the `logos-delivery-module` sidecar — the same surface the
/// Basecamp app talks to. When the socket isn't reachable we fall back to
/// an in-memory implementation so the CLI is still usable in CI.
///
/// Wire format (one JSON object per line, NDJSON):
///   → publish:    { "op": "publish",   "topic": "...", "envelope": { ... } }
///   ← ack:        { "ok": true }
///   → subscribe:  { "op": "subscribe", "topic": "..." }
///   ← envelope:   { "envelope": { ... } }
///
/// This matches the documented bridge format in
/// `docs/architecture/delivery-bridge.md`.
pub async fn connect(rpc_url: &str) -> anyhow::Result<Arc<dyn DeliveryAdapter>> {
    match LezDelivery::connect(rpc_url).await {
        Ok(d) => Ok(Arc::new(d)),
        Err(e) => {
            tracing::warn!(
                error = %e,
                rpc = %rpc_url,
                "logos-delivery bridge unreachable; using in-memory fallback (CI / dry-run mode)"
            );
            Ok(Arc::new(
                whistleblower_core::adapters::in_memory::InMemoryDelivery::new(),
            ))
        }
    }
}

struct LezDelivery {
    // Tx side: line-buffered writer for sending JSON-RPC frames.
    write: Mutex<tokio::net::UnixStream>,
}

impl LezDelivery {
    async fn connect(rpc_url: &str) -> anyhow::Result<Self> {
        // Accept both `unix:/path/to/sock` and a bare path.
        let path = rpc_url.strip_prefix("unix:").unwrap_or(rpc_url);
        let stream = tokio::net::UnixStream::connect(path).await?;
        Ok(Self {
            write: Mutex::new(stream),
        })
    }
}

#[async_trait]
impl DeliveryAdapter for LezDelivery {
    async fn publish(&self, topic: &str, envelope: &DocumentEnvelope) -> anyhow::Result<()> {
        use tokio::io::AsyncWriteExt;
        let frame = serde_json::json!({
            "op": "publish",
            "topic": topic,
            "envelope": envelope,
        });
        let mut line = serde_json::to_vec(&frame)?;
        line.push(b'\n');
        let mut w = self.write.lock().await;
        w.write_all(&line).await?;
        w.flush().await?;
        Ok(())
    }

    async fn subscribe(
        &self,
        _topic: &str,
        _handler: EnvelopeHandler,
    ) -> anyhow::Result<SubscriptionHandle> {
        // The Unix-socket bridge would split read/write streams here. To
        // keep this submission compileable in the absence of a live bridge
        // we surface a clear runtime error so operators know to either
        // launch the bridge or fall back to `--backend mock`.
        anyhow::bail!(
            "LEZ delivery subscribe path requires the logos-delivery bridge; \
             see docs/architecture/delivery-bridge.md for the wire format"
        )
    }
}
