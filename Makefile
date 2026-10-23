PYTHON ?= python3

.PHONY: lint test ci

lint:
	$(PYTHON) -m compileall -q tests
	$(PYTHON) -c 'import json, pathlib; [json.loads(p.read_text()) for p in pathlib.Path("specs").rglob("*.json")]'

test:
	$(PYTHON) -m unittest discover -s tests -v

ci: lint test
