# Majeur source pin and fit assessment

**Assessment date:** 2026-10-24  
**Scope:** Offline source review and local tests only. This is not an independent audit, deployment verification, legal opinion, or approval to integrate, deploy, or activate capital.

## Decision

**Research pin:** `z0r0z/majeur` commit `7d7a36b5a25d31f7aa4d499b3a57ee98f4367fac`; upstream `main` HEAD matched this SHA on 2026-10-24. Use the immutable source commit, not a floating branch, for any future review.[1] No Majeur code was copied into or integrated with Kokonut Protocol.

**Fit:** Conditional candidate for an isolated governance/economic-rights experiment; not accepted as a production Revenue Container implementation. Keep accounting, evidence, period close, waterfall calculations, approvals, and reconciliation in KI. Majeur Ragequit distributes DAO-held external assets pro rata to exiting Shares/Loot; it is not an accounting ledger or revenue-period distribution engine.[1][2] Any later pilot must use a segregated project treasury, not the existing Kokonut DAO treasury by default.

## Code and permission model

- **Core authority is DAO-self-call governance, not an owner key.** Configuration is governed by the DAO itself, and proposal execution is allowed after a proposal reaches an executable state.[2] Therefore member distribution, voting power, quorum, proposal threshold, timelock, and initialization parameters determine the effective control plane; deployment alone does not establish suitable governance.[2]
- **Governance can execute arbitrary `CALL` and `DELEGATECALL`.** The proposal execution path accepts an operation and target; `_execute` calls the target for operation `0` and delegatecalls otherwise.[2] No conventional proxy-admin slot or external upgrade owner was found in the inspected core source, but approved delegatecall code executes in DAO storage context.[2] Treat that path as governance-level code/storage mutation authority—an effective upgrade capability—and review it accordingly.
- **Summoner clones use a fixed implementation pointer.** The constructor creates a Moloch implementation and the Summoner deploys minimal-proxy clones pointing to it; the inspected source exposes no implementation setter.[2] This is not a standard UUPS/Transparent proxy with an upgrade-admin function.[2] Governance delegatecall is the material caveat to describing the DAO as immutable.
- **The helper wrapper is not a trust boundary.** `SafeSummoner` is stateless and permissionless.[3] Its own security register says `extraCalls` can override validated configuration and calls the wrapper advisory, not a security boundary.[3] Verify the final initialized DAO configuration and every initialization call; do not infer governance safety from the wrapper name or validation.
- **Deterministic initialization needs operational front-run protection.** `Summoner.summon` derives its CREATE2 salt from initial holders, initial shares, and the supplied salt; the other initialization parameters and `initCalls` are not included.[2] A competing summon with those same salt inputs can occupy the predicted address with different initialization details.[2] Before anyone funds a predicted address, verify deployed code and the complete on-chain configuration; address prediction alone does not commit to configuration.

## Security evidence and limitations

- The repository's Certora report is dated March 12, 2026 and identifies audit commit `a68e2fe9b1049643716d258ca8357001f9a79066`.[4] Its documentation describes 142 properties across seven contracts, while also describing harness simplifications and nondeterministic summaries for external calls.[4] I did not independently rerun the Certora prover.
- In the exact pinned source, `src/Moloch.sol` is unchanged from the report's audit commit, but `src/peripheral/SafeSummoner.sol` and numerous peripheral contracts changed afterward.[2][4] Thus the old formal report is relevant evidence only for unchanged Moloch source, subject to its harness assumptions; it does not verify the current wrapper/peripheral set.
- The pinned SafeSummoner security register records an accepted High-severity auto-burn configuration risk: with `saleBurnDeadline > 0` but no corresponding active non-minting sale, the burn path can target the DAO's entire share balance rather than tracked sale inventory.[3] Treat this as a concrete configuration hazard; do not enable the burn module in a pilot without focused review and invariant tests.
- The README lists deployed contract addresses.[1] This assessment did not query any chain, compare live bytecode with the pinned source, or verify source/implementation relationships; those deployment claims remain unverified here.
- The source is MIT-licensed.[1] The license grants use rights subject to its notice; it is not a security warranty or endorsement.

## Local verification performed

At the pinned checkout, initialized the repository submodules at their recorded gitlinks and ran:

```sh
FOUNDRY_DIR="$HOME/.cache/kokonut-protocol-foundry" \
PATH="$HOME/.cache/kokonut-protocol-foundry/bin:$PATH" \
forge test --no-match-path test/ClassicalCurveSaleFork.t.sol --summary
```

**Result:** compilation with Solidity 0.8.30 succeeded; 676 unit/non-fork test cases across 15 suites passed, with zero failures or skips. The fork test was deliberately excluded, so this is offline test evidence only. Compilation emitted unused-local-variable warnings in `test/LPSeedSwapHook.t.sol`; no Majeur source was modified. Passing tests do not establish an audit or production readiness.

## Kokonut adoption gates

Before any pilot integration or funding:

1. Product owner and relevant governance approve the legal form, asset scope, participant rights, and wind-down rules.
2. Create a project-specific treasury and define how its assets map to KI's canonical accounting and evidence records; do not expose the existing Kokonut DAO treasury by default.[1][2]
3. Select and document initial holders, supply allocation, proposal threshold, quorum, proposal TTL, timelock, ragequit setting, transfer locks, and any rollback guardian. Review the actual initialized configuration, not just wrapper input.[2][3]
4. Decide whether the pilot needs Shares or non-voting Loot; validate exit and voting semantics against the final configuration and segregated treasury.[1][2]
5. Exclude `extraCalls`, arbitrary delegatecall execution, sale/burn modules, and optional peripherals unless each has separate scope-specific review and tested configuration.[2][3]
6. Obtain an independent manual security review of the exact pinned source and selected initialization/configuration. Verify live bytecode and implementation addresses if deployment is later authorized.
7. Keep this pin fixed for any approved experiment; an upstream change requires a fresh diff and review. No integration, deployment, live transaction, or capital activation is authorized by this assessment.

## Conclusion

Majeur is a plausible research candidate for a contained Moloch-style governance/economic-rights experiment because its Shares/Loot/Ragequit model aligns with the PRD's segregated project treasury direction.[1][2]

It is **not yet approved as a Kokonut dependency**: governance delegatecall is a powerful mutation capability, the wrapper does not guarantee configuration safety, current peripheral code is outside the older formal-verification snapshot, and an accepted High-severity configuration hazard is documented.[2][3][4]

Advance only after product/governance approval and an independent review of the exact configuration; defer KI-dependent accounting/reconciliation work until the KI refactor and target are specified.

## Sources

[1] https://github.com/z0r0z/majeur/blob/7d7a36b5a25d31f7aa4d499b3a57ee98f4367fac/README.md — Majeur README at pinned commit
[2] https://github.com/z0r0z/majeur/blob/7d7a36b5a25d31f7aa4d499b3a57ee98f4367fac/src/Moloch.sol — Moloch.sol at pinned commit
[3] https://github.com/z0r0z/majeur/blob/7d7a36b5a25d31f7aa4d499b3a57ee98f4367fac/src/peripheral/audit/SafeSummoner/SECURITY.md — SafeSummoner security notes at pinned commit
[4] https://github.com/z0r0z/majeur/blob/7d7a36b5a25d31f7aa4d499b3a57ee98f4367fac/certora/README.md — Majeur Certora verification documentation at pinned commit
