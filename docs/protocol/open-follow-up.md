# Open Follow-up — Protocol Authority and Address Provisioning

**Status:** Provisioning worksheet only. No addresses have been supplied or verified; no deployment or live operation is authorized.
**Purpose:** Collect each public account/address under its actual role before wiring the NEAR Hub and EVM factory configuration. Do not send private keys, seed phrases, signing credentials, or recovery codes.

## Role distinction at a glance

There is **no single `protocolAdmin` variable** in the current EVM project-factory contract. “Protocol Admin” may describe the Kokonut governance body that approves the role assignments, but the factory stores two distinct, immutable EVM authority addresses:

| Variable / role | Network format | What it controls | When provisioned |
|---|---|---|---|
| NEAR Hub `administrator` | NEAR account ID | Root Hub configuration, including chain/project, signer, policy, and pause controls | For a NEAR Hub deployment/configuration; governance control must be verified separately |
| EVM factory `configurationAuthority` | EVM address, per factory/network | Adds or revokes allowlisted component creation-code hashes | As a factory constructor argument, before deploying that factory instance |
| EVM factory `deploymentAuthority` | EVM address, per factory/network | Calls `deployProject` using already-allowlisted creation code | As a factory constructor argument, before deploying that factory instance |
| Project component `projectAdmin` | EVM contract address, per project deployment | Controls the deployed component's project-admin surface | **Deferred until a specific project deployment is scoped**; passed to `deployProject` for that deployment |

The NEAR Hub administrator is not an EVM address and is not either EVM factory authority. The two EVM factory authorities are also not a per-project `ProjectAdmin`.

## What “Protocol Admin” means in the current factory

The current `KokonutProjectFactory` constructor accepts `configurationAuthority` and `deploymentAuthority`. It requires both to be nonzero and different. These values are `immutable`; there is no function to set or replace them after factory deployment. The `configurationAuthority` can change the creation-code allowlist, but cannot appoint or replace the `deploymentAuthority`. The `deploymentAuthority` can deploy allowlisted components, but cannot change the allowlist.

If “Protocol Admin” means the governance body that decides who should hold these powers, record that body and its approval reference separately. Provide the two role-specific addresses separately even if one governance organization controls both. Under the current code, the same EVM address cannot fill both factory roles. Changing either assignment after construction would require a separately designed and reviewed architecture; do not assume an admin setter exists.

## Addresses and governance evidence to provision before factory wiring

### A. NEAR Hub root administrator

- NEAR network/environment: `[decision required; no public deployment implied]`
- `administrator` account ID: `[provide when authorized for Hub configuration]`
- Account/controller type and governing process: `[pending]`
- Human/DAO approval reference and threshold/recovery procedure: `[pending; see near-governance-decision-memo.md]`
- Verification evidence (account control and approved governance): `[pending]`

This account controls the NEAR Hub's root methods. The contract itself does not implement a multisignature quorum, timelock, or administrator-rotation workflow; those controls must exist in the account's external governance and be reviewed.

### B. EVM factory authorities — repeat once per intended factory/network

Do not assume a network from existing adapter examples. List only the destination network(s) selected for the factory.

| Field | Value to provide |
|---|---|
| Network name and environment | `[pending]` |
| EVM chain ID (`eip155`) | `[pending]` |
| `configurationAuthority` address | `[pending]` |
| Configuration authority type (Safe/DAO/other contract/EOA) | `[pending]` |
| Configuration authority governance (owners, threshold, modules/guards, upgrade control) | `[pending]` |
| `deploymentAuthority` address | `[pending]` |
| Deployment authority type (Safe/DAO/Chain-Signature-derived account/other) | `[pending]` |
| Deployment authority governance and permitted callers/process | `[pending]` |
| Governing body approving both assignments | `[pending]` |
| Approval/reference for these exact chain-qualified assignments | `[pending]` |
| Confirm `configurationAuthority != deploymentAuthority` | `[must be yes under current constructor]` |

The factory source checks only that the two addresses are nonzero and unequal. That check does not prove either address is governed safely; record and review the actual control configuration independently.

### C. Factory deployment transaction — operational inputs, not factory roles

- Factory deployer / transaction submitter for the chosen chain: `[pending when deployment is separately authorized]`
- Approved factory creation-code hash / source revision: `[pending after source review]`
- Factory address: `[generated by deployment; record after separately authorized deployment]`
- Constructor arguments: the two addresses above, verified against the approved record before submission.

The transaction submitter is not automatically a lasting factory authority. Do not confuse the factory deployer with `configurationAuthority`, `deploymentAuthority`, or the NEAR Hub administrator.

## ProjectAdmin — intentionally deferred

No `ProjectAdmin` address is requested in this provisioning round. It is supplied as the `projectAdmin` argument when a particular project component is deployed; it is not a global factory setting. Under the current prototype, the factory initializes that component directly with the supplied per-project address and verifies the reported value and declared authority manifest.

When a real project deployment is scoped, provide its final project Safe/DAO (or other explicitly approved project controller) on that destination chain, together with the project ID and exact component/version. The current prototype requires `projectAdmin` to be nonzero, have deployed code at deployment time, and differ from the factory address, `configurationAuthority`, and `deploymentAuthority`. The project-specific component and its complete privileged/upgrade surfaces still require review. Do not supply this value early or treat it as one Protocol-wide admin.

## Provisioning and verification rules

1. Provide public addresses/account IDs only—never private keys or credentials.
2. Keep one record per network and chain ID; the same label on two chains does not prove it is the same governed controller.
3. Verify address/account existence, contract code where applicable, and actual governance configuration before wiring values into configuration or deployment code.
4. Confirm the two EVM factory authority addresses are distinct and match the approved role assignment. The factory constructor arguments are immutable after deployment.
5. Keep the NEAR Hub administrator, EVM factory authorities, transaction submitter, and future project `ProjectAdmin` as separate named fields even if governance relationships connect them.
6. Supplying addresses is not approval to deploy. Public-network deployment, live signing, broadcast, or production operation still requires separate explicit authorization and the repository's security gates.

## Related decision materials

- `near-governance-decision-memo.md` — NEAR administrator governance, signer rotation, and storage decisions.
- `revenue-container-review-packet.md` — project-specific economic/legal review preparation; no pilot approval.
- `evm-factory.md` — current factory prototype behavior and limitations.
- `research-backed-prd.md` — architecture and remaining gates.

## Completion record

- NEAR Hub administrator provisioned and verified: `[pending]`
- EVM target networks selected: `[pending]`
- `configurationAuthority` provisioned and verified on each selected network: `[pending]`
- `deploymentAuthority` provisioned and verified on each selected network: `[pending]`
- Role separation and governance approvals recorded: `[pending]`
- Factory source/creation code and constructor input review completed: `[pending]`
- `ProjectAdmin`: intentionally deferred until a specific project deployment.
