//! Concrete adapters that wire `whistleblower-core` to live Logos services.
//!
//! These adapters are intentionally thin: they translate between the
//! generic adapter traits and the actual RPC / FFI surface of the running
//! sequencer + delivery module. They are only compiled into the binary —
//! the `whistleblower-core` library never imports them, keeping the core
//! reusable from any environment.
//!
//! ## Status
//!
//! - `delivery_lez` ships a JSON-RPC client speaking to the
//!   `logos-delivery-module` IPC bridge. The bridge runs as a sidecar of
//!   the LEZ sequencer and forwards `send`/`subscribe` calls to the C++
//!   delivery module. When `LOGOS_DELIVERY_RPC` is unset the module
//!   falls back to in-memory delivery (useful for hermetic CI).
//!
//! - `anchor_lez` invokes `spel` as a subprocess against the live
//!   sequencer (the same pattern whisper-wall uses). It expects a wallet
//!   at `NSSA_WALLET_HOME_DIR` and a deployed registry program.

pub mod anchor_lez;
pub mod delivery_lez;
