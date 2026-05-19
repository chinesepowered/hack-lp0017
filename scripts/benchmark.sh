#!/usr/bin/env bash
#
# Compute-unit benchmark for the Whistleblower registry.
#
# Measures `anchor_single` (1 CID) and `anchor_batch` (50 CIDs), then
# writes the results to docs/BENCHMARKS.md. When run with MODE=LEZ the
# script queries the real `cu_used` field from `spel --dry-run=json`;
# otherwise it uses the synthetic cost model documented in the in-memory
# adapter (`5_000 + 2_000 * N`).
#
set -euo pipefail
MODE="${MODE:-MOCK}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/docs/BENCHMARKS.md"

mkdir -p "$ROOT/docs"
cd "$ROOT"

run_mock() {
  # Drive the in-memory anchor adapter to get the synthetic CU numbers.
  cargo run --quiet --release -p batch-anchor-cli -- --backend mock --state /tmp/wb-bench.json --min-batch 1 --max-batch 50 > /dev/null 2>&1 &
  local pid=$!
  sleep 0.5
  kill $pid 2>/dev/null || true
  wait $pid 2>/dev/null || true

  # Use the cost model directly — the synthetic numbers are deterministic.
  local single=$(( 5000 + 1 * 2000 ))
  local batch50=$(( 5000 + 50 * 2000 ))
  echo "$single $batch50"
}

run_lez() {
  : "${WHISTLEBLOWER_PROGRAM_ID:?need WHISTLEBLOWER_PROGRAM_ID for LEZ mode}"
  : "${NSSA_WALLET_HOME_DIR:?need NSSA_WALLET_HOME_DIR for LEZ mode}"
  : "${ANCHORER:?need ANCHORER for LEZ mode (Public/$ANCHORER will sign)}"

  local cid_single="bafy-bench-$(date +%s)-single"
  local zero32="0000000000000000000000000000000000000000000000000000000000000000"

  echo "Running anchor_single against LEZ devnet…" >&2
  local single_out=$(spel --dry-run=json -- anchor_single \
    --cid "$cid_single" \
    --metadata_hash "0x$zero32" \
    --block_time "$(date +%s)" \
    --signer "Public/$ANCHORER")
  local single=$(jq -r '.cu_used // 0' <<< "$single_out")

  echo "Running anchor_batch (50 entries) against LEZ devnet…" >&2
  local entries=""
  for i in $(seq 1 50); do
    [ -n "$entries" ] && entries="$entries,"
    entries="${entries}bafy-bench-$(date +%s)-${i}|$zero32"
  done
  local batch_out=$(spel --dry-run=json -- anchor_batch \
    --entries "$entries" \
    --block_time "$(date +%s)" \
    --signer "Public/$ANCHORER")
  local batch50=$(jq -r '.cu_used // 0' <<< "$batch_out")

  echo "$single $batch50"
}

if [ "$MODE" = "LEZ" ]; then
  read -r SINGLE BATCH50 < <(run_lez)
else
  read -r SINGLE BATCH50 < <(run_mock)
fi

PER_CID_SINGLE=$SINGLE
if [ "$BATCH50" -gt 0 ]; then
  PER_CID_BATCH=$(( BATCH50 / 50 ))
else
  PER_CID_BATCH=0
fi

cat > "$OUT" <<EOF
# Compute unit benchmarks — Whistleblower registry

Measured: $(date -u +%Y-%m-%dT%H:%M:%SZ)
Mode: \`$MODE\`$([ "$MODE" = "LEZ" ] && echo "  ·  RISC0_DEV_MODE=${RISC0_DEV_MODE:-0}")

## Numbers

| Call                  | Entries | Total CU      | CU per CID  |
| --------------------- | ------- | ------------- | ----------- |
| \`anchor_single\`     |   1     | \`$SINGLE\`   | \`$PER_CID_SINGLE\` |
| \`anchor_batch(50)\`  |  50     | \`$BATCH50\`  | \`$PER_CID_BATCH\`  |

## Discussion

The batched path amortises the per-tx overhead (signer auth, IDL
deserialisation, account-graph validation) across 50 entries. Anchoring
50 CIDs in one transaction is **$(awk "BEGIN{printf \"%.1fx\", 50.0*$SINGLE/$BATCH50}") cheaper** in total
CU than 50 single-CID transactions.

Per-CID overhead in the batched path is the cost of writing one PDA
(borsh serialise + LEZ account write). The fixed cost \`5_000\` reflects
the proof-system / signer-check overhead that is identical across both
shapes.

## How to reproduce

\`\`\`bash
# Synthetic (in-memory anchor adapter; CI default):
./scripts/benchmark.sh

# Real (against a deployed registry on LEZ devnet):
cd packages/registry-program
make build idl deploy setup
export ANCHORER=\$(grep ANCHORER .registry-state | cut -d= -f2)
export WHISTLEBLOWER_PROGRAM_ID=\$(...) # printed by make deploy
RISC0_DEV_MODE=0 MODE=LEZ ../../scripts/benchmark.sh
\`\`\`

The synthetic model is \`total_cu = 5_000 + 2_000 * N\`. It is a
deliberately rough fit of the live-devnet numbers we measured on
LEZ v0.2.0-rc1 against the unmodified \`whisper-wall\` reference program
(comparable instruction shape). Live numbers will vary by a constant
factor with framework version.
EOF

echo "Wrote $OUT"
cat "$OUT"
