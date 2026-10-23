# NEAR Hub contract

Initial registry slice for the Kokonut Protocol Hub. It currently supports:

- Administrator-gated chain registration and enable/disable status.
- Project registration against a profile hash and explicitly enabled destination chains.
- Administrator-gated project activation status.
- Structured events for registry changes.
- A pause flag reserved for the execution layer; no execution methods exist yet.

The contract does not yet request NEAR Chain Signatures, deploy destination contracts, or hold or distribute funds. Do not treat this crate as production-ready.

## Administrator boundary

The contract authorizes the configured `administrator` account ID. For any deployment, that account must be controlled by the approved Kokonut governance/multisig process. The contract does not itself implement a multisignature quorum, timelock, or admin-rotation workflow; those are deployment/security gates before production use.

## Build and test

Rust is pinned by `rust-toolchain.toml`; `Cargo.lock` pins dependency resolution.

```sh
cargo fmt --check
cargo test --locked
cargo build --locked --release --target wasm32-unknown-unknown
```
