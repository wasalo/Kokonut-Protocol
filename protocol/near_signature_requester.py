"""Status-aware, injected NEAR Chain Signatures request coordination."""

from __future__ import annotations

import re
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any, Protocol

_HASH_32 = re.compile(r"^0x[0-9a-fA-F]{64}$")
_SIGNATURE_STATES = {"NotRequested", "Pending", "Issued", "Failed"}


@dataclass(frozen=True)
class SignatureObservation:
    """Fresh NEAR view of one approved execution and its signature status."""

    request_id: str
    execution_status: str
    signature_status: str
    action_hash: str
    signing_payload_hash: str
    signature_response: Mapping[str, Any] | None = None


class SignatureStateReader(Protocol):
    def read_signature_state(self, request_id: str) -> SignatureObservation: ...


class SignatureRequestSubmitter(Protocol):
    def submit_signature_request(self, request_id: str, expected_action_hash: str) -> None: ...


class IssuedSignatureWaiter(Protocol):
    def wait_for_issued_signature(self, request_id: str) -> SignatureObservation: ...


class SignatureRequestError(RuntimeError):
    """Base class for status-aware signature request failures."""


class SignatureRequestAmbiguous(SignatureRequestError):
    """Submission outcome is unresolved; a later attempt must read status first."""


class SignatureRequestFailed(SignatureRequestError):
    """NEAR reports the external signature request as failed and retryable."""


class SignatureRequestPending(SignatureRequestError):
    """The bounded wait ended before NEAR reported an issued signature."""


class StatusAwareSignatureRequester:
    """Request/reuse one signature without blindly replaying a NEAR write.

    Transport, transaction submission, and bounded polling remain injected. A
    submitter must report completion only after the request transaction reaches
    the finality required by its caller. A waiter must be bounded and return a
    fresh observation. Neither port may write a second request while status is
    Pending.
    """

    def __init__(
        self,
        reader: SignatureStateReader,
        submitter: SignatureRequestSubmitter,
        waiter: IssuedSignatureWaiter,
    ) -> None:
        self._reader = reader
        self._submitter = submitter
        self._waiter = waiter

    def request_signature(
        self, request_id: str, action_hash: str, digest: str
    ) -> Mapping[str, Any]:
        self._validate_hash(action_hash, "action_hash")
        self._validate_hash(digest, "digest")
        observation = self._read_validated(request_id, action_hash, digest)

        if observation.signature_status == "Issued":
            return self._issued_response(observation)
        if observation.signature_status == "Pending":
            return self._wait_for_issued(request_id, action_hash, digest)

        # The contract permits a write only from NotRequested or retryable Failed.
        try:
            self._submitter.submit_signature_request(request_id, action_hash)
        except (TimeoutError, ConnectionError) as exc:
            current = self._read_validated(request_id, action_hash, digest)
            if current.signature_status == "Issued":
                return self._issued_response(current)
            if current.signature_status == "Pending":
                return self._wait_for_issued(request_id, action_hash, digest)
            raise SignatureRequestAmbiguous(
                "signature request outcome is unresolved; retry only after a fresh status read"
            ) from exc

        current = self._read_validated(request_id, action_hash, digest)
        if current.signature_status == "Issued":
            return self._issued_response(current)
        if current.signature_status == "Pending":
            return self._wait_for_issued(request_id, action_hash, digest)
        if current.signature_status == "Failed":
            raise SignatureRequestFailed("NEAR reports a retryable signature request failure")
        raise SignatureRequestAmbiguous(
            "signature request was submitted but no state transition is observable"
        )

    def _wait_for_issued(
        self, request_id: str, action_hash: str, digest: str
    ) -> Mapping[str, Any]:
        observation = self._waiter.wait_for_issued_signature(request_id)
        self._validate_observation(observation, request_id, action_hash, digest)
        if observation.signature_status == "Issued":
            return self._issued_response(observation)
        if observation.signature_status == "Failed":
            raise SignatureRequestFailed("NEAR reports a retryable signature request failure")
        if observation.signature_status == "NotRequested":
            raise SignatureRequestAmbiguous(
                "signature status returned to NotRequested; re-read before any future write"
            )
        raise SignatureRequestPending("signature remains Pending after the bounded wait")

    def _read_validated(
        self, request_id: str, action_hash: str, digest: str
    ) -> SignatureObservation:
        observation = self._reader.read_signature_state(request_id)
        self._validate_observation(observation, request_id, action_hash, digest)
        return observation

    @classmethod
    def _validate_observation(
        cls,
        observation: SignatureObservation,
        request_id: str,
        action_hash: str,
        digest: str,
    ) -> None:
        if not isinstance(observation, SignatureObservation):
            raise ValueError("signature state reader returned a malformed observation")
        if observation.request_id != request_id:
            raise ValueError("signature state belongs to a different request ID")
        if observation.execution_status != "Approved":
            raise ValueError("execution request is no longer approved")
        if observation.signature_status not in _SIGNATURE_STATES:
            raise ValueError("signature state reader returned an unknown status")
        cls._validate_hash(observation.action_hash, "observed action_hash")
        cls._validate_hash(observation.signing_payload_hash, "observed signing_payload_hash")
        if observation.action_hash.lower() != action_hash.lower():
            raise ValueError("observed action hash differs from the requested commitment")
        if observation.signing_payload_hash.lower() != digest.lower():
            raise ValueError("observed signing digest differs from the requested commitment")

    @staticmethod
    def _validate_hash(value: str, field: str) -> None:
        if not isinstance(value, str) or not _HASH_32.fullmatch(value):
            raise ValueError(f"{field} must be a 32-byte 0x-prefixed hash")

    @staticmethod
    def _issued_response(observation: SignatureObservation) -> Mapping[str, Any]:
        response = observation.signature_response
        if not isinstance(response, Mapping):
            raise ValueError("Issued signature state has no structured signature response")
        return response
