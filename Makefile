PYTHON ?= python3
SEMGREP ?= semgrep
GITLEAKS ?=

.PHONY: lint test lint-near test-near build-near install-near-sandbox test-near-sandbox lint-evm test-evm test-evm-anvil audit-evm audit-aderyn audit-semgrep audit-secrets ci

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

install-near-sandbox:
	bash scripts/install-near-sandbox-pinned.sh

test-near-sandbox: build-near install-near-sandbox
	cd contracts/near && NEAR_SANDBOX_BIN_PATH="$(CURDIR)/contracts/near/target/ci-tools/near-sandbox" cargo test --locked --features sandbox-tests --test sandbox

lint-evm:
	cd contracts/evm && forge fmt --check && forge build --sizes

test-evm:
	cd contracts/evm && forge test

test-evm-anvil: lint-evm
	command -v anvil >/dev/null
	PYTHONPATH=.:tests $(PYTHON) tests/evm_anvil_integration.py -v

audit-evm:
	cd contracts/evm && slither . --foundry-out-directory out

# Keep the constructor nonpayable: accepting ETH is not part of this factory's API.
# The upstream performance-only rule recommends payable constructors for gas savings.
# The runner fetches the licensed public pack to a temporary file and fails closed
# if its reviewed digest changes; never vendor rule definitions in this repo.
audit-semgrep:
	SEMGREP="$(SEMGREP)" bash scripts/audit-semgrep-pinned.sh

audit-aderyn: lint-evm
	cd contracts/evm && aderyn . --highs-only --output out/aderyn-high.json
	$(PYTHON) -c 'import json, pathlib; report=json.loads(pathlib.Path("contracts/evm/out/aderyn-high.json").read_text()); count=report["issue_count"]["high"]; print(f"Aderyn high-severity findings: {count}"); raise SystemExit(count != 0)'

audit-secrets:
	GITLEAKS="$(GITLEAKS)" bash scripts/audit-gitleaks-pinned.sh

ci: lint test lint-near test-near build-near test-near-sandbox lint-evm test-evm test-evm-anvil audit-evm audit-semgrep audit-aderyn audit-secrets
