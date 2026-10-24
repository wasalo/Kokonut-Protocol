# Kokonut Protocol

Chain-neutral policy and interoperability layer for Kokonut Network. Kokonut Intelligence remains the canonical operational, accounting, evidence, and approval system. NEAR is modeled as an interoperability/signing hub; destination networks retain their own settlement and rights.

## Foundation slice

This repository establishes chain capabilities and an initial NEAR Hub registry, policy, execution-approval, and testnet-only signing flow. It includes schema-validated examples for a NEAR hub and two proposed EVM test destinations; a Rust contract for chain/project profiles, versioned signer bindings and execution policies, policy-checked execution proposals with replay protection and approval quorum; a configurable asynchronous NEAR Chain Signatures request; an offline EIP-1559 signing-payload verifier; a testnet-only EVM transaction adapter; and a local-only EVM project-factory prototype. These remain control-plane and client-side primitives. The test-only Anvil fixture is deployed only to ephemeral loopback nodes; no Protocol contract has been deployed to a public/testnet network or used for a live destination transaction, and the Kokonut Intelligence bridge/reconciliation is not implemented. The examples are proposed configurations, not production approvals.

## Current status and boundaries

- The tracked [Protocol PRD](docs/protocol/research-backed-prd.md) distinguishes offline prototype evidence from KI integration, testnet operation, production readiness, and independent review.
- KI remains canonical for farm operations, accounting, evidence, and approvals. Do not add KI migrations or bridge integration until the KI refactor is finalized and Wasabi specifies the KI PR target.
- No live signing, destination-chain broadcast, deployment, or production financial activity is authorized by this repository. Obtain separate authorization for any such operation.
- The EVM project factory and its invariant tests use fixtures only. They do not establish a real component's manifest completeness, Safe/DAO identity, governance suitability, or permanent authority.
- The [dual-Anvil integration test](docs/protocol/evm-anvil-integration.md) exercises real JSON-RPC and local transaction receipts on two isolated loopback nodes; it does not test public-chain consensus finality or authorize a testnet deployment.

See [ADR 0001](docs/adr/0001-protocol-boundary-and-chain-profiles.md), the [NEAR execution-controller notes](docs/protocol/execution-controller.md), the [testnet EVM adapter notes](docs/protocol/evm-adapter.md), the [local dual-Anvil integration scope](docs/protocol/evm-anvil-integration.md), the [dependency-maintenance review](docs/protocol/dependency-maintenance.md), the [offline EVM factory prototype](docs/protocol/evm-factory.md), the [Majeur source pin and fit assessment](docs/protocol/majeur-assessment.md), the [testnet relayer orchestration notes](docs/protocol/relayer.md), the [development/testnet recovery runbook](docs/protocol/runbook.md), the [Protocol threat model](docs/protocol/threat-model.md), the [agent proposal-only boundary](docs/protocol/agent-boundary.md), the [offline Revenue Container simulator](docs/protocol/revenue-containers.md), and [AGENTS.md](AGENTS.md) for architecture, execution invariants, adapter lifecycle, orchestration, recovery constraints, and contribution rules.

## Development

Requires Python 3.10+ and the development dependencies in `requirements-dev.txt` (JSON Schema validation, EVM Keccak, secp256k1 signature, and RLP support). EVM contract checks require the Foundry version pinned in `contracts/evm/.foundry-version`. CI also runs version-pinned Slither and Semgrep plus Aderyn with a SHA-256-pinned Linux binary; the corresponding pins are in `contracts/evm/`.

```sh
python3 -m pip install -r requirements-dev.txt
python3 -m pip install "slither-analyzer==$(< contracts/evm/.slither-version)" "semgrep==$(< contracts/evm/.semgrep-version)"
foundryup --install "$(tr -d '\\r\\n' < contracts/evm/.foundry-version)"
make lint
make test
make lint-evm
make test-evm
make audit-evm
make audit-semgrep
make audit-aderyn
make audit-secrets
make ci
```

Local `make ci` requires `slither` and `semgrep` on `PATH`, plus `aderyn` installed from the version and checksum pinned in `contracts/evm/.aderyn-version` and `contracts/evm/.aderyn-linux-x86_64-sha256`. Set `TMPDIR` or `RUNNER_TEMP` to an existing directory for the Gitleaks scanner's temporary files. `make audit-secrets` verifies the scanner checksum, self-tests redacted historical/working-tree detection, then scans the working tree and all locally fetched Git refs.

GitHub Actions installs `cargo-near` and Foundry from versioned release archives with checked-in SHA-256 pins, replacing the mutable bootstrap installers. CI also verifies the pinned cargo-near source commit signature and Foundry's attestation against the expected release workflow, tag, and source commit. The cargo-near archive has no upstream artifact attestation, so its hash pin protects integrity but does not prove how the binary was built; see the [security audit](docs/protocol/security-audit-2026-10-24.md) for that residual gate. The Semgrep check uses the public `p/smart-contracts` ruleset, disables metrics, and excludes the performance-only rule that recommends making the factory constructor payable; the factory intentionally does not accept ETH.
