import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "specs" / "chain-capabilities.schema.json"
EXAMPLES = ROOT / "specs" / "examples"


class ChainCapabilitySchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = json.loads(SCHEMA_PATH.read_text())
        Draft202012Validator.check_schema(cls.schema)
        cls.validator = Draft202012Validator(cls.schema)

    def test_near_hub_example_is_valid(self):
        config = json.loads((EXAMPLES / "near-hub.json").read_text())
        self.assertEqual([], list(self.validator.iter_errors(config)))

    def test_evm_destination_example_is_valid(self):
        config = json.loads((EXAMPLES / "base-sepolia.json").read_text())
        self.assertEqual([], list(self.validator.iter_errors(config)))

    def test_evm_chain_requires_eip155_namespace_and_numeric_chain_id(self):
        config = json.loads((EXAMPLES / "base-sepolia.json").read_text())
        config["namespace"] = "near"
        self.assertTrue(list(self.validator.iter_errors(config)))

    def test_unknown_capabilities_are_rejected(self):
        config = json.loads((EXAMPLES / "near-hub.json").read_text())
        config["capabilities"]["custody_everything"] = True
        self.assertTrue(list(self.validator.iter_errors(config)))


if __name__ == "__main__":
    unittest.main()
