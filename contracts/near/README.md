# NEAR Hub contract

Initial NEAR Hub registry, policy-controller, and testnet-only Chain Signatures request slice. It currently supports:

- Administrator-gated chain registration and enable/disable status.
- Project registration against a profile hash and explicitly enabled destination chains.
- Administrator-gated project activation status.
- Versioned signer bindings scoped to project, destination chain, and purpose, with explicit activation and preserved history during rotation.
- Immutable, versioned execution policies scoped to project and chain, with operator/approver roles, operation/target allowlists, a per-request uint256 value cap, and a SHA-256 policy commitment.
- Policy-checked execution proposals with request-ID and idempotency-key replay protection.
- Exact-request-hash approvals with distinct approvers, self-approval prevention, and threshold-based status transition.
- EVM requests commit a `u32` signer key version (including `0`); `request_signature` rejects any active-binding rotation that no longer matches the approved version.
- Proposer/admin cancellation and permissionless expiration with structured events.
- A pause flag that blocks proposals, approvals, and signature requests.
- Administrator configuration of a **testnet-only** Chain Signatures contract and its secp256k1 public key.
- `request_signature`, which rechecks approval, expiry, pause, active project/chain/policy and signer binding, then sends only the approved 32-byte EVM signing digest, derivation path, and active `key_version` to the configured testnet contract.
- Asynchronous signature success/failure handling, with `NotRequested`, `Pending`, `Issued`, and `Failed` signature states. The raw response and configured public-key snapshot are retained with the execution record; a failed request may be retried, while a pending or issued request cannot be requested again.

The administrator must configure the Chain Signatures contract account and secp256k1 public key from verified testnet deployment data; neither is hard-coded. Each request forwards exactly 1 NEAR and allocates 250 Tgas to the signer plus 20 Tgas to the callback; confirm those budgets against the target testnet deployment before live use. The signature callback validates the expected response shape, but it does not cryptographically recover the EVM signer address or prove that it matches the configured public key or registered derived address; the separate testnet EVM adapter performs signature recovery and address matching before transaction assembly or broadcast.

A signature is not execution. Once the asynchronous signing request has been sent, pausing or changing policy cannot revoke a signature already produced by the external signer. Relayers must re-read the current NEAR request and policy before using any returned signature.

The contract does not call an EVM chain or broadcast transactions. The separate testnet adapter implements EVM nonce leasing, assembly, fee estimation, simulation, broadcast, finality checks, and expected-effect verification. Kokonut Intelligence reconciliation, cumulative spend limits, and financial contracts are not implemented. The per-request policy cap is not a cumulative budget.

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
