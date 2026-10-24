# Offline EVM project factory prototype

`contracts/evm/src/KokonutProjectFactory.sol` is a local, offline prototype for the PRD's allowlisted project-component factory. It is exercised only against Foundry's in-process test VM. No public RPC, wallet, signing key, or deployed contract is used.

## Prototype behavior

- A configuration authority controls an allowlist of exact component *creation-code hashes*. The hash covers the full creation code supplied at deployment; it is not a runtime-code hash or an audit certificate.
- A separate deployment authority may deploy only allowlisted creation code. The two authority addresses must differ.
- A project ID is unique in the factory and supplies a domain-separated CREATE2 salt, so the destination address can be predicted for a given factory and creation-code hash.
- The factory creates the component and calls `initialize(projectId, projectAdmin)` in the same transaction. It then reads both values back and reverts the complete transaction if initialization did not take effect as requested.
- The project admin must be nonzero and have deployed code at deployment time. This checks only that the address contains code; it does not prove the contract is a Safe, a DAO, or configured with a suitable quorum.
- A configuration authority may revoke an allowed creation-code hash. Revocation affects future deployments, not already deployed components.
- No assets, treasury balances, economic rights, upgrades, custody, or autonomous approval behavior are implemented.

## Explicit limits

This is development/test code, not a production factory, security audit, or deployment approval. Creation-code allowlisting is only as trustworthy as the authority that approves the hash. The prototype does not validate the component's source, governance semantics, vulnerabilities, project-specific authorization payload, or legal/economic rights. A production design still needs reviewed binding between the human-approved NEAR request and all factory calldata, an appropriately governed configuration authority, audited component implementations, destination-chain controls, and independent contract review.

The test toolchain is pinned in `contracts/evm/.foundry-version` and `contracts/evm/foundry.toml`. Run from the repository root:

```sh
make lint-evm
make test-evm
make audit-evm
```

The Slither analysis uses the version pinned in `contracts/evm/.slither-version`; it reports no findings for the current factory. All targets run offline. They do not connect to an EVM network. Do not treat a passing local test or static analysis as evidence of deployment readiness.
