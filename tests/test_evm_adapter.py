import json
import os
import sqlite3
import stat
import tempfile
import threading
import unittest
from contextlib import closing
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

import rlp
from eth_hash.auto import keccak
from eth_keys import keys

from protocol.evm_adapter import (
    TESTNETS,
    CrossCheckingJsonRpcClient,
    EvmAdapter,
    EvmNetworkConfig,
    JsonRpcClient,
    JsonRpcError,
    NonceManager,
    PreparedTransaction,
    encode_signed_eip1559,
    hash_expected_effect,
    verify_expected_effect,
)
from protocol.evm_signing_payload import prepare_approved_eip1559_signing_payload

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = json.loads((ROOT / "specs/examples/evm-signing-payload.json").read_text())
REQUEST = FIXTURE["request"]
EFFECT = FIXTURE["expected_effect"]
CALLDATA = FIXTURE["calldata_hex"]
ACTIVE_POLICY = FIXTURE["active_policy"]


def _chainsig_response(digest_hex):
    digest = bytes.fromhex(digest_hex[2:])
    signature = keys.PrivateKey((1).to_bytes(32, "big")).sign_msg_hash(digest)
    parity, r, s = signature.vrs
    field_modulus = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEFFFFFC2F
    alpha = (pow(r, 3, field_modulus) + 7) % field_modulus
    y = pow(alpha, (field_modulus + 1) // 4, field_modulus)
    if y % 2 != parity:
        y = field_modulus - y
    return {
        "big_r": {"affine_point": "0x" + ("03" if parity else "02") + f"{r:064x}"},
        "s": {"scalar": "0x" + f"{s:064x}"},
        "recovery_id": parity,
    }


class FakeRpc:
    def __init__(self, responses):
        self.responses = responses
        self.calls = []

    def call(self, method, params):
        self.calls.append((method, params))
        value = self.responses.get(method)
        if isinstance(value, Exception):
            raise value
        if callable(value):
            return value(params)
        return value


class LoopbackJsonRpcServer:
    """HTTP JSON-RPC fixture bound only to an ephemeral loopback port."""

    def __init__(self, responder):
        self.calls = []
        self.responder = responder
        owner = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                request_size = int(self.headers.get("Content-Length", "0"))
                request = json.loads(self.rfile.read(request_size))
                owner.calls.append(request)
                status, payload = owner.responder(request)
                encoded = json.dumps(payload).encode()
                self.send_response(status)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(encoded)))
                self.end_headers()
                self.wfile.write(encoded)

            def log_message(self, _format, *_args):
                pass

        self._server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)
        self._thread.start()
        self.url = f"http://127.0.0.1:{self._server.server_port}/"

    def close(self):
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=2)


def _json_rpc_result(request, result):
    return 200, {"jsonrpc": "2.0", "id": request["id"], "result": result}


class EvmAdapterTests(unittest.TestCase):
    def test_network_config_rejects_mainnet_chain_ids(self):
        with self.assertRaisesRegex(ValueError, "allowlisted testnet"):
            EvmNetworkConfig("Ethereum Mainnet", 1, "ETHEREUM_RPC_URL")

    def test_network_configs_cover_base_sepolia_and_gnosis_chiado(self):
        self.assertEqual(TESTNETS["base-sepolia"].chain_id, 84532)
        self.assertEqual(TESTNETS["gnosis-chiado"].chain_id, 10200)
        self.assertTrue(all(network.testnet for network in TESTNETS.values()))

    def test_environment_adapter_requires_two_distinct_rpc_endpoints(self):
        network = TESTNETS["base-sepolia"]
        with self.assertRaisesRegex(ValueError, "BASE_SEPOLIA_VERIFY_RPC_URL"):
            network.rpc_urls({"BASE_SEPOLIA_RPC_URL": "https://rpc-one.example/key"})
        with self.assertRaisesRegex(ValueError, "must be distinct"):
            network.rpc_urls(
                {
                    "BASE_SEPOLIA_RPC_URL": "https://rpc-one.example/key",
                    "BASE_SEPOLIA_VERIFY_RPC_URL": "https://rpc-one.example/key",
                }
            )
        urls = network.rpc_urls(
            {
                "BASE_SEPOLIA_RPC_URL": "https://rpc-one.example/key",
                "BASE_SEPOLIA_VERIFY_RPC_URL": "https://rpc-two.example/key",
            }
        )
        self.assertEqual(urls, ("https://rpc-one.example/key", "https://rpc-two.example/key"))

    def test_environment_adapter_uses_cross_checking_rpc_client(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        adapter = EvmAdapter.from_env(
            TESTNETS["base-sepolia"],
            manager,
            {
                "BASE_SEPOLIA_RPC_URL": "https://rpc-one.example/key",
                "BASE_SEPOLIA_VERIFY_RPC_URL": "https://rpc-two.example/key",
            },
        )
        self.assertIsInstance(adapter.rpc, CrossCheckingJsonRpcClient)

    def test_rpc_endpoint_requires_tls_except_for_loopback(self):
        with self.assertRaisesRegex(ValueError, "loopback"):
            TESTNETS["base-sepolia"].rpc_url({"BASE_SEPOLIA_RPC_URL": "http://rpc.example"})
        self.assertEqual(
            TESTNETS["base-sepolia"].rpc_url({"BASE_SEPOLIA_RPC_URL": "http://127.0.0.1:8545"}),
            "http://127.0.0.1:8545",
        )

    def test_cross_checking_rpc_requires_agreement_for_reads(self):
        primary = FakeRpc({"eth_chainId": "0x14a34"})
        verifier = FakeRpc({"eth_chainId": "0x14a34"})
        rpc = CrossCheckingJsonRpcClient(primary, verifier)

        self.assertEqual(rpc.call("eth_chainId", []), "0x14a34")
        self.assertEqual(len(primary.calls), 1)
        self.assertEqual(len(verifier.calls), 1)

    def test_cross_checking_rpc_fails_closed_on_read_disagreement(self):
        rpc = CrossCheckingJsonRpcClient(
            FakeRpc({"eth_chainId": "0x14a34"}),
            FakeRpc({"eth_chainId": "0x1"}),
        )

        with self.assertRaisesRegex(JsonRpcError, "different results"):
            rpc.call("eth_chainId", [])

    def test_cross_checking_rpc_uses_conservative_estimates(self):
        rpc = CrossCheckingJsonRpcClient(
            FakeRpc({"eth_estimateGas": "0x5208", "eth_maxPriorityFeePerGas": "0x2"}),
            FakeRpc({"eth_estimateGas": "0x5300", "eth_maxPriorityFeePerGas": "0x3"}),
        )

        self.assertEqual(rpc.call("eth_estimateGas", []), "0x5300")
        self.assertEqual(rpc.call("eth_maxPriorityFeePerGas", []), "0x3")

    def test_cross_checking_rpc_broadcasts_only_through_primary(self):
        primary = FakeRpc({"eth_sendRawTransaction": "0x" + "ab" * 32})
        verifier = FakeRpc({"eth_sendRawTransaction": RuntimeError("must not broadcast twice")})
        rpc = CrossCheckingJsonRpcClient(primary, verifier)

        self.assertEqual(rpc.call("eth_sendRawTransaction", ["0xdeadbeef"]), "0x" + "ab" * 32)
        self.assertEqual(len(primary.calls), 1)
        self.assertEqual(verifier.calls, [])

    def test_cross_checking_rpc_rejects_unreviewed_methods(self):
        primary = FakeRpc({})
        verifier = FakeRpc({})
        rpc = CrossCheckingJsonRpcClient(primary, verifier)

        with self.assertRaisesRegex(JsonRpcError, "not enabled"):
            rpc.call("eth_sendTransaction", [{"to": "0x"}])
        self.assertEqual(primary.calls, [])
        self.assertEqual(verifier.calls, [])

    def test_cross_checking_rpc_redacts_provider_errors(self):
        rpc = CrossCheckingJsonRpcClient(
            FakeRpc({"eth_chainId": RuntimeError("https://rpc.example/sensitive-key")}),
            FakeRpc({"eth_chainId": "0x14a34"}),
        )

        with self.assertRaises(JsonRpcError) as raised:
            rpc.call("eth_chainId", [])
        self.assertNotIn("sensitive-key", str(raised.exception))

    def test_cross_checking_rpc_requires_separate_clients(self):
        provider = FakeRpc({})
        with self.assertRaisesRegex(ValueError, "separate instances"):
            CrossCheckingJsonRpcClient(provider, provider)

    def test_loopback_json_rpc_transport_cross_checks_matching_reads(self):
        primary = LoopbackJsonRpcServer(lambda request: _json_rpc_result(request, "0x14a34"))
        verifier = LoopbackJsonRpcServer(lambda request: _json_rpc_result(request, "0x14a34"))
        self.addCleanup(primary.close)
        self.addCleanup(verifier.close)
        rpc = CrossCheckingJsonRpcClient(JsonRpcClient(primary.url), JsonRpcClient(verifier.url))

        self.assertEqual(rpc.call("eth_chainId", []), "0x14a34")
        self.assertEqual([request["method"] for request in primary.calls], ["eth_chainId"])
        self.assertEqual([request["method"] for request in verifier.calls], ["eth_chainId"])

    def test_loopback_json_rpc_transport_fails_closed_on_disagreement(self):
        primary = LoopbackJsonRpcServer(lambda request: _json_rpc_result(request, "0x14a34"))
        verifier = LoopbackJsonRpcServer(lambda request: _json_rpc_result(request, "0x1"))
        self.addCleanup(primary.close)
        self.addCleanup(verifier.close)
        rpc = CrossCheckingJsonRpcClient(JsonRpcClient(primary.url), JsonRpcClient(verifier.url))

        with self.assertRaisesRegex(JsonRpcError, "different results"):
            rpc.call("eth_chainId", [])
        self.assertEqual(len(primary.calls), 1)
        self.assertEqual(len(verifier.calls), 1)

    def test_loopback_json_rpc_transport_redacts_provider_failure(self):
        primary = LoopbackJsonRpcServer(lambda request: _json_rpc_result(request, "0x14a34"))
        verifier = LoopbackJsonRpcServer(
            lambda request: (503, {"message": "provider-private-marker"})
        )
        self.addCleanup(primary.close)
        self.addCleanup(verifier.close)
        rpc = CrossCheckingJsonRpcClient(JsonRpcClient(primary.url), JsonRpcClient(verifier.url))

        with self.assertRaises(JsonRpcError) as raised:
            rpc.call("eth_chainId", [])
        self.assertNotIn("provider-private-marker", str(raised.exception))
        self.assertEqual(len(primary.calls), 1)
        self.assertEqual(len(verifier.calls), 1)

    def test_loopback_json_rpc_transport_sends_broadcast_to_primary_only(self):
        transaction_hash = "0x" + "ab" * 32
        primary = LoopbackJsonRpcServer(lambda request: _json_rpc_result(request, transaction_hash))
        verifier = LoopbackJsonRpcServer(lambda request: _json_rpc_result(request, "unexpected"))
        self.addCleanup(primary.close)
        self.addCleanup(verifier.close)
        rpc = CrossCheckingJsonRpcClient(JsonRpcClient(primary.url), JsonRpcClient(verifier.url))

        self.assertEqual(rpc.call("eth_sendRawTransaction", ["0xdeadbeef"]), transaction_hash)
        self.assertEqual([request["method"] for request in primary.calls], ["eth_sendRawTransaction"])
        self.assertEqual(verifier.calls, [])

    def test_adapter_does_not_broadcast_after_loopback_read_disagreement(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        sender = "0x" + keys.PrivateKey((1).to_bytes(32, "big")).public_key.to_canonical_address().hex()
        warm_rpc = FakeRpc(
            {
                "eth_chainId": "0x14a34",
                "eth_getTransactionCount": "0x0",
                "eth_getBlockByNumber": lambda params: (
                    {"baseFeePerGas": "0x1", "number": "0x10"}
                    if params[0] == "latest"
                    else {"number": "0x10", "hash": "0x" + "cc" * 32}
                ),
                "eth_maxPriorityFeePerGas": "0xf4240",
                "eth_call": "0x",
                "eth_estimateGas": "0x5208",
            }
        )
        warm_adapter = EvmAdapter(TESTNETS["base-sepolia"], warm_rpc, manager)
        record = {
            "status": "Approved",
            "action_hash": FIXTURE["expected_action_hash"],
            "policy_hash": FIXTURE["policy_hash"],
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }
        warm_adapter.reserve_approved_nonce(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
            signer_address=sender,
        )
        digest = prepare_approved_eip1559_signing_payload(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
        )
        signature = _chainsig_response(digest)
        prepared = warm_adapter.prepare_approved_transaction(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
            signature_response=signature,
            signer_address=sender,
            expected_effect=EFFECT,
        )

        def matching_result(request):
            method = request["method"]
            if method == "eth_chainId":
                result = "0x14a34"
            elif method == "eth_getBlockByNumber":
                result = {"baseFeePerGas": "0x1", "number": "0x10"}
            elif method == "eth_maxPriorityFeePerGas":
                result = "0xf4240"
            elif method == "eth_call":
                result = "0x"
            elif method == "eth_estimateGas":
                result = "0x5208"
            else:
                raise AssertionError(f"unexpected JSON-RPC method: {method}")
            return _json_rpc_result(request, result)

        primary = LoopbackJsonRpcServer(matching_result)

        def verifier_result(request):
            if request["method"] == "eth_call":
                return _json_rpc_result(request, "0x01")
            return matching_result(request)

        verifier = LoopbackJsonRpcServer(verifier_result)
        self.addCleanup(primary.close)
        self.addCleanup(verifier.close)
        adapter = EvmAdapter(
            TESTNETS["base-sepolia"],
            CrossCheckingJsonRpcClient(JsonRpcClient(primary.url), JsonRpcClient(verifier.url)),
            manager,
        )

        with self.assertRaisesRegex(JsonRpcError, "different results"):
            adapter.broadcast(
                prepared,
                record=record,
                calldata_hex=CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
                signature_response=signature,
                signer_address=sender,
                expected_effect=EFFECT,
            )
        self.assertFalse(any(request["method"] == "eth_sendRawTransaction" for request in primary.calls))
        self.assertFalse(any(request["method"] == "eth_sendRawTransaction" for request in verifier.calls))

    def test_expected_effect_hash_is_canonical_and_matches_approved_request(self):
        reordered = dict(reversed(list(EFFECT.items())))
        self.assertEqual(hash_expected_effect(EFFECT), hash_expected_effect(reordered))
        self.assertEqual(hash_expected_effect(EFFECT), REQUEST["expected_effect_hash"])

    def test_event_effect_verification_requires_committed_log_match(self):
        receipt = {
            "status": "0x1",
            "logs": [{
                "address": EFFECT["address"],
                "topics": EFFECT["topics"],
                "data": EFFECT["data"],
            }]
        }
        self.assertTrue(verify_expected_effect(EFFECT, receipt))
        receipt["logs"][0]["data"] = "0x01"
        self.assertFalse(verify_expected_effect(EFFECT, receipt))

    def test_signed_type2_transaction_recovers_expected_signer(self):
        adapter = EvmAdapter(TESTNETS["base-sepolia"], FakeRpc({}), nonce_manager=None)
        digest = prepare_approved_eip1559_signing_payload(
            {
                "status": "Approved",
                "action_hash": FIXTURE["expected_action_hash"],
                "policy_hash": FIXTURE["policy_hash"],
                "expires_at_ns": FIXTURE["expires_at_ns"],
                "request": REQUEST,
            },
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
        )
        signature = _chainsig_response(digest)
        sender = "0x" + keys.PrivateKey((1).to_bytes(32, "big")).public_key.to_canonical_address().hex()
        raw = encode_signed_eip1559(REQUEST, CALLDATA, signature, sender)
        self.assertTrue(raw.startswith("0x02"))
        decoded = rlp.decode(bytes.fromhex(raw[4:]), strict=True)
        self.assertEqual(len(decoded), 12)
        self.assertEqual(int.from_bytes(decoded[0], "big"), int(REQUEST["chain_id"]))
        self.assertEqual(int.from_bytes(decoded[1], "big"), int(REQUEST["nonce"]))
        self.assertEqual(decoded[4], int(REQUEST["gas_limit"]).to_bytes(3, "big"))
        self.assertEqual(decoded[5], bytes.fromhex(REQUEST["target"][2:]))
        self.assertEqual(decoded[7], bytes.fromhex(CALLDATA[2:]))
        self.assertEqual(decoded[8], [])
        self.assertEqual(
            "0x" + keccak(bytes([2]) + rlp.encode(decoded[:9])).hex(),
            REQUEST["signing_payload_hash"],
        )
        self.assertEqual(adapter.transaction_hash(raw), "0x" + keccak(bytes.fromhex(raw[2:])).hex())

    def test_approved_request_reserves_its_committed_pending_nonce(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        sender = "0x" + keys.PrivateKey((1).to_bytes(32, "big")).public_key.to_canonical_address().hex()
        record = {
            "status": "Approved",
            "action_hash": FIXTURE["expected_action_hash"],
            "policy_hash": FIXTURE["policy_hash"],
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }
        rpc = FakeRpc({
            "eth_chainId": "0x14a34",
            "eth_getTransactionCount": "0x0",
        })
        adapter = EvmAdapter(TESTNETS["base-sepolia"], rpc, manager)

        nonce = adapter.reserve_approved_nonce(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
            signer_address=sender,
        )

        self.assertEqual(nonce, int(REQUEST["nonce"]))
        self.assertEqual(manager.get(REQUEST["request_id"])["state"], "reserved")

    def test_approved_transaction_flows_through_broadcast_finality_and_effect_check(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        sender = "0x" + keys.PrivateKey((1).to_bytes(32, "big")).public_key.to_canonical_address().hex()
        rpc = FakeRpc({
            "eth_chainId": "0x14a34",
            "eth_getTransactionCount": "0x0",
            "eth_getBlockByNumber": lambda params: (
                {"baseFeePerGas": "0x1", "number": "0x10"}
                if params[0] == "latest"
                else {"number": "0x10", "hash": "0x" + "cc" * 32}
            ),
            "eth_maxPriorityFeePerGas": "0xf4240",
            "eth_call": "0x",
            "eth_estimateGas": "0x5208",
            "eth_getTransactionReceipt": {
                "blockHash": "0x" + "cc" * 32,
                "status": "0x1",
                "blockNumber": "0x10",
                "logs": [{
                    "address": EFFECT["address"],
                    "topics": EFFECT["topics"],
                    "data": EFFECT["data"],
                }],
            },
        })
        adapter = EvmAdapter(TESTNETS["base-sepolia"], rpc, manager)
        record = {
            "status": "Approved",
            "action_hash": FIXTURE["expected_action_hash"],
            "policy_hash": FIXTURE["policy_hash"],
            "expires_at_ns": FIXTURE["expires_at_ns"],
            "request": REQUEST,
        }
        self.assertEqual(
            adapter.reserve_approved_nonce(
                record,
                CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
                signer_address=sender,
            ),
            int(REQUEST["nonce"]),
        )
        digest = prepare_approved_eip1559_signing_payload(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
        )
        signature = _chainsig_response(digest)
        raw = encode_signed_eip1559(REQUEST, CALLDATA, signature, sender, digest=digest)
        rpc.responses["eth_getTransactionReceipt"]["transactionHash"] = adapter.transaction_hash(raw)
        rpc.responses["eth_sendRawTransaction"] = lambda params: adapter.transaction_hash(params[0])

        prepared = adapter.prepare_approved_transaction(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
            signature_response=signature,
            signer_address=sender,
            expected_effect=EFFECT,
        )
        resumed = adapter.prepare_approved_transaction(
            record,
            CALLDATA,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
            signature_response=signature,
            signer_address=sender,
            expected_effect=EFFECT,
        )
        self.assertEqual(resumed.raw_transaction, prepared.raw_transaction)
        self.assertEqual(prepared.raw_transaction, raw)
        with self.assertRaisesRegex(ValueError, "(?i)approved"):
            adapter.broadcast(
                prepared,
                record={**record, "status": "Cancelled"},
                calldata_hex=CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
                signature_response=signature,
                signer_address=sender,
                expected_effect=EFFECT,
            )
        self.assertFalse(any(method == "eth_sendRawTransaction" for method, _ in rpc.calls))
        self.assertEqual(
            adapter.broadcast(
                prepared,
                record=record,
                calldata_hex=CALLDATA,
                active_policy=ACTIVE_POLICY,
                near_block_timestamp_ns=1,
                signature_response=signature,
                signer_address=sender,
                expected_effect=EFFECT,
            ),
            prepared.transaction_hash,
        )
        finality = adapter.check_finality(prepared.transaction_hash)
        self.assertTrue(finality.finalized)
        self.assertTrue(adapter.finalize_and_verify_effect(prepared, EFFECT))
        self.assertTrue(adapter.finalize_and_verify_effect(prepared, EFFECT))
        self.assertEqual(manager.get(REQUEST["request_id"])["state"], "confirmed")

    def test_signed_type2_transaction_rejects_wrong_derived_address(self):
        digest = "0x" + "01" * 32
        signature = _chainsig_response(digest)
        with self.assertRaisesRegex(ValueError, "does not match"):
            encode_signed_eip1559(REQUEST, CALLDATA, signature, "0x" + "22" * 20)

    def test_signed_type2_transaction_rejects_signature_response_extras(self):
        signature = _chainsig_response(REQUEST["signing_payload_hash"])
        signature["unexpected"] = True
        with self.assertRaisesRegex(ValueError, "response shape"):
            encode_signed_eip1559(REQUEST, CALLDATA, signature, "0x" + "7e5f4552091a69125d5dfcb7b8c2659029395bdf")

    def test_nonce_manager_creates_database_with_owner_only_permissions(self):
        with tempfile.TemporaryDirectory() as directory:
            database_path = Path(directory) / "nonce.sqlite"
            previous_umask = os.umask(0o022)
            try:
                manager = NonceManager(str(database_path))
            finally:
                os.umask(previous_umask)
            try:
                self.assertEqual(stat.S_IMODE(database_path.stat().st_mode), 0o600)
                manager.reserve_exact("84532", "0x" + "11" * 20, "req", 0, 0)
                sidecars = [
                    Path(f"{database_path}{suffix}")
                    for suffix in ("-wal", "-shm")
                    if Path(f"{database_path}{suffix}").exists()
                ]
                self.assertTrue(sidecars)
                for sidecar in sidecars:
                    self.assertEqual(stat.S_IMODE(sidecar.stat().st_mode) & 0o077, 0)
            finally:
                manager.close()

    def test_nonce_manager_rejects_existing_database_with_unsafe_permissions(self):
        with tempfile.TemporaryDirectory() as directory:
            database_path = Path(directory) / "nonce.sqlite"
            database_path.touch(mode=0o600)
            database_path.chmod(0o644)

            with self.assertRaisesRegex(PermissionError, "owner-only"):
                NonceManager(str(database_path))

    def test_nonce_manager_rejects_group_writable_parent_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            unsafe_directory = Path(directory) / "shared"
            unsafe_directory.mkdir()
            unsafe_directory.chmod(0o770)

            with self.assertRaisesRegex(PermissionError, "directory"):
                NonceManager(str(unsafe_directory / "nonce.sqlite"))

    def test_nonce_manager_rejects_symlinked_database_path(self):
        with tempfile.TemporaryDirectory() as directory:
            target_path = Path(directory) / "target.sqlite"
            link_path = Path(directory) / "nonce.sqlite"
            target_path.touch(mode=0o600)
            link_path.symlink_to(target_path)

            with self.assertRaisesRegex(PermissionError, "symlink"):
                NonceManager(str(link_path))

    def test_nonce_manager_persists_reservation_and_state_transitions(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        nonce = manager.reserve_next("84532", "0x" + "11" * 20, "req-1", 7)
        self.assertEqual(nonce, 7)
        self.assertEqual(manager.reserve_next("84532", "0x" + "11" * 20, "req-1", 8), 7)
        self.assertEqual(manager.reserve_next("84532", "0x" + "11" * 20, "req-2", 7), 8)
        manager.mark_signed("req-1", "0x02abcd")
        manager.mark_broadcast("req-1", "0x" + "aa" * 32)
        manager.mark_confirmed("req-1", "0x" + "aa" * 32)
        self.assertEqual(manager.get("req-1")["state"], "confirmed")

    def test_nonce_manager_backup_restore_preserves_lease_state_and_signed_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            source_path = Path(directory) / "source.sqlite"
            backup_path = Path(directory) / "backup.sqlite"
            restored_path = Path(directory) / "restored.sqlite"
            backup_path.touch(mode=0o600)
            restored_path.touch(mode=0o600)
            sender = "0x" + "11" * 20
            source = NonceManager(str(source_path))
            source.reserve_exact("84532", sender, "req-broadcast", 7, 7)
            broadcast_bytes = "0x02" + "ab" * 80
            broadcast_hash = "0x" + "cd" * 32
            source.mark_signed("req-broadcast", broadcast_bytes)
            source.mark_broadcast("req-broadcast", broadcast_hash)
            source.reserve_exact("84532", sender, "req-confirmed", 8, 8)
            confirmed_bytes = "0x02" + "ef" * 80
            confirmed_hash = "0x" + "12" * 32
            source.mark_signed("req-confirmed", confirmed_bytes)
            source.mark_broadcast("req-confirmed", confirmed_hash)
            source.mark_confirmed("req-confirmed", confirmed_hash)

            backup_connection = sqlite3.connect(str(backup_path))
            try:
                source.backup_to(backup_connection)
            finally:
                backup_connection.close()
                source.close()

            with closing(sqlite3.connect(str(backup_path))) as backup_connection, closing(
                sqlite3.connect(str(restored_path))
            ) as restored_connection:
                backup_connection.backup(restored_connection)

            restored = NonceManager(str(restored_path))
            try:
                broadcast = restored.get("req-broadcast")
                if broadcast is None:
                    self.fail("broadcast lease was not restored")
                self.assertEqual(broadcast["state"], "broadcast")
                self.assertEqual(broadcast["raw_transaction"], broadcast_bytes)
                self.assertEqual(broadcast["transaction_hash"], broadcast_hash)

                confirmed = restored.get("req-confirmed")
                if confirmed is None:
                    self.fail("confirmed lease was not restored")
                self.assertEqual(confirmed["state"], "confirmed")
                self.assertEqual(confirmed["raw_transaction"], confirmed_bytes)
                self.assertEqual(confirmed["transaction_hash"], confirmed_hash)
                self.assertEqual(restored.reserve_next("84532", sender, "req-next", 7), 9)
            finally:
                restored.close()

    def test_nonce_manager_backup_rejects_unsafe_destination_connections(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        with self.assertRaisesRegex(ValueError, "separate database connection"):
            manager.backup_to(manager._connection)

        destination = sqlite3.connect(":memory:")
        self.addCleanup(destination.close)
        destination.execute("BEGIN")
        with self.assertRaisesRegex(ValueError, "active transaction"):
            manager.backup_to(destination)

    def test_confirmed_nonce_is_never_released_for_reuse(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        manager.reserve_next("84532", "0x" + "11" * 20, "req-1", 7)
        manager.mark_signed("req-1", "0x02abcd")
        manager.mark_broadcast("req-1", "0x" + "aa" * 32)
        manager.mark_confirmed("req-1", "0x" + "aa" * 32)
        self.assertEqual(
            manager.reserve_next("84532", "0x" + "11" * 20, "req-2", 7),
            8,
        )

    def test_adapter_fails_closed_on_wrong_rpc_chain_id(self):
        adapter = EvmAdapter(
            TESTNETS["gnosis-chiado"],
            FakeRpc({"eth_chainId": "0x14a34"}),
            nonce_manager=None,
        )
        with self.assertRaisesRegex(ValueError, "configured chain ID"):
            adapter.assert_network()

    def test_fee_estimation_rejects_market_fees_above_approved_caps(self):
        rpc = FakeRpc({
            "eth_getBlockByNumber": {"baseFeePerGas": "0x3b9aca00"},
            "eth_maxPriorityFeePerGas": "0x989680",
            "eth_chainId": "0x14a34",
        })
        adapter = EvmAdapter(TESTNETS["base-sepolia"], rpc, nonce_manager=None)
        request = dict(REQUEST, gas_policy={"max_fee": "100000000", "max_priority_fee": "1000000"})
        with self.assertRaisesRegex(ValueError, "approved fee cap"):
            adapter.estimate_fees(request)

    def test_simulation_uses_committed_fields_and_rejects_excess_gas(self):
        rpc = FakeRpc({
            "eth_chainId": "0x14a34",
            "eth_call": "0x",
            "eth_estimateGas": "0x1d4c1",
        })
        adapter = EvmAdapter(TESTNETS["base-sepolia"], rpc, nonce_manager=None)
        with self.assertRaisesRegex(ValueError, "approved gas limit"):
            adapter.simulate(REQUEST, CALLDATA, "0x" + "11" * 20)
        call = next(params for method, params in rpc.calls if method == "eth_call")
        self.assertEqual(call[0]["to"], REQUEST["target"])
        self.assertEqual(call[0]["data"], CALLDATA)
        self.assertEqual(call[0]["gas"], hex(int(REQUEST["gas_limit"])))
        self.assertEqual(call[0]["maxFeePerGas"], hex(int(REQUEST["gas_policy"]["max_fee"])))
        self.assertEqual(
            call[0]["maxPriorityFeePerGas"],
            hex(int(REQUEST["gas_policy"]["max_priority_fee"])),
        )
        self.assertEqual(call[0]["nonce"], hex(int(REQUEST["nonce"])))
        self.assertEqual(call[0]["chainId"], hex(int(REQUEST["chain_id"])))

    def test_finality_rejects_receipt_for_different_transaction(self):
        transaction_hash = "0x" + "ab" * 32
        rpc = FakeRpc({
            "eth_chainId": "0x14a34",
            "eth_getTransactionReceipt": {
                "transactionHash": "0x" + "cd" * 32,
                "status": "0x1",
                "blockNumber": "0x10",
                "logs": [],
            },
            "eth_getBlockByNumber": lambda params: {"number": "0x10"},
        })
        adapter = EvmAdapter(TESTNETS["base-sepolia"], rpc, nonce_manager=None)

        with self.assertRaisesRegex(ValueError, "receipt transaction hash"):
            adapter.check_finality(transaction_hash)

    def test_finality_does_not_accept_a_noncanonical_receipt_block(self):
        transaction_hash = "0x" + "ab" * 32
        rpc = FakeRpc({
            "eth_chainId": "0x14a34",
            "eth_getTransactionReceipt": {
                "transactionHash": transaction_hash,
                "blockHash": "0x" + "11" * 32,
                "status": "0x1",
                "blockNumber": "0x10",
                "logs": [],
            },
            "eth_getBlockByNumber": lambda params: (
                {"number": "0x10", "hash": "0x" + "22" * 32}
            ),
        })
        adapter = EvmAdapter(TESTNETS["base-sepolia"], rpc, nonce_manager=None)

        result = adapter.check_finality(transaction_hash)

        self.assertEqual(result.state, "reorged")
        self.assertFalse(result.finalized)

    def test_unfinalized_observation_cannot_confirm_nonce_lease(self):
        manager = NonceManager(":memory:")
        self.addCleanup(manager.close)
        sender = "0x" + "11" * 20
        request_id = "chiado-unfinalized-test"
        transaction_hash = "0x" + "ab" * 32
        manager.reserve_exact("10200", sender, request_id, 0, 0)
        manager.mark_signed(request_id, "0x02abcd")
        manager.mark_broadcast(request_id, transaction_hash)
        receipt = {
            "transactionHash": transaction_hash,
            "blockHash": "0x" + "aa" * 32,
            "status": "0x1",
            "blockNumber": "0x10",
            "logs": [{
                "address": EFFECT["address"],
                "topics": EFFECT["topics"],
                "data": EFFECT["data"],
            }],
        }
        rpc = FakeRpc({
            "eth_chainId": "0x27d8",
            "eth_getTransactionReceipt": receipt,
            "eth_getBlockByNumber": lambda params: (
                None
                if params[0] == "finalized"
                else {"number": "0x10", "hash": "0x" + "aa" * 32}
                if params[0] == "0x10"
                else {"number": "0x2f"}
            ),
        })
        adapter = EvmAdapter(TESTNETS["gnosis-chiado"], rpc, manager)
        prepared = PreparedTransaction(
            request_id, 10200, 0, sender, "0x02abcd", transaction_hash, hash_expected_effect(EFFECT)
        )
        self.assertFalse(adapter.finalize_and_verify_effect(prepared, EFFECT))
        self.assertEqual(manager.get(request_id)["state"], "broadcast")

    def test_chiado_finality_fallback_requires_configured_depth(self):
        transaction_hash = "0x" + "ab" * 32
        blocks = {"latest_number": "0x2e"}

        def block_response(params):
            if params[0] == "finalized":
                return None
            if params[0] == "0x10":
                return {"number": "0x10", "hash": "0x" + "aa" * 32}
            return {"number": blocks["latest_number"]}

        rpc = FakeRpc({
            "eth_chainId": "0x27d8",
            "eth_getTransactionReceipt": {
                "transactionHash": transaction_hash,
                "blockHash": "0x" + "aa" * 32,
                "status": "0x1",
                "blockNumber": "0x10",
                "logs": [],
            },
            "eth_getBlockByNumber": block_response,
        })
        adapter = EvmAdapter(TESTNETS["gnosis-chiado"], rpc, nonce_manager=None)
        pending = adapter.check_finality(transaction_hash)
        self.assertEqual(pending.state, "confirming")
        self.assertEqual(pending.confirmations, 31)

        blocks["latest_number"] = "0x2f"
        finalized = adapter.check_finality(transaction_hash)
        self.assertEqual(finalized.state, "confirmation_threshold_met")
        self.assertFalse(finalized.finalized)
        self.assertEqual(finalized.confirmations, 32)

    def test_storage_and_code_effect_verification_uses_receipt_block_state(self):
        receipt = {"status": "0x1", "blockNumber": "0x10", "logs": []}
        address = "0x" + "12" * 20
        slot = "0x" + "34" * 32
        value = "0x" + "56" * 32
        storage_rpc = FakeRpc({"eth_getStorageAt": value})
        self.assertTrue(
            verify_expected_effect(
                {"kind": "storage", "address": address, "slot": slot, "value": value},
                receipt,
                rpc=storage_rpc,
            )
        )
        self.assertEqual(storage_rpc.calls[0][1][2], "0x10")

        code = "0x6000"
        code_effect = {
            "kind": "code",
            "address": address,
            "code_hash": "0x" + keccak(bytes.fromhex("6000")).hex(),
        }
        self.assertTrue(
            verify_expected_effect(code_effect, receipt, rpc=FakeRpc({"eth_getCode": code}))
        )

    def test_event_effect_verification_rejects_wrong_topic(self):
        receipt = {"status": "0x1", "logs": [{
            "address": EFFECT["address"],
            "topics": ["0x" + "f" * 64],
            "data": EFFECT["data"],
        }]}
        self.assertFalse(verify_expected_effect(EFFECT, receipt))


if __name__ == "__main__":
    unittest.main()
