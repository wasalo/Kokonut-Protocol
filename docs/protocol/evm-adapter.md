# Testnet EVM adapter

`protocol/evm_adapter.py` is the client-side destination adapter. It is restricted to the allowlisted Base Sepolia (chain ID `84532`) and Gnosis Chiado (chain ID `10200`) profiles; those identifiers match [Base’s `eth_chainId` reference](https://docs.base.org/base-chain/api-reference/ethereum-json-rpc-api/eth_chainId) and [Gnosis’s Chiado network page](https://docs.gnosischain.com/about/networks/chiado). It does not configure mainnet. RPC endpoints come from `BASE_SEPOLIA_RPC_URL` or `GNOSIS_CHIADO_RPC_URL`. HTTPS is required except for loopback development endpoints, and endpoint strings are omitted from RPC errors.

## Bound execution flow

1. Read the approved NEAR execution record and the current active policy through the trusted NEAR read path. Provide the exact committed calldata and registered EVM signer address.
2. Call `reserve_approved_nonce(...)`. This revalidates approval, expiry, request/action hash, active policy and payload commitments, reads the RPC `pending` nonce, and atomically leases only the nonce already committed by the approved request. It rejects stale or already-leased nonces; it never edits the approved request to choose a new nonce.
3. After obtaining the NEAR Chain Signature result, call `prepare_approved_transaction(...)`. It repeats the approval/policy/expiry and payload checks, verifies the expected-effect predicate hash, estimates EIP-1559 fees against the approved caps, simulates with the committed transaction fields, verifies the Chain Signature response and recovers its EVM address, then assembles the type-2 RLP transaction. The exact signed bytes are persisted in SQLite before returning.
4. Call `broadcast(...)` with a fresh NEAR record/policy snapshot and exact signature/effect inputs. It repeats the gate and simulation, requires the persisted bytes to match, then broadcasts those same bytes. Retries use the identical signed transaction; they do not re-sign.
5. `check_finality(...)` reads the destination receipt, requires its transaction hash to match the query, and checks the receipt block hash against the block currently canonical at that height. A mismatch is reported as reorged and cannot advance the nonce lease. It then checks the RPC's consensus `finalized` block tag. A configured confirmation-depth threshold is reported only as `confirmation_threshold_met`; it is deliberately **not** treated as consensus finality. `finalize_and_verify_effect(...)` obtains a fresh finality result itself, verifies the committed event/storage/code predicate at the receipt block, and only then marks the lease confirmed. A mismatch moves the request to manual review.

The NEAR call only obtains a signature. Transaction construction, nonce leasing, simulation, broadcast, receipt/finality, and effect checks are separate client-side responsibilities. An EVM transaction hash is not proof of finality, and EVM effect verification is not reconciliation into Kokonut Intelligence.

## Nonce-store snapshot and recovery boundary

`NonceManager.backup_to(destination)` takes a consistent SQLite online snapshot while holding the manager's in-process lock. The destination is supplied by the caller and must be a separate, idle SQLite connection. It contains exact signed transaction bytes and lease metadata, so provision the destination with restrictive filesystem access and keep it out of source control, tickets, and chat. Do not copy an active database file or WAL sidecars as an alternative.

The automated test backs up a database containing broadcast and confirmed leases, restores the snapshot into a fresh database, verifies the exact signed bytes and transaction hashes, and confirms the restored nonce history does not reuse an earlier nonce. This demonstrates local SQLite snapshot mechanics only. It does not provide backup scheduling, encryption/key management, off-host retention, corruption monitoring, HA/failover, lease ownership across hosts, or an operator restore runbook. The development/testnet runbook remains non-production.

## Safety and current limits

- These profiles and tests are testnet-only; no live EVM RPC, signing request, transaction, or broadcast was performed as part of the local test suite.
- The adapter receives a signature result; it does not control NEAR keys or administer the Chain Signatures contract/public key.
- `NonceManager` stores raw signed transactions and lease metadata in SQLite. `NonceManager.backup_to(destination)` uses SQLite's online backup API under the manager lock to produce a consistent snapshot into a caller-owned, separate, idle connection. The destination contains sensitive signed bytes; the caller must securely create and restrict the destination file. Do not copy an active database file or its WAL sidecars as a substitute. The test suite exercises snapshot, restore into a fresh database, and preservation of broadcast/confirmed lease state and exact signed bytes; this is not a production backup/restore service or operator procedure.
- The transaction must already commit to `gas_limit`, `signing_payload_hash`, `signer_key_version`, and `expected_effect_hash`. The adapter does not amend an approved request.
- If the RPC does not expose a finalized block, a confirmation threshold is informational only. The adapter will not mark the request finalized or reconcile it from that threshold.
- Kokonut Intelligence remains canonical for operational, accounting, evidence, and human-approval records. The KI bridge/reconciliation remains a separate integration step.
- No production network, financial contract, custody behavior, or autonomous approval is supported.

Run `make ci` from the repository root to validate schemas, Python tests, the NEAR contract, and optimized Wasm build.
