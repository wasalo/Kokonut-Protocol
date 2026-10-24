# Repository guidance

- Kokonut Intelligence is the canonical source for farm operations, finances, evidence, and human approvals. Do not duplicate that state here.
- Do not implement the KI bridge or edit KI migrations while its refactor is underway. Resume KI-owned integration only after the refactor is finalized and Wasabi specifies the KI PR target.
- Treat NEAR as an interoperability/signing hub, not as a replacement for destination chains such as Gnosis or Celo.
- Keep chain capability declarations chain-neutral and explicit; only advertise a capability after its tests and operational requirements are defined.
- A signature is not execution, a transaction hash is not finality, and finality is not reconciliation.
- No live signing, destination-chain broadcast, deployment, or production operation without separate explicit authorization. Do not infer authorization from testnet profiles, mocks, or passing CI.
- Do not deploy to any public network until the full Protocol, component-authority, and dependency security audit is complete, Critical/High findings are resolved, and testnet deployment has separate explicit authorization. Ephemeral loopback Anvil fixtures are test-only and are not deployment evidence.
- Keep direct/transitive dependencies and CI actions current; check authoritative security advisories and update lockfiles/pins together. Do not silently accept breaking or unreviewed dependency changes.
- The SQLite nonce database and its backups contain sensitive raw signed transaction bytes. Keep them access-controlled; never place them in Git, tickets, or chat.
- Do not add production financial contracts, custody behavior, or autonomous financial authorization without a separately reviewed scope.
- Before writing Solidity or shipping anything onchain, read https://ethskills.com/SKILL.md and follow it.
- Do all active development on `develop`; target pull requests to `develop`. Only target `main` after the PRD is finalized and the product owner explicitly directs that release PR.
- Run `make ci` before proposing changes. Add focused tests before behavior or schema changes.
