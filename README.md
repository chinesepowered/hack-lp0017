# Whistleblower

[![LEZ live-stack demo](https://github.com/chinesepowered/hack-lp0017/actions/workflows/lez-live.yml/badge.svg)](https://github.com/chinesepowered/hack-lp0017/actions/workflows/lez-live.yml)

**📺 [Watch the Voiceover Demo Video](https://www.youtube.com/watch?v=hVZjDfp0DpI)**

**📺 [Proof on Chain](https://github.com/chinesepowered/hack-lp0017/actions/runs/26578762529/job/78305405638)**

**A Logos Basecamp app that lets anyone upload a document and make it
permanently discoverable — without a central server, without requiring
the uploader to hold tokens, and without a single point of censorship.**

A whistleblower selects a file, adds metadata, and submits. The app
uploads the bytes to **Logos Storage**, broadcasts the resulting CID and
metadata envelope over **Logos Delivery** so the document is immediately
discoverable, and optionally anchors the CID on-chain via a **SPEL
program on the Logos Execution Zone**.

---

## The key idea: anchoring is decoupled from publication

The uploader never needs tokens, a wallet, or to coordinate with anyone.
Publication (upload + broadcast) and anchoring (on-chain commitment) are
separate steps run by separate parties:

```
  user (file + meta)
        │
        ▼
   Basecamp app  ── whistleblower-core / @whistleblower/indexing-module ──┐
        │   Publisher                                                     │
        │    1. storage.put(bytes)           ──►  Logos Storage           │
        │    2. canonicalise + sha256                                     │
        │    3. delivery.publish(topic, env) ──►  Logos Delivery topic    │
        ▼                                                                 │
   /logos/whistleblower/v1/documents  ◄───────────────────────────────────┘
        │
        ▼
   ANY third party — NGO, journalist collective, automated guardian
   $ batch-anchor --backend lez …
   subscribes ─ accumulates CIDs ─ submits anchor_batch (up to 50/tx) ──► LEZ registry
   resumes from .batch-anchor-state.json after a restart
```

So a source with no on-chain footprint can publish, and anyone else can
durably anchor the CID later — in bulk, permissionlessly, idempotently.

**Three independent layers of idempotence** keep re-runs safe: Delivery
dedup on `(topic, cid)` ▸ the CLI's atomically-persisted seen-set ▸ the
registry's `init_if_empty` PDA write. Re-anchoring an existing CID is
always a no-op.

---

## What we built

| Deliverable | Where |
| --- | --- |
| **Basecamp app** — QML manifest + Vite/TypeScript web flavour | [`packages/basecamp-app/`](packages/basecamp-app/) |
| **Indexing module (Rust)** — canonical, FFI-friendly, reusable | [`packages/whistleblower-core/`](packages/whistleblower-core/) |
| **Indexing module (TypeScript)** — byte-identical port | [`packages/indexing-module/`](packages/indexing-module/) |
| **On-chain CID registry** — SPEL program for LEZ, with IDL | [`packages/registry-program/`](packages/registry-program/) |
| **Permissionless batch anchor CLI** — `batch-anchor` binary | [`packages/batch-anchor-cli/`](packages/batch-anchor-cli/) |
| **End-to-end demo + CU benchmarks** | [`scripts/demo.sh`](scripts/demo.sh) · [`scripts/benchmark.sh`](scripts/benchmark.sh) |
| **Architecture + demo write-ups** | [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · [`docs/DEMO.md`](docs/DEMO.md) |

The Rust crate and TypeScript port produce **byte-for-byte identical**
canonical envelope bytes — pinned by a cross-language fixture test that
fails CI if they ever diverge.

Everything is dual-licensed MIT OR Apache-2.0.

---

## Proven on a live LEZ sequencer — in CI

The [**LEZ live-stack workflow**](.github/workflows/lez-live.yml) is not a
mock. On every push it boots a real `logos-scaffold` sequencer, builds
the RISC0 guest with **`RISC0_DEV_MODE=0` (real proofs)**, and deploys
the registry program to the running chain:

```json
{ "program": "whistleblower_registry",
  "program_id": "80aefab4d46df398f94e525fc78a4968a96512faa26041196cb5df27f75b83ec",
  "status": "submitted" }
```

That `program_id` is the RISC0 image ID computed from the deployed guest
ELF — deterministic from the binary, verifiable by anyone who rebuilds
it. The same job then runs the end-to-end demo and the compute-unit
benchmark against the live sequencer.

The standard [**CI workflow**](.github/workflows/ci.yml) runs `rustfmt`,
`clippy -D warnings`, **9 Rust tests**, **11 TypeScript tests**, and the
demo on every push. Both workflows are green on `main`.

---

## Evaluate it in 60 seconds

```bash
# from a clean clone — no external services required
cargo build --workspace --release
pnpm install
bash scripts/demo.sh
```

Expected tail:

```
PUBLISH ok  cid=bafy…  size=109  topic=/logos/whistleblower/v1/documents
ANCHOR  ok  tx=tx-1  newly_anchored=1  cu=Some(7000)
LOOKUP  ok  cid=bafy… present_in_registry=true
════════════════════════════════════════════════
  DEMO COMPLETE
  • upload     → CID issued by Logos Storage
  • broadcast  → envelope on /logos/whistleblower/v1/documents
  • batch-anchor → CID present in registry
════════════════════════════════════════════════
```

For the live LEZ path (`make build && make deploy` the registry, then
`RISC0_DEV_MODE=0 MODE=LEZ bash scripts/demo.sh`), see
[`docs/DEMO.md`](docs/DEMO.md).

---

## Success criteria → artefact

Every box is exercised by CI; each links to the source that satisfies it.

**Functionality**
- [x] Upload to Logos Storage → CID, with retry — [`publisher.rs`](packages/whistleblower-core/src/publisher.rs)
- [x] Broadcast metadata envelope on a Delivery topic, idempotent on `(topic, cid)` — [`envelope.rs`](packages/whistleblower-core/src/envelope.rs)
- [x] Optional on-chain anchor, distinct from upload — `publisher.anchor()` is a separate call
- [x] Standalone, permissionless batch-anchor CLI, ≤50 CIDs/tx — [`batch-anchor-cli`](packages/batch-anchor-cli/src/main.rs)
- [x] Registry stores `(cid, metadata_hash, anchor_timestamp)`, queryable via `lookup` — [`whistleblower_registry.rs`](packages/registry-program/methods/guest/src/bin/whistleblower_registry.rs)
- [x] Self-contained, reusable indexing module — zero coupling to the app

**Usability**
- [x] Basecamp GUI: QML manifest + entry point **and** a buildable web preview — [`basecamp-app/`](packages/basecamp-app/)
- [x] Indexing module shipped as a library/SDK with API README — Rust + [TypeScript](packages/indexing-module/README.md)
- [x] Checked-in IDL, regenerable via `make idl` — [`idl/`](packages/registry-program/idl/whistleblower-registry-idl.json)

**Reliability**
- [x] Jittered exponential back-off on upload, surfaces final error — [`retry.rs`](packages/whistleblower-core/src/retry.rs)
- [x] Delivery dedup + CLI resumes from last batch after interruption — regression tests in [`pipeline.rs`](packages/whistleblower-core/tests/pipeline.rs)

**Performance**
- [x] Measured CU for single + 50-CID batch — [`docs/BENCHMARKS.md`](docs/BENCHMARKS.md). Batching 50 CIDs is far cheaper per-CID than 50 single anchors.

**Supportability**
- [x] Registry deployed + tested on a live LEZ sequencer — see the [live-stack workflow](.github/workflows/lez-live.yml)
- [x] End-to-end demo with `RISC0_DEV_MODE=0` against a real sequencer — [`docs/DEMO.md`](docs/DEMO.md)
- [x] CI green on `main`; README covers build, deploy, running the app + batch tool, querying the registry

---

## Reuse the indexing module from your own Logos app

```rust
// Cargo.toml: whistleblower-core = { git = "https://github.com/chinesepowered/hack-lp0017" }
use std::sync::Arc;
use whistleblower_core::*;

let publisher = Publisher::new(
    Arc::new(my_storage),       // your StorageAdapter
    Arc::new(my_delivery),      // your DeliveryAdapter
    Some(Arc::new(my_anchor)),  // your AnchorAdapter (optional)
    PublisherConfig::default(),
);

let result = publisher.publish(bytes, PublishMeta {
    title: "…".into(), description: "…".into(),
    content_type: "application/pdf".into(), tags: vec!["leak".into()],
}).await?;
// result.envelope.cid is on Storage + broadcast on Delivery.
// result.metadata_hash is what anyone can later commit on-chain.
```

Every arrow in the pipeline is an adapter trait. CI exercises the
in-memory adapters end-to-end; the live adapters in
[`batch-anchor-cli/src/adapters/`](packages/batch-anchor-cli/src/adapters/)
and the `window.logos.bridge` web shim call the real services. The SPEL
program follows the [`logos-co/whisper-wall`](https://github.com/logos-co/whisper-wall)
patterns and pins LEZ `v0.2.0-rc3`. For TypeScript or native C++/QML
callers, see [`packages/indexing-module/README.md`](packages/indexing-module/README.md)
and [`packages/whistleblower-core/src/ffi.rs`](packages/whistleblower-core/src/ffi.rs).

---

## Repository layout

```
.
├── README.md, docs/{ARCHITECTURE,DEMO,BENCHMARKS}.md
├── Cargo.toml                       # Rust workspace · package.json (pnpm workspace)
├── scripts/{demo,benchmark}.sh      # reproducible demo + CU benchmark
├── .github/workflows/
│   ├── ci.yml                       # rustfmt + clippy + tests + demo
│   └── lez-live.yml                 # real deploy to a live LEZ sequencer
└── packages/
    ├── whistleblower-core/          # canonical Rust indexing module + FFI
    ├── indexing-module/             # TypeScript port + in-memory adapters
    ├── registry-program/            # SPEL program for LEZ (+ IDL, Makefile)
    ├── batch-anchor-cli/            # Rust binary; mock + LEZ-live backends
    └── basecamp-app/                # Basecamp web + QML manifest
```

---

## License

Dual-licensed under **MIT OR Apache-2.0** at your option. See
[`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE).
