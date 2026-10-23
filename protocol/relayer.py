"""Offline-first orchestration for an approved EVM execution.

The worker owns no queue or execution ledger. Call it from the existing
Durable event system; EVM nonce leases and signed bytes remain in NonceManager.
"""

from __future__ import annotations

from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any, Protocol

from protocol.evm_adapter import hash_expected_effect
from protocol.evm_signing_payload import prepare_approved_eip1559_signing_payload


@dataclass(frozen=True)
class RelayerSnapshot:
    """Fresh read-only state needed to validate and relay one request."""

    record: Mapping[str, Any]
    active_policy: Mapping[str, Any]
    near_block_timestamp_ns: int
    calldata_hex: str
    signer_address: str
    expected_effect: Mapping[str, Any]


@dataclass(frozen=True)
class RelayerResult:
    request_id: str
    state: str
    transaction_hash: str
    nonce: int
    effect_verified: bool | None


class SnapshotReader(Protocol):
    def read(self, request_id: str) -> RelayerSnapshot:
        """Return a fresh NEAR record, policy, and committed EVM payload."""
        ...


class SignatureRequester(Protocol):
    def request_signature(
        self, request_id: str, action_hash: str, digest: str
    ) -> Mapping[str, Any]:
        """Return the signature for this request/digest without blind replay.

        An implementation must read the NEAR signature status first: return an
        already-issued response, wait while Pending, and only submit the
        contract call when its current state permits it. The digest is supplied
        as an expected value for request/response correlation; the contract
        signs the digest committed in the approved request.
        """
        ...


class EvmAdapterPort(Protocol):
    def reserve_approved_nonce(self, record: Mapping[str, Any], calldata_hex: str, *,
                               active_policy: Mapping[str, Any], near_block_timestamp_ns: int,
                               signer_address: str) -> int: ...

    def prepare_approved_transaction(self, record: Mapping[str, Any], calldata_hex: str, *,
                                     active_policy: Mapping[str, Any], near_block_timestamp_ns: int,
                                     signature_response: Mapping[str, Any], signer_address: str,
                                     expected_effect: Mapping[str, Any]) -> Any: ...

    def broadcast(self, prepared: Any, *, record: Mapping[str, Any], calldata_hex: str,
                  active_policy: Mapping[str, Any], near_block_timestamp_ns: int,
                  signature_response: Mapping[str, Any], signer_address: str,
                  expected_effect: Mapping[str, Any]) -> str: ...

    def check_finality(self, transaction_hash: str) -> Any: ...

    def finalize_and_verify_effect(self, prepared: Any,
                                   expected_effect: Mapping[str, Any]) -> bool: ...


class ExecutionRelayer:
    """Run one approved request through signing, broadcast, and finality checks.

    This method is a single resumable worker attempt, not a polling loop. The
    caller's durable event system is responsible for retry scheduling and
    dead-letter handling; the adapter supplies safe signed-transaction reuse.
    """

    def __init__(self, snapshots: SnapshotReader, signatures: SignatureRequester,
                 adapter: EvmAdapterPort) -> None:
        self._snapshots = snapshots
        self._signatures = signatures
        self._adapter = adapter

    def process(self, request_id: str) -> RelayerResult:
        snapshot = self._snapshots.read(request_id)
        digest = self._validate_snapshot(snapshot, request_id)
        request = snapshot.record["request"]
        action_hash = snapshot.record["action_hash"]
        committed_nonce = int(request["nonce"])
        reserved_nonce = self._adapter.reserve_approved_nonce(
            snapshot.record,
            snapshot.calldata_hex,
            active_policy=snapshot.active_policy,
            near_block_timestamp_ns=snapshot.near_block_timestamp_ns,
            signer_address=snapshot.signer_address,
        )
        if reserved_nonce != committed_nonce:
            raise ValueError("nonce manager returned a nonce that differs from the approved request")

        signature = self._signatures.request_signature(request_id, action_hash, digest)
        if not isinstance(signature, Mapping):
            raise ValueError("signature requester returned a malformed response")

        fresh = self._snapshots.read(request_id)
        fresh_digest = self._validate_snapshot(fresh, request_id)
        if fresh_digest.lower() != digest.lower():
            raise ValueError("approved signing payload changed after signature request")

        prepared = self._adapter.prepare_approved_transaction(
            fresh.record,
            fresh.calldata_hex,
            active_policy=fresh.active_policy,
            near_block_timestamp_ns=fresh.near_block_timestamp_ns,
            signature_response=signature,
            signer_address=fresh.signer_address,
            expected_effect=fresh.expected_effect,
        )
        transaction_hash = self._adapter.broadcast(
            prepared,
            record=fresh.record,
            calldata_hex=fresh.calldata_hex,
            active_policy=fresh.active_policy,
            near_block_timestamp_ns=fresh.near_block_timestamp_ns,
            signature_response=signature,
            signer_address=fresh.signer_address,
            expected_effect=fresh.expected_effect,
        )
        if transaction_hash.lower() != prepared.transaction_hash.lower():
            raise ValueError("broadcast transaction hash differs from the prepared transaction")

        finality = self._adapter.check_finality(transaction_hash)
        effect_verified: bool | None = None
        state = finality.state
        if state == "finalized":
            effect_verified = self._adapter.finalize_and_verify_effect(
                prepared, fresh.expected_effect
            )
            if not effect_verified:
                state = "manual_review"

        return RelayerResult(
            request_id=request_id,
            state=state,
            transaction_hash=transaction_hash,
            nonce=committed_nonce,
            effect_verified=effect_verified,
        )

    @staticmethod
    def _validate_snapshot(snapshot: RelayerSnapshot, request_id: str) -> str:
        record = snapshot.record
        request = record.get("request")
        if not isinstance(request, Mapping) or request.get("request_id") != request_id:
            raise ValueError("NEAR snapshot does not match the requested execution ID")
        digest = prepare_approved_eip1559_signing_payload(
            record,
            snapshot.calldata_hex,
            active_policy=snapshot.active_policy,
            near_block_timestamp_ns=snapshot.near_block_timestamp_ns,
        )
        if hash_expected_effect(snapshot.expected_effect).lower() != str(
            request.get("expected_effect_hash", "")
        ).lower():
            raise ValueError("expected effect does not match the approved request commitment")
        return digest
