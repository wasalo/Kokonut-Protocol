# NEAR Hub Governance Decision Memo

**Status:** Draft for owner / governance decisions; not approved policy.
**Prepared:** 2026-10-25
**Scope:** Root administrator governance, signer-key rotation during pending requests, and storage funding/retention.
**Non-scope:** KI integration, contract changes, deployment, live signing, or public-chain activity.

## Executive summary

Three policy decisions remain before the NEAR Hub can be considered for any live operation:

1. What account and human governance process controls the Hub's root `administrator`?
2. What happens to a signature request already pending when its signer version is rotated, especially during a compromise response?
3. Who pays for persistent Hub storage, what volume is allowed, and which records must remain for replay protection and auditability?

This memo presents choices and a recommended discussion baseline. It does not set a quorum, name account IDs, change contract behavior, or approve deployment. Record the selected policy first; implement and test any required behavior change afterward.

## Verified current behavior and limits

- The Hub has one configured root administrator account. The contract does not implement administrator multisig, a timelock, or an in-contract administrator rotation workflow. External governance must control the configured account.
- Execution operators and approvers are distinct scoped roles in versioned project/chain policies. Approvals bind the exact request action hash and require the configured threshold.
- Signer bindings are versioned and historical bindings are retained. A signer rotation before `request_signature` causes the request's committed key version to fail the active-binding check.
- Once a signature request is `Pending`, activating another signer version alone does not invalidate its callback. Pause, cancellation, expiry, or a policy-version change can invalidate the pending callback. A signature already issued by an external signer cannot be revoked by the Hub.
- Execution requests and idempotency records are retained in persistent maps. No storage deposit/reimbursement, quota, reclamation, or retention mechanism is implemented. Deleting replay-protection records without a replacement tombstone could permit request reuse.

See `contracts/near/README.md`, `docs/protocol/execution-controller.md`, `docs/protocol/security-audit-2026-10-24.md`, and `contracts/near/src/lib.rs` for the code-level basis.

## Decision 1 — Root administrator control

### Options

- **A. Single human-controlled key:** simplest, but concentrates configuration and pause authority. Not recommended for live operation.
- **B. Dedicated multi-party-controlled NEAR account:** the contract's administrator remains one account ID, while account-level governance enforces a quorum. Requires verified account control, member/offboarding procedures, recovery, and a documented threshold.
- **C. DAO-governed account or controller contract:** governance proposals authorize root actions. Requires review of proposal latency, emergency pause/recovery, upgrade or controller authority, and the controller's own security.

### Recommended discussion baseline

Use an externally governed, multi-party-controlled NEAR account for the root administrator rather than a personal key. Keep root configuration authority distinct from day-to-day execution operators and approval voters. Do not choose members, quorum, timelock, or recovery account in this memo; those are owner/governance decisions.

### Record the decision

- Administrator NEAR account ID: `[decision required]`
- Control mechanism and exact implementation/version: `[decision required]`
- Approver/member identities and threshold: `[decision required; do not put private credentials here]`
- Timelock for routine configuration changes: `[decision required]`
- Emergency pause authority and threshold: `[decision required]`
- Lost-account, compromised-member, and controller-upgrade recovery: `[decision required]`
- Separation from execution operators, policy approvers, and destination-chain multisigs: `[decision required]`
- Evidence required to verify account control before deployment: `[decision required]`

## Decision 2 — Pending signature requests during signer rotation

The policy should distinguish **planned rotation** from **suspected compromise**. The same rule need not apply to both.

### Option A — Preserve the original approval on planned rotation

A request already pending remains bound to the key version committed and approved for that request. It may complete under that version only if there is no compromise signal, the request remains approved/unexpired/unpaused, and the relayer re-reads current state before using the signature. New requests must use the newly active version. This avoids silently rebinding an approved request to a different key.

### Option B — Revoke all old-version pending requests on every rotation

Activating a new signer version invalidates outstanding requests using an older version. This requires an explicit on-chain stale-version/revocation check in the callback and corresponding tests; it is not the current behavior. Affected actions must be reproposed/reapproved under the new key version.

### Option C — Emergency pause-and-cancel only

Keep current callback invalidation controls. During a compromise response, pause the Hub, cancel or expire all affected pending requests, and denylist the compromised signer version in the relayer before any destination broadcast. This is operationally explicit, but relies on an effective incident procedure and complete enumeration of affected requests.

### Recommended discussion baseline

- **Routine rotation without compromise:** do not silently rebind a request. Preserve its recorded key version and require a fresh state/policy/expiry check before any downstream use. Decide whether it may finish under the old version or must be cancelled and reproposed.
- **Suspected or confirmed compromise:** pause immediately, cancel/revoke pending requests tied to the affected version, and prohibit downstream use of any signature from that version. A signature already issued cannot be cryptographically recalled; the relayer and operators must treat a compromised version as unusable even if the Hub retains an `Issued` record.
- If governance wants every rotation—routine or emergency—to invalidate pending requests, authorize a narrowly scoped contract change and regression tests before deployment.

### Record the decision

- Routine rotation treatment: `[preserve under original version / cancel and repropose / other]`
- Emergency compromise treatment: `[decision required]`
- Who can declare compromise and trigger pause/cancellation: `[decision required]`
- Relayer denylist source and freshness requirement: `[decision required; no live relayer integration exists]`
- Treatment of already-issued signatures and destination-side delay/window: `[decision required]`
- Whether contract callback must reject a signer version that is no longer active: `[decision required]`

## Decision 3 — Storage funding, quotas, and retention

The contract currently retains execution and idempotency data without a deposit model or reclamation path. A policy must balance permissionless/operational usability, cost predictability, and replay safety.

### Local prototype measurement (October 25, 2026)

A local-only NEAR sandbox measurement observed 1,288 additional account-storage bytes from the configured baseline through one request's `Issued` state: +850 at proposal, +99 at approval, +78 while signature-pending, and +261 after the mock callback persisted a representative-length response. A second request added 1,037 bytes through its cancelled state; cancellation and its late callback did not reduce or further grow storage. The focused scenario passed four runs with identical values. Full state-by-state measurements and limitations are in [`NEAR Hub Local Storage Measurement`](near-storage-measurement.md).

These are deterministic prototype byte counts, not a network storage-fee quote or a cost/cap recommendation. The fixture covers one project and a small request count; its signature fields have representative lengths but placeholder values that were not generated or verified cryptographically. The result suggests that cancellation must not be assumed to reclaim retained request storage. It does not select a storage payer, reserve, quota, payload cap, archival approach, or retention period.

### Funding options

- **Operations-funded reserve:** a governed operations budget funds storage. Simple for approved operators, but requires monitoring, replenishment authority, and a stop condition when reserve is low.
- **Per-request storage deposit:** the request sponsor pays based on measured storage. Requires an exact accounting/refund policy, including what happens to deposits after cancellation, expiry, or failed signing.
- **Hybrid:** operations funds baseline capacity, with per-project/request quotas and separately approved exceptional usage.

No exact fee, deposit, or cap is proposed because no request-volume or current-cost model was supplied.

### Retention options

- **Retain complete on-chain records:** strongest local replay/audit availability, continuing storage cost.
- **Archive terminal detail but preserve on-chain tombstones:** potentially reduces cost only if an independently approved, tamper-evident archive and lookup/replay contract are designed. This repository does not implement that archive; do not create a parallel accounting or event ledger.
- **Delete records after a period:** not acceptable unless a durable replay-protection tombstone, expiry rule, and recovery/audit procedure remain. Never delete an idempotency key in a way that permits the same request to be accepted again.

### Recommended discussion baseline

Begin with a governed operations-funded reserve plus explicit per-project/request volume and payload limits. Keep request IDs and idempotency tombstones for at least the full validity/replay horizon; do not authorize deletion until an archive/tombstone design has been reviewed and tested. Derive actual caps and replenishment thresholds from measured contract storage, expected request volume, and a documented cost model before writing storage-management code.

### Record the decision

- Funding payer/source and replenishment authority: `[decision required]`
- Per-project and per-operator request limits: `[decision required after volume estimate]`
- Maximum request/payload size: `[decision required]`
- Minimum storage reserve and alert/stop threshold: `[decision required after cost measurement]`
- Full-record retention period: `[decision required]`
- Idempotency tombstone/replay horizon: `[decision required; must not be shorter than validity/replay policy]`
- Archival owner, format, integrity, access, and restore procedure if adopted: `[decision required]`
- Cancellation/expiry/refund handling for any storage deposit: `[decision required]`

## Approval and implementation gates

1. Wasabi and the relevant governance authority resolve the decision fields above; unresolved items remain explicit.
2. Verify the exact NEAR administrator account is controlled by the approved process. Do not infer control from an account name or documentation.
3. If behavior changes are selected, write focused tests first for rotation during each pending/issued state and for storage exhaustion, replay tombstones, refunds, and retention boundaries.
4. Update the threat model, runbook, and PRD after a policy is selected. Keep KI-owned accounting/event storage deferred until the KI refactor and explicit KI PR target are cleared.
5. No public deployment, live signing, or production operation is authorized by this memo.

## Decision record

- Decision owner(s): `[pending]`
- Governance reference / approval record: `[pending]`
- Chosen root administrator model: `[pending]`
- Chosen routine/emergency signer-rotation policy: `[pending]`
- Chosen storage payer, limits, and retention policy: `[pending]`
- Follow-up implementation scope and reviewer: `[pending]`
