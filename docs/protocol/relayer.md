# Testnet relayer orchestration

`protocol/relayer.py` provides a single-attempt, synchronous `ExecutionRelayer` that coordinates the existing NEAR payload verifier and EVM adapter. It deliberately owns no queue, workflow database, keys, or chain transport. The future KI integration should call this worker from the existing durable event system rather than add a second queue.

## One attempt

1. Read an approved execution, current policy, NEAR block timestamp, exact committed calldata, signer address, and expected-effect predicate through `SnapshotReader`.
2. Reconstruct and validate the approved EIP-1559 digest and effect commitment before reserving a nonce or requesting a signature.
3. Ask `SignatureRequester` for the signature bound to the request ID, action hash, and digest.
4. Re-read the NEAR execution and policy. Reconstruct the digest again; stop if approval, expiry, policy, or any committed transaction payload changed.
5. Prepare and broadcast only through `EvmAdapter`. The adapter revalidates the fresh approval and policy, verifies the signature and derived address, and reuses the persisted signed bytes on a retry.
6. Return the adapter's finality state. Verify the expected effect only after consensus finality; an effect mismatch returns `manual_review`.

`process()` is not a polling loop. A `pending`, `confirming`, or `confirmation_threshold_met` result is not finality and does not verify the effect. The caller should schedule another durable attempt. Exceptions are allowed to propagate to the caller's existing retry/dead-letter handling.

## Adapter contracts

- `SnapshotReader.read(request_id)` must return current state, not a cached approval. It must reject missing or mismatched request IDs.
- `SignatureRequester.request_signature(request_id, action_hash, digest)` must be status-aware and replay-safe. Read the NEAR record before submitting a write: return a stored `Issued` result; while `Pending`, wait/poll without making another request; submit `request_signature` only for a state the contract permits (`NotRequested` or retryable `Failed`). On an ambiguous timeout, read status before retrying. The digest is an expected correlation value; the contract signs the digest committed in the approved request.
- Do not bypass the adapter's durable nonce lease, EIP-1559 simulation, signature recovery, signed-byte persistence, broadcast hash check, canonical receipt check, finality gate, or effect verification.

## Current limits

This is only the offline orchestration boundary. No concrete NEAR RPC or signature-request client is included, and no live RPC, NEAR signature request, EVM broadcast, or deployment is performed by the worker tests. Testnet-only network restrictions remain enforced by `EvmAdapter`. Kokonut Intelligence remains canonical for operational state and approvals; its refactor and bridge integration remain separate work.
