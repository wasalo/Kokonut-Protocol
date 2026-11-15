# Kokonut patch: `near-kit` 0.20.1

This vendored crate is copied from the crates.io archive `near-kit-0.20.1.crate`.
Its SHA-256 is `294bd31dcea04d2ae423cf2d499737ebabd6d8da04eeba5a13f1c45a646ea0cf`, matching the checksum in crates.io's version metadata.

## Local changes

1. The target-specific `reqwest` dependency in `Cargo.toml` sets `default-features = false`; the test client needs JSON over local HTTP, not TLS.
2. Four upstream `#[cfg(test)]` functions containing credential-shaped fixture strings are omitted: `test_parse_full_format` and `test_parse_simple_format` in `src/client/keyring_signer.rs`, plus `test_parse_rpc_error_unknown_access_key_experimental` and `test_parse_rpc_error_unknown_gas_key` in `src/client/rpc.rs`. These functions are not compiled when `near-kit` is a dependency. Their fixture strings are not retained; no production code was changed.
3. Five trailing-whitespace-only blank Rustdoc lines in `src/client/near.rs` were cleaned so the vendored source passes `git diff --check`; this has no behavior effect.

Every other retained package/source file was byte-compared against the checksum-verified crate archive. The upstream `Cargo.lock` is omitted because this library crate is a path dependency and its development lock does not control the parent project; the active graph is pinned by `contracts/near/Cargo.lock`. The 0.20.0 migration to `near-account-id` 3 and `near-global-contracts` 0.3 is upstream's reviewed breaking change; this repository only uses `Near`, `CryptoHash`, and transaction status types in its sandbox test, and the updated code is validated against the local 2.14.1 sandbox.

The sandbox harness uses `near-kit` only against the sandbox's loopback HTTP RPC. This patched development dependency is intentionally not suitable for public RPC, HTTPS endpoints, or application transport. Any future need for HTTPS requires a separate feature/provenance review and a compatible TLS provider/dependency resolution. The patch does not change the NEAR contract's runtime or Wasm dependency graph.

The upstream package manifest declares `MIT OR Apache-2.0`. Upstream package: `https://crates.io/crates/near-kit/0.20.1`.
