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

The exact OSV querybatch review covered 279 Cargo.lock package/version pairs, 51 installed Protocol/Slither Python pairs, and 66 installed Semgrep Python pairs. It returned five advisory matches: three Rust lockfile matches, no match in the Protocol/Slither Python environment, and two Semgrep-only Python matches. The exact findings follow.[23][24][25]

- `crypto-bigint 0.5.5`, pulled through `near-sdk`'s `unit-testing` feature → `near-vm-runner` → `p256 0.13.2`, matches RustSec `RUSTSEC-2026-0387`, published **October 23, 2026**. The advisory describes secret-dependent timing variability and marks `0.7.6` as fixed. The current NEAR stable dependency path constrains `p256` to the incompatible `crypto-bigint 0.5` line; do not force a major-version override. `cargo tree --target wasm32-unknown-unknown --edges normal` does not include this test-only package. Revisit when NEAR publishes a compatible stable dependency update; include it in the full audit.[5][23]
- Semgrep v1.182.1 still requires `mcp==1.29.0` and `opentelemetry-api~=1.37.0`.[30]
- `mcp 1.29.0` matches `GHSA-84m7-p3x7-pcfv` (fixed in `1.30.0`).[17][24]
- `opentelemetry-api 1.37.0` matches `GHSA-xcq7-p3pf-p88h`, published **October 22, 2026** and fixed in `1.45.1`.[18][25]
- Semgrep v1.182.1's exact/range requirements prevent safely upgrading those transitive packages independently. CI runs Semgrep's scan command in an isolated environment; it does not start an MCP or telemetry service. Keep this exception visible and re-evaluate when Semgrep releases compatible dependencies.[30]
- The NEAR unit-testing graph also retains `lru 0.16.4` (RustSec `RUSTSEC-2026-0253`, fixed in `0.18.2`) and `unsafe-libyaml 0.2.11` (RustSec `RUSTSEC-2024-0449`, unmaintained). These are test/development dependencies in the current stable NEAR stack, not evidence of a clean dependency audit.[6][7]

GitHub rates the MCP and OpenTelemetry records **MODERATE**.[17][18]

The RustSec records for `crypto-bigint`, `lru`, and `unsafe-libyaml` provide no numeric severity score in the OSV severity field; this snapshot does not assign one.[5][6][7]

These findings remain open where upstream stable constraints prevent a safe update. A green CI run does not resolve them or satisfy the full audit gate. No public/testnet or production deployment is authorized by this snapshot.

## Addendum — NEAR sandbox test dependency advisories (2026-10-25)

A fresh OSV querybatch compared 278 registry package/version pairs at `origin/develop` with 437 in the sandbox-test lockfile. It returned nine advisory matches across five current package/version pairs. The three existing pairs (`crypto-bigint 0.5.5`, `lru 0.16.4`, and `unsafe-libyaml 0.2.11`) remain as documented above. The newly added test-only graph adds `rustls 0.23.43` (RUSTSEC-2026-0285/0346/0348/0349, plus its GHSA record) and `rustls-webpki 0.103.13` (RUSTSEC-2026-0382).[38][39][40][41][42]

These packages enter through the `near-kit` and `near-sandbox` development dependencies. The NEAR test uses only the sandbox's loopback HTTP RPC endpoint, and `cargo tree --target wasm32-unknown-unknown --edges normal` contains none of the new TLS/test-client packages. This limits runtime exposure; it does not erase the advisory matches or establish a clean dependency audit.

Attempts to move the graph to `rustls 0.23.46` and `rustls-webpki 0.103.16` failed to resolve: the selected TLS features require `aws-lc-rs >=1.18`, while the current stable NEAR stack (`near-sdk 5.29.1` → `near-crypto 0.37.4`) pins `aws-lc-rs =1.16.2`. No incompatible override was forced. Record these test-tool advisories as open dependency debt; resolve them through a compatible stable NEAR/client update or a separately reviewed test-client feature change before any public/testnet deployment. No public deployment is authorized by this addendum.

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
