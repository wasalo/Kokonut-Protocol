"""Reconstruct and verify EIP-1559 signing commitments before any signer call."""

from __future__ import annotations

import hashlib
import json
import re
from collections.abc import Mapping
from typing import TypeAlias

from eth_hash.auto import keccak

_UINT256_MAX = (1 << 256) - 1
_HEX_32 = re.compile(r"^0x[0-9a-fA-F]{64}$")
_HEX_ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
_DECIMAL = re.compile(r"^(0|[1-9][0-9]*)$")
RLPItem: TypeAlias = bytes | list["RLPItem"]


def _require_decimal(
    request: Mapping[str, object], field: str, *, positive: bool = False
) -> int:
    value = request.get(field)
    if not isinstance(value, str) or not _DECIMAL.fullmatch(value):
        raise ValueError(f"{field} must be a canonical decimal string")
    number = int(value)
    if number > _UINT256_MAX or (positive and number == 0):
        raise ValueError(f"{field} is outside its allowed range")
    return number


def _minimal_big_endian(value: int) -> bytes:
    if value == 0:
        return b""
    return value.to_bytes((value.bit_length() + 7) // 8, "big")


def _rlp_encode_bytes(value: bytes) -> bytes:
    length = len(value)
    if length == 1 and value[0] < 0x80:
        return value
    if length < 56:
        return bytes([0x80 + length]) + value
    encoded_length = _minimal_big_endian(length)
    return bytes([0xB7 + len(encoded_length)]) + encoded_length + value


def _rlp_encode(value: RLPItem) -> bytes:
    if isinstance(value, bytes):
        return _rlp_encode_bytes(value)
    payload = b"".join(_rlp_encode(item) for item in value)
    length = len(payload)
    if length < 56:
        return bytes([0xC0 + length]) + payload
    encoded_length = _minimal_big_endian(length)
    return bytes([0xF7 + len(encoded_length)]) + encoded_length + payload


def _rlp_encode_list(items: list[RLPItem]) -> bytes:
    return _rlp_encode(items)


def _read_hex(value: object, field: str) -> bytes:
    if not isinstance(value, str) or not value.startswith("0x"):
        raise ValueError(f"{field} must be 0x-prefixed hex")
    digits = value[2:]
    if len(digits) % 2 or not all(
        character in "0123456789abcdefABCDEF" for character in digits
    ):
        raise ValueError(f"{field} must be even-length hexadecimal")
    return bytes.fromhex(digits)


def verify_eip1559_signing_payload(request: Mapping[str, object], calldata_hex: str) -> str:
    """Rebuild type-2 signing digest and verify both committed hashes.

    Protocol v1 supports EIP-1559 transactions with an empty access list. This
    function only verifies; it never obtains a key, signs, or broadcasts.
    """
    if request.get("protocol_version") != "1":
        raise ValueError("unsupported protocol version")
    if request.get("chain_namespace") != "eip155":
        raise ValueError("only eip155 requests are supported")

    chain_id = _require_decimal(request, "chain_id", positive=True)
    nonce = _require_decimal(request, "nonce")
    gas_limit = _require_decimal(request, "gas_limit", positive=True)
    value = _require_decimal(request, "value_base_units")

    gas_policy = request.get("gas_policy")
    if not isinstance(gas_policy, Mapping) or set(gas_policy) != {
        "max_fee",
        "max_priority_fee",
    }:
        raise ValueError("gas_policy must contain only max_fee and max_priority_fee")
    max_fee = _require_decimal(gas_policy, "max_fee")
    max_priority_fee = _require_decimal(gas_policy, "max_priority_fee")
    if max_priority_fee > max_fee:
        raise ValueError("max_priority_fee exceeds max_fee")

    target = request.get("target")
    if not isinstance(target, str) or not _HEX_ADDRESS.fullmatch(target):
        raise ValueError("target must be a 20-byte EVM address")
    destination = bytes.fromhex(target[2:])

    calldata = _read_hex(calldata_hex, "calldata")
    calldata_hash = request.get("calldata_hash")
    if not isinstance(calldata_hash, str) or not _HEX_32.fullmatch(calldata_hash):
        raise ValueError("calldata hash must be a 32-byte hex digest")
    actual_calldata_hash = "0x" + keccak(calldata).hex()
    if actual_calldata_hash.lower() != calldata_hash.lower():
        raise ValueError("calldata hash does not match supplied calldata")

    unsigned_fields = [
        _minimal_big_endian(chain_id),
        _minimal_big_endian(nonce),
        _minimal_big_endian(max_priority_fee),
        _minimal_big_endian(max_fee),
        _minimal_big_endian(gas_limit),
        destination,
        _minimal_big_endian(value),
        calldata,
        [],  # Protocol v1 fixes the EIP-2930 access list to empty.
    ]
    preimage = b"\x02" + _rlp_encode_list(unsigned_fields)
    actual_signing_payload_hash = "0x" + keccak(preimage).hex()

    committed_hash = request.get("signing_payload_hash")
    if not isinstance(committed_hash, str) or not _HEX_32.fullmatch(committed_hash):
        raise ValueError("signing payload hash must be a 32-byte hex digest")
    if actual_signing_payload_hash.lower() != committed_hash.lower():
        raise ValueError("signing payload hash does not match reconstructed transaction")
    return actual_signing_payload_hash


def prepare_approved_eip1559_signing_payload(
    record: Mapping[str, object],
    calldata_hex: str,
    *,
    active_policy: Mapping[str, object],
    near_block_timestamp_ns: int,
) -> str:
    """Return the digest only for a current, approved record with matching payload."""
    if record.get("status") != "Approved":
        raise ValueError("execution request is not approved on NEAR")
    expires_at_ns = record.get("expires_at_ns")
    if (
        not isinstance(expires_at_ns, int)
        or isinstance(expires_at_ns, bool)
        or expires_at_ns < 0
        or not isinstance(near_block_timestamp_ns, int)
        or isinstance(near_block_timestamp_ns, bool)
        or near_block_timestamp_ns < 0
    ):
        raise ValueError("NEAR expiry or block timestamp is invalid")
    if near_block_timestamp_ns >= expires_at_ns:
        raise ValueError("execution request is expired")
    if not isinstance(active_policy, Mapping):
        raise ValueError("active policy record is missing")

    request = record.get("request")
    if not isinstance(request, Mapping):
        raise ValueError("NEAR record is missing its request object")
    policy_chain = active_policy.get("chain")
    if not isinstance(policy_chain, Mapping):
        raise ValueError("active policy chain scope is invalid")
    if (
        active_policy.get("project_id") != request.get("project_id")
        or policy_chain.get("namespace") != request.get("chain_namespace")
        or policy_chain.get("chain_id") != request.get("chain_id")
    ):
        raise ValueError("active policy scope does not match the request")
    if active_policy.get("version") != request.get("policy_version"):
        raise ValueError("execution request policy version is no longer active")
    record_policy_hash = record.get("policy_hash")
    active_policy_hash = active_policy.get("policy_hash")
    if (
        not isinstance(record_policy_hash, str)
        or not _HEX_32.fullmatch(record_policy_hash)
        or not isinstance(active_policy_hash, str)
        or not _HEX_32.fullmatch(active_policy_hash)
        or record_policy_hash.lower() != active_policy_hash.lower()
    ):
        raise ValueError("active policy hash does not match the approved record")

    request_fields = (
        "protocol_version",
        "request_id",
        "project_id",
        "chain_namespace",
        "chain_id",
        "operation",
        "target",
        "value_base_units",
        "gas_limit",
        "calldata_hash",
        "signing_payload_hash",
        "nonce",
        "gas_policy",
        "expires_at",
        "policy_version",
        "idempotency_key",
    )
    if set(request) != set(request_fields):
        raise ValueError("approved request has an unexpected field set")
    gas_policy = request.get("gas_policy")
    if not isinstance(gas_policy, Mapping) or set(gas_policy) != {
        "max_fee",
        "max_priority_fee",
    }:
        raise ValueError("approved request has an invalid gas_policy")

    ordered_request = {field: request[field] for field in request_fields}
    ordered_request["gas_policy"] = {
        "max_fee": gas_policy["max_fee"],
        "max_priority_fee": gas_policy["max_priority_fee"],
    }
    encoded_request = json.dumps(
        ordered_request, separators=(",", ":"), ensure_ascii=False, allow_nan=False
    ).encode("utf-8")
    expected_action_hash = "0x" + hashlib.sha256(encoded_request).hexdigest()
    action_hash = record.get("action_hash")
    if not isinstance(action_hash, str) or not _HEX_32.fullmatch(action_hash):
        raise ValueError("NEAR record has an invalid action hash")
    if expected_action_hash.lower() != action_hash.lower():
        raise ValueError("request does not match the approved action hash")

    return verify_eip1559_signing_payload(request, calldata_hex)
