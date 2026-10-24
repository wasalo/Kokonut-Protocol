# Local dual-Anvil EVM integration test

## Scope

`tests/evm_anvil_integration.py` runs the Protocol EVM adapter against two independent Anvil processes bound only to `127.0.0.1`, using the configured Base Sepolia and Gnosis Chiado chain IDs. It does not fork a public network, read RPC endpoint environment variables, contact a public RPC, or deploy anything outside the ephemeral local nodes. `contracts/evm/test/AnvilEffectTarget.sol` is a test fixture, not a project component or production contract.

The test compiles and locally deploys the fixture to each node, reserves nonce `0` for the same deterministic test address on both distinct chain IDs, rejects using a prepared transaction through the other-chain adapter, broadcasts one locally signed transaction to each node, and checks the actual JSON-RPC receipts, emitted event, and resulting storage value. The private scalar used in the test is a public, deterministic fixture key; it must never be reused for a real account or network.

Run it with Foundry/Anvil available:

```sh
make test-evm-anvil
```

The target first builds the fixture, then starts both nodes inside the test process and terminates them during teardown. CI runs this target only in the Foundry-enabled EVM job.

## Finality limit

Anvil is a local EVM development node, not a consensus-backed chain. The test mines additional blocks and observes either a `finalized` tag if the node exposes one, or `confirmation_threshold_met`. The adapter must not promote a confirmation threshold to consensus finality: when only the threshold is available, the lease remains `broadcast`, even though the local receipt's event and storage effect can be inspected. This is intentional fail-closed evidence; it does not prove public-chain finality, testnet behavior, or reconciliation into Kokonut Intelligence.

## Slice checklist

- [x] Exercise JSON-RPC chain IDs, fee estimation, simulation, local transaction broadcast, receipts, event/state effects, and nonce isolation on two loopback-only Anvil nodes.
- [x] Reject cross-chain reuse of a prepared transaction.
- [x] Preserve the distinction between a mined/confirmed-depth local receipt and consensus finality.
- [ ] Identify the real EVM project component, exact source/version, and intended deployment configuration.
- [ ] Identify the intended project Safe/DAO address and target chain; review its owners, threshold, modules/guards, fallback handler, and governance policy.
- [ ] Trace the real component's complete privileged surface, including initializer/reinitializer, roles, proxy/beacon/implementation/admin, upgrade, delegatecall, and inherited control paths; verify that runtime manifests do not omit any authority.
- [ ] Prove the selected Safe/DAO permanently controls every relevant component authority, with component-specific tests and invariants.
- [ ] Complete the full Protocol/component/dependency security audit, resolve Critical/High findings, and obtain separate explicit authorization before any public/testnet deployment.

The unchecked authority/audit items cannot be established by the fixture, Anvil run, static-analysis tools, or a green CI result. No public/testnet or production deployment is part of this slice.
