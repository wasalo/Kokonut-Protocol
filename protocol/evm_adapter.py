"""Fail-closed testnet EIP-1559 adapter with durable nonce leases."""

from __future__ import annotations

import errno
import hashlib
import json
import os
import re
import sqlite3
import stat
import threading
from collections.abc import Mapping
from dataclasses import dataclass
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit
from urllib.request import Request, urlopen

from eth_hash.auto import keccak
from eth_keys import keys

from protocol.evm_signing_payload import (
    _minimal_big_endian,
    _rlp_encode,
    prepare_approved_eip1559_signing_payload,
    verify_eip1559_signing_payload,
)

_UINT256_MAX = (1 << 256) - 1
_SUPPORTED_TESTNET_CHAIN_IDS = frozenset({84532, 10200})
_SECP256K1_ORDER = int("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", 16)
_SECP256K1_HALF_ORDER = _SECP256K1_ORDER // 2
_FIELD_MODULUS = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
_HEX_DATA = re.compile(r"^0x(?:[0-9a-fA-F]{2})*$")
_HEX32 = re.compile(r"^0x[0-9a-fA-F]{64}$")
_ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
_DECIMAL = re.compile(r"^(0|[1-9][0-9]*)$")


def _validate_rpc_endpoint(endpoint: str) -> None:
    try:
        parsed = urlsplit(endpoint)
        host = parsed.hostname
        _ = parsed.port
    except (TypeError, ValueError):
        raise ValueError("JSON-RPC endpoint must be a valid HTTP(S) URL") from None
    if not host or parsed.username or parsed.password or parsed.fragment:
        raise ValueError("JSON-RPC endpoint must not include userinfo or a fragment")
    if parsed.scheme == "https":
        return
    if parsed.scheme == "http" and host.lower() in {"localhost", "127.0.0.1", "::1"}:
        return
    raise ValueError("JSON-RPC endpoint must use HTTPS (HTTP is allowed only on loopback)")


@dataclass(frozen=True)
class EvmNetworkConfig:
    name: str
    chain_id: int
    rpc_url_env: str
    confirmations_required: int = 12
    testnet: bool = True

    def __post_init__(self) -> None:
        if not self.name or self.chain_id not in _SUPPORTED_TESTNET_CHAIN_IDS or not self.testnet:
            raise ValueError("only the allowlisted testnet chain IDs are supported")
        if not re.fullmatch(r"[A-Z][A-Z0-9_]*", self.rpc_url_env):
            raise ValueError("RPC URL environment variable must be an uppercase identifier")
        if self.confirmations_required < 1:
            raise ValueError("at least one fallback confirmation is required")

    def rpc_url(self, environ: Mapping[str, str] | None = None) -> str:
        source = os.environ if environ is None else environ
        value = source.get(self.rpc_url_env, "")
        if not value:
            raise ValueError(f"configure {self.rpc_url_env} with an HTTP(S) JSON-RPC endpoint")
        _validate_rpc_endpoint(value)
        return value


TESTNETS = {
    "base-sepolia": EvmNetworkConfig("Base Sepolia", 84532, "BASE_SEPOLIA_RPC_URL", 12),
    "gnosis-chiado": EvmNetworkConfig("Gnosis Chiado", 10200, "GNOSIS_CHIADO_RPC_URL", 32),
}


class JsonRpcError(RuntimeError):
    def __init__(self, method: str, message: str, code: int | None = None):
        super().__init__(f"{method} failed: {message}")
        self.method = method
        self.code = code


class JsonRpcClient:
    """Small JSON-RPC client; endpoint values are never included in errors."""

    def __init__(self, endpoint: str, timeout_seconds: float = 15.0):
        _validate_rpc_endpoint(endpoint)
        if timeout_seconds <= 0:
            raise ValueError("timeout must be positive")
        self._endpoint = endpoint
        self._timeout = timeout_seconds
        self._request_id = 0
        self._lock = threading.Lock()

    def call(self, method: str, params: list[Any]) -> Any:
        with self._lock:
            self._request_id += 1
            request_id = self._request_id
        body = json.dumps(
            {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params},
            separators=(",", ":"),
        ).encode()
        req = Request(self._endpoint, data=body, headers={"Content-Type": "application/json"})
        try:
            with urlopen(req, timeout=self._timeout) as response:
                payload = json.loads(response.read(2_000_001))
        except (HTTPError, URLError, TimeoutError, OSError, json.JSONDecodeError) as exc:
            raise JsonRpcError(method, type(exc).__name__) from None
        if not isinstance(payload, dict) or payload.get("jsonrpc") != "2.0" or payload.get("id") != request_id:
            raise JsonRpcError(method, "malformed JSON-RPC response")
        if "error" in payload:
            error = payload["error"]
            if not isinstance(error, dict):
                raise JsonRpcError(method, "malformed error response")
            raise JsonRpcError(method, str(error.get("message", "unknown error")), error.get("code"))
        if "result" not in payload:
            raise JsonRpcError(method, "response omitted result")
        return payload["result"]


def _quantity(value: object, field: str) -> int:
    if isinstance(value, bool):
        raise ValueError(f"{field} is not a valid integer quantity")
    if isinstance(value, int):
        number = value
    elif isinstance(value, str) and value.startswith("0x"):
        try:
            number = int(value[2:] or "0", 16)
        except ValueError as exc:
            raise ValueError(f"{field} is not a valid hexadecimal quantity") from exc
    elif isinstance(value, str) and _DECIMAL.fullmatch(value):
        number = int(value)
    else:
        raise ValueError(f"{field} is not a valid integer quantity")
    if number < 0 or number > _UINT256_MAX:
        raise ValueError(f"{field} is outside uint256")
    return number


def _decimal(request: Mapping[str, Any], field: str, *, positive: bool = False) -> int:
    value = request.get(field)
    if not isinstance(value, str) or not _DECIMAL.fullmatch(value):
        raise ValueError(f"{field} must be a canonical decimal string")
    number = int(value)
    if number > _UINT256_MAX or (positive and number == 0):
        raise ValueError(f"{field} is outside its allowed range")
    return number


def _hex_bytes(value: object, field: str) -> bytes:
    if not isinstance(value, str) or not _HEX_DATA.fullmatch(value):
        raise ValueError(f"{field} must be 0x-prefixed even-length hex")
    return bytes.fromhex(value[2:])


def _rlp_integer(value: int) -> bytes:
    if value < 0:
        raise ValueError("RLP integer cannot be negative")
    return _minimal_big_endian(value)


def _normalize_effect(effect: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(effect, Mapping):
        raise ValueError("expected effect must be an object")
    kind = effect.get("kind")
    if kind == "event":
        if set(effect) - {"kind", "address", "topics", "data"} or not {"kind", "address", "topics"} <= set(effect):
            raise ValueError("event effect has invalid fields")
        address = effect["address"]
        topics = effect["topics"]
        if not isinstance(address, str) or not _ADDRESS.fullmatch(address):
            raise ValueError("event effect address is invalid")
        if not isinstance(topics, list) or not topics or len(topics) > 4 or any(
            not isinstance(topic, str) or not _HEX32.fullmatch(topic) for topic in topics
        ):
            raise ValueError("event effect topics must contain 1 to 4 bytes32 values")
        normalized = {"kind": kind, "address": address.lower(), "topics": [t.lower() for t in topics]}
        if "data" in effect:
            data = effect["data"]
            _hex_bytes(data, "event effect data")
            normalized["data"] = data.lower()
        return normalized
    if kind == "storage":
        if set(effect) != {"kind", "address", "slot", "value"}:
            raise ValueError("storage effect requires exactly kind, address, slot, and value")
        address = effect["address"]
        if not isinstance(address, str) or not _ADDRESS.fullmatch(address):
            raise ValueError("storage effect address is invalid")
        if any(not isinstance(effect[key], str) or not _HEX32.fullmatch(effect[key]) for key in ("slot", "value")):
            raise ValueError("storage effect slot and value must be bytes32")
        return {"kind": kind, "address": address.lower(), "slot": effect["slot"].lower(), "value": effect["value"].lower()}
    if kind == "code":
        if set(effect) != {"kind", "address", "code_hash"}:
            raise ValueError("code effect requires exactly kind, address, and code_hash")
        address = effect["address"]
        if not isinstance(address, str) or not _ADDRESS.fullmatch(address):
            raise ValueError("code effect address is invalid")
        if not isinstance(effect["code_hash"], str) or not _HEX32.fullmatch(effect["code_hash"]):
            raise ValueError("code effect hash must be bytes32")
        return {"kind": kind, "address": address.lower(), "code_hash": effect["code_hash"].lower()}
    raise ValueError("expected effect kind must be event, storage, or code")


def hash_expected_effect(effect: Mapping[str, Any]) -> str:
    normalized = _normalize_effect(effect)
    canonical = json.dumps(normalized, sort_keys=True, separators=(",", ":")).encode()
    return "0x" + hashlib.sha256(canonical).hexdigest()


def verify_expected_effect(
    effect: Mapping[str, Any],
    receipt: Mapping[str, Any],
    *,
    rpc: Any = None,
) -> bool:
    """Verify an approved event, storage value, or deployed-code hash."""
    normalized = _normalize_effect(effect)
    if _quantity(receipt.get("status"), "receipt status") != 1:
        return False
    kind = normalized["kind"]
    if kind == "event":
        logs = receipt.get("logs")
        if not isinstance(logs, list):
            return False
        for log in logs:
            if not isinstance(log, Mapping) or str(log.get("address", "")).lower() != normalized["address"]:
                continue
            topics = log.get("topics")
            if not isinstance(topics, list) or len(topics) < len(normalized["topics"]):
                continue
            if [str(topic).lower() for topic in topics[: len(normalized["topics"])]] != normalized["topics"]:
                continue
            if "data" in normalized and str(log.get("data", "")).lower() != normalized["data"]:
                continue
            return True
        return False
    if rpc is None:
        raise ValueError(f"{kind} effect verification requires JSON-RPC")
    state_block = hex(_quantity(receipt.get("blockNumber"), "receipt block number"))
    if kind == "storage":
        actual = rpc.call("eth_getStorageAt", [normalized["address"], normalized["slot"], state_block])
        if not isinstance(actual, str) or not re.fullmatch(r"0x[0-9a-fA-F]{64}", actual):
            return False
        return actual.lower() == normalized["value"]
    code = rpc.call("eth_getCode", [normalized["address"], state_block])
    code_bytes = _hex_bytes(code, "eth_getCode result")
    return "0x" + keccak(code_bytes).hex() == normalized["code_hash"]


@dataclass(frozen=True)
class PreparedTransaction:
    request_id: str
    chain_id: int
    nonce: int
    sender: str
    raw_transaction: str
    transaction_hash: str
    expected_effect_hash: str


@dataclass(frozen=True)
class FinalityResult:
    state: str
    transaction_hash: str
    receipt: Mapping[str, Any] | None = None
    confirmations: int = 0
    finalized: bool = False


def _open_nonce_database(database_path: str) -> sqlite3.Connection:
    options = {"timeout": 30, "isolation_level": None, "check_same_thread": False}
    if database_path == ":memory:":
        return sqlite3.connect(database_path, **options)

    absolute_path = os.path.abspath(os.fspath(database_path))
    parent_path = os.path.dirname(absolute_path)
    parent_info = os.stat(parent_path, follow_symlinks=False)
    if not stat.S_ISDIR(parent_info.st_mode) or parent_info.st_mode & 0o022:
        raise PermissionError("nonce database directory must not be group/world-writable")

    no_follow = getattr(os, "O_NOFOLLOW", None)
    if no_follow is None:
        raise PermissionError("platform cannot safely reject a symlinked nonce database")
    flags = os.O_RDWR | os.O_CREAT | no_follow | getattr(os, "O_CLOEXEC", 0)
    try:
        descriptor = os.open(absolute_path, flags, 0o600)
    except OSError as exc:
        if exc.errno == errno.ELOOP:
            raise PermissionError("nonce database path must not be a symlink") from None
        raise

    try:
        opened_info = os.fstat(descriptor)
        opened_mode = stat.S_IMODE(opened_info.st_mode)
        if (
            not stat.S_ISREG(opened_info.st_mode)
            or opened_info.st_uid != os.geteuid()
            or opened_mode & 0o077
            or opened_mode & 0o600 != 0o600
        ):
            raise PermissionError("nonce database must be an owner-only regular file")

        connection = sqlite3.connect(absolute_path, **options)
        try:
            current_info = os.stat(absolute_path, follow_symlinks=False)
            if (
                not stat.S_ISREG(current_info.st_mode)
                or current_info.st_dev != opened_info.st_dev
                or current_info.st_ino != opened_info.st_ino
            ):
                raise PermissionError("nonce database path changed while opening")
        except Exception:
            connection.close()
            raise
        return connection
    finally:
        os.close(descriptor)


class NonceManager:
    """SQLite-backed nonce leases; signed bytes are retained for safe retries."""

    def __init__(self, database_path: str):
        self._connection = _open_nonce_database(database_path)
        self._connection.row_factory = sqlite3.Row
        self._lock = threading.RLock()
        self._connection.execute("PRAGMA journal_mode=WAL")
        self._connection.execute(
            """CREATE TABLE IF NOT EXISTS evm_nonce_leases (
                request_id TEXT PRIMARY KEY,
                chain_id TEXT NOT NULL,
                sender TEXT NOT NULL,
                nonce INTEGER NOT NULL CHECK(nonce >= 0),
                state TEXT NOT NULL CHECK(state IN ('reserved','signed','broadcast','confirmed','manual_review')),
                raw_transaction TEXT,
                transaction_hash TEXT,
                UNIQUE(chain_id, sender, nonce)
            )"""
        )

    def close(self) -> None:
        with self._lock:
            self._connection.close()

    def backup_to(self, destination: sqlite3.Connection) -> None:
        """Back up a consistent snapshot to a caller-owned, separate connection.

        The destination contains raw signed transactions; its path and access
        controls remain the caller's responsibility.
        """
        if destination is self._connection:
            raise ValueError("backup destination must be a separate database connection")
        if destination.in_transaction:
            raise ValueError("backup destination must not have an active transaction")
        with self._lock:
            self._connection.backup(destination)

    def reserve_exact(
        self, chain_id: str, sender: str, request_id: str, nonce: int, pending_nonce: int
    ) -> int:
        if not _DECIMAL.fullmatch(chain_id) or int(chain_id) <= 0:
            raise ValueError("chain_id must be a positive decimal string")
        if not _ADDRESS.fullmatch(sender) or not request_id:
            raise ValueError("valid sender and request ID are required")
        sqlite_max = (1 << 63) - 1
        if any(isinstance(value, bool) or not isinstance(value, int) for value in (nonce, pending_nonce)):
            raise ValueError("nonce values must be integers")
        if nonce < 0 or pending_nonce < 0 or nonce > sqlite_max or pending_nonce > sqlite_max:
            raise ValueError("nonce is outside the durable lease range")
        sender = sender.lower()
        with self._lock:
            self._connection.execute("BEGIN IMMEDIATE")
            try:
                existing = self._connection.execute(
                    "SELECT chain_id, sender, nonce FROM evm_nonce_leases WHERE request_id = ?", (request_id,)
                ).fetchone()
                if existing:
                    if (
                        existing["chain_id"] != chain_id
                        or existing["sender"] != sender
                        or int(existing["nonce"]) != nonce
                    ):
                        raise ValueError("request ID is already leased to another chain, sender, or nonce")
                    self._connection.execute("COMMIT")
                    return nonce
                if nonce < pending_nonce:
                    raise ValueError("approved nonce is below the RPC pending nonce")
                conflict = self._connection.execute(
                    "SELECT request_id FROM evm_nonce_leases WHERE chain_id = ? AND sender = ? AND nonce = ?",
                    (chain_id, sender, nonce),
                ).fetchone()
                if conflict:
                    raise ValueError("approved nonce is already leased to another request")
                self._connection.execute(
                    "INSERT INTO evm_nonce_leases(request_id,chain_id,sender,nonce,state) VALUES(?,?,?,?, 'reserved')",
                    (request_id, chain_id, sender, nonce),
                )
                self._connection.execute("COMMIT")
                return nonce
            except Exception:
                if self._connection.in_transaction:
                    self._connection.execute("ROLLBACK")
                raise

    def reserve_next(self, chain_id: str, sender: str, request_id: str, pending_nonce: int) -> int:
        if not _DECIMAL.fullmatch(chain_id) or int(chain_id) <= 0:
            raise ValueError("chain_id must be a positive decimal string")
        if not _ADDRESS.fullmatch(sender) or not request_id:
            raise ValueError("valid sender and request ID are required")
        if pending_nonce < 0:
            raise ValueError("pending nonce cannot be negative")
        sender = sender.lower()
        with self._lock:
            self._connection.execute("BEGIN IMMEDIATE")
            try:
                existing = self._connection.execute(
                    "SELECT chain_id, sender, nonce FROM evm_nonce_leases WHERE request_id = ?", (request_id,)
                ).fetchone()
                if existing:
                    if existing["chain_id"] != chain_id or existing["sender"] != sender:
                        raise ValueError("request ID is already leased to another chain or sender")
                    nonce = int(existing["nonce"])
                else:
                    row = self._connection.execute(
                        "SELECT MAX(nonce) AS max_nonce FROM evm_nonce_leases WHERE chain_id = ? AND sender = ?",
                        (chain_id, sender),
                    ).fetchone()
                    next_local = 0 if row["max_nonce"] is None else int(row["max_nonce"]) + 1
                    nonce = max(pending_nonce, next_local)
                    self._connection.execute(
                        "INSERT INTO evm_nonce_leases(request_id,chain_id,sender,nonce,state) VALUES(?,?,?,?, 'reserved')",
                        (request_id, chain_id, sender, nonce),
                    )
                self._connection.execute("COMMIT")
                return nonce
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def get(self, request_id: str) -> dict[str, Any] | None:
        with self._lock:
            row = self._connection.execute("SELECT * FROM evm_nonce_leases WHERE request_id = ?", (request_id,)).fetchone()
            return dict(row) if row else None

    def require_lease(self, request_id: str, chain_id: str, sender: str, nonce: int) -> dict[str, Any]:
        lease = self.get(request_id)
        if lease is None or lease["chain_id"] != chain_id or lease["sender"] != sender.lower() or lease["nonce"] != nonce:
            raise ValueError("approved request has no matching durable nonce lease")
        return lease

    def _transition(self, request_id: str, expected: tuple[str, ...], new_state: str, **updates: str) -> None:
        with self._lock:
            self._connection.execute("BEGIN IMMEDIATE")
            try:
                row = self._connection.execute("SELECT state FROM evm_nonce_leases WHERE request_id = ?", (request_id,)).fetchone()
                if row is None:
                    raise ValueError("nonce lease not found")
                if row["state"] not in expected:
                    raise ValueError(f"invalid nonce lease transition from {row['state']} to {new_state}")
                assignments = ["state = ?"] + [f"{field} = ?" for field in updates]
                values = [new_state, *updates.values(), request_id]
                self._connection.execute(
                    f"UPDATE evm_nonce_leases SET {', '.join(assignments)} WHERE request_id = ?", values
                )
                self._connection.execute("COMMIT")
            except Exception:
                self._connection.execute("ROLLBACK")
                raise

    def mark_signed(self, request_id: str, raw_transaction: str) -> None:
        current = self.get(request_id)
        if current and current["state"] == "signed":
            if current["raw_transaction"] != raw_transaction:
                raise ValueError("a different signed transaction is already stored for this lease")
            return
        self._transition(request_id, ("reserved",), "signed", raw_transaction=raw_transaction)

    def mark_broadcast(self, request_id: str, transaction_hash: str) -> None:
        current = self.get(request_id)
        if current and current["state"] == "broadcast":
            if current["transaction_hash"] != transaction_hash:
                raise ValueError("broadcast transaction hash does not match existing lease")
            return
        self._transition(request_id, ("signed",), "broadcast", transaction_hash=transaction_hash)

    def mark_confirmed(self, request_id: str, transaction_hash: str) -> None:
        current = self.get(request_id)
        if current is None or current["transaction_hash"] != transaction_hash:
            raise ValueError("confirmed transaction hash does not match lease")
        if current["state"] == "confirmed":
            return
        self._transition(request_id, ("broadcast",), "confirmed")

    def mark_manual_review(self, request_id: str) -> None:
        self._transition(request_id, ("reserved", "signed", "broadcast"), "manual_review")


class EvmAdapter:
    def __init__(self, network: EvmNetworkConfig, rpc: Any, nonce_manager: NonceManager | None):
        if not network.testnet:
            raise ValueError("this adapter release supports testnets only")
        self.network = network
        self.rpc = rpc
        self.nonce_manager = nonce_manager

    def get_nonce_lease(self, request_id: str) -> dict[str, Any] | None:
        if self.nonce_manager is None:
            raise ValueError("durable nonce manager is required")
        return self.nonce_manager.get(request_id)

    @classmethod
    def from_env(cls, network: EvmNetworkConfig, nonce_manager: NonceManager, environ: Mapping[str, str] | None = None):
        return cls(network, JsonRpcClient(network.rpc_url(environ)), nonce_manager)

    def assert_network(self) -> None:
        actual = _quantity(self.rpc.call("eth_chainId", []), "eth_chainId")
        if actual != self.network.chain_id:
            raise ValueError(f"RPC endpoint chain ID {actual} does not match configured chain ID {self.network.chain_id}")

    def estimate_fees(self, request: Mapping[str, Any]) -> dict[str, int]:
        self.assert_network()
        block = self.rpc.call("eth_getBlockByNumber", ["latest", False])
        if not isinstance(block, Mapping) or block.get("baseFeePerGas") is None:
            raise ValueError("EIP-1559 base fee is unavailable")
        base_fee = _quantity(block["baseFeePerGas"], "baseFeePerGas")
        priority = _quantity(self.rpc.call("eth_maxPriorityFeePerGas", []), "maxPriorityFeePerGas")
        gas_policy = request.get("gas_policy")
        if not isinstance(gas_policy, Mapping):
            raise ValueError("approved gas policy is missing")
        max_fee_cap = _decimal(gas_policy, "max_fee")
        priority_cap = _decimal(gas_policy, "max_priority_fee")
        max_fee = base_fee * 2 + priority
        if priority > priority_cap or max_fee > max_fee_cap:
            raise ValueError("current EIP-1559 estimate exceeds the approved fee cap")
        return {"base_fee": base_fee, "max_priority_fee": priority, "max_fee": max_fee}

    def simulate(self, request: Mapping[str, Any], calldata_hex: str, sender: str) -> dict[str, Any]:
        self.assert_network()
        if not _ADDRESS.fullmatch(sender):
            raise ValueError("simulation sender must be an EVM address")
        if str(request.get("chain_id")) != str(self.network.chain_id):
            raise ValueError("approved request chain ID does not match adapter configuration")
        calldata = _hex_bytes(calldata_hex, "calldata")
        if "0x" + keccak(calldata).hex() != str(request.get("calldata_hash", "")).lower():
            raise ValueError("calldata does not match approved commitment")
        tx = {
            "from": sender.lower(),
            "to": str(request.get("target", "")).lower(),
            "value": hex(_decimal(request, "value_base_units")),
            "data": "0x" + calldata.hex(),
            "gas": hex(_decimal(request, "gas_limit", positive=True)),
            "maxFeePerGas": hex(_decimal(request.get("gas_policy", {}), "max_fee")),
            "maxPriorityFeePerGas": hex(_decimal(request.get("gas_policy", {}), "max_priority_fee")),
            "nonce": hex(_decimal(request, "nonce")),
            "chainId": hex(_decimal(request, "chain_id", positive=True)),
        }
        result = self.rpc.call("eth_call", [tx, "latest"])
        estimated = _quantity(self.rpc.call("eth_estimateGas", [tx]), "eth_estimateGas")
        gas_limit = _decimal(request, "gas_limit", positive=True)
        if estimated > gas_limit:
            raise ValueError("simulation gas estimate exceeds approved gas limit")
        return {"return_data": result, "gas_estimate": estimated}

    def reserve_approved_nonce(
        self,
        record: Mapping[str, Any],
        calldata_hex: str,
        *,
        active_policy: Mapping[str, Any],
        near_block_timestamp_ns: int,
        signer_address: str,
    ) -> int:
        """Lease only the nonce already committed by a currently approved request."""
        if self.nonce_manager is None:
            raise ValueError("durable nonce manager is required")
        if not isinstance(signer_address, str) or not _ADDRESS.fullmatch(signer_address):
            raise ValueError("signer address must be an EVM address")
        prepare_approved_eip1559_signing_payload(
            record,
            calldata_hex,
            active_policy=active_policy,
            near_block_timestamp_ns=near_block_timestamp_ns,
        )
        request = record["request"]
        if str(request["chain_id"]) != str(self.network.chain_id):
            raise ValueError("approved request chain ID does not match adapter configuration")
        self.assert_network()
        pending_nonce = _quantity(
            self.rpc.call("eth_getTransactionCount", [signer_address.lower(), "pending"]),
            "pending transaction count",
        )
        return self.nonce_manager.reserve_exact(
            str(request["chain_id"]),
            signer_address,
            str(request["request_id"]),
            _decimal(request, "nonce"),
            pending_nonce,
        )

    def prepare_approved_transaction(
        self,
        record: Mapping[str, Any],
        calldata_hex: str,
        *,
        active_policy: Mapping[str, Any],
        near_block_timestamp_ns: int,
        signature_response: Mapping[str, Any],
        signer_address: str,
        expected_effect: Mapping[str, Any],
    ) -> PreparedTransaction:
        if self.nonce_manager is None:
            raise ValueError("durable nonce manager is required")
        digest = prepare_approved_eip1559_signing_payload(
            record, calldata_hex, active_policy=active_policy, near_block_timestamp_ns=near_block_timestamp_ns
        )
        request = record["request"]
        if str(request["chain_id"]) != str(self.network.chain_id):
            raise ValueError("approved request chain ID does not match adapter configuration")
        effect_hash = hash_expected_effect(expected_effect)
        if effect_hash.lower() != str(request["expected_effect_hash"]).lower():
            raise ValueError("expected effect does not match approved commitment")
        self.assert_network()
        nonce = _decimal(request, "nonce")
        lease = self.nonce_manager.require_lease(
            str(request["request_id"]), str(request["chain_id"]), signer_address, nonce
        )
        self.estimate_fees(request)
        self.simulate(request, calldata_hex, signer_address)
        if lease["state"] in ("signed", "broadcast") and lease["raw_transaction"]:
            raw = lease["raw_transaction"]
            expected_raw = encode_signed_eip1559(
                request, calldata_hex, signature_response, signer_address, digest=digest
            )
            if raw != expected_raw:
                self.nonce_manager.mark_manual_review(str(request["request_id"]))
                raise ValueError("persisted signed transaction differs from the approved payload/signature")
            return PreparedTransaction(
                str(request["request_id"]), self.network.chain_id, nonce,
                signer_address.lower(), raw, self.transaction_hash(raw), effect_hash
            )
        if lease["state"] != "reserved":
            raise ValueError("nonce lease is not available for signing")
        raw = encode_signed_eip1559(request, calldata_hex, signature_response, signer_address, digest=digest)
        tx_hash = self.transaction_hash(raw)
        self.nonce_manager.mark_signed(str(request["request_id"]), raw)
        return PreparedTransaction(str(request["request_id"]), self.network.chain_id, nonce, signer_address.lower(), raw, tx_hash, effect_hash)

    @staticmethod
    def transaction_hash(raw_transaction: str) -> str:
        raw = _hex_bytes(raw_transaction, "signed transaction")
        return "0x" + keccak(raw).hex()

    def broadcast(
        self,
        prepared: PreparedTransaction,
        *,
        record: Mapping[str, Any],
        calldata_hex: str,
        active_policy: Mapping[str, Any],
        near_block_timestamp_ns: int,
        signature_response: Mapping[str, Any],
        signer_address: str,
        expected_effect: Mapping[str, Any],
    ) -> str:
        if prepared.chain_id != self.network.chain_id:
            raise ValueError("prepared transaction belongs to another chain")
        if self.nonce_manager is None:
            raise ValueError("durable nonce manager is required")
        validated = self.prepare_approved_transaction(
            record,
            calldata_hex,
            active_policy=active_policy,
            near_block_timestamp_ns=near_block_timestamp_ns,
            signature_response=signature_response,
            signer_address=signer_address,
            expected_effect=expected_effect,
        )
        if validated != prepared:
            raise ValueError("prepared transaction differs from the current approved request and policy")
        lease = self.nonce_manager.get(prepared.request_id)
        if lease is None or lease["state"] not in ("signed", "broadcast") or lease["raw_transaction"] != prepared.raw_transaction:
            raise ValueError("prepared transaction does not match a signed nonce lease")
        expected_hash = self.transaction_hash(prepared.raw_transaction)
        if expected_hash != prepared.transaction_hash:
            raise ValueError("prepared transaction hash is invalid")
        self.assert_network()
        try:
            returned_hash = self.rpc.call("eth_sendRawTransaction", [prepared.raw_transaction])
        except JsonRpcError as exc:
            if "already known" not in str(exc).lower() and "known transaction" not in str(exc).lower():
                raise
            returned_hash = expected_hash
        if not isinstance(returned_hash, str) or returned_hash.lower() != expected_hash:
            self.nonce_manager.mark_manual_review(prepared.request_id)
            raise ValueError("RPC returned a transaction hash that differs from the signed bytes")
        self.nonce_manager.mark_broadcast(prepared.request_id, expected_hash)
        return expected_hash

    def check_finality(self, transaction_hash: str) -> FinalityResult:
        if not _HEX32.fullmatch(transaction_hash):
            raise ValueError("transaction hash must be bytes32")
        self.assert_network()
        receipt = self.rpc.call("eth_getTransactionReceipt", [transaction_hash])
        if receipt is None:
            return FinalityResult("pending", transaction_hash)
        if not isinstance(receipt, Mapping):
            raise ValueError("RPC returned a malformed transaction receipt")
        receipt_hash = receipt.get("transactionHash")
        if not isinstance(receipt_hash, str) or receipt_hash.lower() != transaction_hash.lower():
            raise ValueError("receipt transaction hash does not match the requested transaction")
        if _quantity(receipt.get("status"), "receipt status") != 1:
            return FinalityResult("reverted", transaction_hash, receipt)
        receipt_block = _quantity(receipt.get("blockNumber"), "receipt block number")
        receipt_block_hash = receipt.get("blockHash")
        if not isinstance(receipt_block_hash, str) or not _HEX32.fullmatch(receipt_block_hash):
            raise ValueError("RPC returned a malformed receipt block hash")
        canonical_block = self.rpc.call("eth_getBlockByNumber", [hex(receipt_block), False])
        if (
            not isinstance(canonical_block, Mapping)
            or _quantity(canonical_block.get("number"), "canonical block number") != receipt_block
            or not isinstance(canonical_block.get("hash"), str)
            or canonical_block["hash"].lower() != receipt_block_hash.lower()
        ):
            return FinalityResult("reorged", transaction_hash, receipt)
        try:
            finalized = self.rpc.call("eth_getBlockByNumber", ["finalized", False])
        except JsonRpcError:
            finalized = None
        if isinstance(finalized, Mapping) and finalized.get("number") is not None:
            finalized_number = _quantity(finalized["number"], "finalized block number")
            if finalized_number >= receipt_block:
                return FinalityResult("finalized", transaction_hash, receipt, finalized_number - receipt_block + 1, True)
        latest = self.rpc.call("eth_getBlockByNumber", ["latest", False])
        latest_number = _quantity(latest.get("number"), "latest block number") if isinstance(latest, Mapping) else receipt_block
        confirmations = max(0, latest_number - receipt_block + 1)
        if confirmations >= self.network.confirmations_required:
            return FinalityResult(
                "confirmation_threshold_met", transaction_hash, receipt, confirmations, False
            )
        return FinalityResult("confirming", transaction_hash, receipt, confirmations, False)

    def finalize_and_verify_effect(
        self,
        prepared: PreparedTransaction,
        expected_effect: Mapping[str, Any],
    ) -> bool:
        if self.nonce_manager is None:
            raise ValueError("durable nonce manager is required")
        if prepared.chain_id != self.network.chain_id:
            raise ValueError("prepared transaction belongs to another chain")
        finality = self.check_finality(prepared.transaction_hash)
        if finality.state != "finalized":
            return False
        if hash_expected_effect(expected_effect).lower() != prepared.expected_effect_hash.lower():
            raise ValueError("expected effect does not match prepared transaction commitment")
        if finality.receipt is None or not verify_expected_effect(
            expected_effect, finality.receipt, rpc=self.rpc
        ):
            self.nonce_manager.mark_manual_review(prepared.request_id)
            return False
        self.nonce_manager.mark_confirmed(prepared.request_id, prepared.transaction_hash)
        return True


def encode_signed_eip1559(
    request: Mapping[str, Any],
    calldata_hex: str,
    signature_response: Mapping[str, Any],
    expected_signer_address: str,
    *,
    digest: str | None = None,
) -> str:
    """Verify a NEAR Chain Signature and encode a type-2 transaction."""
    calculated_digest = verify_eip1559_signing_payload(request, calldata_hex)
    if digest is not None and calculated_digest.lower() != digest.lower():
        raise ValueError("supplied signing digest does not match approved transaction")
    if not isinstance(expected_signer_address, str) or not _ADDRESS.fullmatch(expected_signer_address):
        raise ValueError("expected signer address is invalid")
    if not isinstance(signature_response, Mapping) or set(signature_response) != {
        "big_r",
        "s",
        "recovery_id",
    }:
        raise ValueError("Chain Signature response shape is invalid")
    if (
        not isinstance(signature_response["big_r"], Mapping)
        or set(signature_response["big_r"]) != {"affine_point"}
        or not isinstance(signature_response["s"], Mapping)
        or set(signature_response["s"]) != {"scalar"}
    ):
        raise ValueError("Chain Signature response shape is invalid")
    try:
        big_r = signature_response["big_r"]["affine_point"]
        scalar = signature_response["s"]["scalar"]
        recovery_id = signature_response["recovery_id"]
        if not isinstance(big_r, str) or not isinstance(scalar, str):
            raise TypeError
        point = big_r[2:] if big_r.startswith("0x") else big_r
        scalar_hex = scalar[2:] if scalar.startswith("0x") else scalar
        if len(point) != 66 or point[:2].lower() not in {"02", "03"} or not re.fullmatch(r"[0-9a-fA-F]+", point):
            raise ValueError("Chain Signature R point must be compressed secp256k1 hex")
        if not re.fullmatch(r"[0-9a-fA-F]{1,64}", scalar_hex):
            raise ValueError("Chain Signature scalar is malformed")
        x = int(point[2:], 16)
        alpha = (pow(x, 3, _FIELD_MODULUS) + 7) % _FIELD_MODULUS
        y = pow(alpha, (_FIELD_MODULUS + 1) // 4, _FIELD_MODULUS)
        if pow(y, 2, _FIELD_MODULUS) != alpha:
            raise ValueError("Chain Signature R point is not on secp256k1")
        point_parity = int(point[:2], 16) & 1
        if y % 2 != point_parity:
            y = _FIELD_MODULUS - y
        r, s = x, int(scalar_hex, 16)
        if x >= _SECP256K1_ORDER:
            raise ValueError("Chain Signature R coordinate is outside scalar range")
        if isinstance(recovery_id, bool) or recovery_id not in (0, 1) or point_parity != recovery_id:
            raise ValueError("Chain Signature recovery ID is invalid")
        if not 1 <= r < _SECP256K1_ORDER or not 1 <= s < _SECP256K1_ORDER:
            raise ValueError("Chain Signature scalars are outside secp256k1 range")
        if s > _SECP256K1_HALF_ORDER:
            s = _SECP256K1_ORDER - s
            recovery_id ^= 1
        recovered = keys.Signature(vrs=(recovery_id, r, s)).recover_public_key_from_msg_hash(bytes.fromhex(calculated_digest[2:]))
        address = "0x" + recovered.to_canonical_address().hex()
        if address.lower() != expected_signer_address.lower():
            raise ValueError("recovered signer does not match approved derived address")
    except (KeyError, TypeError, ValueError) as exc:
        if isinstance(exc, ValueError):
            raise
        raise ValueError("Chain Signature response is malformed") from exc

    chain_id = _decimal(request, "chain_id", positive=True)
    nonce = _decimal(request, "nonce")
    priority_fee = _decimal(request["gas_policy"], "max_priority_fee")
    max_fee = _decimal(request["gas_policy"], "max_fee")
    gas_limit = _decimal(request, "gas_limit", positive=True)
    value = _decimal(request, "value_base_units")
    target = _hex_bytes(request.get("target"), "target")
    if len(target) != 20:
        raise ValueError("target must be a 20-byte EVM address")
    data = _hex_bytes(calldata_hex, "calldata")
    signed_fields = [
        _rlp_integer(chain_id), _rlp_integer(nonce), _rlp_integer(priority_fee), _rlp_integer(max_fee),
        _rlp_integer(gas_limit), target, _rlp_integer(value), data, [],
        _rlp_integer(recovery_id), _rlp_integer(r), _rlp_integer(s),
    ]
    return "0x02" + _rlp_encode(signed_fields).hex()
