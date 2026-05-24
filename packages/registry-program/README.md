# whistleblower-registry

On-chain CID registry for the Whistleblower app — a SPEL program for the
Logos Execution Zone (LEZ).

## What it does

Stores `(cid, metadata_hash, anchor_timestamp)` per document in a PDA
keyed by `["wb_v1", cid]`. Two write instructions:

- `anchor_single(cid, metadata_hash, block_time)` — anchor one CID.
- `anchor_batch(entries, block_time)` — anchor up to **50** CIDs in a
  single transaction. The success criterion is "≥ 10 per transaction";
  we accept up to 50.

Both are **idempotent**: re-anchoring an already-registered CID is a
no-op (silently skipped, the transaction still succeeds). Idempotence is
enforced server-side by checking the PDA's data field — a non-empty PDA
means it has been written before, so we skip the write.

One read instruction:

- `lookup(cid)` — returns the anchor PDA so the client can decode via
  `spel inspect --type AnchorAccount`.

## Why LEZ program vs. zone-SDK direct submission

We chose **(a) a LEZ program** over (b) direct zone-SDK submission for
three reasons:

1. **Open registry**. Any signer can call `anchor_batch` from any
   wallet. The zone-SDK path requires a single designated actor to perform
   consensus inscription (decentralised sequencers for zones are not yet
   shipped), which contradicts the permissionless requirement.
2. **Idempotence at the protocol level**. A program can `init_if_empty`
   the PDA and skip duplicates. Replicating that logic at the
   consensus-layer submission layer means the client must coordinate
   on which entries are "already anchored", which races with concurrent
   anchorers.
3. **Cheaper batch semantics**. The program collapses N PDA writes into
   one RISC0 proof, paid once per batch. Direct submission would require
   N consensus messages.

This trade-off is documented in `docs/ARCHITECTURE.md`.

## Build & deploy

```bash
# 1. Install toolchain (one-time)
cargo install --git https://github.com/logos-co/logos-scaffold
cargo install --git https://github.com/logos-co/spel spel
curl -L https://risczero.com/install | bash && rzup install

# 2. Boot a local sequencer
logos-scaffold setup
logos-scaffold localnet start
export NSSA_WALLET_HOME_DIR="$PWD/.scaffold/wallet"

# 3. Build + deploy the program
cd packages/registry-program
make build       # 5–15 min first time (RISC0 guest compile)
make idl         # writes idl/whistleblower-registry-idl.json
make setup       # creates + funds an anchorer wallet
make deploy      # uploads the ELF to the sequencer
```

The deployed program id is printed by `make deploy` and stored as
`PROGRAM_ID` in `.registry-state`. Pass it to the batch-anchor CLI via
`--program-id` or `WHISTLEBLOWER_PROGRAM_ID`.

## Call from the CLI directly (without the batcher)

```bash
# Anchor a single CID
make cli ARGS="anchor_single \
  --cid bafy0001example \
  --metadata_hash 0xa1b2c3...64hexchars \
  --block_time $(date +%s) \
  --signer Public/$ANCHORER"

# Inspect what was written
spel inspect --type AnchorAccount $(spel pda --seed wb_v1 --seed bafy0001example)
```

## Compute unit benchmarks

`make benchmark` runs `anchor_single` once and `anchor_batch` with 50
entries, then writes the measured CU numbers to `../../docs/BENCHMARKS.md`.

When run against the in-memory mock backend (CI), it uses the synthetic
cost model `5_000 + 2_000 * N` documented in `whistleblower-core`. When run
against a live devnet (`make deploy` first), it uses the `cu_used` field
returned by `spel --dry-run=json`.

## Idempotence guarantee — formal statement

For any sequence of calls `anchor_batch(B₁), anchor_batch(B₂), …, anchor_batch(Bₖ)`
where `B_i` is a (possibly overlapping) batch of entries, the final state
of the registry is identical to `anchor_batch(B₁ ∪ B₂ ∪ … ∪ Bₖ)` evaluated
with no duplicates. No call fails due to duplicates. Counterexample policy:
the unit tests in `tests/idempotence.rs` will fail if a duplicate
submission causes a transaction error.
