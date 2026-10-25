# NEAR Hub Local Storage Measurement

**Status:** Local sandbox observation for governance discussion only; not a storage policy, fee quote, or deployment evidence.
**Measured:** October 25, 2026
**Pushed baseline:** `592cf94caab1d8a49702e21e0d3a045ed3bcc1df` (`origin/develop`); hosted Actions run `40211865013` passed all four jobs with zero annotations for this SHA.
**Measurement/test commit:** `520db5a32aeca873470f9a05608dd593245606db` (local `develop`, unpushed; no hosted run).
**Validation:** Full local `make ci` passed on the exact tree committed as `520db5a`; hosted run `40211865013` covers the pushed baseline only.

## Method

The local asynchronous mock-signature sandbox scenario samples the Hub account's `storage_usage` at block-pinned receipts executed by the Hub. It records a normal request lifecycle, a cancelled request and late callback, three repeated maximum-bound requests retained on-chain, and a second EVM request whose mock callback persists a 1,024-byte response. Receipt selection uses the Hub executor ID rather than assuming the last receipt belongs to the Hub.

The maximum-bound request uses a registered non-EVM `other` chain so the current accepted limits can be exercised together: namespace 64 bytes, chain ID 128, operation 80, target 256, policy version 128, idempotency key 256, and canonical uint256 values (78 decimal digits). The three request IDs and one EVM idempotency key are distinct. The EVM request also exercises the existing EVM-specific field shape.

The normal request path uses the local mock Chain Signatures fixture. Signature fields are representative or boundary-sized placeholder hex strings; they are not generated or cryptographically verified. No live signer is used. `eip155:8453` is request data only; no destination-chain RPC, public RPC, testnet, transaction broadcast, or deployment is involved.

The absolute account `storage_usage` includes the NEAR account footprint, including contract code and contract storage ([NEAR account data model](https://github.com/near/nearcore/blob/master/docs/DataStructures/Account.md)). Deltas below are byte observations, not fees or NEAR-denominated funding requirements.

## Observed results

The original lifecycle checkpoints remain unchanged:

| State | Total account storage (bytes) | Delta from prior state (bytes) | Delta from lifecycle baseline (bytes) |
|---|---:|---:|---:|
| Configured baseline | 306,151 | +0 | +0 |
| Request awaiting approval | 307,001 | +850 | +850 |
| Request approved | 307,100 | +99 | +949 |
| Request signature pending | 307,178 | +78 | +1,027 |
| Request signature issued | 307,439 | +261 | +1,288 |
| Second request awaiting approval | 308,299 | +860 | +2,148 |
| Second request approved | 308,398 | +99 | +2,247 |
| Second request signature pending | 308,476 | +78 | +2,325 |
| Second request cancelled | 308,476 | +0 | +2,325 |
| Late callback after cancellation | 308,476 | +0 | +2,325 |

The maximum-bound and callback-response scenario uses a separate baseline after its chain, project, and policy are configured:

| State | Total account storage (bytes) | Delta from prior state (bytes) | Delta from maximum-input baseline (bytes) |
|---|---:|---:|---:|
| Maximum-input configured baseline | 311,071 | +0 | +0 |
| First maximum-bound request retained | 313,128 | +2,057 | +2,057 |
| Second maximum-bound request retained | 315,185 | +2,057 | +4,114 |
| Third maximum-bound request retained | 317,242 | +2,057 | +6,171 |
| Maximum-idempotency EVM request awaiting approval | 318,562 | +1,320 | +7,491 |
| EVM request approved | 318,661 | +99 | +7,590 |
| EVM signature pending | 318,739 | +78 | +7,668 |
| 1,024-byte mock callback issued | 319,767 | +1,028 | +8,696 |

The maximum-bound scenario passed four focused runs with identical values at all 18 checkpoints (the captured run plus three reruns). The three repeated retained requests each added 2,057 bytes in this fixture. The 1,024-byte response-to-issued transition changed storage by 1,028 bytes; that delta includes all callback state changes and should not be attributed solely to response bytes.

Unit tests directly inject mock `PromiseResult` data to characterize the contract's existing `promise_result_checked(0, 4096)` boundary: a valid-shape 4,096-byte response is accepted and persisted, while 4,097 bytes, malformed JSON, empty/odd/non-hex fields, and an out-of-range recovery ID are rejected. The pinned local sandbox's yield-resume path has a separate 1,024-byte limit: attempting to resume the mock yield with a 4,096-byte result failed with `Yield resume payload is 4096 bytes which exceeds the 1024 byte limit`. Therefore 4,096-byte callback acceptance is unit-level characterization, not a sandbox storage measurement; the sandbox storage row uses the maximum response that its mock yield path can deliver.

## Validation and limits

- Full local `make ci` passed: 96 Python tests, 73 NEAR unit tests, 2 NEAR sandbox tests, 28 Foundry tests, 1 dual-Anvil integration, and configured static-analysis/secret scans.
- The focused extended sandbox scenario passed four times with identical 18-checkpoint storage values; `make test-near-sandbox` passed both sandbox tests.
- Boundary tests cover accepted maxima and one-over rejection for bounded chain identifiers, non-EVM targets, operations, policy versions, expiration length, idempotency keys, and uint256 values. The sandbox persists the maximum-bound request fields and repeated retained requests; no production code or limits changed.
- These are deterministic observations using pinned local NEAR sandbox 2.13.4, not a workload distribution or statistical estimate. They do not establish throughput, long-term accumulation, storage exhaustion behavior, network storage pricing, or the impact of future schema/contract changes.
- The 4,096-byte callback boundary is exercised through the unit-test PromiseResult mock because the sandbox's yield-resume fixture is limited to 1,024 bytes. The mock response values are syntactically shaped placeholders, not valid or verified signatures.
- Storage values are not gas costs, storage-deposit amounts, fee quotes, funding requirements, or recommended quotas. Apply the currently relevant network storage-cost rules and a governed safety margin separately.
- No retention, archival, deposit, reimbursement, or pruning mechanism was implemented or tested. Cancellation and the late callback did not reclaim or further grow storage in the measured lifecycle; that does not decide a retention policy.

## Governance use

Use these observations as one input to the open storage-funding and retention discussion in [`near-governance-decision-memo.md`](near-governance-decision-memo.md). They do not select the payer, reserve, quota, payload cap, archival approach, retention period, or tombstone policy. Those remain owner/governance decisions, followed by test coverage for any selected policy before implementation.
