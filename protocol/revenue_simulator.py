"""Offline integer-only Revenue Container waterfall simulation."""

from __future__ import annotations

from collections.abc import Iterable
from dataclasses import dataclass


_MONEY_FIELDS = (
    "gross_receipts_base_units",
    "refunds_base_units",
    "direct_operating_expenses_base_units",
    "taxes_base_units",
    "contractual_liabilities_base_units",
    "approved_reserves_base_units",
)


def _require_identifier(value: object, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{field} must be a non-empty string")
    return value


def _require_nonnegative_integer(value: object, field: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        raise ValueError(f"{field} must be a non-negative integer in base units")
    return value


@dataclass(frozen=True)
class RevenuePeriod:
    """One project's realized, single-asset period inputs in integer base units."""

    project_id: str
    asset_id: str
    gross_receipts_base_units: int
    refunds_base_units: int
    direct_operating_expenses_base_units: int
    taxes_base_units: int
    contractual_liabilities_base_units: int
    approved_reserves_base_units: int

    def __post_init__(self) -> None:
        _require_identifier(self.project_id, "project_id")
        _require_identifier(self.asset_id, "asset_id")
        for field in _MONEY_FIELDS:
            _require_nonnegative_integer(getattr(self, field), field)

    @property
    def distributable_base_units(self) -> int:
        deductions = sum(getattr(self, field) for field in _MONEY_FIELDS[1:])
        distributable = self.gross_receipts_base_units - deductions
        if distributable < 0:
            raise ValueError("period deductions exceed gross receipts")
        return distributable


@dataclass(frozen=True)
class DistributionAllocation:
    """A proposed allocation, not an approval or settlement instruction."""

    distribution_id: str
    project_id: str
    asset_id: str
    beneficiary_id: str
    amount_base_units: int

    def __post_init__(self) -> None:
        _require_identifier(self.distribution_id, "distribution_id")
        _require_identifier(self.project_id, "project_id")
        _require_identifier(self.asset_id, "asset_id")
        _require_identifier(self.beneficiary_id, "beneficiary_id")
        _require_nonnegative_integer(self.amount_base_units, "amount_base_units")


@dataclass(frozen=True)
class SimulationResult:
    """Exact waterfall totals; this result does not authorize distribution."""

    distributable_base_units: int
    allocated_base_units: int
    unallocated_base_units: int


def simulate_period(
    period: RevenuePeriod,
    allocations: Iterable[DistributionAllocation],
) -> SimulationResult:
    """Check one period's exact cash waterfall and proposed allocations.

    All monetary values must already be reconciled integer base units for the
    same asset. This function performs no conversion, persistence, approval,
    redemption, signing, or settlement.
    """
    if not isinstance(period, RevenuePeriod):
        raise ValueError("period must be a RevenuePeriod")

    distributable = period.distributable_base_units
    distribution_ids: set[str] = set()
    allocated = 0
    for allocation in allocations:
        if not isinstance(allocation, DistributionAllocation):
            raise ValueError("allocations must contain DistributionAllocation values")
        if allocation.project_id != period.project_id or allocation.asset_id != period.asset_id:
            raise ValueError("allocation does not match period scope")
        if allocation.distribution_id in distribution_ids:
            raise ValueError("duplicate distribution ID")
        distribution_ids.add(allocation.distribution_id)
        allocated += allocation.amount_base_units

    if allocated > distributable:
        raise ValueError("total allocation exceeds distributable cash")

    return SimulationResult(
        distributable_base_units=distributable,
        allocated_base_units=allocated,
        unallocated_base_units=distributable - allocated,
    )
