import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "specs" / "execution-request.schema.json"

VALID_REQUEST = {
    "protocol_version": "1",
    "request_id": "018f4f7e-10bd-7d98-9b23-9ec89a1d9c3a",
    "project_id": "018f4f7e-10bd-7d98-9b23-9ec89a1d9c3b",
    "chain_namespace": "eip155",
    "chain_id": "84532",
    "operation": "deploy_project_component",
    "target": "0x1234567890abcdef1234567890abcdef12345678",
    "value_base_units": "0",
    "calldata_hash": "0x" + "a" * 64,
    "nonce": "0",
    "gas_policy": {"max_fee": "1000000000", "max_priority_fee": "1000000"},
    "expires_at": "2026-10-24T12:00:00Z",
    "policy_version": "protocol-policy-v1",
    "idempotency_key": "deploy-project-001",
}


class ExecutionRequestSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = json.loads(SCHEMA_PATH.read_text())
        Draft202012Validator.check_schema(cls.schema)
        cls.validator = Draft202012Validator(cls.schema, format_checker=FormatChecker())

    def test_canonical_request_is_valid(self):
        self.assertEqual([], list(self.validator.iter_errors(VALID_REQUEST)))

    def test_namespace_must_be_lowercase_identifier(self):
        request = dict(VALID_REQUEST, chain_namespace="EIP155")
        self.assertTrue(list(self.validator.iter_errors(request)))

    def test_eip155_chain_id_must_be_positive_decimal(self):
        request = dict(VALID_REQUEST, chain_id="base-sepolia")
        self.assertTrue(list(self.validator.iter_errors(request)))

    def test_eip155_target_must_be_an_evm_address(self):
        request = dict(VALID_REQUEST, target="not-an-address")
        self.assertTrue(list(self.validator.iter_errors(request)))

    def test_sample_execution_request_is_valid(self):
        sample = json.loads((ROOT / "specs" / "examples" / "execution-request.json").read_text())
        self.assertEqual([], list(self.validator.iter_errors(sample)))

    def test_financial_quantities_must_be_decimal_strings(self):
        request = dict(VALID_REQUEST, value_base_units=0)
        self.assertTrue(list(self.validator.iter_errors(request)))

    def test_calldata_hash_must_be_32_byte_hex(self):
        request = dict(VALID_REQUEST, calldata_hash="0x1234")
        self.assertTrue(list(self.validator.iter_errors(request)))

    def test_unrecognized_fields_are_rejected(self):
        request = dict(VALID_REQUEST, agent_approved=True)
        self.assertTrue(list(self.validator.iter_errors(request)))


if __name__ == "__main__":
    unittest.main()
