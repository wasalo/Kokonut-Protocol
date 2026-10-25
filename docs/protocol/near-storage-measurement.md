# NEAR Hub Local Storage Measurement

**Status:** Local sandbox observation for governance discussion only; not a storage policy, fee quote, or deployment evidence.
**Measured:** October 25, 2026
**Base:** `develop` at pushed `747859cea7e27be77d47de0c0722a8ab4c97d2a6`; measurement changes are local-only and have no hosted run.

## Method

The existing local asynchronous mock-signature sandbox scenario now samples the Hub account's `storage_usage` at block-pinned receipts executed by the Hub. Samples cover a configured baseline, proposal, approval, pending signature request, issued callback, a second pending request, cancellation, and its late callback. The sample sequence is emitted by `deployed_hub_issues_and_persists_mock_chain_signature_callback` in `contracts/near/tests/sandbox.rs`.

The request path uses the current contract state machine and local mock Chain Signatures fixture. The issued-response fixture has representative field lengths—65 bytes for `big_r.affine_point`, 32 bytes for `s.scalar`, and a recovery ID—but its repeated hex values are deliberately not a valid cryptographic signature. No live signer is used. `eip155:8453` is request data only; no destination-chain RPC, public RPC, testnet, transaction broadcast, or deployment is involved.

The configured baseline is sampled after chain, project, signer, and execution-policy setup. The absolute account `storage_usage` includes the NEAR account footprint, including contract code and contract storage ([NEAR account data model](https://github.com/near/nearcore/blob/master/docs/DataStructures/Account.md)); the deltas below are the useful request-lifecycle observations. Each query is pinned to the block hash of a receipt whose executor is the NEAR Hub, not an adjacent signer-fixture receipt.

## Observed results

The focused sandbox scenario was run four times (the first measurement plus three repetitions). All four runs passed and returned identical byte counts.

| State | Total account storage (bytes) | Delta from prior state (bytes) | Delta from configured baseline (bytes) |
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

In this fixture, one request through `Issued` used 1,288 additional bytes over the configured baseline. The second request added 1,037 bytes from the first request's `Issued` state through its `Cancelled` state; its idempotency-key fixture differs in length, so this is not a controlled per-request-size comparison. Cancellation retained the stored request and did not reduce storage. Resuming the mock signer after cancellation did not change Hub storage or restore the signature response.

## Validation and limits

- `make test-near-sandbox` passed both sandbox tests with pinned NEAR sandbox 2.13.4.
- The focused measurement test passed four consecutive runs with identical samples.
- The scenario uses one configured project/chain, one signer, one approver, one issued request, and one cancelled request. It is a deterministic fixture, not a workload distribution or statistical estimate.
- Request fields, identifiers, idempotency keys, response payload, contract version, and lifecycle count are fixed. The fixture does not establish worst-case payload size, throughput, long-term accumulation, storage exhaustion behavior, reclamation, or the effect of a future schema/contract change.
- These are account `storage_usage` byte counts, not gas costs, storage-deposit amounts, NEAR-denominated funding requirements, or a recommended quota. Apply the currently relevant network storage-cost rules and a governed safety margin separately before selecting a reserve.
- No retention, archival, deposit, reimbursement, or pruning mechanism was implemented or tested. In particular, this result supports not assuming cancellation frees storage; it does not decide whether terminal records should be retained or archived.

## Governance use

Use the observed deltas as one input to the open storage-funding and retention discussion in [`near-governance-decision-memo.md`](near-governance-decision-memo.md). They do not select the payer, reserve, quota, payload cap, retention period, or tombstone policy. Those remain owner/governance decisions, followed by test coverage for any selected policy before implementation.
