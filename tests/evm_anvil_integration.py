"""Local-only integration of the EVM adapter against two isolated Anvil nodes."""

from __future__ import annotations

import copy
import hashlib
import json
import shutil
import socket
import subprocess
import sys
import time
import unittest
import uuid
from pathlib import Path

import rlp
from eth_hash.auto import keccak
from eth_keys import keys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from test_evm_adapter import _chainsig_response

from protocol.evm_adapter import (
    TESTNETS,
    EvmAdapter,
    JsonRpcClient,
    JsonRpcError,
    NonceManager,
    hash_expected_effect,
    verify_expected_effect,
)
from protocol.evm_signing_payload import prepare_approved_eip1559_signing_payload

PRIVATE_KEY = keys.PrivateKey((1).to_bytes(32, "big"))  # deterministic local-only test key
SIGNER = "0x" + PRIVATE_KEY.public_key.to_canonical_address().hex()
POLICY_HASH = "0x" + "cc" * 32
PROJECT_ID = "018f4f7e-10bd-7d98-9b23-9ec89a1d9c3b"
REQUEST_FIELDS = (
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
    "signer_key_version",
    "expected_effect_hash",
    "nonce",
    "gas_policy",
    "expires_at",
    "policy_version",
    "idempotency_key",
)


class RecordingRpc:
    def __init__(self, endpoint: str):
        self.client = JsonRpcClient(endpoint, timeout_seconds=1.0)
        self.calls: list[str] = []

    def call(self, method: str, params: list):
        self.calls.append(method)
        return self.client.call(method, params)


def _unused_loopback_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def _int_bytes(value: int) -> bytes:
    return b"" if value == 0 else value.to_bytes((value.bit_length() + 7) // 8, "big")


def _signing_digest(request: dict, calldata: bytes) -> str:
    fields = [
        _int_bytes(int(request["chain_id"])),
        _int_bytes(int(request["nonce"])),
        _int_bytes(int(request["gas_policy"]["max_priority_fee"])),
        _int_bytes(int(request["gas_policy"]["max_fee"])),
        _int_bytes(int(request["gas_limit"])),
        bytes.fromhex(request["target"][2:]),
        _int_bytes(int(request["value_base_units"])),
        calldata,
        [],
    ]
    return "0x" + keccak(b"\x02" + rlp.encode(fields)).hex()


def _action_hash(request: dict) -> str:
    ordered = {field: request[field] for field in REQUEST_FIELDS}
    ordered["gas_policy"] = {
        "max_fee": request["gas_policy"]["max_fee"],
        "max_priority_fee": request["gas_policy"]["max_priority_fee"],
    }
    encoded = json.dumps(ordered, separators=(",", ":"), ensure_ascii=False, allow_nan=False)
    return "0x" + hashlib.sha256(encoded.encode("utf-8")).hexdigest()


class TwoChainAnvilIntegrationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        anvil = shutil.which("anvil")
        if anvil is None:
            raise unittest.SkipTest("Anvil is not installed; run the dedicated Make target with Foundry available")

        artifact_path = ROOT / "contracts/evm/out/AnvilEffectTarget.sol/AnvilEffectTarget.json"
        if not artifact_path.is_file():
            raise RuntimeError("missing local fixture artifact; run make lint-evm before this integration test")
        artifact = json.loads(artifact_path.read_text())
        cls.creation_bytecode = artifact["bytecode"]["object"]
        if not isinstance(cls.creation_bytecode, str) or not cls.creation_bytecode.startswith("0x"):
            raise RuntimeError("Anvil fixture artifact has no creation bytecode")

        cls.processes: list[subprocess.Popen] = []
        cls.rpcs: dict[str, RecordingRpc] = {}
        try:
            for name, network in (
                ("base-sepolia", TESTNETS["base-sepolia"]),
                ("gnosis-chiado", TESTNETS["gnosis-chiado"]),
            ):
                port = _unused_loopback_port()
                process = subprocess.Popen(
                    [
                        anvil,
                        "--host",
                        "127.0.0.1",
                        "--port",
                        str(port),
                        "--chain-id",
                        str(network.chain_id),
                        "--silent",
                    ],
                    cwd=ROOT / "contracts/evm",
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                )
                cls.processes.append(process)
                endpoint = f"http://127.0.0.1:{port}"
                rpc = RecordingRpc(endpoint)
                cls.rpcs[name] = rpc
                deadline = time.monotonic() + 15
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise RuntimeError(f"local Anvil process for {name} exited during startup")
                    try:
                        chain_id = int(rpc.call("eth_chainId", []), 16)
                    except JsonRpcError:
                        time.sleep(0.1)
                        continue
                    if chain_id != network.chain_id:
                        raise RuntimeError(f"local Anvil {name} reported chain ID {chain_id}")
                    break
                else:
                    raise RuntimeError(f"local Anvil {name} did not become ready")
        except Exception:
            cls.tearDownClass()
            raise

    @classmethod
    def tearDownClass(cls):
        for process in getattr(cls, "processes", []):
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)

    def _deploy_target(self, rpc: RecordingRpc) -> str:
        accounts = rpc.call("eth_accounts", [])
        self.assertGreaterEqual(len(accounts), 1)
        tx_hash = rpc.call(
            "eth_sendTransaction",
            [{"from": accounts[0], "data": self.creation_bytecode, "gas": hex(2_000_000)}],
        )
        deadline = time.monotonic() + 10
        receipt = None
        while time.monotonic() < deadline:
            receipt = rpc.call("eth_getTransactionReceipt", [tx_hash])
            if receipt is not None:
                break
            time.sleep(0.05)
        self.assertIsNotNone(receipt, "local fixture deployment did not receive a receipt")
        address = receipt.get("contractAddress")
        self.assertIsInstance(address, str)
        self.assertEqual(len(address), 42)
        return address

    def _execution(self, name: str, network_name: str, chain_id: int, manager: NonceManager):
        rpc = self.rpcs[name]
        rpc.call("anvil_setBalance", [SIGNER, hex(100 * 10**18)])
        target = self._deploy_target(rpc)
        request_uuid = str(uuid.UUID(int=chain_id))
        request_id_bytes = uuid.UUID(request_uuid).bytes + (b"\x00" * 16)
        value = chain_id + 17
        selector = keccak(b"setValue(bytes32,uint256)")[:4]
        calldata = selector + request_id_bytes + value.to_bytes(32, "big")
        calldata_hex = "0x" + calldata.hex()
        event_topic = "0x" + keccak(b"ValueSet(bytes32,uint256)").hex()
        effect = {
            "kind": "event",
            "address": target,
            "topics": [event_topic, "0x" + request_id_bytes.hex()],
            "data": "0x" + value.to_bytes(32, "big").hex(),
        }

        request = copy.deepcopy(json.loads((ROOT / "specs/examples/evm-signing-payload.json").read_text())["request"])
        request.update(
            {
                "request_id": request_uuid,
                "project_id": PROJECT_ID,
                "chain_id": str(chain_id),
                "target": target,
                "operation": "set_local_value",
                "nonce": "0",
                "calldata_hash": "0x" + keccak(calldata).hex(),
                "expected_effect_hash": hash_expected_effect(effect),
                "idempotency_key": f"local-anvil-{network_name}",
                "gas_policy": {"max_fee": "100000000000", "max_priority_fee": "1000000000"},
            }
        )
        request["signing_payload_hash"] = _signing_digest(request, calldata)
        record = {
            "status": "Approved",
            "action_hash": _action_hash(request),
            "policy_hash": POLICY_HASH,
            "expires_at_ns": 2_051_222_400_000_000_000,
            "request": request,
        }
        policy = {
            "project_id": PROJECT_ID,
            "chain": {"namespace": "eip155", "chain_id": str(chain_id)},
            "version": request["policy_version"],
            "policy_hash": POLICY_HASH,
        }
        adapter = EvmAdapter(TESTNETS[network_name], rpc, manager)
        leased_nonce = adapter.reserve_approved_nonce(
            record,
            calldata_hex,
            active_policy=policy,
            near_block_timestamp_ns=1,
            signer_address=SIGNER,
        )
        self.assertEqual(leased_nonce, 0)
        digest = prepare_approved_eip1559_signing_payload(
            record,
            calldata_hex,
            active_policy=policy,
            near_block_timestamp_ns=1,
        )
        signature = _chainsig_response(digest)
        prepared = adapter.prepare_approved_transaction(
            record,
            calldata_hex,
            active_policy=policy,
            near_block_timestamp_ns=1,
            signature_response=signature,
            signer_address=SIGNER,
            expected_effect=effect,
        )
        return adapter, record, policy, calldata_hex, signature, effect, prepared, target, value

    def test_two_local_chains_execute_isolated_nonces_and_verify_effects(self):
        manager = NonceManager(":memory:")
        try:
            flows = []
            for name, network_name in (
                ("base-sepolia", "base-sepolia"),
                ("gnosis-chiado", "gnosis-chiado"),
            ):
                network = TESTNETS[network_name]
                flows.append((name, self._execution(name, network_name, network.chain_id, manager)))

            _, _, _, _, _, _, base_prepared, _, _ = flows[0][1]
            gnosis_adapter, gnosis_record, gnosis_policy, gnosis_calldata, gnosis_signature, gnosis_effect, _, _, _ = flows[1][1]
            with self.assertRaisesRegex(ValueError, "another chain"):
                gnosis_adapter.broadcast(
                    base_prepared,
                    record=gnosis_record,
                    calldata_hex=gnosis_calldata,
                    active_policy=gnosis_policy,
                    near_block_timestamp_ns=1,
                    signature_response=gnosis_signature,
                    signer_address=SIGNER,
                    expected_effect=gnosis_effect,
                )

            for name, values in flows:
                adapter, record, policy, calldata, signature, effect, prepared, target, expected_value = values
                rpc = self.rpcs[name]
                self.assertEqual(
                    adapter.broadcast(
                        prepared,
                        record=record,
                        calldata_hex=calldata,
                        active_policy=policy,
                        near_block_timestamp_ns=1,
                        signature_response=signature,
                        signer_address=SIGNER,
                        expected_effect=effect,
                    ),
                    prepared.transaction_hash,
                )
                finality = adapter.check_finality(prepared.transaction_hash)
                if finality.state != "finalized":
                    rpc.call("anvil_mine", [hex(40)])
                    finality = adapter.check_finality(prepared.transaction_hash)
                self.assertIn(finality.state, {"finalized", "confirmation_threshold_met"})

                receipt = rpc.call("eth_getTransactionReceipt", [prepared.transaction_hash])
                self.assertIsNotNone(receipt)
                self.assertTrue(verify_expected_effect(effect, receipt, rpc=rpc))
                if finality.state == "finalized":
                    self.assertTrue(adapter.finalize_and_verify_effect(prepared, effect))
                    expected_lease_state = "confirmed"
                else:
                    self.assertFalse(adapter.finalize_and_verify_effect(prepared, effect))
                    expected_lease_state = "broadcast"
                lease = manager.get(record["request"]["request_id"])
                if lease is None:
                    self.fail(f"{name} nonce lease disappeared")
                self.assertEqual(lease["state"], expected_lease_state)
                stored_value = rpc.call(
                    "eth_call",
                    [{"to": target, "data": "0x" + keccak(b"value()")[:4].hex()}, "latest"],
                )
                self.assertEqual(int(stored_value, 16), expected_value)
                self.assertEqual(
                    self.rpcs[name].calls.count("eth_sendRawTransaction"),
                    1,
                    f"{name} should broadcast exactly once",
                )

            base_lease = manager.get(flows[0][1][1]["request"]["request_id"])
            gnosis_lease = manager.get(flows[1][1][1]["request"]["request_id"])
            self.assertIsNotNone(base_lease)
            self.assertIsNotNone(gnosis_lease)
            self.assertEqual(base_lease["sender"], gnosis_lease["sender"])
            self.assertEqual(base_lease["nonce"], gnosis_lease["nonce"])
            self.assertNotEqual(base_lease["chain_id"], gnosis_lease["chain_id"])
        finally:
            manager.close()


if __name__ == "__main__":
    unittest.main(verbosity=2)
