# Independent Smart-Contract Audit Handoff — Preparation Only

**Status:** Handoff materials prepared; no independent audit has been commissioned or completed. This document is not an audit opinion, certification, deployment approval, or authorization for live activity.

## Review target

- **Repository:** `wasalo/Kokonut-Protocol`
- **Branch:** `develop`
- **Contract-source baseline:** `c589d400d022d5f892f6fbb333dc675454a856f0`
- **Target verification:** This `develop` commit is local-only and has not been pushed. The last verified `origin/develop` ref is `806b2dafea888dc3a46709d0577572b92f46695e`. The audit coordinator must securely furnish the exact `c589d40...` source tree to the reviewer and have them verify the SHA before work starts. If contract/build inputs change, stop and agree a new target SHA and diff with the auditor.
- **Scope boundary:** The target contains a NEAR Hub prototype and an offline EVM project-factory prototype. It does not contain a production EVM component, project Safe/DAO target, live signer integration, or deployed production financial contract.

This handoff describes preparation by the project team. Only a suitably qualified reviewer independent of the implementation team can satisfy the PRD's independent-audit gate.

## Requested review scope

1. **NEAR Hub authorization and state transitions**
   - Project/chain registration, execution policy, distinct approval threshold, pause, cancellation, expiration, replay protection, signer binding and key-version rules.
   - `request_signature` and `on_signature_result`: attached deposit/gas assumptions, promise result handling, callback authorization, stale or invalidated requests, response shape and signature-verification boundary.
   - Storage growth, administrator authority, upgrade/deployment assumptions, event contents, and recovery/rotation controls.
2. **EVM project factory**
   - Creation-code allowlisting, separated configuration/deployment authorities, CREATE2 identity, initializer behavior, project-admin assignment, read-back checks, authority-manifest validation, and event/state consistency.
   - Determine which properties are established by code/tests and which require review of an actual component's source and Safe/DAO governance. The factory's self-reported manifest cannot prove completeness.
3. **Cross-component assumptions**
   - Review the Python signing/adapter/nonce boundary only as needed to assess whether NEAR signature status and factory behavior are used safely. Confirm that signature issuance is not treated as destination execution, finality, or reconciliation.

Do not infer that mock fixtures, static analysis, this handoff, or a passing CI run proves a real component or governance target safe.

## Primary materials

- `contracts/near/src/lib.rs` and `contracts/near/src/registry.rs`
- NEAR unit tests in `contracts/near/src/lib.rs` and local sandbox integration in `contracts/near/tests/sandbox.rs`
- `contracts/evm/src/KokonutProjectFactory.sol`
- `contracts/evm/test/KokonutProjectFactory.t.sol` and `contracts/evm/test/KokonutProjectFactoryInvariant.t.sol`
- Related execution boundary: `protocol/evm_signing_payload.py`, `protocol/evm_adapter.py`, `protocol/relayer.py`
- Design and risk context: `docs/protocol/research-backed-prd.md`, `docs/protocol/threat-model.md`, `docs/protocol/execution-controller.md`, `docs/protocol/evm-factory.md`, `docs/protocol/evm-adapter.md`, and `docs/protocol/runbook.md`
- Prior bounded review: `docs/protocol/security-audit-2026-10-24.md` (context only; not independent and not a substitute for re-review at the target SHA)
- Toolchain and controls: `Makefile`, `.github/workflows/protocol-ci.yml`, Rust lockfile/toolchain, Foundry version/configuration, and pinned analyzer configuration

## Existing validation evidence

For the specified source baseline, full local `make ci` passed against the tree committed as `c589d400d022d5f892f6fbb333dc675454a856f0`: 96 Python tests; 68 NEAR unit tests plus 2 local sandbox integrations; 28 Foundry tests; 1 dual-Anvil integration; Slither (0 findings), Semgrep (49 rules, 0 findings), Aderyn (0 high findings), and clean Gitleaks worktree/history scanning and self-test. Hosted Actions run `40156816567` passed all four expected jobs with zero annotations for the earlier `806b2dafea888dc3a46709d0577572b92f46695e` upstream baseline only. No hosted run exists for the local audit target because it has not been pushed. Local validation is functional/supply-chain evidence, not an audit conclusion; the auditor should independently verify source, commands, and commit association.

Local commands, when the pinned tools are available:

```sh
make test-near
make test-near-sandbox
make lint-evm
make test-evm
make audit-evm
make audit-semgrep
make audit-aderyn
make ci
```

The existing sandbox suite at the specified baseline uses loopback-only RPC and test-scoped accounts. It now includes a test-only mock signer fixture that holds the asynchronous promise yield, confirms `Pending`, resumes placeholder response data, and checks callback persistence; a second request is cancelled before the mock yield resumes, and the late callback must leave the execution `Cancelled` and signature status `Failed`. The placeholder fields are deliberately invalid, not cryptographic, and not production evidence. No real Chain Signatures contract, public network, signer, destination chain, or deployment is used. Any mock EVM fixture is likewise test-only.

## Prior internal-review findings and open questions

The prior report describes a bounded internal source review and several remediations. Reproduce or reassess each item against the target source; do not inherit its severity or closure without review.

- **NEAR callback guard:** A late callback after cancellation, expiry, policy change, or pause was previously identified and regression-tested. Confirm the current checks and assess additional asynchronous state changes, especially signer-version rotation while a request is pending.
- **Signature trust boundary:** The NEAR callback checks response shape but does not cryptographically recover the EVM signer. Confirm downstream digest reconstruction, public-key/address binding, approval freshness, and stale-signature rejection before any broadcast.
- **NEAR administrator:** The contract has one configured root administrator and does not implement multisig, timelock, or in-contract rotation. Deployment governance and recovery must be reviewed separately.
- **Storage economics:** Execution and idempotency records are retained; request volume, storage funding, and retention/reclamation require a reviewed policy that preserves replay protection.
- **EVM authority manifest:** Runtime validation checks declared surfaces but cannot prove source-level completeness. No real component, immutable implementation hash, or intended project Safe/DAO has been selected for this target.
- **Destination rights and production controls:** There is no production factory deployment, live destination authorization, shared production event/nonce store, or KI reconciliation path in this repository.
- **Dependency advisories:** The October 25 recheck in `docs/protocol/dependency-maintenance.md` found 3 known Rust lockfile matches and 2 known Python matches in Semgrep's latest pinned scanner dependency graph. No compatible stable fix is currently available; these remain open findings for the independent reviewer. The recheck is not a full dependency audit.

## Explicit exclusions and release gates

This package does not cover or authorize:

- KI migrations, bridge implementation, or changes to KI canonical operations/accounting/evidence/approval records; that work remains deferred until the KI refactor is finalized and Wasabi specifies the KI PR target.
- A specific production EVM component, deployment configuration, project Safe/DAO, Majeur integration, or Revenue Container pilot; those require exact scope and product/governance decisions.
- Public testnet/mainnet deployment, live Chain Signatures, destination signing/broadcast, or production operation.

Public testnet consideration remains blocked on the full Protocol/dependency/component-authority review, resolution or explicit governance exception for Critical/High findings, confirmed NEAR administrator governance and destination controls, and separate explicit deployment authorization. Economic/legal approval is a separate gate before any outside capital or production financial rights.

## Auditor engagement record

Before work starts, the audit coordinator should record outside this repository:

- auditor/firm and independence/conflict check;
- agreed contract and system scope, target commit SHA, and exclusions;
- report format, severity rubric, dates, and retest expectations;
- approved contact and secure issue-submission channel;
- any fee/budget approval (do not infer it from this preparation).

Keep secrets, private keys, raw signed transactions, and credentials out of the handoff and audit findings. Do not mark the independent-audit PRD checklist complete until the external review and required remediation/retest are documented.
