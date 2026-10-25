# Kokonut patch: `near-kit` 0.19.0

This vendored crate is copied from the crates.io archive `near-kit-0.19.0.crate`.
Its SHA-256 is `6ac835efbcc11c23fdee426155995a7316c4f893ec05f6c0c5d355182795ac92`, matching the archive checksum recorded in the pre-patch `contracts/near/Cargo.lock`.

## Local changes

1. The target-specific `reqwest` dependency in `Cargo.toml` sets `default-features = false`; the test client needs JSON over local HTTP, not TLS.
2. Four upstream `#[cfg(test)]` functions containing credential-shaped fixture strings are omitted: `test_parse_full_format` and `test_parse_simple_format` in `src/client/keyring_signer.rs`, plus `test_parse_rpc_error_unknown_access_key_experimental` and `test_parse_rpc_error_unknown_gas_key` in `src/client/rpc.rs`. These functions are not compiled when `near-kit` is a dependency. Their fixture strings are not retained; no production code was changed.
3. Five trailing-whitespace-only blank Rustdoc lines in `src/client/near.rs` were cleaned so the vendored source passes `git diff --check`; this has no behavior effect.

Every other archive file was byte-compared against the checksum-verified crate archive.

The sandbox harness uses `near-kit` only against the sandbox's loopback HTTP RPC. This patched development dependency is intentionally not suitable for public RPC, HTTPS endpoints, or application transport. Any future need for HTTPS requires a separate feature/provenance review and a compatible TLS provider/dependency resolution. The patch does not change the NEAR contract's runtime or Wasm dependency graph.

The upstream package manifest declares `MIT OR Apache-2.0`. Upstream package: `https://crates.io/crates/near-kit/0.19.0`.
