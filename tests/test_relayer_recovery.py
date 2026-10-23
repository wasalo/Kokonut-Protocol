import json
from pathlib import Path
import tempfile
import unittest

from eth_hash.auto import keccak
from eth_keys import keys

from protocol.evm_adapter import EvmAdapter, JsonRpcError, NonceManager, TESTNETS
from protocol.relayer import ExecutionRelayer, RelayerSnapshot

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
PRIVATE_KEY = keys.PrivateKey((1).to_bytes(32, "big"))
SIGNER_ADDRESS = "0x" + PRIVATE_KEY.public_key.to_canonical_address().hex()
BLOCK_HASH = "0x" + "cc" * 32


def chainsig_response(digest_hex):
    signature = PRIVATE_KEY.sign_msg_hash(bytes.fromhex(digest_hex[2:]))
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
    def __init__(self):
        self.calls = []
        self.broadcast_attempts = 0
        self.known_transaction_hash = None
        self.known_raw_transaction = None
        self.drop_next_broadcast_ack = False

    def call(self, method, params):
        self.calls.append((method, params))
        if method == "eth_chainId":
            return hex(TESTNETS["base-sepolia"].chain_id)
        if method == "eth_getTransactionCount":
            return hex(int(REQUEST["nonce"]))
        if method == "eth_getBlockByNumber":
            if params[0] == "latest":
                return {"number": "0x10", "baseFeePerGas": "0x1"}
            if params[0] == "finalized":
                return {"number": "0x10", "hash": BLOCK_HASH}
            return {"number": "0x10", "hash": BLOCK_HASH}
        if method == "eth_maxPriorityFeePerGas":
            return "0xf4240"
        if method == "eth_call":
            return "0x"
        if method == "eth_estimateGas":
            return hex(21_000)
        if method == "eth_sendRawTransaction":
            self.broadcast_attempts += 1
            raw_transaction = params[0]
            raw = bytes.fromhex(raw_transaction[2:])
            transaction_hash = "0x" + keccak(raw).hex()
            if self.known_transaction_hash is not None:
                if (
                    transaction_hash != self.known_transaction_hash
                    or raw_transaction != self.known_raw_transaction
                ):
                    raise AssertionError("retry changed the already-submitted transaction")
                raise JsonRpcError(method, "already known")
            self.known_transaction_hash = transaction_hash
            self.known_raw_transaction = raw_transaction
            if self.drop_next_broadcast_ack:
                self.drop_next_broadcast_ack = False
                raise TimeoutError("injected lost broadcast acknowledgement")
            return transaction_hash
        if method == "eth_getTransactionReceipt":
            if params[0] != self.known_transaction_hash:
                return None
            return {
                "transactionHash": self.known_transaction_hash,
                "blockHash": BLOCK_HASH,
                "blockNumber": "0x10",
                "status": "0x1",
                "logs": [{
                    "address": EFFECT["address"],
                    "topics": EFFECT["topics"],
                    "data": EFFECT["data"],
                }],
            }
        raise AssertionError(f"unexpected JSON-RPC method: {method}")


class SnapshotReader:
    def __init__(self):
        self.read_count = 0

    def read(self, request_id):
        self.read_count += 1
        return RelayerSnapshot(
            record=RECORD,
            active_policy=ACTIVE_POLICY,
            near_block_timestamp_ns=1,
            calldata_hex=CALLDATA,
            signer_address=SIGNER_ADDRESS,
            expected_effect=EFFECT,
        )


class IssuedSignatureReader:
    def __init__(self):
        self.request_count = 0
        self.issue_count = 0
        self.issued = None

    def request_signature(self, request_id, action_hash, digest):
        self.request_count += 1
        if self.issued is None:
            self.issue_count += 1
            self.issued = chainsig_response(digest)
        return self.issued


class CrashBeforeBroadcast(EvmAdapter):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.crash_once = True

    def broadcast(self, *args, **kwargs):
        if self.crash_once:
            self.crash_once = False
            raise RuntimeError("injected crash after signed bytes were persisted")
        return super().broadcast(*args, **kwargs)


class CrashAfterEffectConfirmation(EvmAdapter):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.crash_once = True

    def finalize_and_verify_effect(self, prepared, expected_effect):
        verified = super().finalize_and_verify_effect(prepared, expected_effect)
        if verified and self.crash_once:
            self.crash_once = False
            raise RuntimeError("injected crash after durable confirmation")
        return verified


class RelayerRecoveryTests(unittest.TestCase):
    def test_crash_windows_recover_same_transaction_and_confirmed_replay_is_noop(self):
        rpc = FakeRpc()
        snapshots = SnapshotReader()
        signatures = IssuedSignatureReader()
        with tempfile.TemporaryDirectory() as directory:
            database_path = str(Path(directory) / "nonce-leases.sqlite")

            manager_after_signing = NonceManager(database_path)
            adapter_after_signing = CrashBeforeBroadcast(
                TESTNETS["base-sepolia"], rpc, manager_after_signing
            )
            relayer_after_signing = ExecutionRelayer(
                snapshots, signatures, adapter_after_signing
            )
            with self.assertRaisesRegex(RuntimeError, "signed bytes were persisted"):
                relayer_after_signing.process(REQUEST["request_id"])
            signed_lease = manager_after_signing.get(REQUEST["request_id"])
            if signed_lease is None:
                self.fail("signed nonce lease was not persisted before the crash")
            self.assertEqual(signed_lease["state"], "signed")
            raw_transaction = signed_lease["raw_transaction"]
            self.assertIsNone(signed_lease["transaction_hash"])
            self.assertEqual(rpc.broadcast_attempts, 0)
            manager_after_signing.close()

            rpc.drop_next_broadcast_ack = True
            manager_after_timeout = NonceManager(database_path)
            try:
                adapter_after_timeout = EvmAdapter(
                    TESTNETS["base-sepolia"], rpc, manager_after_timeout
                )
                relayer_after_timeout = ExecutionRelayer(
                    snapshots, signatures, adapter_after_timeout
                )
                with self.assertRaisesRegex(TimeoutError, "lost broadcast acknowledgement"):
                    relayer_after_timeout.process(REQUEST["request_id"])
                timeout_lease = manager_after_timeout.get(REQUEST["request_id"])
                if timeout_lease is None:
                    self.fail("nonce lease disappeared after ambiguous broadcast")
                self.assertEqual(timeout_lease["state"], "signed")
                self.assertEqual(timeout_lease["raw_transaction"], raw_transaction)
                self.assertIsNone(timeout_lease["transaction_hash"])
                self.assertIsNotNone(rpc.known_transaction_hash)
                self.assertEqual(rpc.broadcast_attempts, 1)
            finally:
                manager_after_timeout.close()

            manager_after_confirmation = NonceManager(database_path)
            try:
                adapter_after_confirmation = CrashAfterEffectConfirmation(
                    TESTNETS["base-sepolia"], rpc, manager_after_confirmation
                )
                relayer_after_confirmation = ExecutionRelayer(
                    snapshots, signatures, adapter_after_confirmation
                )
                with self.assertRaisesRegex(RuntimeError, "after durable confirmation"):
                    relayer_after_confirmation.process(REQUEST["request_id"])
                confirmed_lease = manager_after_confirmation.get(REQUEST["request_id"])
                if confirmed_lease is None:
                    self.fail("confirmed nonce lease was not persisted before the crash")
                self.assertEqual(confirmed_lease["state"], "confirmed")
                self.assertEqual(confirmed_lease["raw_transaction"], raw_transaction)
                transaction_hash = confirmed_lease["transaction_hash"]
                self.assertEqual(transaction_hash, rpc.known_transaction_hash)
                self.assertEqual(rpc.known_raw_transaction, raw_transaction)
                self.assertEqual(rpc.broadcast_attempts, 2)
            finally:
                manager_after_confirmation.close()

            manager_after_restart = NonceManager(database_path)
            try:
                adapter_after_restart = EvmAdapter(
                    TESTNETS["base-sepolia"], rpc, manager_after_restart
                )
                relayer_after_restart = ExecutionRelayer(
                    snapshots, signatures, adapter_after_restart
                )
                result = relayer_after_restart.process(REQUEST["request_id"])

                self.assertEqual(result.state, "finalized")
                self.assertTrue(result.effect_verified)
                self.assertEqual(result.transaction_hash, transaction_hash)
                self.assertEqual(result.nonce, int(REQUEST["nonce"]))
                replayed_lease = manager_after_restart.get(REQUEST["request_id"])
                if replayed_lease is None:
                    self.fail("confirmed nonce lease disappeared after restart")
                self.assertEqual(replayed_lease["raw_transaction"], raw_transaction)
                self.assertEqual(rpc.broadcast_attempts, 2)
                self.assertEqual(signatures.issue_count, 1)
                self.assertEqual(signatures.request_count, 3)
                self.assertEqual(snapshots.read_count, 7)
            finally:
                manager_after_restart.close()


if __name__ == "__main__":
    unittest.main()
