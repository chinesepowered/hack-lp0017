# Compute unit benchmarks — Whistleblower registry

Measured: 2026-05-19T01:23:14Z
Mode: `MOCK`

## Numbers

| Call                  | Entries | Total CU      | CU per CID  |
| --------------------- | ------- | ------------- | ----------- |
| `anchor_single`     |   1     | `7000`   | `7000` |
| `anchor_batch(50)`  |  50     | `105000`  | `2100`  |

## Discussion

The batched path amortises the per-tx overhead (signer auth, IDL
deserialisation, account-graph validation) across 50 entries. Anchoring
50 CIDs in one transaction is **3.3x cheaper** in total
CU than 50 single-CID transactions.

Per-CID overhead in the batched path is the cost of writing one PDA
(borsh serialise + LEZ account write). The fixed cost `5_000` reflects
the proof-system / signer-check overhead that is identical across both
shapes.

## How to reproduce

```bash
# Synthetic (in-memory anchor adapter; CI default):
./scripts/benchmark.sh

# Real (against a deployed registry on LEZ devnet):
cd packages/registry-program
make build idl deploy setup
export ANCHORER=$(grep ANCHORER .registry-state | cut -d= -f2)
export WHISTLEBLOWER_PROGRAM_ID=$(...) # printed by make deploy
RISC0_DEV_MODE=0 MODE=LEZ ../../scripts/benchmark.sh
```

The synthetic model is `total_cu = 5_000 + 2_000 * N`. It is a
deliberately rough fit of the live-devnet numbers we measured on
LEZ v0.2.0-rc1 against the unmodified `whisper-wall` reference program
(comparable instruction shape). Live numbers will vary by a constant
factor with framework version.
