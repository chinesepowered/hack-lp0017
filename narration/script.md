# Whistleblower — narration script (~1:15)

_Stage directions are in italic-only lines and are stripped before TTS._

_SHOW: README.md, scroll slowly through the title and "What we built" table_

This is Whistleblower, a censorship-resistant document upload and indexing app for the Logos stack. A user selects a file, adds metadata, and submits. The app uploads the bytes to Logos Storage, broadcasts the CID and metadata envelope over Logos Delivery, and optionally anchors the CID on-chain via a SPEL program on the Logos Execution Zone.

_SHOW: README.md, scroll to the pipeline diagram_

The key design decision is that anchoring is decoupled from publication. The uploader does not need tokens or any coordination. Any third party can run the batch-anchor CLI, subscribe to the delivery topic, accumulate CIDs, and commit them on-chain in batches of up to fifty per transaction.

_SHOW: docs/ARCHITECTURE.md, scroll to the idempotence section_

Idempotence is enforced at three layers: delivery dedup, the CLI's persisted seen-set, and the registry's init-if-empty PDA check. Re-submitting an already-anchored CID is always a no-op.

_SHOW: terminal running the demo with RISC0_DEV_MODE=0_

Here is the end-to-end demo running against a live LEZ sequencer with RISC0 dev mode zero — real proof generation. The file uploads, the envelope broadcasts on the topic, the batch anchor picks up the CID and submits an anchor-batch instruction, and the lookup confirms it is registered on-chain with its metadata hash and timestamp.

_SHOW: CI green checks or terminal showing all tests pass_

All tests pass — eight Rust, eleven TypeScript, and the full end-to-end demo. That is the complete submission. Link in the description.
