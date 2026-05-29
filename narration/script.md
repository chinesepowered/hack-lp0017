# Whistleblower — narration script (~1:30)

_Stage directions are italic-only lines and are stripped before TTS._

_SHOW: README.md top — title, CI badges, one-line pitch._

This is Whistleblower — a censorship-resistant document upload and indexing app for the Logos stack, built as a Basecamp app. A user picks a file, adds metadata, and submits. The app uploads the bytes to Logos Storage, broadcasts the CID and metadata over Logos Delivery so it's instantly discoverable, and can anchor the CID on-chain through a SPEL program on the Logos Execution Zone.

_SHOW: scroll slowly to the pipeline diagram._

The key decision is that anchoring is decoupled from publication. The uploader needs no tokens and no coordination. Any third party can run the batch-anchor CLI, accumulate CIDs from the topic, and commit up to fifty per transaction — idempotently, enforced at three independent layers.

_SHOW: scroll to the "What we built" table._

It ships the Basecamp app, a reusable indexing module in Rust and TypeScript with byte-identical envelopes, the SPEL registry program with its IDL, and the permissionless batch-anchor CLI.

_SHOW: scroll to "Proven on a live LEZ sequencer", then cut to the GitHub Actions page._

Now the proof — and it isn't mocked. On every push, the LEZ live-stack workflow boots a real sequencer, builds the RISC0 guest with dev-mode off, meaning real proofs, and deploys the registry to the running chain.

_SHOW: the green lez-live run; expand the deploy step showing the program_id JSON._

Here's the green run. Deploy returns a real program ID — the RISC0 image ID computed from the binary, so anyone can rebuild and verify it. Standard CI also runs clippy, nine Rust tests, and eleven TypeScript tests, all green on main. That's the submission. Link in the description.
