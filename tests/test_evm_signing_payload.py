import json
from pathlib import Path
import unittest
from unittest.mock import patch

from protocol.evm_signing_payload import (
    prepare_approved_eip1559_signing_payload,
    verify_eip1559_signing_payload,
)

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = json.loads((ROOT / "specs" / "examples" / "evm-signing-payload.json").read_text())
CALLDATA = FIXTURE["calldata_hex"]
REQUEST = FIXTURE["request"]
ACTION_HASH = FIXTURE["expected_action_hash"]
SIGNING_HASH = FIXTURE["expected_signing_payload_hash"]
POLICY_HASH = FIXTURE["policy_hash"]
ACTIVE_POLICY = FIXTURE["active_policy"]


class Eip1559SigningPayloadTests(unittest.TestCase):
    def test_verifies_exact_calldata_and_signing_payload_commitments(self):
        request = dict(REQUEST)

        digest = verify_eip1559_signing_payload(request, CALLDATA)

        self.assertEqual(SIGNING_HASH, digest)

    def test_rejects_signing_digest_outside_secp256k1_scalar_order(self):
        calldata_digest = bytes.fromhex(REQUEST["calldata_hash"][2:])
        invalid_digest = bytes.fromhex(
            "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141"
        )
        request = dict(REQUEST, signing_payload_hash="0x" + invalid_digest.hex())
        with patch(
            "protocol.evm_signing_payload.keccak",
            side_effect=[calldata_digest, invalid_digest],
        ):
            with self.assertRaisesRegex(ValueError, "secp256k1 scalar"):
                verify_eip1559_signing_payload(request, CALLDATA)

    def test_rejects_calldata_that_does_not_match_its_commitment(self):
        with self.assertRaisesRegex(ValueError, "calldata hash"):
            verify_eip1559_signing_payload(REQUEST, "0x1235")

    def test_rejects_signing_payload_that_does_not_match_its_commitment(self):
        request = dict(REQUEST, signing_payload_hash="0x" + "11" * 32)

        with self.assertRaisesRegex(ValueError, "signing payload hash"):
            verify_eip1559_signing_payload(request, CALLDATA)

    def test_rejects_unsupported_chain_namespace(self):
        request = dict(REQUEST, chain_namespace="cosmos")

        with self.assertRaisesRegex(ValueError, "eip155"):
            verify_eip1559_signing_payload(request, CALLDATA)

    def test_rejects_zero_gas_limit(self):
        request = dict(REQUEST, gas_limit="0")

        with self.assertRaisesRegex(ValueError, "gas_limit"):
            verify_eip1559_signing_payload(request, CALLDATA)

    def test_rejects_priority_fee_above_max_fee(self):
        request = dict(
            REQUEST,
            gas_policy={"max_fee": "1", "max_priority_fee": "2"},
        )

        with self.assertRaisesRegex(ValueError, "max_priority_fee"):
            verify_eip1559_signing_payload(request, CALLDATA)

    def test_signing_gate_requires_approved_near_record(self):
        record = {
            "status": "AwaitingApproval",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }

        with self.assertRaisesRegex(ValueError, "not approved"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
            )

    def test_approved_near_record_returns_only_verified_signing_digest(self):
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }

        digest = prepare_approved_eip1559_signing_payload(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
        )

        self.assertEqual(REQUEST["signing_payload_hash"], digest)

    def test_signing_gate_rejects_request_that_differs_from_approved_action_hash(self):
        modified_request = dict(REQUEST, gas_limit="120001")
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": modified_request,
        }

        with self.assertRaisesRegex(ValueError, "action hash"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
            )

    def test_signing_gate_canonicalizes_nested_gas_policy_field_order(self):
        reordered_request = dict(REQUEST)
        reordered_request["gas_policy"] = dict(reversed(list(REQUEST["gas_policy"].items())))
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": reordered_request,
        }

        digest = prepare_approved_eip1559_signing_payload(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
        )

        self.assertEqual(SIGNING_HASH, digest)

    def test_signing_gate_rejects_expired_record(self):
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }

        with self.assertRaisesRegex(ValueError, "expired"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=FIXTURE["expires_at_ns"],
            )

    def test_signing_gate_rejects_missing_signer_key_version(self):
        request = dict(REQUEST)
        request.pop("signer_key_version")
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": request,
        }

        with self.assertRaisesRegex(ValueError, "signer key version"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
            )

    def test_signing_gate_rejects_negative_signer_key_version(self):
        request = dict(REQUEST, signer_key_version=-1)
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": request,
        }

        with self.assertRaisesRegex(ValueError, "signer key version"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
            )

    def test_signing_gate_rejects_missing_expected_effect_hash(self):
        request = dict(REQUEST)
        request.pop("expected_effect_hash")
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": request,
        }

        with self.assertRaisesRegex(ValueError, "expected effect hash"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
            )

    def test_signing_gate_rejects_superseded_policy_version(self):
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }
        stale_policy = dict(ACTIVE_POLICY, version="policy-v2")

        with self.assertRaisesRegex(ValueError, "policy version"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=stale_policy,
                near_block_timestamp_ns=1,
            )

    def test_signing_gate_rejects_active_policy_for_another_project(self):
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }
        wrong_project_policy = dict(ACTIVE_POLICY, project_id="another-project")

        with self.assertRaisesRegex(ValueError, "policy scope"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=wrong_project_policy,
                near_block_timestamp_ns=1,
            )

    def test_signing_gate_rejects_policy_hash_that_differs_from_proposal(self):
        record = {
            "status": "Approved",
            "action_hash": ACTION_HASH,
            "policy_hash": POLICY_HASH,
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }
        different_policy = dict(ACTIVE_POLICY, policy_hash="0x" + "dd" * 32)

        with self.assertRaisesRegex(ValueError, "policy hash"):
            prepare_approved_eip1559_signing_payload(
                record,
                CALLDATA,
                active_policy=different_policy,
                near_block_timestamp_ns=1,
            )


if __name__ == "__main__":
    unittest.main()
