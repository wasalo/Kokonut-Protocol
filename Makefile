PYTHON ?= python3

.PHONY: lint test lint-near test-near build-near lint-evm test-evm ci

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

ci: lint test lint-near test-near build-near lint-evm test-evm
