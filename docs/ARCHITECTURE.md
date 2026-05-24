# Whistleblower architecture

## Pipeline

```
       ┌────────────────────┐
       │   user            │
       │  (file + meta)    │
       └─────────┬──────────┘
                 │
                 ▼
       ┌────────────────────┐
       │  Basecamp app      │
       │  (QML or web)      │
       └─────────┬──────────┘
                 │ whistleblower-core / @whistleblower/indexing-module
                 ▼
   ┌────────────────────────────────────────┐
   │   Publisher                            │
   │     1. storage.put(bytes) → cid        │  ──► Logos Storage
   │     2. build envelope, sha256          │
   │     3. delivery.publish(topic, env)    │  ──► Logos Delivery
   │   (optional 4) anchor.anchor_single()  │  ──► LEZ registry
   └────────────────────────────────────────┘

                 │ broadcast envelope
                 ▼
   ┌────────────────────────────────────────┐
   │   ANY third party                      │
   │   $ batch-anchor --backend lez …       │
   │     a. delivery.subscribe(topic)       │
   │     b. accumulate (cid, hash)          │
   │     c. anchor.anchor_batch(entries)    │ ──► LEZ registry
   │   resumes from .batch-anchor-state.json│
   └────────────────────────────────────────┘
```

Every arrow on the right-hand side is an adapter trait. In CI / demo
mode the in-memory adapters fill these in. In production the live
adapters in `packages/batch-anchor-cli/src/adapters/` (and the
`window.logos.bridge` shim in the web app) call into the real services.

## Repository layout

| Path                                        | Contents                                                                                |
| ------------------------------------------- | --------------------------------------------------------------------------------------- |
| `packages/whistleblower-core/`              | **Canonical** Rust indexing module. Used by the CLI + the FFI surface for native apps.  |
| `packages/indexing-module/`                 | TypeScript port of the same module. Used by the web Basecamp app.                       |
| `packages/registry-program/`                | SPEL program for the on-chain CID registry. RISC0 zkVM guest + IDL.                     |
| `packages/batch-anchor-cli/`                | Permissionless batch-anchor CLI. Subscribes to delivery, anchors in batches.            |
| `packages/basecamp-app/`                    | Basecamp app — web preview (Vite/TS), QML manifest, FFI integration notes.              |
| `scripts/demo.sh`                           | Reproducible end-to-end demo. Runs in CI in MOCK mode; LEZ mode for real devnet.        |
| `scripts/benchmark.sh`                      | CU benchmark for single + 50-CID batch. Writes `docs/BENCHMARKS.md`.                    |
| `tests/integration/`                        | High-level integration test scaffolds.                                                  |

## Key design decisions

### 1. Why one canonical Rust crate + a TS port instead of just TS?

The bounty wants the module reusable by other Logos apps. The
ecosystem's actual primitives — Basecamp apps (C++/QML), the
`logos-delivery-module` (Qt/C++), LEZ programs (Rust on RISC0), the
`spel` and `wallet` CLIs (Rust) — are all native. A Rust crate exposes
a stable `rlib` for other Rust callers and a `cdylib` + C ABI for QML /
C++ callers via `whistleblower-core --features ffi`. The TypeScript
port exists because the web flavour of the Basecamp app is the most
ergonomic way to render a live feed; it shares the envelope schema
byte-for-byte with the Rust crate, pinned by a cross-language fixture
test (`packages/whistleblower-core/tests/cross_lang.rs` +
`packages/indexing-module/test/cross_lang.test.ts`).

### 2. Why a LEZ program over a zone-SDK consensus submission?

Bounty asks the submitter to choose and justify. We chose **LEZ program**
because:

- **Permissionless** — any signer can call `anchor_batch`. The zone-SDK
  consensus-layer path currently requires a single designated actor
  (decentralised sequencers for zones aren't shipped yet), which would
  re-introduce a coordination requirement the bounty explicitly forbids.
- **Idempotence at the program level** — `init_if_empty` PDA lets the
  program skip duplicates without the client needing to coordinate.
- **Cheaper batched semantics** — one RISC0 proof per 50-CID batch
  instead of 50 separate consensus submissions.

### 3. Envelope canonicalisation

`metadata_hash` is a SHA-256 over a deterministic JSON serialisation
with a pinned field order. We do NOT use a library default (e.g.
`serde_json` with a BTreeMap) because:

- Different serialisers disagree on number formatting (e.g. trailing
  zeroes on floats), unicode escaping, and key ordering.
- The hash goes on-chain. A mismatch is silently fatal.

The canonicalisation is implemented by hand in both ports and pinned by
a shared fixture. Any change requires updating both ports + the
fixture, in the same PR.

### 4. Idempotence at three layers

Defence in depth — re-broadcasting or re-anchoring a CID is safe at:

1. **Logos Delivery** — adapter dedups on `(topic, cid)`, so subscribers
   never see duplicates.
2. **Batch anchor CLI** — `seen` set persisted to disk; pre-check via
   `is_anchored` before adding to pending; skips already-anchored CIDs.
3. **Registry program** — `init_if_empty` PDA + post-write check on data
   non-empty. A duplicate in `anchor_batch` is silently skipped, the
   rest of the batch proceeds.

The cross-language fixture, the integration tests, and the
`fixture_a_matches_pinned_hash` test cover layer 1 + 2. The registry
test suite (`packages/registry-program/methods/guest/src/bin/...`) is
extended in a follow-up — see "Out of scope for this PR" below.

### 5. Decoupling publisher from anchor

The `Publisher` does NOT require an `AnchorAdapter`. A publisher who
holds no on-chain funds can still upload + broadcast; an altruistic
third party (NGO, journalist collective, automated guardian) picks the
CID up off the topic and anchors it. This is the central reason the
bounty exists, and it's why `publisher.anchor()` is a separate optional
method rather than part of `publish()`.

## What's mocked vs. live

| Component            | Mock (in-memory)                      | Live                                                 |
| -------------------- | ------------------------------------- | ---------------------------------------------------- |
| Storage adapter      | `InMemoryStorage` (content-addressed) | Wraps `liblogosstorage`; bridge or FFI from app side |
| Delivery adapter     | `InMemoryDelivery` (tokio broadcast)  | Unix-socket JSON-RPC to `logos-delivery-module`      |
| Anchor adapter       | `InMemoryAnchor` (HashMap registry)   | `spel` subprocess against a deployed LEZ program     |
| Registry program     | Synthetic CU model `5_000 + 2_000·N`  | RISC0 zkVM guest, built via `make build`             |
| Basecamp host        | Vite preview at 127.0.0.1:4173        | `logos-basecamp install` of the `.lgx` bundle        |

CI runs everything in MOCK mode. The demo script's `MODE=LEZ` mode
runs the same flow against a real local sequencer (requires
`logos-scaffold setup`, `make build idl deploy`, and `RISC0_DEV_MODE=0`).
