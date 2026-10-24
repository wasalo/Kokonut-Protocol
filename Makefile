PYTHON ?= python3

.PHONY: lint test lint-near test-near build-near lint-evm test-evm audit-evm audit-aderyn audit-semgrep ci

lint:
	$(PYTHON) -m compileall -q tests protocol
	$(PYTHON) -c 'import json, pathlib; [json.loads(p.read_text()) for p in pathlib.Path("specs").rglob("*.json")]'

test:
	$(PYTHON) -m unittest discover -s tests -v

lint-near:
	cd contracts/near && cargo fmt --check && cargo near check --locked

test-near:
	cd contracts/near && cargo test --locked

build-near:
	cd contracts/near && cargo near build non-reproducible-wasm --locked --out-dir target/deploy

lint-evm:
	cd contracts/evm && forge fmt --check && forge build --sizes

test-evm:
	cd contracts/evm && forge test

audit-evm:
	cd contracts/evm && slither . --foundry-out-directory out

# Keep the constructor nonpayable: accepting ETH is not part of this factory's API.
# The upstream performance-only rule recommends payable constructors for gas savings.
audit-semgrep:
	SEMGREP_SEND_METRICS=off SEMGREP_ENABLE_VERSION_CHECK=0 semgrep scan --config p/smart-contracts --exclude-rule solidity.performance.non-payable-constructor.non-payable-constructor --error contracts/evm/src

audit-aderyn: lint-evm
	cd contracts/evm && aderyn . --highs-only --output out/aderyn-high.json
	$(PYTHON) -c 'import json, pathlib; report=json.loads(pathlib.Path("contracts/evm/out/aderyn-high.json").read_text()); count=report["issue_count"]["high"]; print(f"Aderyn high-severity findings: {count}"); raise SystemExit(count != 0)'

ci: lint test lint-near test-near build-near lint-evm test-evm audit-evm audit-semgrep audit-aderyn
