# NEAR Hub execution proposal and approval slice

This document describes the current contract behavior. It is a policy/control-plane record, not an execution or custody system.

## Proposal validation

`propose_execution` accepts the `ExecutionRequest` shape from `specs/execution-request.schema.json`. The contract:

- Requires protocol version `1`, UUID-shaped request and project IDs, canonical decimal `uint256` values, a valid 32-byte calldata hash, and an RFC 3339 expiry after the current NEAR block timestamp.
- For EVM-family requests, requires a positive `gas_limit`, a 32-byte `signing_payload_hash`, a `signer_key_version` (`u32`, including version `0`), and a 32-byte `expected_effect_hash`; other chain families omit these EVM-specific fields.
- Requires an active project and enabled, project-approved destination chain.
- Requires the caller to be listed as an operator in the active policy for that project and chain.
- Enforces the active policy version, operation allowlist, target allowlist, and per-request value cap. EVM target comparisons are case-insensitive.
- Requires `max_priority_fee <= max_fee` and rejects reused request IDs or idempotency keys.
- Stores a commitment to the exact request and emits `execution_proposed`.

The request's action hash is SHA-256 over the compact JSON bytes serialized from the typed `ExecutionRequest` structure, including its nested `GasPolicy`. Approvers must provide the exact stored hash; approvals cannot be transferred to a modified request.

## EVM signing-payload verification

For `eip155` requests, `gas_limit`, `signing_payload_hash`, `signer_key_version`, and `expected_effect_hash` are required and included in that approved action hash. The key version binds approval to a versioned signer binding; if the active binding rotates before `request_signature`, the request fails rather than silently switching keys. The `signing_payload_hash` is the Keccak-256 digest of the EIP-1559 type-2 unsigned signing payload reconstructed from chain ID, nonce, priority/max fee, gas limit, destination, value, and supplied calldata. Protocol v1 fixes the EIP-2930 access list to empty. `expected_effect_hash` commits to a canonical event, storage, or deployed-code predicate that must be checked after destination finality.

Before passing a digest to any signer, the relayer must call `protocol.evm_signing_payload.prepare_approved_eip1559_signing_payload` with the NEAR execution record, the current policy record for the same project and chain, the latest block timestamp obtained from its trusted NEAR read path, and the exact calldata bytes. It fails closed unless the execution record is `Approved`, its request reproduces the on-chain action hash, the request is unexpired, the active policy scope/version/hash matches the proposal, and both calldata and transaction signing hashes match. It returns the digest only after verification; this helper performs no signing or broadcasting.

The contract's testnet-only `request_signature` method independently rechecks pause status, expiry, approved status/action hash, active project and chain, active policy version/hash, and the active EVM signer binding; it rejects if the active `key_version` differs from the request's committed `signer_key_version`. It requires an operator or administrator caller and exactly 1 NEAR, which is forwarded to the configured testnet signer. The `sign` request contains only the committed 32-byte digest as a byte array, the registered derivation path, and the active binding's `key_version`. The administrator-configured public key is snapshotted in the execution record for downstream signature verification; it is not sent as a signing argument. The current call allocates 250 Tgas to the signer and 20 Tgas to the callback; confirm these budgets against the configured testnet deployment before live use.

The contract requires a canonical 32-byte digest that is a valid secp256k1 scalar (strictly below the curve group order), but does not itself reconstruct the EVM transaction. The client-side verifier is therefore a mandatory pre-signing control; the on-chain gate verifies the stored approval and policy but does not replace digest reconstruction.

## Approval and terminal pre-signing states

- Only listed approvers may approve; the proposer cannot approve their own request.
- Each approver can vote once. The request becomes `Approved` only after the configured distinct-approver threshold is reached.
- Approval rechecks pause status, project and chain activity, and that the request's immutable policy version is still the active version.
- The proposer or administrator may cancel before expiry. Anyone may mark an expired request `Expired`; approval and cancellation also transition a request to `Expired` if its deadline has passed.
- Execution statuses are `AwaitingApproval`, `Approved`, `Cancelled`, and `Expired`; the separate signature statuses are `NotRequested`, `Pending`, `Issued`, and `Failed`.

A failed external request may be retried; pending and issued requests cannot be requested again. Cancelling or expiring an execution after a signature request does not revoke a signature that the external signer has already produced. Relayers must check current NEAR state and policy before using it.

## Explicit limits

`Approved` means only that the on-chain policy quorum approved the exact request commitment. Signature status is tracked separately. `Issued` means the configured testnet signer returned the expected `big_r.affine_point`, `s.scalar`, and `recovery_id` response shape; the contract does not cryptographically verify the signature or recover the EVM address to prove it matches the configured public key or registered derived address. Neither status means a destination transaction was broadcast, finalized, or reconciled.

The NEAR contract is only the policy/signature-request controller; it does not call an EVM chain or broadcast destination transactions. The separate client-side adapter is testnet-only and supports Base Sepolia and Gnosis Chiado transaction construction, nonce leases, fee estimation, simulation, broadcast, consensus-finality reads, and effect checks. It has only been exercised with mocked JSON-RPC in this repository; no live testnet transaction was sent.

The combined implementation does not include:

- A live NEAR signer deployment or end-to-end request against a funded NEAR account.
- Verification that the registered signer address was derived from the configured Chain Signatures path.
- Daily or cumulative project spending caps; the configured amount is a per-request cap only.
- Reconciliation of destination transaction/effect records into Kokonut Intelligence.
- Destination-chain authorization or contract behavior.

See [the EVM adapter notes](evm-adapter.md) for nonce, broadcast, finality, effect, and recovery controls. Kokonut Intelligence remains canonical for operational records, accounting, evidence, and human approval workflows. The NEAR administrator must be governed by the approved multisig process; this contract does not implement its own multisig or timelock.
