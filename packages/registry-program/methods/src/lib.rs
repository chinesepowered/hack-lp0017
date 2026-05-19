//! Build-time outputs for the Whistleblower registry SPEL program.
//!
//! Mirrors the `methods/` crate pattern from logos-co/whisper-wall.
//! `build.rs` invokes `risc0-build` (when the `build-guest` feature is on
//! and the RISC0 toolchain is installed) to compile the guest ELF into
//! a deterministic `whistleblower_registry.bin`. The resulting bytes are
//! re-exported here as constants for in-process testing.

#[cfg(feature = "build-guest")]
include!(concat!(env!("OUT_DIR"), "/methods.rs"));

#[cfg(not(feature = "build-guest"))]
pub const WHISTLEBLOWER_REGISTRY_ELF: &[u8] = &[];

#[cfg(not(feature = "build-guest"))]
pub const WHISTLEBLOWER_REGISTRY_ID: [u32; 8] = [0; 8];
