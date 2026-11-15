# Protocol dependency maintenance snapshot

**Checked:** 2026-10-24 (UTC)

**Scope:** `Kokonut-Protocol` manifests and lockfiles, the NEAR test graph, and the Python environments used by the EVM CI job. This is a dependency-maintenance review, not the full security audit required before any public/testnet deployment.

## Updates and current stable pins

- `jsonschema>=4.26.0,<5` and `eth-hash[pycryptodome]>=0.8.0,<1` are the current stable PyPI releases.[33][34]
- `eth-keys>=0.8.0,<1` and `rlp>=5.0.0,<6` are the current stable PyPI releases; Python 3.12 resolved `eth-keys 0.8.0` and `rlp 5.0.0` successfully.[35][36]
- `near-sdk` remains at latest stable `5.29.1`; `5.30.0-rc.1` is a prerelease and is not adopted. The latest stable `near-vm-runner` in the `0.x` line is `0.37.4`; `0.38.0-rc.6` is a prerelease.[31][32] `time` remains at `0.3.55`.[37]
- The Rust toolchain remains at stable `1.99.1`, and `cargo-near` remains at stable `0.22.0`.[10][11]
- `Cargo.lock` was refreshed. `smallvec` advanced to `1.16.4`; disabling near-sdk's optional default `wee_alloc` feature removed `wee_alloc` and related packages. The NEAR contract passes its pinned check, 68 unit tests, and Wasm build.
- The resolved lockfile no longer contains `wee_alloc`, which RUSTSEC-2022-0054 reports as unmaintained; Cargo confirms it is absent from the graph.[22]
- Foundry `1.8.6`, Solidity `0.8.38`, and Slither `0.11.6` were already the latest stable releases found in their official release feeds.[12][13][14]
- Semgrep was bumped from `1.182.0` to the latest PyPI release `1.182.1`.[15][29] Aderyn `0.6.8` remains current.[16]
- `actions/checkout` is pinned to v7.0.1 and `actions/setup-python` to v7.1.0 by full commit SHA.[8][9]
- Hosted runners are pinned to `ubuntu-26.04` instead of the moving `ubuntu-latest` alias.[21]
- Semgrep's current release requires `jsonschema~=4.25.1`, while the Protocol test requirements now require `jsonschema>=4.26.0`. The EVM workflow therefore installs Semgrep in its own Python 3.12 virtual environment; both environments resolve and run independently. This avoids downgrading the Protocol's current JSON Schema dependency.[30]
- Semgrep scans the public `p/smart-contracts` pack through `scripts/audit-semgrep-pinned.sh`. The downloaded rules YAML must match SHA-256 `9c4ac2cee5e1ea25d4db4b1f9becb862e95cbf7d403a784433f40fa7243b1d9b` (`contracts/evm/.semgrep-rules.sha256`) or the scan fails closed. This pins the exact fetched contents without checking the pack into the repository. The snapshot inspected for this pin contained 50 community rules with `Semgrep Rules License v1.0` metadata; the scan intentionally excludes only `solidity.performance.non-payable-constructor.non-payable-constructor`, a performance-only rule. Review the upstream pack and license before changing the digest.[27][28]

## Recent upstream security fixes covered by current pins

- Foundry `1.8.6`, released October 21, 2026, is a security patch release. It updates bundled `rustls` to `0.23.46` and `zip` to `9.0.4`, addressing RustSec `RUSTSEC-2026-0346` through `0349` and `RUSTSEC-2026-0367` through `0369`. The current Foundry pin includes these fixes. These bundled tool dependencies were not part of the Cargo.lock/Python OSV package-pair query and are not included in its five-match count.[12]
- Solidity advisory `SOL-2026-7`, disclosed October 22, 2026, is rated low severity and affects optimized Yul under the conditions in the compiler advisory; it is fixed in `0.8.38`. This repository pins Solidity `0.8.38` and enables the optimizer, so new builds use the fixed compiler. This does not assess any artifact produced with an earlier compiler version.[26]
- On October 24, 2026, the authenticated GitHub Dependabot Alerts API query for this repository with `state=all` returned zero records. This is a dated repository-alert snapshot, separate from the package-version OSV query above.
- `requirements-dev.txt` uses version ranges and there is no tracked Python lockfile. The Python OSV inventory reflects the resolved environment snapshots checked for this review, not a fully locked Python dependency graph.

## Advisory review and residuals

The October 24 baseline OSV querybatch review covered 279 Cargo.lock package/version pairs, 51 installed Protocol/Slither Python pairs, and 66 installed Semgrep Python pairs. It returned five advisory matches: three Rust lockfile matches, no match in the Protocol/Slither Python environment, and two Semgrep-only Python matches. This is a historical pre-sandbox snapshot; the current NEAR lockfile and its comparison are recorded in the October 25 addendum below.[23][24][25]

- `crypto-bigint 0.5.5`, pulled through `near-sdk`'s `unit-testing` feature → `near-vm-runner` → `p256 0.13.2`, matches RustSec `RUSTSEC-2026-0387`, published **October 23, 2026**. The advisory describes secret-dependent timing variability and marks `0.7.6` as fixed. The current NEAR stable dependency path constrains `p256` to the incompatible `crypto-bigint 0.5` line; do not force a major-version override. `cargo tree --target wasm32-unknown-unknown --edges normal` does not include this test-only package. Revisit when NEAR publishes a compatible stable dependency update; include it in the full audit.[5][23]
- Semgrep v1.182.1 still requires `mcp==1.29.0` and `opentelemetry-api~=1.37.0`.[30]
- `mcp 1.29.0` matches `GHSA-84m7-p3x7-pcfv` (fixed in `1.30.0`).[17][24]
- `opentelemetry-api 1.37.0` matches `GHSA-xcq7-p3pf-p88h`, published **October 22, 2026** and fixed in `1.45.1`.[18][25]
- Semgrep v1.182.1's exact/range requirements prevent safely upgrading those transitive packages independently. CI runs Semgrep's scan command in an isolated environment; it does not start an MCP or telemetry service. Keep this exception visible and re-evaluate when Semgrep releases compatible dependencies.[30]
- The NEAR unit-testing graph also retains `lru 0.16.4` (RustSec `RUSTSEC-2026-0253`, fixed in `0.18.2`) and `unsafe-libyaml 0.2.11` (RustSec `RUSTSEC-2024-0449`, unmaintained). These are test/development dependencies in the current stable NEAR stack, not evidence of a clean dependency audit.[6][7]

GitHub rates the MCP and OpenTelemetry records **MODERATE**.[17][18]

The RustSec records for `crypto-bigint`, `lru`, and `unsafe-libyaml` provide no numeric severity score in the OSV severity field; this snapshot does not assign one.[5][6][7]

The remaining findings stay open where upstream stable constraints prevent a safe update. A green CI run does not resolve them or satisfy the full audit gate. No public/testnet or production deployment is authorized by this snapshot.

## Addendum — NEAR sandbox test dependency advisories (2026-10-25)

A fresh OSV querybatch compared 278 registry package/version pairs at the pre-sandbox `origin/develop` revision with 437 in the original sandbox-test lockfile. It returned nine advisory matches across five current package/version pairs. The three pre-existing pairs (`crypto-bigint 0.5.5`, `lru 0.16.4`, and `unsafe-libyaml 0.2.11`) remain as documented above. The original test-only graph also added `rustls 0.23.43` (RUSTSEC-2026-0285/0346/0348/0349, plus its GHSA record) and `rustls-webpki 0.103.13` (RUSTSEC-2026-0382).[38][39][40][41][42]

These packages entered through the `near-kit` and `near-sandbox` development dependencies. The NEAR test uses only the sandbox's loopback HTTP RPC endpoint, and `cargo tree --target wasm32-unknown-unknown --edges normal` contains none of the TLS/test-client packages. This limits runtime exposure; it does not erase advisory matches or establish a clean dependency audit.

Initial attempts to move the original feature graph to `rustls 0.23.46` and `rustls-webpki 0.103.16` failed to resolve: `near-kit`'s default `reqwest` TLS features activated an `aws-lc-rs >=1.18` requirement, conflicting with the current stable NEAR stack's `aws-lc-rs =1.16.2` pin. No incompatible override was forced. The reviewed feature-level remediation and its remaining limits follow.

### Resolution update — HTTP-only sandbox client (2026-10-25)

The repository vendors the exact `near-kit 0.19.0` crates.io archive. Its SHA-256 (`6ac835efbcc11c23fdee426155995a7316c4f893ec05f6c0c5d355182795ac92`) matched the checksum in the original registry lock entry. The vendored copy has one manifest feature change (`reqwest` `default-features = false`), omits four upstream `#[cfg(test)]` functions containing credential-shaped fixtures detected by the pinned secret scanner, and removes trailing whitespace from five blank Rustdoc lines so the source passes `git diff --check`. The omitted tests are not compiled when `near-kit` is a dependency, and no production code was changed. All remaining archive files were byte-compared and matched. `cargo tree -e features -i reqwest` shows only the `json` feature for this patched `near-kit` dependency. The sandbox integration uses loopback HTTP only. Do not use this patched test client for HTTPS/public RPC or application transport; any such use requires separate feature and provider review. Patch provenance and scope are in [`contracts/near/vendor/near-kit/README.kokonut.md`](../../contracts/near/vendor/near-kit/README.kokonut.md).[43]

The sandbox tool's remaining TLS path is `near-sandbox 0.3.16 → ureq 3.4.2 → rustls`. The lockfile now resolves `rustls 0.23.46` and `rustls-webpki 0.103.16`; `ureq` selects the `ring` provider, so the prior `aws-lc-rs` conflict is no longer present. The updated current lockfile contains 395 registry package/version pairs versus 437 at the sandbox baseline (`f8261c6e895b0f445de9c9ce2943ea2e0c55f218`). A fresh OSV query returned only the three previously known Rust package/version matches (`crypto-bigint 0.5.5`, `lru 0.16.4`, `unsafe-libyaml 0.2.11`), with zero newly introduced affected pairs; all six Rustls/Rustls-WebPKI advisory matches from the original graph no longer match the updated versions.[38][39][40][41][42]

`make test-near-sandbox` and the complete local `make ci` passed on October 25 with the patched dependency graph. Full CI included 96 Python tests; 68 NEAR unit tests plus 1 sandbox integration test; 28 Foundry tests; 1 dual-Anvil integration test; Slither (0 findings), Semgrep (49 rules, 0 findings), Aderyn (0 high findings), and a clean Gitleaks worktree/history scan and self-test. Hosted Actions run `40155706229` for this dependency-remediation revision completed successfully; all four expected jobs passed with zero annotations. The three retained Rust advisories and Semgrep's two Python dependency advisories remain separate full-audit items; this scoped sandbox-tool remediation does not satisfy the complete predeployment security gate. No live signer, public RPC, broadcast, or deployment was used.

## Current advisory recheck — 2026-10-25

A fresh OSV `querybatch` against the current `contracts/near/Cargo.lock` covered 395 crates.io package/version pairs and returned three matches, all previously known: `crypto-bigint 0.5.5` (`RUSTSEC-2026-0387`), `lru 0.16.4` (`RUSTSEC-2026-0253`), and `unsafe-libyaml 0.2.11` (`RUSTSEC-2024-0449`).[45][46][47] The normal `wasm32-unknown-unknown` dependency graph contains none of these packages; they remain in the host/unit-test graph.

- `crypto-bigint 0.5.5` is reached through `near-vm-runner 0.37.4 → p256 0.13.2 → elliptic-curve 0.13.8`. OSV lists `0.7.6` as fixed, but `elliptic-curve 0.13.8` requires `crypto-bigint =0.5`; a forced major-version override is incompatible and was not attempted.[45]
- `lru 0.16.4` is required by `near-vm-runner 0.37.4` with the `0.16.3` semver line; OSV lists `0.18.2` as fixed, which is outside that line.[46]
- `unsafe-libyaml 0.2.11` is pulled through `near-parameters 0.37.4 → serde_yaml 0.9`. The OSV record is an unmaintained-package notice and lists no fixed release; no dependency override was made.[47]

PyPI still reports Semgrep `1.182.1` as its latest release on this check. Its published metadata retains `mcp==1.29.0` and `opentelemetry-api~=1.37.0`.[48][49] Fresh OSV records show the MCP advisory fixed at `1.30.0` and the OpenTelemetry advisory fixed at `1.45.1`; those versions do not satisfy Semgrep's exact/range requirements. Semgrep runs in an isolated scanner environment and its CI job does not start an MCP or telemetry service, but the installed packages remain advisory matches. No safe compatible Semgrep release was available, so no Python dependency change was made.[48][49][50][51]

This is a package-pair recheck, not a full dependency audit. The Python environment is resolved at install time from Semgrep metadata; the repository does not have a tracked Python lockfile. Keep these matches open for upstream-compatible updates and the independent review. No live activity or deployment is authorized by this update.

## Current refresh — 2026-11-15

The Rust toolchain is now pinned to `1.100.0`; cargo-near is `0.23.0`, Foundry is `1.9.0`, near-sdk is `5.30.2`, near-kit is `0.20.1`, near-sandbox is `0.4.1`, Semgrep is `1.185.0`, and the CI Python installer is `pip 26.3`.[65][66] Their exact release sources/checksums and the refreshed Cargo locks are pinned in the repository. The Debian snapshot used to build the Rust tools is `20261112T000000Z`.

The updated NEAR contract lockfile contains 391 registry package/version pairs and two OSV matches. The Foundry source lock after its reviewed refresh contains 1,130 pairs and four matches. The cargo-near source lock after its refresh contains 658 pairs and five matches. These are separate tool and contract graphs; their counts are not a full audit and should not be added to a deployment risk score.

The refresh removes the currently recorded high-severity `quick-xml` advisories by moving both pinned tool locks to `0.41.0`; RustSec lists `0.41.0` as fixed for the quadratic attribute-check denial of service and unbounded namespace-allocation denial of service.[52][53] The `zip` locks now resolve only to `9.6.0`, above GitHub's fixed version `9.0.5` for the critical dual-header archive advisory.[54]

Foundry's bundled SQLite path previously resolved `rusqlite 0.38.0` / `libsqlite3-sys 0.36.0`, affected by CVE-2026-119375 when opening a crafted database and sibling WAL file. The pinned Foundry source now uses the reviewed MPP commit `fa567a699a70f58fd6852630db332f5443cef068`; it resolves `rusqlite 0.41.1` / `libsqlite3-sys 0.39.1`, which RustSec lists as patched.[55] The MPP commit is pinned by SHA and its narrow upstream diff was reviewed; it is not signed. CI verifies the signed Foundry source commit, then applies the checked-in lock/manifest patch and checks these versions before building.

A fresh OSV pass also found newly recorded advisories matching Foundry's `zstd 0.13.3`, `zstd-safe 7.3.0`, and `rustix 1.1.5`. The reviewed lock refresh moves these to `zstd 0.13.4`, `zstd-safe 7.4.1`, and `rustix 1.1.6`, at or above the RustSec fixed versions.[79][80][81][82]

The new vendor checks bind `symbolic-debuginfo 13.9.0`, `self_update 0.42.0`, and `email-address-parser 2.0.2` to their crates.io archive checksums and compare every retained file. The self-update compatibility patch preserves XML entity unescaping and adds a focused regression test. `email-address-parser` changes only its `quick-xml` build-dependency declaration. The cargo-near lock also updates its active `git2` dependency to `0.21.0`; the remaining `git2 0.20.4` OSV matches are reached only by the source tree's integration-test dev dependency, and both unsoundness advisories list `0.21.0` as patched.[56][57]

Residual Cargo matches remain visible. The contract lock and Foundry graph both contain `crypto-bigint 0.5.5`, matching RustSec's timing-variability advisory; the Foundry binary reaches it through `elliptic-curve 0.13` / `p256 0.13`. RustSec lists `0.7.6` as fixed, but the current `elliptic-curve 0.13` line requires the incompatible `0.5` major line, so no override was forced.[58] The NEAR occurrence is confined to the host/unit-testing graph, while Foundry reaches it in its normal/build graph. The remaining `unsafe-libyaml`, `number_prefix`, `proc-macro-error`, `derivative`, `paste`, and `pin-utils` matches are unmaintained-package notices with no patched releases listed by RustSec.[59][60][61][62][63][64]

The Python OSV recheck covered 52 resolved Protocol/Slither package-version pairs and returned zero matches. The isolated Semgrep environment covered 68 pairs and returned six **MODERATE** matches: four on `mcp 1.29.0` and two on `opentelemetry-api 1.37.0`.[67][68][69][70][71][72] Semgrep `1.185.0` pins `mcp==1.29.0` and constrains `opentelemetry-api` to `~=1.37.0`, so the advisory-fixed versions (`mcp 1.30.0` and OpenTelemetry API `1.45.1`/`1.45.2`) do not satisfy its published requirements.[66][67][68][69][70][71][72] CI runs Semgrep in an isolated environment and does not start an MCP or telemetry service; the package matches remain open until Semgrep publishes compatible requirements. The Python environments have no tracked full lockfile, so these counts describe the exact install snapshots checked on this date.

Aderyn `0.6.8` was the latest tagged release, but its maintainer retired the project and withdrew its release binaries on November 13, 2026.[78] The CI download now returns 404, and the retained release tag/commit is unsigned. The Aderyn binary pin was removed. To preserve a high-impact blocker, the pinned Slither `0.11.6` audit now uses its supported `--fail-high` option; Slither and Semgrep remain active. Slither does not have identical detector coverage to Aderyn, so this is a tool replacement with a coverage difference, not parity. Reassess analyzer coverage when selecting a maintained replacement.

The Gitleaks scanner remains a toolchain security residual. Its pinned `8.30.0` source manifest includes `golang.org/x/sys 0.30.0`, `golang.org/x/text 0.22.0`, `golang.org/x/crypto 0.35.0`, `ulikunitz/xz 0.5.12`, `nwaples/rardecode/v2 2.1.0`, and `klauspost/compress 1.17.11`; upstream issue reports identify these dependency lines in the `8.30.1` release as vulnerable.[73][74][75] The upstream `8.30.1` tag is also documented as not being an ancestor of `master`, with the maintainer acknowledging the release mistake; it still has open dependency CVE reports, so it was not substituted for the pinned release.[76][77] No clean, verifiable Gitleaks release was available on this check. Revisit the scanner pin when upstream publishes a corrected release.

The current OSV query reports no Critical or High match in the three Cargo locks after the ZIP and XML updates. The Foundry cryptographic timing advisory remains security-relevant despite having no numeric CVSS score, the unmaintained-package notices remain open, and Semgrep's six Moderate matches remain unresolved. This dependency refresh is not the full Protocol, component-authority, or dependency audit required before any public/testnet deployment; no signing, broadcast, or deployment was performed.

### Follow-up verification — 2026-11-15

The official Semgrep release page still lists `1.185.0` as latest, and its PyPI metadata still pins `mcp==1.29.0` and `opentelemetry-api~=1.37.0`. The compatible Semgrep release therefore remains affected by the documented Moderate matches; no independent transitive override was applied.[83][84]

The Gitleaks Releases API still identifies `8.30.1` as latest. Its upstream CVE reports remain open, and the project discussion says fixes are being worked on in `master`; there is not yet a newer release artifact to pin. Keep the existing verified `8.30.0` pin and recheck before the next toolchain refresh.[85][86][87]

The current RustSec records continue to list `crypto-bigint >=0.7.6` and `lru >=0.18.2` as the fixed lines, while `unsafe-libyaml` has no patched version. The affected NEAR/Foundry parent constraints do not admit safe overrides, so these findings remain tracked and must be revisited with compatible upstream releases.[88][89][90]

## Sources

[5] https://rustsec.org/advisories/RUSTSEC-2026-0387.html
[6] https://rustsec.org/advisories/RUSTSEC-2026-0253.html
[7] https://rustsec.org/advisories/RUSTSEC-2024-0449.html
[8] https://github.com/actions/checkout/releases/tag/v7.0.1
[9] https://github.com/actions/setup-python/releases/tag/v7.1.0
[10] https://github.com/rust-lang/rust/releases/tag/1.99.1
[11] https://github.com/near/cargo-near/releases/tag/cargo-near-v0.22.0
[12] https://github.com/foundry-rs/foundry/releases/tag/v1.8.6
[13] https://github.com/ethereum/solidity/releases/tag/v0.8.38
[14] https://github.com/crytic/slither/releases/tag/0.11.6
[15] https://github.com/semgrep/semgrep/releases/tag/v1.182.0
[16] https://github.com/Cyfrin/aderyn/releases/tag/aderyn-v0.6.8
[17] https://github.com/advisories/GHSA-84m7-p3x7-pcfv
[18] https://github.com/advisories/GHSA-xcq7-p3pf-p88h
[21] https://docs.github.com/en/actions/reference/runners/github-hosted-runners
[22] https://rustsec.org/advisories/RUSTSEC-2022-0054.html
[23] https://api.osv.dev/v1/vulns/RUSTSEC-2026-0387
[24] https://api.osv.dev/v1/vulns/GHSA-84m7-p3x7-pcfv
[25] https://api.osv.dev/v1/vulns/GHSA-xcq7-p3pf-p88h
[26] https://www.soliditylang.org/blog/2026/10/22/yul-optimizer-knowledge-base-stale-bookkeeping-bug
[27] https://semgrep.dev/legal/rules-license
[28] https://semgrep.dev/c/p/smart-contracts
[29] https://pypi.org/pypi/semgrep/json
[30] https://pypi.org/pypi/semgrep/1.182.1/json
[31] https://crates.io/api/v1/crates/near-sdk
[32] https://crates.io/api/v1/crates/near-vm-runner
[33] https://pypi.org/pypi/jsonschema/json
[34] https://pypi.org/pypi/eth-hash/json
[35] https://pypi.org/pypi/eth-keys/json
[36] https://pypi.org/pypi/rlp/json
[37] https://crates.io/api/v1/crates/time
[38] https://rustsec.org/advisories/RUSTSEC-2026-0285.html
[39] https://rustsec.org/advisories/RUSTSEC-2026-0346.html
[40] https://rustsec.org/advisories/RUSTSEC-2026-0348.html
[41] https://rustsec.org/advisories/RUSTSEC-2026-0349.html
[42] https://rustsec.org/advisories/RUSTSEC-2026-0382.html
[43] https://crates.io/api/v1/crates/near-kit/0.19.0
[44] https://github.com/wasalo/Kokonut-Protocol/actions/runs/40151976435
[45] https://api.osv.dev/v1/vulns/RUSTSEC-2026-0387
[46] https://api.osv.dev/v1/vulns/RUSTSEC-2026-0253
[47] https://api.osv.dev/v1/vulns/RUSTSEC-2024-0449
[48] https://pypi.org/pypi/semgrep/1.182.1/json
[49] https://pypi.org/pypi/semgrep/json
[50] https://api.osv.dev/v1/vulns/GHSA-84m7-p3x7-pcfv
[51] https://api.osv.dev/v1/vulns/GHSA-xcq7-p3pf-p88h
[52] https://rustsec.org/advisories/RUSTSEC-2026-0194.html
[53] https://rustsec.org/advisories/RUSTSEC-2026-0195.html
[54] https://github.com/advisories/GHSA-m665-v7pm-7grj
[55] https://rustsec.org/advisories/RUSTSEC-2026-0459.html
[56] https://rustsec.org/advisories/RUSTSEC-2026-0183.html
[57] https://rustsec.org/advisories/RUSTSEC-2026-0184.html
[58] https://rustsec.org/advisories/RUSTSEC-2026-0387.html
[59] https://rustsec.org/advisories/RUSTSEC-2024-0449.html
[60] https://rustsec.org/advisories/RUSTSEC-2025-0119.html
[61] https://rustsec.org/advisories/RUSTSEC-2024-0370.html
[62] https://rustsec.org/advisories/RUSTSEC-2024-0388.html
[63] https://rustsec.org/advisories/RUSTSEC-2024-0436.html
[64] https://rustsec.org/advisories/RUSTSEC-2026-0441.html
[65] https://pypi.org/project/pip/26.3/
[66] https://pypi.org/pypi/semgrep/1.185.0/json
[67] https://github.com/advisories/GHSA-5h93-6whr-6q8j
[68] https://github.com/advisories/GHSA-84m7-p3x7-pcfv
[69] https://github.com/advisories/GHSA-rwrf-2pqf-9j8j
[70] https://github.com/advisories/GHSA-w4fh-qvv9-3v23
[71] https://github.com/advisories/GHSA-mf5v-w3w3-f6cg
[72] https://github.com/advisories/GHSA-xcq7-p3pf-p88h
[73] https://github.com/gitleaks/gitleaks/blob/v8.30.0/go.mod
[74] https://github.com/gitleaks/gitleaks/issues/2305
[75] https://github.com/gitleaks/gitleaks/issues/2330
[76] https://github.com/gitleaks/gitleaks/issues/2086
[77] https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1

[78] https://github.com/Cyfrin/aderyn/releases/tag/aderyn-v0.6.8
[79] https://rustsec.org/advisories/RUSTSEC-2026-0391.html
[80] https://rustsec.org/advisories/RUSTSEC-2026-0395.html
[81] https://rustsec.org/advisories/RUSTSEC-2026-0406.html
[82] https://rustsec.org/advisories/RUSTSEC-2026-0486.html
[83] https://github.com/semgrep/semgrep/releases
[84] https://pypi.org/pypi/semgrep/1.185.0/json
[85] https://api.github.com/repos/gitleaks/gitleaks/releases/latest
[86] https://github.com/gitleaks/gitleaks/issues/2305
[87] https://github.com/gitleaks/gitleaks/issues/2330
[88] https://rustsec.org/advisories/RUSTSEC-2026-0387.html
[89] https://rustsec.org/advisories/RUSTSEC-2026-0253.html
[90] https://rustsec.org/advisories/RUSTSEC-2024-0449.html
