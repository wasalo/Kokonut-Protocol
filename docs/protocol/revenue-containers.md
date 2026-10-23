# Revenue Container waterfall simulator

`protocol/revenue_simulator.py` is a pure, offline calculator for the PRD's first cash-waterfall invariants. It uses integer base units only and has no database, RPC, token, legal, or accounting-system integration.

## Scope

A `RevenuePeriod` represents one project, one asset, and caller-supplied amounts that are assumed to be realized and reconciled:

```text
gross receipts
- refunds / reversals
- direct operating expenses
- taxes
- contractual liabilities
- approved reserve funding
= distributable cash
```

`simulate_period(period, allocations)` checks that deductions do not exceed receipts, each proposed allocation uses the same project and asset scope as the period, distribution IDs are unique within the proposal, and total allocations do not exceed distributable cash. It returns exact distributable, allocated, and unallocated base-unit totals.

## Safety boundary

- The simulator does not establish that receipts are real, reconciled, or authorized; callers must not treat its input as evidence.
- The asset identifier is opaque. There is no price feed, foreign-exchange conversion, decimal conversion, or mixing of assets.
- An allocation is a proposal for arithmetic validation, not a beneficiary approval, entitlement, legal claim, or settlement instruction.
- The simulator does not persist periods, enforce approvals, mint rights, process redemptions/Ragequit, select waterfall percentages, sign, or transfer assets.
- It does not implement a Revenue Container product or establish production readiness. No production capital or live financial contract is involved.

All values must be Python integers greater than or equal to zero; booleans, floats, and decimal strings are rejected. The caller supplies exact base-unit values and asset identity. No percentage allocation or return assumption is embedded.

## Offline validation

```sh
~/.cache/kokonut-protocol-venv/bin/python -m unittest discover \
  -s tests -p 'test_revenue_simulator.py' -v
```

The focused tests cover exact large-integer arithmetic, deduction overflow, allocation caps, unique distribution IDs, and project/asset isolation. Full repository checks run with `PATH="$HOME/.cache/kokonut-protocol-venv/bin:$PATH" make ci`.
