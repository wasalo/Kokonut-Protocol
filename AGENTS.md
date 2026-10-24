# Repository guidance

- Kokonut Intelligence is the canonical source for farm operations, finances, evidence, and human approvals. Do not duplicate that state here.
- Treat NEAR as an interoperability/signing hub, not as a replacement for destination chains such as Gnosis or Celo.
- Keep chain capability declarations chain-neutral and explicit; only advertise a capability after its tests and operational requirements are defined.
- A signature is not execution, a transaction hash is not finality, and finality is not reconciliation.
- Do not add production financial contracts, custody behavior, or autonomous financial authorization without a separately reviewed scope.
- Before writing Solidity or shipping anything onchain, read https://ethskills.com/SKILL.md and follow it.
- Do all active development on `develop`; target pull requests to `develop`. Only target `main` after the PRD is finalized and the product owner explicitly directs that release PR.
- Run `make ci` before proposing changes. Add focused tests before behavior or schema changes.
