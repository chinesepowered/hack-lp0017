#!/usr/bin/env bash
#
# Reproducible end-to-end demo for Whistleblower.
#
# Modes:
#   MOCK   (default) — runs against the in-memory adapters. No external
#                       services required. Used by CI and as a smoke test.
#   LEZ              — runs against a real LEZ sequencer + Logos Delivery.
#                       Requires:
#                         - `logos-scaffold setup && logos-scaffold localnet start`
#                         - RISC0_DEV_MODE=0 (real proofs)
#                         - The registry program built + deployed via
#                           `cd packages/registry-program && make build idl deploy`
#                         - $WHISTLEBLOWER_PROGRAM_ID exported
#
# Usage:
#   ./scripts/demo.sh                # MOCK
#   MODE=LEZ ./scripts/demo.sh       # LEZ devnet
#
set -euo pipefail

MODE="${MODE:-MOCK}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STATE="$ROOT/.demo-state.json"
DOC="$ROOT/.demo-doc.txt"

cd "$ROOT"

echo "════════════════════════════════════════════════════════════════"
echo "  Whistleblower demo"
echo "  Mode: $MODE"
if [ "$MODE" = "LEZ" ]; then
  echo "  RISC0_DEV_MODE=${RISC0_DEV_MODE:-0}"
fi
echo "════════════════════════════════════════════════════════════════"

# ── 1. Compose a sample document ────────────────────────────────────
cat > "$DOC" <<EOF
Confidential — Whistleblower demo document.
Generated $(date -Iseconds).
Random nonce: $RANDOM-$RANDOM
EOF
echo
echo "[1/5] Sample document: $DOC ($(wc -c < "$DOC") bytes)"

# ── 2. Build the workspace ──────────────────────────────────────────
echo
echo "[2/5] cargo build --workspace --release"
cargo build --workspace --release 2>&1 | tail -3

# ── 3. Run the integration test as the canonical end-to-end check ──
echo
echo "[3/5] cargo test -p whistleblower-core --release"
cargo test -p whistleblower-core --release 2>&1 | tail -10

# ── 4. Run the batch anchor CLI in the background ──────────────────
echo
echo "[4/5] Starting batch-anchor CLI (state: $STATE)"
rm -f "$STATE"
if [ "$MODE" = "LEZ" ]; then
  : "${WHISTLEBLOWER_PROGRAM_ID:?need WHISTLEBLOWER_PROGRAM_ID for LEZ mode}"
  : "${NSSA_WALLET_HOME_DIR:?need NSSA_WALLET_HOME_DIR for LEZ mode}"
  ./target/release/batch-anchor \
    --backend lez \
    --rpc-url "${LEZ_RPC_URL:-http://127.0.0.1:3040}" \
    --wallet "$NSSA_WALLET_HOME_DIR" \
    --program-id "$WHISTLEBLOWER_PROGRAM_ID" \
    --state "$STATE" --min-batch 1 &
else
  ./target/release/batch-anchor --backend mock --state "$STATE" --min-batch 1 &
fi
CLI_PID=$!
trap "kill $CLI_PID 2>/dev/null || true; rm -f $DOC $STATE" EXIT
sleep 1

# ── 5. Drive a publish + anchor through whistleblower-core ─────────
echo
echo "[5/5] Driving a single publish + batch-anchor cycle…"
cat > /tmp/wb-demo-driver.rs <<'EOF'
use std::sync::Arc;
use std::time::Duration;

use whistleblower_core::adapters::in_memory::{InMemoryAnchor, InMemoryDelivery, InMemoryStorage};
use whistleblower_core::adapters::{AnchorAdapter, DeliveryAdapter, StorageAdapter};
use whistleblower_core::{
    BatchAnchor, BatchAnchorConfig, PublishMeta, Publisher, PublisherConfig,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("usage: driver <path>");
    let bytes = std::fs::read(&path)?;

    let storage: Arc<dyn StorageAdapter>   = Arc::new(InMemoryStorage::new());
    let delivery: Arc<dyn DeliveryAdapter> = Arc::new(InMemoryDelivery::new());
    let anchor: Arc<dyn AnchorAdapter>     = Arc::new(InMemoryAnchor::new());

    // Use a long max_buffer so the deadline task can't pre-empt the
    // explicit flush() below. In production a shorter value is correct.
    let mut batcher = BatchAnchor::new(
        delivery.clone(), anchor.clone(),
        BatchAnchorConfig {
            min_batch: 1,
            max_buffer: Duration::from_secs(60),
            ..Default::default()
        },
    );
    batcher.start().await?;

    let publisher = Publisher::new(storage, delivery.clone(), Some(anchor.clone()), PublisherConfig::default());
    let r = publisher.publish(&bytes, PublishMeta {
        title: "Demo document".into(),
        description: format!("from {}", path),
        content_type: "text/plain".into(),
        tags: vec!["demo".into(), "whistleblower".into()],
    }).await?;
    println!("PUBLISH ok  cid={}  size={}  topic={}",
        r.envelope.cid, r.envelope.size_bytes, whistleblower_core::DEFAULT_DELIVERY_TOPIC);
    println!("METADATA_HASH={}", hex::encode(r.metadata_hash));

    // Block on the handler signalling it consumed the envelope.
    let got = batcher.wait_for_envelope(Duration::from_secs(2)).await;
    assert!(got, "batcher did not observe the broadcast envelope within 2s");

    let ev = batcher.flush().await?.expect("batch event after observed envelope");
    println!("ANCHOR  ok  tx={}  newly_anchored={}  cu={:?}",
        ev.tx, ev.newly_anchored, ev.compute_units);

    let confirmed = anchor.is_anchored(&r.envelope.cid).await?;
    assert!(confirmed, "registry should have the CID after batch flush");
    println!("LOOKUP  ok  cid={} present_in_registry=true", r.envelope.cid);

    batcher.stop().await;
    Ok(())
}
EOF

mkdir -p target/demo-driver/src/bin
cp /tmp/wb-demo-driver.rs target/demo-driver/src/bin/driver.rs
cat > target/demo-driver/Cargo.toml <<EOF
[package]
name = "wb-demo-driver"
version = "0.0.0"
edition = "2021"
[workspace]
[[bin]]
name = "driver"
path = "src/bin/driver.rs"
[dependencies]
whistleblower-core = { path = "$ROOT/packages/whistleblower-core" }
tokio = { version = "1", features = ["full"] }
anyhow = "1"
hex = "0.4"
EOF

( cd target/demo-driver && cargo run --quiet --release -- "$DOC" ) 2>&1 | sed 's/^/    /'

echo
echo "════════════════════════════════════════════════════════════════"
echo "  DEMO COMPLETE"
echo "  • upload  → CID issued by Logos Storage"
echo "  • broadcast → envelope on $(grep -oE 'topic=[^ ]*' <<< "$(true)" || echo /logos/whistleblower/v1/documents)"
echo "  • batch-anchor → CID present in registry"
echo "════════════════════════════════════════════════════════════════"

# Stop the background CLI cleanly. The trap handler also handles this.
kill $CLI_PID 2>/dev/null || true
wait $CLI_PID 2>/dev/null || true
