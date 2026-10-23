import unittest

from protocol.revenue_simulator import (
    DistributionAllocation,
    RevenuePeriod,
    simulate_period,
)


class RevenueSimulatorTests(unittest.TestCase):
    def period(self):
        return RevenuePeriod(
            project_id="project-a",
            asset_id="eip155:84532/0xstable",
            gross_receipts_base_units=100_000_000_000_000_000_003,
            refunds_base_units=3,
            direct_operating_expenses_base_units=10,
            taxes_base_units=5,
            contractual_liabilities_base_units=7,
            approved_reserves_base_units=11,
        )

    def allocation(
        self,
        distribution_id="distribution-1",
        project_id="project-a",
        asset_id="eip155:84532/0xstable",
        amount_base_units=20,
    ):
        return DistributionAllocation(
            distribution_id=distribution_id,
            project_id=project_id,
            asset_id=asset_id,
            beneficiary_id="beneficiary-1",
            amount_base_units=amount_base_units,
        )

    def test_period_uses_exact_integer_waterfall_for_all_deduction_classes(self):
        result = simulate_period(self.period(), [])

        self.assertEqual(result.distributable_base_units, 99_999_999_999_999_999_967)
        self.assertEqual(result.allocated_base_units, 0)
        self.assertEqual(result.unallocated_base_units, result.distributable_base_units)

    def test_allocations_cannot_exceed_distributable_cash(self):
        period = self.period()
        allocation = self.allocation(amount_base_units=period.distributable_base_units + 1)

        with self.assertRaisesRegex(ValueError, "exceeds distributable"):
            simulate_period(period, [allocation])

    def test_valid_allocations_leave_an_exact_unallocated_remainder(self):
        period = self.period()
        result = simulate_period(
            period,
            [
                self.allocation(amount_base_units=20),
                self.allocation("distribution-2", amount_base_units=30),
            ],
        )

        self.assertEqual(result.allocated_base_units, 50)
        self.assertEqual(
            result.unallocated_base_units,
            period.distributable_base_units - 50,
        )

    def test_period_rejects_deductions_above_realized_receipts(self):
        period = RevenuePeriod(
            project_id="project-a",
            asset_id="eip155:84532/0xstable",
            gross_receipts_base_units=10,
            refunds_base_units=11,
            direct_operating_expenses_base_units=0,
            taxes_base_units=0,
            contractual_liabilities_base_units=0,
            approved_reserves_base_units=0,
        )

        with self.assertRaisesRegex(ValueError, "exceed gross receipts"):
            simulate_period(period, [])

    def test_money_fields_reject_float_bool_negative_and_string_values(self):
        for invalid in (1.5, True, -1, "1"):
            with self.subTest(value=invalid):
                with self.assertRaisesRegex(ValueError, "non-negative integer"):
                    RevenuePeriod(
                        project_id="project-a",
                        asset_id="eip155:84532/0xstable",
                        gross_receipts_base_units=invalid,
                        refunds_base_units=0,
                        direct_operating_expenses_base_units=0,
                        taxes_base_units=0,
                        contractual_liabilities_base_units=0,
                        approved_reserves_base_units=0,
                    )

    def test_duplicate_distribution_ids_are_rejected(self):
        period = self.period()
        first = self.allocation("distribution-1", amount_base_units=5)
        duplicate = self.allocation("distribution-1", amount_base_units=7)

        with self.assertRaisesRegex(ValueError, "duplicate distribution ID"):
            simulate_period(period, [first, duplicate])

    def test_cross_project_or_cross_asset_allocations_are_rejected(self):
        period = self.period()
        wrong_project = self.allocation(project_id="project-b", amount_base_units=1)
        wrong_asset = self.allocation(asset_id="eip155:84532/0xother", amount_base_units=1)

        for allocation in (wrong_project, wrong_asset):
            with self.subTest(allocation=allocation):
                with self.assertRaisesRegex(ValueError, "does not match period scope"):
                    simulate_period(period, [allocation])


if __name__ == "__main__":
    unittest.main()
