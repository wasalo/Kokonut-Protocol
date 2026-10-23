PYTHON ?= python3

.PHONY: lint test lint-near test-near build-near ci

lint:
	$(PYTHON) -m compileall -q tests
	$(PYTHON) -c 'import json, pathlib; [json.loads(p.read_text()) for p in pathlib.Path("specs").rglob("*.json")]'

test:
	$(PYTHON) -m unittest discover -s tests -v

lint-near:
	cd contracts/near && cargo fmt --check

test-near:
	cd contracts/near && cargo test --locked

build-near:
	cd contracts/near && cargo build --locked --release --target wasm32-unknown-unknown

ci: lint test lint-near test-near build-near
