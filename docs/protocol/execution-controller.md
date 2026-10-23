# NEAR Hub execution proposal and approval slice

This document describes the current contract behavior. It is a policy/control-plane record, not an execution or custody system.

## Proposal validation

`propose_execution` accepts the `ExecutionRequest` shape from `specs/execution-request.schema.json`. The contract:

- Requires protocol version `1`, UUID-shaped request and project IDs, canonical decimal `uint256` values, a valid 32-byte calldata hash, and an RFC 3339 expiry after the current NEAR block timestamp.
- For EVM-family requests, requires a positive `gas_limit` and a 32-byte `signing_payload_hash`; other chain families omit these EVM-specific fields.
- Requires an active project and enabled, project-approved destination chain.
- Requires the caller to be listed as an operator in the active policy for that project and chain.
- Enforces the active policy version, operation allowlist, target allowlist, and per-request value cap. EVM target comparisons are case-insensitive.
- Requires `max_priority_fee <= max_fee` and rejects reused request IDs or idempotency keys.
- Stores a commitment to the exact request and emits `execution_proposed`.

The request's action hash is SHA-256 over the compact JSON bytes serialized from the typed `ExecutionRequest` structure, including its nested `GasPolicy`. Approvers must provide the exact stored hash; approvals cannot be transferred to a modified request.

## EVM signing-payload verification

For `eip155` requests, `gas_limit` and `signing_payload_hash` are required and included in that approved action hash. The `signing_payload_hash` is the Keccak-256 digest of the EIP-1559 type-2 unsigned signing payload reconstructed from chain ID, nonce, priority/max fee, gas limit, destination, value, and supplied calldata. Protocol v1 fixes the EIP-2930 access list to empty.

Before passing a digest to any signer, the relayer must call `protocol.evm_signing_payload.prepare_approved_eip1559_signing_payload` with the NEAR execution record, the current policy record for the same project and chain, the latest block timestamp obtained from its trusted NEAR read path, and the exact calldata bytes. It fails closed unless the execution record is `Approved`, its request reproduces the on-chain action hash, the request is unexpired, the active policy scope/version/hash matches the proposal, and both calldata and transaction signing hashes match. It returns the digest only after verification; the future signer-calling contract must still recheck pause status and project/chain activity at request time. This helper performs no signing or broadcasting.

The NEAR contract validates the presence and format of the EVM digest but does not itself reconstruct the EVM transaction. The client-side verifier is therefore a mandatory pre-signing control, not a substitute for the future on-chain signature-request gate.

## Approval and terminal pre-signing states

- Only listed approvers may approve; the proposer cannot approve their own request.
- Each approver can vote once. The request becomes `Approved` only after the configured distinct-approver threshold is reached.
- Approval rechecks pause status, project and chain activity, and that the request's immutable policy version is still the active version.
- The proposer or administrator may cancel before expiry. Anyone may mark an expired request `Expired`; approval and cancellation also transition a request to `Expired` if its deadline has passed.
- Statuses in this slice are `AwaitingApproval`, `Approved`, `Cancelled`, and `Expired`.

## Explicit limits

`Approved` means only that the on-chain policy quorum approved the exact request commitment. It does not mean a Chain Signature was requested or issued, a destination transaction was broadcast, or any destination state changed.

This slice does not implement:

- Chain Signature requests or verification of derived addresses.
- Destination nonce reservation/leases. `nonce` is validated and committed, but is not reserved against concurrent requests.
- Daily or cumulative project spending caps; the configured amount is a per-request cap only.
- Relaying, receipt/finality tracking, effect verification, or reconciliation into Kokonut Intelligence.
- Destination-chain authorization or contract behavior.

Kokonut Intelligence remains canonical for operational records, accounting, evidence, and human approval workflows. The NEAR administrator must be governed by the approved multisig process; this contract does not implement its own multisig or timelock.
