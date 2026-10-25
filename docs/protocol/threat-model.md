# Kokonut Protocol Threat Model

**Status:** Prototype threat model for offline/testnet development. This is not an audit, production approval, or authorization to sign, broadcast, or deploy.

## Security objective and scope

Protocol coordinates approved cross-chain actions. The critical property is that an attacker or failure cannot convert a proposal, signature, RPC response, or agent output into an unauthorized or duplicate destination-chain effect.

The security boundary spans:

1. Kokonut Intelligence (KI), which remains canonical for operational truth, evidence, accounting, and human approvals.
2. The Protocol client/relayer and its durable nonce-lease store.
3. The NEAR Hub, which checks project, policy, signer, approval, expiry, pause, and replay conditions before requesting a signature and rechecks execution status, active policy version, expiry, and pause state when the asynchronous signature callback resolves.
4. NEAR Chain Signatures, which can produce a signature but do not broadcast or finalize a destination transaction.
5. Destination RPCs and chains, which construct, accept, finalize, and expose local state.
6. Project-local factories, Safe/DAO controls, and treasury boundaries.

This repository currently contains a NEAR contract prototype and offline/testnet-only EVM client components. It has no live KI connector, production queue consumer, production signer client, production database, production financial contract, or deployed destination factory. The controls below describe code/tests where present and name residual risks; they do not imply the architecture is production-ready.

## Assets requiring protection

- Human approval and active policy binding to one exact action.
- NEAR signer configuration, key version, and project/chain/purpose derivation path.
- Nonce leases and exact signed transaction bytes.
- Destination-chain funds, contract administration, and project-specific rights.
- Reconciled revenue, liabilities, reserves, distributions, and redemption state.
- Private farm, stakeholder, accounting, and evidence data.
- Audit history linking request, approval, signature, broadcast, finality, effect, and later KI reconciliation.

## Trust boundaries

| Boundary | Trust assumption | Required control |
|---|---|---|
| KI → Protocol | KI provides current, authorized records; Protocol cannot replace KI's canonical data. | Authenticated read path, least privilege, fresh reads, action/policy hash checks, and governed reconciliation. The bridge is not implemented here. |
| Protocol → NEAR Hub | Contract state and authorization are checked before signature requests. | Role checks, project/chain scoping, immutable policy versions, quorum, expiry, pause, and request replay protection. |
| NEAR → destination signer | Signature is valid only for the approved digest and configured signer binding. | EIP-155 chain binding, digest reconstruction, signature recovery, key-version checks, and purpose-separated paths. |
| Relayer → RPC | RPC data may be stale, dishonest, unavailable, or inconsistent. | TLS except loopback development, expected chain-ID checks, transaction/receipt identity checks, canonical block checks, and consensus-finality checks. Multi-provider verification remains unimplemented. |
| Relayer → durable state | A worker may crash or receive an ambiguous response. | Durable nonce lease, signed-byte persistence, exact-byte retries, state validation, and replay-safe confirmed-result recovery. The current SQLite store is not a production shared-store design. |
| Protocol → destination rights | A valid signature must not make a cross-project treasury or permanent-custody design acceptable. | Local allowlisted contracts and project-specific Safe/DAO ownership. No destination factory or production financial contract exists in this repository. |

## Threat register

| Threat | Severity | Prototype control or status | Residual risk / release gate |
|---|---:|---|---|
| NEAR Hub authorization or upgrade compromise | Critical | Role checks, scoped project/chain policy, approval quorum, pause controls, and policy versioning are exercised by contract tests. | Contract is not deployed or independently audited. Review upgrade authority, key operations, and recovery before any deployment. |
| Authorized signer/key compromise | Critical | Signer bindings are versioned and paths are project/chain/purpose-specific; signing payloads bind EIP-155 chain ID and committed action. | MPC/signature service is not integrated or operationally tested. Define rotation, quorum, key monitoring, and incident authority before use. |
| Approval or policy changes after signing | Critical | Relayer re-reads and revalidates current NEAR state after signature retrieval; adapter repeats execution gates. | No KI bridge or live NEAR read client exists here. Integration must preserve fresh reads and human approval. |
| Cross-chain or duplicate-request replay | Critical | NEAR request/idempotency protections, chain-bound EVM payload, durable nonce lease, and exact signed-byte reuse are tested. | Multi-worker/multi-host behavior needs a production-grade shared durable store and integration tests. |
| Nonce race or crash after signing | Critical | SQLite lease persistence, signed-byte persistence, restart tests, lost-ack injection, confirmed replay tests, and an offline SQLite backup/restore test are present. | The local store is not a production HA database; backup protection/retention, concurrency, failover, lease ownership, and operator recovery procedures remain to be designed and tested. |
| Lost or ambiguous broadcast acknowledgement | High | Retry path reuses the persisted signed bytes; fake RPC tests model acceptance followed by a lost acknowledgement and already-known response. | Live provider behavior and event-bus retry/dead-letter behavior have not been tested. Never use a changed transaction to resolve uncertainty. |
| Destination RPC deception or outage | High | Adapter verifies configured chain ID, TLS requirements, transaction identity, canonical receipt block, and consensus finality. | Independent multi-provider comparison and operational alerting are not implemented. A single provider can still deny service or mislead uncorroborated reads. |
| Chain reorganization or insufficient finality | High | Canonical block-hash checks gate finality; confirmation-depth thresholds alone cannot mark the transaction finalized. | Chain-specific production policies and reorg rehearsals remain required. |
| Signature mistaken for execution, finality, or reconciliation | High | Documentation and code separate signature, broadcast, finality, expected-effect verification, and KI reconciliation. | KI reconciliation remains unimplemented. Do not report completion or accounting effects from signature/hash alone. |
| Destination effect differs from approved intent | Critical | Expected-effect commitment is checked; effect verification occurs only after finality; mismatch stops for manual review. | Each production operation requires a reviewed predicate and adversarial tests. No factory-specific effect verifier exists yet. |
| Cross-project treasury leakage or permanent signer custody | Critical | Current code contains no production treasury contract; architecture requires project-specific boundaries and initialization of permanent administration to a local Safe/DAO. | Destination factory and economic contracts are not implemented. Enforce the boundary in contract design and independent review before any funds. |
| Revenue over-allocation, rounding, or asset mixing | Critical | Offline simulator uses integer base units, rejects floats/bools/negative amounts, checks deductions, caps allocations, and binds a proposal to one project and asset. | Inputs are caller-supplied and not evidence; no KI accounting/reconciliation, approval, settlement, rights, redemption, or economic audit exists. |
| Agent or prompt-injection privilege escalation | Critical | No AI execution integration is present; Protocol approval roles remain distinct from proposal submission. | Any future agent integration must be proposal-only by default, deny self-approval, and require human/DAO authorization for material actions. |
| Private evidence or personal data disclosed on-chain | High | Protocol design stores commitments/references rather than raw farm evidence or personal records. | Review every future schema, event, calldata, and log for data minimization; hashes can still reveal linkability. |
| Supply-chain compromise | High | Python requirements and Rust dependencies are version-constrained/locked in repository files; CI runs local checks. | Lockfile provenance, dependency update review, action pinning, artifact attestation, and reproducible release controls need a separate supply-chain review. |
| AI/TEE output treated as governance authorization | Governance/reputation | No AI/TEE authority is implemented; architecture says computational evidence is not approval. | Preserve this boundary in any later agent or attestation integration. |

## Minimum controls before any authorized testnet execution

1. A human-approved request binds exact operation, target, value, payload/effect commitments, chain, nonce, policy version, expiry, and signer key version.
2. The NEAR reader and signature requester are status-aware and read current state before writes; ambiguous timeouts are resolved by reading state before retry.
3. Destination chain ID, signing digest, signature-derived address, nonce lease, simulation, fees, and expected effect all match the approved request.
4. The exact signed bytes are persisted before broadcast and are the only bytes eligible for retry.
5. Finality uses the destination chain's reviewed policy; effect verification is separate and occurs at the finalized receipt state.
6. Manual-review states stop automation and are routed to a human decision; the durable record is never silently reset.
7. No execution result is marked reconciled until the governed KI integration writes and verifies the canonical record.
8. No production financial contracts, custody behavior, autonomous financial authorization, or live operation is enabled by this prototype.

## Required pre-production reviews

- Independent smart-contract and infrastructure security reviews; resolve all Critical/High findings or record an explicit governance exception.
- Shared durable nonce/event-store design, backup/restore rehearsal, multi-worker concurrency and replay tests, alerting, and tested operator recovery.
- Two-provider or stronger RPC verification for high-value actions, with chain-specific finality and reorg policy.
- KI integration review covering identity, least privilege, human approvals, audit lineage, reconciliation, and failure recovery.
- Destination-factory and project treasury ownership review, including immutable implementation hashes and local Safe/DAO administration.
- Economic, legal, tax, and participant-disclosure review before any Revenue Container accepts outside capital.

Related implementation notes: [execution controller](execution-controller.md), [EVM adapter](evm-adapter.md), [relayer](relayer.md), [recovery runbook](runbook.md), and [Revenue Container simulator](revenue-containers.md).
