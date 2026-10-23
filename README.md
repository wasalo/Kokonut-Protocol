# Kokonut Protocol

Chain-neutral policy and interoperability layer for Kokonut Network. Kokonut Intelligence remains the canonical operational, accounting, evidence, and approval system. NEAR is modeled as an interoperability/signing hub; destination networks retain their own settlement and rights.

## Foundation slice

This repository establishes chain capabilities and an initial NEAR Hub registry, policy, execution-approval, and testnet-only signing flow. It includes schema-validated examples for a NEAR hub and two proposed EVM test destinations; a Rust contract for chain/project profiles, versioned signer bindings and execution policies, policy-checked execution proposals with replay protection and approval quorum; a configurable asynchronous NEAR Chain Signatures request; an offline EIP-1559 signing-payload verifier; and a testnet-only EVM adapter for nonce leases, type-2 transaction encoding, fee estimation, simulation, broadcast, consensus-finality checks, and committed-effect verification. These remain control-plane and client-side primitives: no contract or adapter has been deployed or used for a live destination transaction, and the Kokonut Intelligence bridge/reconciliation is not implemented. The examples are proposed configurations, not production approvals.

See [ADR 0001](docs/adr/0001-protocol-boundary-and-chain-profiles.md), the [NEAR execution-controller notes](docs/protocol/execution-controller.md), the [testnet EVM adapter notes](docs/protocol/evm-adapter.md), the [testnet relayer orchestration notes](docs/protocol/relayer.md), the [development/testnet recovery runbook](docs/protocol/runbook.md), and [AGENTS.md](AGENTS.md) for architecture, execution invariants, adapter lifecycle, orchestration, recovery constraints, and contribution rules.

## Development

Requires Python 3.10+ and the development dependencies in `requirements-dev.txt` (JSON Schema validation, EVM Keccak, secp256k1 signature, and RLP support).

```sh
python3 -m pip install -r requirements-dev.txt
make lint
make test
make ci
```
