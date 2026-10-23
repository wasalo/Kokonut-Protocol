import json
from pathlib import Path
import unittest

from protocol.evm_adapter import FinalityResult, PreparedTransaction, hash_expected_effect
from protocol.relayer import ExecutionRelayer, ManualReviewRequired, RelayerSnapshot

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = json.loads((ROOT / "specs/examples/evm-signing-payload.json").read_text())
REQUEST = FIXTURE["request"]
EFFECT = FIXTURE["expected_effect"]
CALLDATA = FIXTURE["calldata_hex"]
ACTIVE_POLICY = FIXTURE["active_policy"]
RECORD = {
    "status": "Approved",
    "action_hash": FIXTURE["expected_action_hash"],
    "policy_hash": FIXTURE["policy_hash"],
    "expires_at_ns": FIXTURE["expires_at_ns"],
    "request": REQUEST,
}
SIGNER_ADDRESS = "0x" + "11" * 20
TRANSACTION_HASH = "0x" + "ab" * 32


class SnapshotSource:
    def __init__(self, snapshots):
        self.snapshots = list(snapshots)
        self.calls = []

    def read(self, request_id):
        self.calls.append(request_id)
        return self.snapshots.pop(0)


class SignatureSource:
    def __init__(self, events):
        self.events = events

    def request_signature(self, request_id, action_hash, digest):
        self.events.append(("sign", request_id, action_hash, digest))
        return {"signature": "fixture"}


class Adapter:
    def __init__(self, events, finality="finalized", effect_verified=True, lease=None):
        self.events = events
        self.finality = finality
        self.effect_verified = effect_verified
        self.lease = lease

    def get_nonce_lease(self, request_id):
        return self.lease

    def transaction_hash(self, raw_transaction):
        return TRANSACTION_HASH

    def reserve_approved_nonce(self, record, calldata_hex, *, active_policy, near_block_timestamp_ns, signer_address):
        self.events.append(("reserve", record["request"]["request_id"]))
        return int(record["request"]["nonce"])

    def prepare_approved_transaction(self, record, calldata_hex, *, active_policy, near_block_timestamp_ns,
                                     signature_response, signer_address, expected_effect):
        self.events.append(("prepare", record["request"]["request_id"]))
        return PreparedTransaction(
            record["request"]["request_id"], int(record["request"]["chain_id"]),
            int(record["request"]["nonce"]), signer_address, "0x02abcd", TRANSACTION_HASH,
            hash_expected_effect(expected_effect),
        )

    def broadcast(self, prepared, *, record, calldata_hex, active_policy, near_block_timestamp_ns,
                  signature_response, signer_address, expected_effect):
        self.events.append(("broadcast", prepared.request_id))
        return TRANSACTION_HASH

    def check_finality(self, transaction_hash):
        self.events.append(("finality", transaction_hash))
        return FinalityResult(self.finality, transaction_hash)

    def finalize_and_verify_effect(self, prepared, expected_effect):
        self.events.append(("verify_effect", prepared.request_id))
        return self.effect_verified


def snapshot(*, status="Approved", near_block_timestamp_ns=1):
    return RelayerSnapshot(
        record={**RECORD, "status": status},
        active_policy=ACTIVE_POLICY,
        near_block_timestamp_ns=near_block_timestamp_ns,
        calldata_hex=CALLDATA,
        signer_address=SIGNER_ADDRESS,
        expected_effect=EFFECT,
    )


class ExecutionRelayerTests(unittest.TestCase):
    def test_approved_request_runs_through_signing_broadcast_and_finality(self):
        events = []
        snapshots = SnapshotSource([snapshot(), snapshot()])
        adapter = Adapter(events)
        relayer = ExecutionRelayer(snapshots, SignatureSource(events), adapter)

        result = relayer.process(REQUEST["request_id"])

        self.assertEqual(result.state, "finalized")
        self.assertEqual(result.transaction_hash, TRANSACTION_HASH)
        self.assertTrue(result.effect_verified)
        self.assertEqual(
            [event[0] for event in events],
            ["reserve", "sign", "prepare", "broadcast", "finality", "verify_effect"],
        )
        self.assertEqual(snapshots.calls, [REQUEST["request_id"], REQUEST["request_id"]])
        self.assertEqual(events[1][3], REQUEST["signing_payload_hash"])

    def test_rejected_preflight_never_reserves_or_requests_a_signature(self):
        events = []
        snapshots = SnapshotSource([snapshot(status="Cancelled")])
        relayer = ExecutionRelayer(snapshots, SignatureSource(events), Adapter(events))

        with self.assertRaisesRegex(ValueError, "not approved"):
            relayer.process(REQUEST["request_id"])

        self.assertEqual(events, [])

    def test_changed_approval_after_signature_prevents_broadcast(self):
        events = []
        snapshots = SnapshotSource([snapshot(), snapshot(status="Cancelled")])
        relayer = ExecutionRelayer(snapshots, SignatureSource(events), Adapter(events))

        with self.assertRaisesRegex(ValueError, "not approved"):
            relayer.process(REQUEST["request_id"])

        self.assertNotIn("broadcast", [event[0] for event in events])
        self.assertNotIn("prepare", [event[0] for event in events])

    def test_manual_review_lease_raises_nonretryable_signal_before_signing(self):
        events = []
        lease = {"request_id": REQUEST["request_id"], "state": "manual_review"}
        snapshots = SnapshotSource([snapshot()])
        relayer = ExecutionRelayer(
            snapshots, SignatureSource(events), Adapter(events, lease=lease)
        )

        with self.assertRaises(ManualReviewRequired):
            relayer.process(REQUEST["request_id"])

        self.assertEqual(events, [])

    def test_nonfinalized_receipt_does_not_verify_effect(self):
        events = []
        snapshots = SnapshotSource([snapshot(), snapshot()])
        relayer = ExecutionRelayer(
            snapshots, SignatureSource(events), Adapter(events, finality="confirming")
        )

        result = relayer.process(REQUEST["request_id"])

        self.assertEqual(result.state, "confirming")
        self.assertIsNone(result.effect_verified)
        self.assertNotIn("verify_effect", [event[0] for event in events])


if __name__ == "__main__":
    unittest.main()
