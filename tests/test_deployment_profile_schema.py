import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "specs" / "deployment-profile.schema.json"
VALID_PROFILE = {
    "protocol_version": "1",
    "project_id": "018f4f7e-10bd-7d98-9b23-9ec89a1d9c3b",
    "version": "1.0.0",
    "config_hash": "0x" + "b" * 64,
    "status": "proposed",
    "config": {
        "approved_chains": [
            {"namespace": "near", "chain_id": "testnet"},
            {"namespace": "eip155", "chain_id": "84532"},
        ],
        "signer_derivation_policy": {
            "path_template": "kokonut/v1/project/{project_id}/{chain_namespace}/{chain_id}/{purpose}",
            "allowed_purposes": ["deployer", "operator", "settlement"],
        },
    },
}


class DeploymentProfileSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = json.loads(SCHEMA_PATH.read_text())
        Draft202012Validator.check_schema(cls.schema)
        cls.validator = Draft202012Validator(cls.schema, format_checker=FormatChecker())

    def test_valid_profile_is_accepted(self):
        self.assertEqual([], list(self.validator.iter_errors(VALID_PROFILE)))

    def test_signer_path_must_separate_project_chain_and_purpose(self):
        profile = json.loads(json.dumps(VALID_PROFILE))
        profile["config"]["signer_derivation_policy"]["path_template"] = "kokonut/shared"
        self.assertTrue(list(self.validator.iter_errors(profile)))

    def test_duplicate_purposes_are_rejected(self):
        profile = json.loads(json.dumps(VALID_PROFILE))
        profile["config"]["signer_derivation_policy"]["allowed_purposes"] = ["deployer", "deployer"]
        self.assertTrue(list(self.validator.iter_errors(profile)))

    def test_unknown_profile_fields_are_rejected(self):
        profile = dict(VALID_PROFILE, farm_financials={})
        self.assertTrue(list(self.validator.iter_errors(profile)))

    def test_sample_profile_is_valid(self):
        sample = json.loads((ROOT / "specs" / "examples" / "deployment-profile.json").read_text())
        self.assertEqual([], list(self.validator.iter_errors(sample)))


if __name__ == "__main__":
    unittest.main()
