# NEAR Hub contract

Initial NEAR Hub registry and policy-controller slice. It currently supports:

- Administrator-gated chain registration and enable/disable status.
- Project registration against a profile hash and explicitly enabled destination chains.
- Administrator-gated project activation status.
- Versioned signer bindings scoped to project, destination chain, and purpose, with explicit activation and preserved history during rotation.
- Immutable, versioned execution policies scoped to project and chain, with operation/target allowlists, a per-action uint256 value cap, approver accounts, a threshold, and a SHA-256 commitment.
- Structured events for registry and policy changes.
- A pause flag reserved for the execution layer; execution-request and signature methods do not exist yet.

The contract does not yet request NEAR Chain Signatures, verify that a recorded address was actually derived from its path, deploy destination contracts, or hold or distribute funds. Do not treat this crate as production-ready.

## Administrator boundary

The contract authorizes the configured `administrator` account ID. For any deployment, that account must be controlled by the approved Kokonut governance/multisig process. The contract does not itself implement a multisignature quorum, timelock, or admin-rotation workflow; those are deployment/security gates before production use.

## Build and test

Rust is pinned by `rust-toolchain.toml`; `Cargo.lock` pins dependency resolution. NEAR-aware checks and builds use `cargo-near` 0.22.0 so the `near-sdk` host configuration and Wasm optimization are applied.

```sh
cargo fmt --check
cargo near check --locked
cargo test --locked
cargo near build non-reproducible-wasm --locked --out-dir target/deploy
```

The non-reproducible build is for development/CI only; do not deploy it as a production release artifact.
