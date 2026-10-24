# Protocol security audit and remediation status

**Review date:** October 24, 2026 (UTC)
**Repository baseline:** `develop` at `b35351219b2d6cf682f90c6c53b0e16b64695240`
**Status:** Source-review findings validated; two code findings fixed in the current worktree. This is not an external certification, production approval, or authorization to sign, broadcast, or deploy.

## Executive decision

Do **not** deploy to a public network or testnet yet. The review reproduced a High-risk asynchronous NEAR signature-callback race and a Medium local-file exposure of persisted signed transactions; both are fixed in the current worktree with regression tests. No unresolved Critical or High finding was confirmed after those fixes in the reviewed scope. Multiple Medium deployment and operational gates remain, including review of an actual EVM project component and its Safe/DAO authority, NEAR administrator governance, persistent storage economics, hosted validation of the CI source-build jobs, and upstream dependency advisories.

No live signer, public RPC, transaction broadcast, or deployment was used. The review is a source audit supported by repository tests and static checks; it is not an independent human audit.

## Scope and method

Reviewed the NEAR contract and registry, Python signing/adapter/relayer/recovery paths, SQLite nonce storage, EVM factory and fixture tests, schemas and JSON examples, dependency manifests/lockfiles, CI workflow, and security/deployment documentation. Eight specialized read-only review passes covered the EVM factory and governance, NEAR state machine, Python transaction path, dependency advisories, schemas, CI, and deployment boundaries. Findings were checked against the current source and reproduced where a deterministic local test was available. Local findings cite repository paths/line ranges and test names; numbered sources below cover external advisories, upstream release metadata, and ruleset/license facts.

The dependency graph check confirmed `crypto-bigint 0.5.5`, `lru 0.16.4`, and `unsafe-libyaml 0.2.11` in the NEAR development/test dependency graph; the normal `wasm32-unknown-unknown` graph does not include `crypto-bigint 0.5.5`.

- The isolated Semgrep 1.182.1 environment resolves `mcp 1.29.0` and `opentelemetry-api 1.37.0` as required by its package metadata.[10]
- RustSec lists `crypto-bigint` as patched at `0.7.6`, `lru` as fixed at `0.18.2`, and `unsafe-libyaml` as unmaintained with no patched release.[1][2][3]
- GitHub's advisory records identify fixes for MCP at `1.30.0` and OpenTelemetry API at `1.45.1`.[4][5]

## Findings

### 1. High — a late signature callback could issue after cancellation, expiry, policy change, or pause — **fixed in this worktree**

- **Location:** `contracts/near/src/lib.rs:1044–1103` (callback authorization guard), `1186–1227` (`cancel_execution`), and `1229–1256` (`expire_execution`). Regression tests are `cancellation_invalidates_a_pending_signature_callback`, `expiry_invalidates_a_pending_signature_callback`, `signature_callback_rejects_policy_version_changed_while_pending`, `signature_callback_expires_request_at_deadline_without_expire_call`, and `signature_callback_is_suppressed_when_protocol_pauses_while_pending`.
- **Precondition and path:** An authorized operator requests a signature for an Approved execution. Before the asynchronous signer callback returns, the request is cancelled, reaches its expiry timestamp, its active policy version is replaced, or the administrator pauses the protocol. The original callback did not bind the result to those current-state conditions and could persist and return an `Issued` signature.
- **Impact:** A callback could issue a signature for cancelled, expired, policy-revoked, or paused work. A downstream consumer that failed to re-read current NEAR state could treat it as still authorized.
- **Remediation and evidence:** Regression tests reproduced `signature_issued` after cancellation/expiry and after policy rotation, deadline passage, or pause. The callback now requires `Pending`, `Approved`, an unexpired timestamp, the still-active policy version, and an unpaused protocol. Otherwise it fails and clears the pending signature; it marks the execution Expired at the deadline. All five callback/cancellation/expiry tests pass after the fix.
- **Residual limitation:** An already-issued cryptographic signature cannot be revoked by changing NEAR state. Production consumers must re-read approval, active policy, signer-version semantics, expiry, and pause immediately before destination broadcast, and a real destination component still needs reviewed stale-signature/expiry enforcement. Offline relayer and fixture tests do not prove that cross-chain control.
- **Confidence:** High for the reproduced local state-machine races and fixes; production end-to-end behavior remains unverified.

### 2. Medium — nonce database permissions could expose signed transactions — **fixed in this worktree**

- **Location:** `protocol/evm_adapter.py:282–336`; tests in `tests/test_evm_adapter.py` covering creation mode, WAL/SHM sidecars, unsafe existing mode, group-writable parent directories, and symlink paths.
- **Precondition and path:** A file-backed `NonceManager` database was created under a permissive umask or in a directory accessible to another local user. The prior SQLite open used the caller-supplied path without setting or checking file permissions; a local probe under umask `022` produced mode `0644`.
- **Impact:** The database retains exact signed transaction bytes and hashes. A local reader could obtain and submit a signed transaction before the intended relayer.
- **Remediation and evidence:** File-backed initialization now uses `O_NOFOLLOW` and mode `0600`, requires an owner-owned regular file, rejects group/world-readable or writable database modes and group/world-writable parent directories, and verifies the opened file identity. The focused and full Python suites passed, including the sidecar permission assertions.
- **Compatibility note:** Existing databases with broader permissions now fail closed; operators must secure them before reopening. Backup destinations remain caller-provisioned and must use owner-only access controls.
- **Confidence:** High for the mode and symlink checks on the tested Linux environment.

### 3. Medium — the factory cannot prove a component's authority manifest is complete — **open deployment gate**

- **Location:** `contracts/evm/src/KokonutProjectFactory.sol:94–113, 121–161`; interface `:7–11`. The project-admin address check is at `:65–69`.
- **Precondition and path:** A code-hash allowlisted component omits a privileged role from its self-reported `authoritySurfaces()` result, the supplied `projectAdmin` is not the intended, suitably governed Safe/DAO, or the factory configuration/deployment authorities are not controlled by the approved governance. The factory checks manifest shape, role ordering, and reported controller addresses, but cannot discover omitted mutable surfaces or independently establish the governance quality of any supplied authority.
- **Impact:** A component could retain an unreported administrator or upgrade path, or project authority could be assigned to an unsuitable contract. The factory's fixture tests do not establish the security of a real component.
- **Required evidence before any deployment:** Review the exact component source and creation-code hash; enumerate every mutable/upgrade authority; verify the exact intended Safe/DAO and its threshold/owners; independently establish governance for the factory configuration/deployment authorities; exercise every privileged path; confirm no factory or deployer retains control; and obtain a separate review of that component and target.
- **Confidence:** High that the factory's interface is not proof of source completeness; no exploitable defect was confirmed in the factory implementation itself.

### 4. Medium — NEAR has a single root administrator with no in-contract rotation — **open governance gate**

- **Location:** `contracts/near/src/lib.rs:14–16, 29–50` and administrator-gated configuration methods; `contracts/near/README.md:25–27`.
- **Precondition and impact:** If the configured `administrator` is a single key, its compromise permits root chain/project/signer/policy configuration and pause control. The contract has no admin-rotation, multisig quorum, or timelock workflow.
- **Required control:** The README correctly treats governance/multisig control as a deployment gate. Before deployment, identify the exact administrator account, prove its approved multisig/governance control, and document the recovery/rotation path. No account identity was guessed or substituted during this review.
- **Signer-rotation design gate:** `activate_signer` changes the active version but retains earlier signer bindings (`:288–333`). A signature callback for a request already bound to an older key version is not invalidated solely by activating a new version (`:1044–1103`). Decide whether that pending request remains valid under its originally approved key version; if rotation must revoke pending work, add an explicit revocation/version check. Until then, emergency pause/cancellation—not activation alone—is the available invalidation path.
- **Confidence:** High for the authority surface; exploitability depends on the chosen administrator's external governance.

### 5. Medium — execution and idempotency records grow without a storage-recovery policy — **open operational gate**

- **Location:** persistent maps at `contracts/near/src/lib.rs:24–26`; proposal insertion at `:570–753`. No removal of execution or idempotency entries was found.
- **Precondition and path:** A configured execution operator can create new unique requests. Each request and idempotency key is retained after terminal status; proposal creation has no storage-deposit or cleanup/reclamation path.
- **Impact:** Long-running or compromised operator activity can consume contract storage funding and eventually prevent otherwise valid writes. Access is limited to configured operators, so this is not an unauthenticated spam finding.
- **Required decision:** Define storage payment/reimbursement, maximum request volume/size, and archival/retention behavior before live operation. Do not silently delete idempotency history, which could permit replay.
- **Confidence:** Medium; the storage growth path is visible, but operational impact depends on traffic, funding, and NEAR storage economics.

### 6. Medium — CI secret scanning and pinned-source toolchain provenance — **local validation passed; hosted validation pending**

- **Location:** `.github/workflows/protocol-ci.yml` has a full-history `protocol-secrets` job and source-build jobs for cargo-near and Foundry. Gitleaks is pinned in `.gitleaks-version` and `.gitleaks-linux-x86_64-sha256`; the wrapper and supplemental rule are `scripts/audit-gitleaks-pinned.sh` and `.gitleaks.toml`. The mutable cargo-near and Foundry bootstrap/release-archive paths have been removed from CI. Both source-build jobs require `RUNNER_ENVIRONMENT=github-hosted`; the workflow now requests only `contents: read` and has no deployment secret. GitHub documents the runner-environment values and makes default variables available to workflow steps, including container-job steps.[25][32]
- **Scenario and impact:** A committed credential could pass ordinary tests/analyzers unnoticed. A compromised installer or untrusted build runner could execute arbitrary code in CI. The revised workflow verifies the exact signed source commits, builds in a digest-pinned Rust image with a timestamped Debian package snapshot, and checks tool versions/commit identity before use.
- **Secret-scanning remediation and evidence:** Gitleaks `8.30.0` is fetched from its versioned release asset and checked against the repository-pinned SHA-256, matching the release API's published digest.[14] The v8.30.0 release is selected over v8.30.1 because an upstream issue reports that v8.30.1's tag is not reachable from `master`; GitHub marks the v8.30.0 release commit as signed.[15][23] The scanner covers the working tree and all locally fetched refs (`--all`); CI uses `fetch-depth: 0`. Reports stay in an explicit temporary directory, use `--redact=100`, and the wrapper emits only paths/lines/rule IDs on findings. A synthetic self-test checks historical and working-tree detection, the GitHub-PAT rule, the supplemental sensitive-JSON-property rule, and redacted output; it passed 20/20 repeated runs. The repository scan found zero findings. The custom rule addresses the upstream `generic-api-key` false negative reproduced for single-line JSON when another property precedes the credential key.[13]
- **cargo-near source-build evidence:** The pinned source commit `108a48d505148145b419cf1060dc5f3026881984` remains GitHub-verified.[20] CI checks out that exact commit and builds `cargo-near 0.22.0` with `cargo +1.99.1 build --locked --release --package cargo-near`. The upstream workspace declares an unpinned `libudev-dev` prerequisite, and the package enables Ledger support by default.[28][29] `scripts/install-ci-native-deps.sh` therefore pins a Debian snapshot and exact `pkg-config`, `libudev-dev`, and `libudev1` versions. The local source-built binary reports `cargo-near 0.22.0`; the full local `make ci` passes. The hosted source-build workflow has not yet run.
- **Foundry source-build evidence:** The inspected v1.8.6 release certificate identifies the Foundry `release` workflow and pinned source commit, but reports `runnerEnvironment: self-hosted`.[31] That release archive is no longer used in CI. Instead, the workflow builds `forge` and `anvil` from the GitHub-verified source commit `6bad0a66de7c83f7b33695044138f0f5fa4c5990`.[27] The local source-built tools report version `1.8.6` and that exact commit; both run on the host without missing shared libraries. Local EVM validation passed 28 Foundry tests (including 1,024 invariant calls) and one dual-Anvil integration test. Foundry uses the development profile, with debug info and incremental compilation disabled; this is not the upstream PGO release build.
- **Artifact and runner scope:** The `ubuntu-26.04` hosted runner specification lists 14 GB of storage, so the workflow copies only the needed binaries out and removes source/build caches before subsequent checks.[30] These binaries are ephemeral CI test tools and are not uploaded or released. No artifact attestation is generated; GitHub advises against signing frequent builds used only for automated testing.[26] Any future redistribution of these binaries would require a separate release-build and provenance decision.
- **Remaining gate:** Verify the new source-build workflow in hosted GitHub Actions and confirm all jobs pass on the intended runner. This is the remaining CI-provenance check; it does not authorize deployment. Secret scanning also cannot prove detection of every possible secret format.
- **Confidence:** High for the local source builds, version/commit checks, native dependency pins, and secret-scanner tests. Hosted source-build validation remains pending.

### 7. Advisory residuals — no safe compatible update was established

- `crypto-bigint 0.5.5` is reached through the NEAR unit-testing dependency path `near-sdk 5.29.1 → near-vm-runner 0.37.4 → p256 0.13.2`; the advisory's fixed release is `0.7.6`, which is not a compatible drop-in for the current parent constraint.[1]

- Crates.io reports latest stable `near-sdk 5.29.1` and `near-vm-runner 0.37.4` in the `0.x` line; the next releases, `5.30.0-rc.1` and `0.38.0-rc.6`, are release candidates. No compatible stable parent update was available in this snapshot.[11][12]

- `lru 0.16.4` is in the same NEAR development/test graph through `near-vm-runner`; RustSec identifies `0.18.2` as the fix. Do not force a lockfile override that violates the parent dependency's supported range.[2]

- `unsafe-libyaml 0.2.11` is pulled through the NEAR development/test graph and is unmaintained; the advisory lists no patched release.[3]

- The pinned Semgrep CLI was updated from `1.182.0` to PyPI's latest stable `1.182.1` on October 24, 2026.[6][9]

- The v1.182.1 package metadata still requires `mcp==1.29.0` and `opentelemetry-api~=1.37.0`.[10]

- GitHub's MCP advisory identifies `1.30.0` as the 1.x fix.[4]

- GitHub's OpenTelemetry advisory identifies `1.45.1` as the fix.[5]

- The current Semgrep dependency requirements do not permit upgrading those transitive packages independently without changing the scanner's supported environment. The CI invokes the Semgrep scan command; it does not start an MCP server or telemetry service.

- These are retained advisory matches, not a claim that the NEAR Wasm artifact contains all three Rust packages or that the scanner's MCP/telemetry code path is invoked. Re-evaluate when compatible stable upstream releases are available; do not force incompatible overrides.

## Reproducible Semgrep ruleset

The Semgrep CLI 1.182.1 scan now fetches the public `p/smart-contracts` pack to an ephemeral `TMPDIR`/`RUNNER_TEMP` file, verifies SHA-256 `9c4ac2cee5e1ea25d4db4b1f9becb862e95cbf7d403a784433f40fa7243b1d9b`, and fails closed if the digest changes.[8] The hash is stored in `contracts/evm/.semgrep-rules.sha256`; the runner is `scripts/audit-semgrep-pinned.sh`. The inspected snapshot had 50 rule entries; 49 run because the sole excluded rule is the performance-only non-payable-constructor recommendation. The scan completed with zero findings.

The rules carry Semgrep Rules License v1.0 metadata. That license permits internal business use but bars distribution or making the rules available as a service; this repository therefore stores the reviewed digest rather than the rules themselves, and the temporary scan file is removed at exit.[7][8] Any digest update requires manual review of the upstream pack and license.

## Checks that did not reproduce as security bypasses

- The JSON Schema does not compare decimal-string `max_priority_fee` and `max_fee` numerically. This is not an authorization bypass in the reviewed flow: the NEAR proposal path rejects `max_priority_fee > max_fee` (`contracts/near/src/lib.rs:597–607`), and the Python EVM signing gate independently rejects it (`protocol/evm_signing_payload.py:93–102`); regression tests exist in both layers. JSON Schema validation alone is input-shape validation, not authorization.
- No confirmed CREATE2, assembly, role-sorting, or factory access-control exploit was identified in `KokonutProjectFactory.sol`. This does not waive the real-component manifest/governance gate above.
- The previously reported `examples/configs/testnet.example.yaml` discrepancy was not present on this `develop` checkout: the path is absent and no YAML/YML configuration is tracked. The tracked JSON examples are proposals; `README.md:7,13–15` states that no public/testnet deployment or live transaction has occurred.
- No custody, token transfer, payable entry point, pricing math, or oracle dependency was found in the EVM factory under review.

## Validation evidence and limits

- NEAR: `cargo-near 0.22.0` was built locally from the pinned, GitHub-verified source commit. `make ci` passed `cargo fmt --check`, `cargo near check --locked`, `cargo test --locked` (**68 tests**), and `cargo near build non-reproducible-wasm --locked --out-dir target/deploy`. The generated Wasm is development/CI output only and was not deployed.
- EVM: locally source-built Foundry `1.8.6` from the pinned, GitHub-verified commit. `forge fmt --check`, build, and tests passed (**28 tests**, including 32-run/1,024-call invariant campaign). The dual-Anvil local integration test also passed (**1 test**).
- Full local `make ci` — **passed** with the project Python environment and source-built tools: 96 Python tests; Slither `0.11.6` (2 contracts/102 detectors, zero findings); Semgrep `1.182.1` (49 rules, zero findings); Aderyn `0.6.8` (zero High findings); and Gitleaks `8.30.0` (working tree/history both clean; synthetic self-test passed). The earlier hosted run `40116833899` passed the prior workflow revision; it does **not** validate the new pinned-source build jobs. Hosted validation of this revision remains pending.
- The report is bounded to repository source, fixtures, local package metadata, and the cited advisories. No live NEAR signer, EVM RPC, public chain, transaction, or deployment was used.

## Unchecked PRD item reconciliation (October 24, 2026)

The 47 unchecked PRD items were rechecked; none is invalidated by this source audit. Status is classified by the evidence available in this worktree:

- **Deferred—KI prerequisite and integration:** `2095–2110` and `2126–2135` remain blocked by the unfinished KI refactor and unresolved KI PR target. No KI migrations, bridge writes, durable event bus, Baal/EAS integration, or KI recovery work was undertaken; KI remains canonical and paused.
- **Blocked—NEAR sandbox:** `2117` remains open. The 68 local unit tests and optimized Wasm build are not sandbox integration evidence.
- **Blocked—real EVM component and permanent-admin proof:** `2138–2142` and `2145` remain open. The audited factory and fixture/invariant tests do not identify a real component, version/network configuration, Safe/DAO target, or component-specific privileged paths.
- **Open—Revenue Container implementation:** `2150–2155` remain open. The offline integer-base-unit simulator is proposal/model-only; it does not supply a pilot, accounting, versioned policy, Shares/Loot, Ragequit implementation, or economic/legal review.
- **Blocked—public/testnet E2E:** `2158` remains prohibited until the full security gate is satisfied and separate explicit deployment authorization is provided.
- **Partial—durable storage recovery:** `2162` remains open. SQLite online backup/restore support and local lease/signed-byte tests exist, but there is no shared KI event store, scheduled/off-host backup policy, retention design, or production restore rehearsal.
- **Open—ARM64/platform certification:** `2164–2168` remain unverified and open; this review produced no multi-architecture image, native ARM64 Kernel run, migration/PostGIS/backup/worker test, or measured Raspberry Pi certification.
- **Partial—independent audit and Critical/High resolution:** `2170–2171` remain open as release gates. The reproduced local High callback race was fixed and no unresolved Critical/High issue was confirmed in this bounded review, but this is not the required independent manual audit and cannot establish that no other high-severity finding exists.
- **Open—operational, economic, and legal review:** `2172–2175` remain open. Current runbooks are development/testnet-only; production operator procedures, economic audit, and jurisdiction-specific legal review are absent.
- **Blocked—release and financial activation:** `2176–2179` remain open. No testnet release, production canary, production cross-chain reconciliation, or financial activation is authorized or evidenced.

## Required gates before testnet consideration

1. Complete the Protocol audit and resolve/review retained dependency advisories; obtain an independent human security review.
2. Supply and audit the actual EVM project component, its full authority/upgrade paths, and the exact intended Safe/DAO target.
3. Confirm NEAR administrator multisig/governance, rotation and recovery procedures, and storage funding/retention policy.
4. Decide and implement production stale-signature/expiry controls at the destination; the NEAR state transition cannot revoke an already-issued signature.
5. Verify the pinned-source cargo-near and Foundry build jobs in hosted GitHub Actions. Keep their outputs ephemeral and unpublished; if either tool is ever redistributed, establish separate release-specific build and provenance evidence.
6. Obtain separate explicit authorization before any testnet deployment, live signature request, or destination-chain activity.

## Sources

[1] https://rustsec.org/advisories/RUSTSEC-2026-0387.html
[2] https://rustsec.org/advisories/RUSTSEC-2026-0253.html
[3] https://rustsec.org/advisories/RUSTSEC-2024-0449.html
[4] https://github.com/advisories/GHSA-84m7-p3x7-pcfv
[5] https://github.com/advisories/GHSA-xcq7-p3pf-p88h
[6] https://github.com/semgrep/semgrep/releases/tag/v1.182.0
[7] https://semgrep.dev/legal/rules-license
[8] https://semgrep.dev/c/p/smart-contracts
[9] https://pypi.org/pypi/semgrep/json
[10] https://pypi.org/pypi/semgrep/1.182.1/json
[11] https://crates.io/api/v1/crates/near-sdk
[12] https://crates.io/api/v1/crates/near-vm-runner
[13] https://github.com/gitleaks/gitleaks/issues/2223
[14] https://api.github.com/repos/gitleaks/gitleaks/releases/tags/v8.30.0
[15] https://github.com/gitleaks/gitleaks/issues/2086
[20] https://api.github.com/repos/near/cargo-near/git/commits/108a48d505148145b419cf1060dc5f3026881984
[23] https://github.com/gitleaks/gitleaks/releases/tag/v8.30.0
[25] https://docs.github.com/en/actions/reference/workflows-and-actions/variables — GitHub Actions variables reference
[26] https://docs.github.com/en/actions/concepts/security/artifact-attestations — GitHub Actions artifact attestations
[27] https://api.github.com/repos/foundry-rs/foundry/git/commits/6bad0a66de7c83f7b33695044138f0f5fa4c5990 — GitHub verification of pinned Foundry source commit
[28] https://github.com/near/cargo-near/blob/108a48d505148145b419cf1060dc5f3026881984/Cargo.toml — Pinned cargo-near workspace native dependency metadata
[29] https://github.com/near/cargo-near/blob/108a48d505148145b419cf1060dc5f3026881984/cargo-near/Cargo.toml — Pinned cargo-near package features
[30] https://docs.github.com/en/actions/reference/runners/github-hosted-runners — GitHub-hosted runner storage specifications
[31] https://github.com/foundry-rs/foundry/attestations — Foundry v1.8.6 release attestations
[32] https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/run-jobs-in-a-container
