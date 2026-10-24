#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected_digest="$(< "$repo_root/contracts/evm/.semgrep-rules.sha256")"
temporary_root="${TMPDIR:-${RUNNER_TEMP:-}}"
if [[ -z "$temporary_root" || ! -d "$temporary_root" ]]; then
  printf '%s\n' 'Set TMPDIR or RUNNER_TEMP to a writable scratch directory.' >&2
  exit 1
fi
rules_file="$(mktemp "${temporary_root%/}/kokonut-smart-contracts.XXXXXX.yml")"
trap 'rm -f "$rules_file"' EXIT

curl --proto '=https' --tlsv1.2 -fsSL \
  https://semgrep.dev/c/p/smart-contracts \
  -o "$rules_file"

if ! printf '%s  %s\n' "$expected_digest" "$rules_file" | sha256sum --check --status; then
  printf '%s\n' \
    'Semgrep ruleset digest changed; review the upstream pack before updating the pin.' \
    >&2
  exit 1
fi

SEMGREP_SEND_METRICS=off SEMGREP_ENABLE_VERSION_CHECK=0 \
  "${SEMGREP:-semgrep}" scan \
    --config "$rules_file" \
    --no-rewrite-rule-ids \
    --exclude-rule solidity.performance.non-payable-constructor.non-payable-constructor \
    --error "$repo_root/contracts/evm/src"
