import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator, FormatChecker
from referencing import Registry, Resource

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "specs" / "agent-execution-proposal.schema.json"
EXECUTION_REQUEST_PATH = ROOT / "specs" / "examples" / "execution-request.json"


class AgentExecutionProposalSchemaTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schema = json.loads(SCHEMA_PATH.read_text())
        execution_schema = json.loads(
            (ROOT / "specs" / "execution-request.schema.json").read_text()
        )
        Draft202012Validator.check_schema(cls.schema)
        registry = Registry().with_resource(
            execution_schema["$id"], Resource.from_contents(execution_schema)
        )
        cls.validator = Draft202012Validator(
            cls.schema, registry=registry, format_checker=FormatChecker()
        )
        cls.valid_proposal = {
            "proposal_version": "1",
            "proposal_id": "018f4f7e-10bd-7d98-9b23-9ec89a1d9c3d",
            "proposed_by": "agent:syntropic-agent",
            "created_at": "2026-10-23T12:00:00Z",
            "execution_request": json.loads(EXECUTION_REQUEST_PATH.read_text()),
            "rationale": "Propose this action for independent human review.",
        }

    def test_sample_proposal_is_valid(self):
        sample = json.loads(
            (ROOT / "specs" / "examples" / "agent-execution-proposal.json").read_text()
        )
        self.assertEqual([], list(self.validator.iter_errors(sample)))

    def test_proposal_without_authority_fields_is_valid(self):
        self.assertEqual([], list(self.validator.iter_errors(self.valid_proposal)))

    def test_agent_cannot_embed_approval_or_execution_authority(self):
        for forbidden in ("approved", "approval", "signature", "execution_status"):
            with self.subTest(forbidden=forbidden):
                proposal = {**self.valid_proposal, forbidden: True}
                self.assertTrue(list(self.validator.iter_errors(proposal)))

    def test_embedded_request_must_match_execution_request_schema(self):
        proposal = {**self.valid_proposal, "execution_request": {"request_id": "bad"}}
        self.assertTrue(list(self.validator.iter_errors(proposal)))

    def test_unknown_proposal_fields_are_rejected(self):
        proposal = {**self.valid_proposal, "agent_approved": True}
        self.assertTrue(list(self.validator.iter_errors(proposal)))


if __name__ == "__main__":
    unittest.main()
