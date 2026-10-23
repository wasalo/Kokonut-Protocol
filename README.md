# Kokonut Protocol

Chain-neutral policy and interoperability layer for Kokonut Network. Kokonut Intelligence remains the canonical operational, accounting, evidence, and approval system. NEAR is modeled as an interoperability/signing hub; destination networks retain their own settlement and rights.

## Foundation slice

This repository establishes the chain capability contract and an initial NEAR Hub registry/policy slice. It includes schema-validated examples for a NEAR hub and an EVM test destination, plus a Rust contract for chain and project profiles, versioned signer bindings, and immutable per-project/per-chain execution policies. The policies define operation and target allowlists, per-action value caps, and approval thresholds. The examples are proposed configurations, not deployed integrations or production approvals. Chain Signatures, execution requests, cross-chain transaction execution, and financial contracts are not implemented yet.

See [ADR 0001](docs/adr/0001-protocol-boundary-and-chain-profiles.md) and [AGENTS.md](AGENTS.md) for boundary and contribution rules.

## Development

Requires Python 3.10+ and `jsonschema` (see `requirements-dev.txt`).

```sh
python3 -m pip install -r requirements-dev.txt
make lint
make test
make ci
```
