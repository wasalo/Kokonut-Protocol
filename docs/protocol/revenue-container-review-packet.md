# Revenue Container Economic and Legal Review Packet

**Status:** Preparation packet for product, economic, governance, accounting, and external-counsel review. Not a legal opinion, investment analysis, accounting policy, offering document, or authorization to pilot.
**Prepared:** 2026-10-25
**KI status:** No KI schema, migration, accounting, event, or approval changes are proposed here. KI remains the canonical system; KI-dependent implementation waits for the refactor and an explicit KI PR target.

## 1. Purpose and scope

This packet frames the decisions and evidence needed before a project-specific Revenue Container pilot is considered. It is deliberately jurisdiction-neutral: the project entity, participant locations, offering/solicitation locations, assets, and relevant service-provider jurisdictions have not been specified. It does not classify a token or arrangement under any law. Qualified counsel must identify applicable jurisdictions and research current primary authorities once the actual parties and pilot scope are known.

No capital raise, participant enrollment, economic-right issuance, treasury deployment, or distribution is authorized by this document.

## 2. Proposed product boundary for review

The PRD's working architecture is one project-specific Revenue Container, a segregated project treasury/accounting boundary, and—only if approved—an explicitly configured project-specific economic-rights mechanism. Do not expose the existing Kokonut DAO treasury to a new project's rights by default.

Keep these concepts separate in every draft and model:

- cash distribution entitlement;
- receipt of a stablecoin or chain-native asset;
- Moloch Loot or Shares under a specific DAO configuration;
- governance/reputation signal;
- project token with separately defined terms;
- existing Kokonut DAO membership.

A label such as “Revenue Container,” “Loot,” or “Shares” does not by itself establish a legal right, accounting treatment, governance design, or participant expectation. Terms and actual contract configuration must be reviewed together.

## 3. Current model evidence and limits

The existing `protocol/revenue_simulator.py` is a pure offline arithmetic model. It accepts caller-supplied amounts assumed to be realized and reconciled, uses integer base units, scopes a period and allocations to one project and one asset, and checks that allocations do not exceed modeled distributable cash. It has no evidence verification, database, KI integration, accounting close, approvals, token, legal, price/FX, settlement, or treasury behavior. Its results are proposals, not entitlements or instructions.

The current illustrative waterfall is:

```text
realized gross receipts
- refunds / reversals
- direct operating expenses
- taxes / statutory obligations
- contractual liabilities
- approved reserve funding
= distributable cash

distributable cash
→ approved beneficiary allocations
→ optional project/DAO allocation
→ separately authorized settlement
```

No percentages, return assumptions, settlement asset, beneficiary class, expense cap, reserve rate, payment cadence, or other distribution term is embedded in the simulator. Do not fill those gaps with invented example terms.

## 4. Economic review worksheet

Complete with verified project records and identify the evidence owner for every input. If evidence is unavailable, label the input “unknown”; do not present a forecast as realized revenue.

### A. Project and revenue source

- Project/container name and unique project reference: `[pending]`
- Project operator and responsible legal entity/entities: `[pending]`
- Goods/services/rights that generate receipts: `[pending]`
- Payer/customer types and contract basis: `[pending]`
- Revenue source evidence (invoices, contracts, bank/on-chain statements, reconciliations): `[pending]`
- Seasonality, concentration, price/volume dependencies, and collection delay: `[pending]`
- Whether any receipts are grants, donations, loans, prepayments, insurance, or asset sales rather than operating revenue: `[pending]`

### B. Period close and deductions

- Period start/end and close authority: `[pending]`
- Gross-receipt recognition rule and evidence cutoff: `[pending]`
- Refund/reversal handling, including reversals after a distribution: `[pending]`
- Direct cost categories and allocation evidence: `[pending]`
- Shared overhead allocation rule, if any: `[pending]`
- Tax and statutory-liability estimate/reconciliation owner: `[pending]`
- Contractual liabilities and priority: `[pending]`
- Reserve purpose, sizing method, cap, custody, and release authority: `[pending]`
- Treatment of zero/negative distributable periods, carry-forward, and losses: `[pending]`
- Post-close correction policy and immutable audit trail: `[pending]`

### C. Assets, valuation, and settlement

- One asset per modeled period and its canonical identifier: `[pending]`
- Token/asset decimals and authoritative balance evidence: `[pending]`
- If converting assets, approved rate source, timestamp, fees, slippage, and valuation policy: `[pending]`
- Treatment of volatile, illiquid, restricted, or in-kind receipts: `[pending]`
- Settlement chain, treasury controller, transaction costs, and finality/effect evidence: `[pending]`
- Unclaimed, failed, returned, or disputed distributions: `[pending]`

Do not combine different assets in a single arithmetic total without an approved valuation policy. Keep exact base units; do not use floating point.

### D. Beneficiaries and waterfall

- Eligible beneficiary classes and exclusion rules: `[pending]`
- Allocation basis (contractual, labor, supply, service, governance, or other): `[pending]`
- Whether allocations are fixed, variable, capped, discretionary, or conditional: `[pending]`
- Treatment of related parties, conflicts, and operator compensation: `[pending]`
- Approval authority for period close and each distribution: `[pending]`
- Whether any allocation is a direct contractual claim, discretionary grant, token right, or other instrument: `[pending]`
- Policy versioning, effective date, amendment authority, and protection of already-closed periods: `[pending]`
- Dispute hold, appeal, fraud correction, and wind-down waterfall: `[pending]`

Economic reviewers should model downside cases using actual data and clearly labeled assumptions: no/low revenue, delayed collection, refunds after close, expense overruns, reserve shortfall, volatile-asset loss, and interrupted operations. Do not state guaranteed yield, return, impact, or repayment.

## 5. Rights and instrument decision matrix

Do not select or issue an instrument until product, governance, and counsel have reviewed the actual rights and terms.

| Candidate | Rights that must be specified | Evidence/decision status |
|---|---|---|
| Direct approved cash distribution | Eligibility, amount formula, timing, priority, correction, dispute, unclaimed funds | `[pending]` |
| Stablecoin/native-asset payment | Asset, chain, conversion, fees, custody, failed settlement, and tax/accounting records | `[pending]` |
| Moloch Loot | Exact configured economic/exit and governance semantics; transfer/redemption limits | `[candidate only; review required]` |
| Moloch Shares | Voting and economic semantics, supply, quorum, proposal thresholds, transfer/exit, dilution | `[candidate only; review required]` |
| Project token or other right | Complete written terms, transfer restrictions, redemption/termination, and control | `[not specified]` |
| Existing Kokonut DAO membership | Existing DAO rules and treasury rights; not automatically attached to a new project | `[no expansion approved]` |

Before choosing a Majeur/Moloch-style pilot, use the exact source pin and configuration from `docs/protocol/majeur-assessment.md`. That offline assessment records material governance/delegatecall and configuration risks; it does not approve integration or deployment. Any pilot needs separate source/configuration review, appropriate invariants, and a segregated project treasury.

## 6. Legal-counsel diligence agenda

This is a checklist of questions for qualified counsel, not a statement that any regime applies. Counsel should build the jurisdiction map first and research current primary authorities for the actual scope.

### A. Jurisdiction and parties

Map each relevant location and role:

- project/farm operations and land or resource rights;
- entity formation, management, treasury, and governing documents;
- issuer/organizer and service providers;
- participants/beneficiaries, residence, solicitation, and transfer locations;
- asset issuer, custody/payment provider, exchange/settlement provider, and deployed-chain operations;
- governing law, dispute forum, and cross-border flows.

Jurisdictions and accountable entities: `[pending; do not infer from the user's location or an example farm]`.

### B. Characterization and permissions

Ask counsel to assess, for each relevant jurisdiction and the exact rights/marketing/flow of funds, whether the proposed structure implicates any requirements concerning:

- securities, investment contracts, collective investment/fund activity, or public offerings;
- cooperative, company, DAO, partnership, fiduciary, or investment-manager status;
- payment services, money transmission, stablecoin/payment instruments, custody, or exchange activity;
- consumer disclosures, financial promotions, solicitation, advertising, or return claims;
- AML/CFT, sanctions screening, participant onboarding, source of funds, and recordkeeping;
- tax, withholding, VAT/sales taxes, revenue recognition, and participant reporting;
- land, labor, agricultural, benefit-sharing, environmental claims, and local permits;
- privacy/data protection, cross-border transfers, identity collection, and retention;
- IP, licensing, insolvency, creditor priority, dispute handling, wind-down, and unclaimed assets.

The checklist is intentionally broad. Counsel determines which topics apply, which do not, and what registrations, exemptions, documents, or controls are needed. No classification or exemption is asserted here.

### C. Documents counsel may need

- entity and governance documents, signatory authority, and treasury control map;
- project operating contracts and source revenue evidence;
- draft participant terms, disclosures, allocation policy, and conflict policy;
- beneficiary/participant categories and jurisdiction map;
- asset/chain/custody/settlement design and key-control roles;
- marketing copy, website/app screens, agent scripts, and any yield/impact language;
- accounting close, tax, refund, dispute, and wind-down procedures;
- exact smart-contract source/version, deployment configuration, multisig/Safe/DAO owners and thresholds, and independent review status.

## 7. Governance, treasury, and operational control review

- Confirm a separate project treasury/control boundary; specify who can deposit, hold, approve, transfer, pause, and wind down.
- Define separation between operator, accounting preparer, reviewer, approval quorum, signer, and beneficiary roles.
- Establish evidence and reconciliation requirements before a period becomes distributable.
- Define conflict disclosures, related-party transactions, reserve use, emergency holds, and appeal process.
- Decide who may amend the waterfall and whether changes apply prospectively only.
- Define insolvency, operator exit, project closure, and residual-asset treatment.
- Require read-back verification of destination-chain ownership after the intended bootstrap-admin handoff: the owner has clarified the intended sequence as Kokonut EVM Multisig temporary admin followed by transfer to the deployment's final project Safe/DAO. The current factory prototype does not implement or verify that generic transfer; no capital or rights should rely on the handoff until a real component-specific path and exact addresses are tested.

## 8. Review sequence and stop conditions

1. **Scope:** identify a real project, parties, revenue source, assets, and candidate pilot boundary. No capital accepted.
2. **Economic review:** reconcile historical inputs, complete cost/liability/reserve rules, and test downside cases. No invented percentages or forecast-as-fact.
3. **Legal review:** counsel supplies the jurisdiction map, characterization, required entity/terms/disclosures, and written conditions. This packet is not that review.
4. **Governance:** approve instrument, treasury, waterfall, policy-change, dispute, and wind-down rules.
5. **Technical/security:** review exact contracts/configuration, authority handoff, treasury protections, and independent test/audit scope.
6. **Separate authorization:** only after all required reviews may a separately authorized pilot be considered. This packet grants no authority to deploy, solicit, receive funds, issue rights, or settle.

Stop if jurisdictions, source revenue, participant rights, the project treasury, final controller, or accounting evidence are unknown; if marketing implies guaranteed returns or verified ecological outcomes without substantiation; if the proposed rights cannot be described in plain language; or if the proposed design exposes the existing Kokonut DAO treasury by default.

## 9. Decisions and review record

- Product owner: `[pending]`
- Project/farm and operating entity: `[pending]`
- Economic reviewer/accounting owner: `[pending]`
- Jurisdictions and external counsel: `[pending]`
- Candidate instrument and complete rights: `[pending]`
- Asset and treasury controller: `[pending]`
- Waterfall and reserve policy: `[pending]`
- Governance approval reference: `[pending]`
- Legal conditions and document owners: `[pending]`
- Security review target/configuration: `[pending]`
- Approved pilot scope and separate authorization: `[not granted]`

## 10. Source materials

- `docs/protocol/research-backed-prd.md` — architecture, Revenue Container proposal, and open owner/DAO decisions.
- `docs/protocol/revenue-containers.md` — offline simulator behavior and limits.
- `docs/protocol/majeur-assessment.md` — pinned-source fit/risk assessment; not legal or deployment approval.
- `docs/protocol/threat-model.md` and `docs/protocol/security-audit-2026-10-24.md` — security and operational gates.

This packet records questions and evidence requirements only. It does not replace KI accounting, external counsel, an independent economic review, governance approval, or an independent smart-contract audit.
