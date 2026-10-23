# Kokonut Protocol

Chain-neutral policy and interoperability layer for Kokonut Network. Kokonut Intelligence remains the canonical operational, accounting, evidence, and approval system. NEAR is modeled as an interoperability/signing hub; destination networks retain their own settlement and rights.

## Foundation slice

This repository establishes chain capabilities and an initial NEAR Hub registry, policy, and execution-approval slice. It includes schema-validated examples for a NEAR hub and an EVM test destination, plus a Rust contract for chain/project profiles, versioned signer bindings and execution policies, and policy-checked execution proposals with replay protection and approval quorum. These approvals are only control-plane state: Chain Signatures, destination-chain signing/broadcast, nonce leases, finality/reconciliation, and financial contracts are not implemented. The examples are proposed configurations, not deployed integrations or production approvals.

See [ADR 0001](docs/adr/0001-protocol-boundary-and-chain-profiles.md), the [NEAR execution-controller notes](docs/protocol/execution-controller.md), and [AGENTS.md](AGENTS.md) for architecture, execution invariants, and contribution rules.

## Development

Requires Python 3.10+ and `jsonschema` (see `requirements-dev.txt`).

```sh
python3 -m pip install -r requirements-dev.txt
make lint
make test
make ci
```
