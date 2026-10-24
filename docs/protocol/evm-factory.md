# Offline EVM project factory prototype

`contracts/evm/src/KokonutProjectFactory.sol` is a local, offline prototype for the PRD's allowlisted project-component factory. It is exercised only against Foundry's in-process test VM. No public RPC, wallet, signing key, or deployed contract is used.

## Prototype behavior

- A configuration authority controls an allowlist of exact component *creation-code hashes*. The hash covers the full creation code supplied at deployment; it is not a runtime-code hash or an audit certificate.
- A separate deployment authority may deploy only allowlisted creation code. The two authority addresses must differ.
- A project ID is unique in the factory and supplies a domain-separated CREATE2 salt, so the destination address can be predicted for a given factory and creation-code hash.
- The project admin must be nonzero, have deployed code at deployment time, and differ from the factory, configuration authority, and deployment authority. This prevents Protocol-side factory authorities from being the component's recorded admin.
- The factory initializes the component with the project ID and supplied project admin, reads both values back in the same transaction, and validates a bounded authority manifest. The manifest must contain nonempty, strictly ascending (therefore unique) role IDs in parallel arrays of equal length, with 1–32 entries. It must include a `KOKONUT_PROJECT_ADMIN` surface controlled by the supplied project admin; every other declared control surface must be controlled by that same project admin or be zero (no controller). A declared separately controlled upgrade/operations role causes deployment to revert; undisclosed roles require source review to detect.
- This check validates only the component's *declared initial* authority surfaces. It does not prove that the manifest is complete, that the address is a Safe/DAO, that its governance is suitable, or that authority cannot later change. Exact creation-code hash approval must therefore follow component-specific source review that enumerates every privileged path, proxy/implementation, and upgrade mechanism; the factory's runtime checks do not replace that review.
- The mock component demonstrates single-use initialization and denies admin changes by the deployment authority. These tests are fixture evidence only; each real component must independently satisfy and test its own initializer and governance-controlled mutation paths.
- A handler-driven Foundry invariant campaign runs 32 sequences at depth 32 against the offline factory fixture. It checks unique project IDs and component addresses, registry/history agreement, an independent CREATE2 prediction, initialized project identity/admin, and bounded manifest properties; the handler caps the run at 64 deployments and fails on any reverted action. This is prototype evidence, not production readiness.
- A configuration authority may revoke an allowed creation-code hash. Revocation affects future deployments, not already deployed components.
- No assets, treasury balances, economic rights, upgrades, custody, or autonomous approval behavior are implemented.

## Explicit limits

This is development/test code, not a production factory, security audit, or deployment approval. Creation-code allowlisting is only as trustworthy as the authority that approves the hash. The prototype does not validate the component's source, governance semantics, vulnerabilities, project-specific authorization payload, or legal/economic rights. It also cannot inspect a project Safe/DAO's owners, threshold, modules, or upgrade/admin semantics; separately review those and confirm that the selected component makes project control durable before any deployment. A production design still needs reviewed binding between the human-approved NEAR request and all factory calldata, an appropriately governed configuration authority, audited component implementations, destination-chain controls, and independent contract review.

The Foundry, Slither, and Aderyn tool versions are pinned under `contracts/evm/`; the Aderyn Linux archive is checksum-verified. Run from the repository root:

```sh
make lint-evm
make test-evm
make audit-evm
make audit-semgrep
make audit-aderyn
```

Slither and Aderyn run locally without network access; Semgrep fetches the public `p/smart-contracts` ruleset, with metrics and version checks disabled. The Semgrep gate excludes only its performance rule recommending a payable constructor: accepting ETH is outside this factory's API. Aderyn gates high-severity findings; inspect its generated `contracts/evm/out/aderyn-high.json` report when the gate fails. These automated checks are not an independent audit and do not establish deployment readiness.
