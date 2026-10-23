# NEAR Hub contract

Initial NEAR Hub registry and policy-controller slice. It currently supports:

- Administrator-gated chain registration and enable/disable status.
- Project registration against a profile hash and explicitly enabled destination chains.
- Administrator-gated project activation status.
- Versioned signer bindings scoped to project, destination chain, and purpose, with explicit activation and preserved history during rotation.
- Immutable, versioned execution policies scoped to project and chain, with operator/approver roles, operation/target allowlists, a per-request uint256 value cap, and a SHA-256 policy commitment.
- Policy-checked execution proposals with request-ID and idempotency-key replay protection.
- Exact-request-hash approvals with distinct approvers, self-approval prevention, and threshold-based status transition.
- Proposer/admin cancellation and permissionless expiration with structured events.
- A pause flag that blocks proposals and approvals.

Approval records do not request NEAR Chain Signatures and do not mean a destination transaction was signed, broadcast, finalized, or reconciled. Destination nonce leasing/reservation, daily cumulative spend limits, relayers, destination finality, and effect verification are not implemented. The per-request policy cap is not a cumulative budget.

The contract does not yet verify that a recorded signer address was actually derived from its path, deploy destination contracts, or hold or distribute funds. Do not treat this crate as production-ready.

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
