# Protocol Execution Recovery Runbook

**Status:** Development/testnet runbook. It is not authorization to operate a production relayer, sign transactions, broadcast to a destination chain, or deploy contracts.

## Scope and system boundary

This document covers recovery decisions for the current offline Protocol execution relayer and testnet EVM adapter. `ExecutionRelayer` makes one attempt; it does not own a queue. The adapter persists nonce-lease state and exact signed transaction bytes in SQLite. Future KI integration must use the existing KI durable event system and canonical execution records. KI remains authoritative for approval, operations, accounting, evidence, and reconciliation.

There is currently no integrated production event consumer, operator CLI, production database backend, live signature-request client, or production-network profile in this repository. Do not infer that this document or the mocked recovery tests make the system production-ready.

## Non-negotiable recovery invariants

- Preserve the approved request, action hash, active policy, signature status, nonce lease, and exact signed bytes as separate evidence.
- Never allocate a replacement nonce, edit an approved payload, or create a replacement transaction merely because a response timed out.
- A retry of an existing signed transaction must reuse the exact persisted signed bytes. Never re-sign as a recovery shortcut.
- A transaction hash is not proof of inclusion; inclusion is not finality; finality is not effect verification; effect verification is not KI reconciliation.
- Do not mark an execution reconciled from the Protocol adapter. That requires the future governed KI reconciliation path.
- Preserve lease and event records during investigation. Do not delete, reset, or hand-edit the nonce database to clear a stuck state.
- Stop for human review on stale or changed approval/policy, expired requests, an unexpected chain/signer/nonce/hash, a reorg, an effect mismatch, corrupt or missing durable state, or any unknown lease state.
- Do not place credentials, private keys, signature material, raw signed transactions, or private farm/economic data in tickets or chat. Raw signed bytes are sensitive operational data and must remain access-controlled.

## Incident procedure

### 1. Contain without destroying evidence

If an execution appears ambiguous, prevent automatic redelivery of that specific item while investigating. Once deployed behind KI's durable event system, use its authorized pause/dead-letter controls and preserve the event and lease. The current Protocol repository has no event consumer or operator command for doing this; do not invent or run a service-control command based on this document.

Do not globally disable unrelated farm operations unless the incident scope requires it and the authorized operator approves that action.

### 2. Record a non-secret incident reference

Capture, in the approved incident system:

- Protocol request ID and event-delivery ID, if available.
- Chain namespace and configured chain ID.
- Current request, approval, policy, and expiry states.
- Nonce-lease state and nonce.
- Expected action/effect commitments and transaction hash, if present.
- Last observed error category and timestamps.
- Software commit and adapter/profile version.

Do not copy the raw signed transaction or endpoint credentials into the incident record. If raw bytes must be examined, use the approved restricted-access procedure and record only the derived transaction hash in ordinary notes.

### 3. Read and cross-check durable state

Use read-only access first. Read the current approved execution and policy from the trusted snapshot source, then inspect the matching durable nonce lease. Confirm that request ID, chain ID, signer address, committed nonce, action/effect commitments, and any transaction hash agree. A missing lease or an unreadable database is an incident condition—not proof that no transaction was signed or broadcast.

The relayer's `get_nonce_lease(request_id)` recovery path checks existing lease state before attempting another signature request. Do not bypass it by calling lower-level broadcast methods manually.

### 4. Decide by persisted lease state

| Lease state | Safe interpretation | Recovery decision |
|---|---|---|
| No lease | No matching lease was read; other state may be missing or damaged. | Stop and investigate the approval, database health, and destination-chain history. Do not infer that no broadcast occurred. |
| `reserved` | A nonce is leased; signed bytes may not yet exist. | Revalidate the approved request, current policy, expiry, and signature status. Do not reserve a second nonce. If state is inconsistent, route to manual review. |
| `signed` | Exact signed bytes are durable; broadcast may not have happened or its acknowledgement may have been lost. | Resolve destination status using the configured adapter. If retry is authorized and current gates still pass, resume through the relayer/adapter so only the persisted bytes are reused. Never construct a changed transaction. |
| `broadcast` | A broadcast was acknowledged, but finality may remain unknown. | Check the recorded transaction and canonical receipt/finality. If another attempt is needed, use only the same persisted bytes and adapter recovery path. Do not treat a timeout or transaction hash as success. |
| `confirmed` | Adapter finality and expected-effect verification were persisted. | Replay is read-only/idempotent: the relayer validates the stored request/chain/signer/nonce/effect/hash relationship and returns the prior finalized result without rebroadcasting. Continue with the authorized KI reconciliation process when available. |
| `manual_review` | Automated processing must stop. | Route `ManualReviewRequired` to human review. Resolve the discrepancy through an explicit governed decision; do not automatically retry or clear the lease. |

An unexpected state, mismatched commitment, stale approval, changed policy, expired request, reorg, RPC disagreement, or effect mismatch always stops automated recovery.

### 5. Verify finality and the expected effect

A human/operator reviewing a testnet incident should verify, through the configured adapter and independent read-only chain observations where available:

1. The transaction hash corresponds to the persisted signed bytes.
2. The receipt belongs to the expected transaction and its block is currently canonical.
3. The chain-specific consensus finality requirement is met. A confirmation-depth threshold alone is not consensus finality in this implementation.
4. The destination state/event matches the exact committed expected-effect predicate.
5. The request and policy are still valid for any further attempt.

If any point is inconclusive, keep the item stopped for review. Do not infer successful execution from a signature, RPC acknowledgement, receipt alone, or an explorer display.

### 6. Resume only through the governed worker

A retry is permitted only when the approved request and active policy still validate, the request has not expired, the signature provider's status-aware rules permit the operation, and the adapter can resume from the existing durable lease. The worker must re-read and validate current approval/policy before transaction preparation and broadcast.

If an approved request has expired or its action/policy changed, do not edit it or reuse its old signature. Obtain a new human-governed request through the authorized workflow. This is design guidance; that KI workflow is not implemented in this repository yet.

After adapter finality/effect verification, the future KI event handler must persist/reconcile the result idempotently before acknowledging the durable event. That bridge step remains out of scope here.

## Failure-injection coverage

The offline recovery tests exercise these restart windows with a fake RPC and a temporary SQLite database:

- crash after signed bytes are persisted but before broadcast;
- destination accepts a broadcast while its acknowledgement is lost;
- retry encounters the already-known transaction and later confirms the effect;
- process crashes after durable confirmation, then restart returns the existing result without another broadcast or signature.

These tests establish local recovery behavior only. They do not exercise a live NEAR signer, a live destination RPC, production database durability, KI event leases, or operational access controls.

## Nonce-store backup boundary

The adapter exposes `NonceManager.backup_to(destination)` and an offline test verifies SQLite snapshot/restore of nonce leases and exact signed bytes. The adapter creates file-backed databases as owner-only (`0600`), rejects symlinked database paths and group/world-writable parent directories, and refuses existing files with broader permissions. Create backup destinations as owner-only files in non-group/world-writable directories; protect them as sensitive transaction material. Do not copy a live database or WAL sidecars. Backup scheduling, retention, off-host storage, and a production restore rehearsal remain open.

## Local validation

From the repository root, run the offline checks:

```sh
PATH="$HOME/.cache/kokonut-protocol-venv/bin:$PATH" make ci
```

Do not point this runbook's test procedure at a live network. Any future testnet signing/broadcast, deployment, or production operation requires its own explicit authorization and an approved operator procedure.
