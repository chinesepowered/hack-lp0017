// Only invokes risc0-build when the build-guest feature is on AND we're
// not running `cargo check` in an environment without the RISC0 toolchain.
// In CI the guest is built explicitly via `make build` inside the
// program's guest directory, which doesn't rely on this build.rs.

#[cfg(feature = "build-guest")]
fn main() {
    risc0_build::embed_methods();
}

#[cfg(not(feature = "build-guest"))]
fn main() {}
