# Kokonut Protocol

Chain-neutral policy and interoperability layer for Kokonut Network. Kokonut Intelligence remains the canonical operational, accounting, evidence, and approval system. NEAR is modeled as an interoperability/signing hub; destination networks retain their own settlement and rights.

## Foundation slice

This repository currently establishes the chain capability contract only. It includes schema-validated examples for a NEAR hub and an EVM test destination. These are proposed profiles, not deployed integrations or production approvals. No financial contracts or transaction execution are implemented yet.

See [ADR 0001](docs/adr/0001-protocol-boundary-and-chain-profiles.md) and [AGENTS.md](AGENTS.md) for boundary and contribution rules.

## Development

Requires Python 3.10+ and `jsonschema` (see `requirements-dev.txt`).

```sh
python3 -m pip install -r requirements-dev.txt
make lint
make test
make ci
```
