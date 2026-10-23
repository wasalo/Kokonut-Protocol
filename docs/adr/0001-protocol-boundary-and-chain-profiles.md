# ADR 0001: Protocol boundary and chain profiles

- **Status:** Accepted for the foundation
- **Date:** 2026-10-23

## Decision

Kokonut Protocol is an interoperability and execution boundary, not a replacement blockchain or a second canonical farm/finance database. Kokonut Intelligence remains authoritative for project operations, accounting, evidence, and human approvals. Destination chains remain authoritative for their own on-chain settlement and rights.

Represent NEAR and each destination chain as separate chain profiles. NEAR may provide policy-controlled Chain Signatures; an EVM destination profile declares its own chain ID, transaction-signing scheme, and supported capabilities. Do not conflate the NEAR hub with an EVM destination or imply that signing alone executes or finalizes a destination transaction.

## Consequences

- Capability declarations are versioned and validated against a shared schema.
- Chain family, namespace, and signing scheme are explicit and cross-validated.
- The first examples cover a NEAR hub and an EVM test destination; they do not imply that either is production-enabled.
- The initial NEAR contract records chains and project-profile commitments only; Chain Signatures, transaction execution, and financial rights remain out of scope until separately reviewed.
- The contract's administrator must be a governance-controlled NEAR account for any deployment; a single externally controlled key is not an accepted production configuration.
