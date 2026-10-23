import unittest

from protocol.near_signature_requester import (
    SignatureObservation,
    SignatureRequestAmbiguous,
    StatusAwareSignatureRequester,
)


REQUEST_ID = "request-1"
ACTION_HASH = "0x" + "a" * 64
DIGEST = "0x" + "b" * 64
SIGNATURE = {
    "big_r": {"affine_point": "0x02" + "c" * 64},
    "s": {"scalar": "0x" + "d" * 64},
    "recovery_id": 0,
}


class SignaturePorts:
    def __init__(self, initial, *, after_submit=None, wait_result=None, submit_error=None):
        self.state = initial
        self.after_submit = after_submit
        self.wait_result = wait_result
        self.submit_error = submit_error
        self.read_count = 0
        self.submit_count = 0
        self.wait_count = 0

    def read_signature_state(self, request_id):
        self.assert_request_id(request_id)
        self.read_count += 1
        return self.state

    def submit_signature_request(self, request_id, expected_action_hash):
        self.assert_request_id(request_id)
        if expected_action_hash != ACTION_HASH:
            raise AssertionError("unexpected action hash")
        self.submit_count += 1
        if self.after_submit is not None:
            self.state = self.after_submit
        if self.submit_error is not None:
            raise self.submit_error

    def wait_for_issued_signature(self, request_id):
        self.assert_request_id(request_id)
        self.wait_count += 1
        if self.wait_result is not None:
            self.state = self.wait_result
        return self.state

    @staticmethod
    def assert_request_id(request_id):
        if request_id != REQUEST_ID:
            raise AssertionError("unexpected request ID")


class StatusAwareSignatureRequesterTests(unittest.TestCase):
    def observation(
        self,
        signature_status,
        *,
        execution_status="Approved",
        action_hash=ACTION_HASH,
        signing_payload_hash=DIGEST,
        signature_response=None,
    ):
        return SignatureObservation(
            request_id=REQUEST_ID,
            execution_status=execution_status,
            signature_status=signature_status,
            action_hash=action_hash,
            signing_payload_hash=signing_payload_hash,
            signature_response=signature_response,
        )

    def requester(self, ports):
        return StatusAwareSignatureRequester(ports, ports, ports)

    def request(self, requester):
        return requester.request_signature(REQUEST_ID, ACTION_HASH, DIGEST)

    def test_issued_signature_is_reused_without_another_write(self):
        ports = SignaturePorts(
            self.observation("Issued", signature_response=SIGNATURE)
        )

        result = self.request(self.requester(ports))

        self.assertEqual(result, SIGNATURE)
        self.assertEqual(ports.submit_count, 0)
        self.assertEqual(ports.wait_count, 0)
        self.assertEqual(ports.read_count, 1)

    def test_pending_status_waits_without_submitting_again(self):
        ports = SignaturePorts(
            self.observation("Pending"),
            wait_result=self.observation("Issued", signature_response=SIGNATURE),
        )

        result = self.request(self.requester(ports))

        self.assertEqual(result, SIGNATURE)
        self.assertEqual(ports.submit_count, 0)
        self.assertEqual(ports.wait_count, 1)

    def test_not_requested_submits_once_then_reads_issued_result(self):
        ports = SignaturePorts(
            self.observation("NotRequested"),
            after_submit=self.observation("Issued", signature_response=SIGNATURE),
        )

        result = self.request(self.requester(ports))

        self.assertEqual(result, SIGNATURE)
        self.assertEqual(ports.submit_count, 1)
        self.assertEqual(ports.wait_count, 0)
        self.assertEqual(ports.read_count, 2)

    def test_failed_status_can_be_retried_once_and_reuses_issued_result(self):
        ports = SignaturePorts(
            self.observation("Failed"),
            after_submit=self.observation("Issued", signature_response=SIGNATURE),
        )

        result = self.request(self.requester(ports))

        self.assertEqual(result, SIGNATURE)
        self.assertEqual(ports.submit_count, 1)

    def test_ambiguous_submit_timeout_reads_pending_then_waits_without_duplicate_write(self):
        ports = SignaturePorts(
            self.observation("NotRequested"),
            after_submit=self.observation("Pending"),
            wait_result=self.observation("Issued", signature_response=SIGNATURE),
            submit_error=TimeoutError("simulated acknowledgement loss"),
        )

        result = self.request(self.requester(ports))

        self.assertEqual(result, SIGNATURE)
        self.assertEqual(ports.submit_count, 1)
        self.assertEqual(ports.wait_count, 1)
        self.assertEqual(ports.read_count, 2)

    def test_ambiguous_timeout_with_no_observed_state_change_stops_without_resubmitting(self):
        ports = SignaturePorts(
            self.observation("NotRequested"),
            submit_error=TimeoutError("simulated acknowledgement loss"),
        )

        with self.assertRaises(SignatureRequestAmbiguous):
            self.request(self.requester(ports))

        self.assertEqual(ports.submit_count, 1)
        self.assertEqual(ports.read_count, 2)

    def test_commitment_mismatch_or_nonapproved_request_fails_before_write(self):
        cases = (
            self.observation("NotRequested", action_hash="0x" + "e" * 64),
            self.observation("NotRequested", signing_payload_hash="0x" + "f" * 64),
            self.observation("NotRequested", execution_status="Expired"),
        )
        for state in cases:
            with self.subTest(state=state):
                ports = SignaturePorts(state)
                with self.assertRaises(ValueError):
                    self.request(self.requester(ports))
                self.assertEqual(ports.submit_count, 0)

    def test_issued_status_without_signature_payload_is_rejected(self):
        ports = SignaturePorts(self.observation("Issued"))

        with self.assertRaises(ValueError):
            self.request(self.requester(ports))
        self.assertEqual(ports.submit_count, 0)


if __name__ == "__main__":
    unittest.main()
