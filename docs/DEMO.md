# Demo walkthrough

Two flavours: a hermetic mock demo that runs anywhere, and the real
end-to-end demo against a live LEZ devnet that evaluators
will reproduce.

## 1. Mock demo (no Logos services required)

```bash
# from the repo root
bash scripts/demo.sh
```

Expected last lines:

```
[5/5] Driving a single publish + batch-anchor cycle…
    PUBLISH ok  cid=bafy…  size=109  topic=/logos/whistleblower/v1/documents
    METADATA_HASH=…
    ANCHOR  ok  tx=tx-1  newly_anchored=1  cu=Some(7000)
    LOOKUP  ok  cid=bafy… present_in_registry=true
════════════════════════════════════════════════════════════════
  DEMO COMPLETE
  • upload  → CID issued by Logos Storage
  • broadcast → envelope on /logos/whistleblower/v1/documents
  • batch-anchor → CID present in registry
════════════════════════════════════════════════════════════════
```

This is what CI runs.

## 2. Real demo (LEZ devnet)

### Prerequisites

Run these once:

```bash
cargo install --git https://github.com/logos-co/logos-scaffold
cargo install --git https://github.com/logos-co/spel spel
curl -L https://risczero.com/install | bash && rzup install
# Docker or Podman must be running (needed for hermetic guest builds)
```

### Boot the sequencer

```bash
logos-scaffold setup
logos-scaffold localnet start
export NSSA_WALLET_HOME_DIR="$PWD/.scaffold/wallet"
```

### Build + deploy the registry

```bash
cd packages/registry-program
make build              # 5–15 min the first time
make idl
make setup              # creates + funds an anchorer wallet → .registry-state
make deploy             # prints the program ID
export WHISTLEBLOWER_PROGRAM_ID=<id printed by make deploy>
export ANCHORER=$(grep ANCHORER .registry-state | cut -d= -f2)
cd ../..
```

### Run the demo against the live stack

```bash
RISC0_DEV_MODE=0 MODE=LEZ bash scripts/demo.sh
```

What you'll see in the terminal:

1. **Upload** — `wallet storage put .demo-doc.txt` returns a CID.
2. **Broadcast** — the envelope appears on the
   `/logos/whistleblower/v1/documents` topic. Tail it with:
   ```bash
   logos-delivery-demo --topic /logos/whistleblower/v1/documents
   ```
3. **Batch anchor** — the running `batch-anchor` process picks up the
   CID off the topic and submits an `anchor_batch` instruction. The
   spel CLI prints the transaction signature + `cu_used`.
4. **Lookup** — `spel inspect $(spel pda --seed wb_v1 --seed <cid>)
   --type AnchorAccount` decodes the on-chain record:
   ```json
   {
     "cid": "bafy…",
     "metadata_hash": "5e90e2cf…",
     "anchor_timestamp": 1700000000,
     "anchored_by": "Public/…"
   }
   ```

### Recording the video

The submission requires terminal output that confirms `RISC0_DEV_MODE=0`. Use:

```bash
asciinema rec -c "RISC0_DEV_MODE=0 MODE=LEZ bash scripts/demo.sh" demo.cast
```

The proof generation lines printed by `risc0-zkvm` (containing
"proof generated in …s" with multi-second timings) are what evaluators
will look for.

## 3. Basecamp web preview

For a visual demo without QML/Basecamp installed:

```bash
pnpm install
pnpm --filter @whistleblower/basecamp-app build
pnpm --filter @whistleblower/basecamp-app preview
# open http://127.0.0.1:4173/
```

The preview page exposes the same upload → broadcast → live feed
workflow. With no `window.logos.bridge` injected by Basecamp, it falls
back to the in-memory adapters so the flow still works in isolation.

## 4. Basecamp native (QML)

Build the FFI library + bundle the manifest:

```bash
cargo build -p whistleblower-core --features ffi --release
cp target/release/libwhistleblower_core.so \
   packages/basecamp-app/lib/
( cd packages/basecamp-app && zip -r whistleblower.lgx \
     basecamp.manifest.json qml dist lib assets )
logos-basecamp install whistleblower.lgx
logos-basecamp launch io.logos.apps.whistleblower
```
