#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(git -C "$script_dir/.." rev-parse --show-toplevel)"
version_file="$repo_root/.gitleaks-version"
checksum_file="$repo_root/.gitleaks-linux-x86_64-sha256"
version="$(tr -d '\r\n' < "$version_file")"
expected_checksum="$(tr -d '\r\n' < "$checksum_file")"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf 'Invalid Gitleaks version pin.\n' >&2
  exit 2
fi
if [[ ! "$expected_checksum" =~ ^[0-9a-f]{64}$ ]]; then
  printf 'Invalid Gitleaks SHA-256 pin.\n' >&2
  exit 2
fi

tmp_base="${RUNNER_TEMP:-${TMPDIR:-}}"
if [[ -z "$tmp_base" || ! -d "$tmp_base" ]]; then
  printf 'Set RUNNER_TEMP or TMPDIR to an existing directory for Gitleaks temporary files.\n' >&2
  exit 2
fi
workdir="$(mktemp -d "${tmp_base%/}/kokonut-gitleaks.XXXXXX")"
trap 'rm -rf -- "$workdir"' EXIT

if [[ -n "${GITLEAKS:-}" ]]; then
  if [[ -x "$GITLEAKS" ]]; then
    scanner="$GITLEAKS"
  else
    scanner="$(command -v -- "$GITLEAKS" || true)"
  fi
  if [[ -z "$scanner" ]]; then
    printf 'Configured GITLEAKS executable was not found.\n' >&2
    exit 2
  fi
else
  if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
    printf 'Pinned Gitleaks bootstrap supports Linux x86_64; set GITLEAKS to a reviewed compatible binary elsewhere.\n' >&2
    exit 2
  fi
  archive_name="gitleaks_${version}_linux_x64.tar.gz"
  archive="$workdir/$archive_name"
  archive_url="https://github.com/gitleaks/gitleaks/releases/download/v${version}/${archive_name}"
  curl --proto '=https' --tlsv1.2 -fsSL "$archive_url" -o "$archive"
  printf '%s  %s\n' "$expected_checksum" "$archive" | sha256sum --check --status || {
    printf 'Gitleaks release archive checksum mismatch.\n' >&2
    exit 1
  }
  tar -xzf "$archive" -C "$workdir" gitleaks
  scanner="$workdir/gitleaks"
fi

actual_version="$("$scanner" version 2>/dev/null || true)"
if [[ "$actual_version" != "$version" ]]; then
  printf 'Gitleaks version mismatch: expected %s.\n' "$version" >&2
  exit 2
fi

python3 "$repo_root/scripts/test-gitleaks-scanner.py" \
  --gitleaks "$scanner" \
  --config "$repo_root/.gitleaks.toml" \
  --snapshot-helper "$repo_root/scripts/create-gitleaks-worktree-snapshot.py" \
  --temp-dir "$workdir"

scan_report() {
  local mode="$1"
  local report_path="$2"
  local scan_target="$3"
  shift 3

  set +e
  "$scanner" "$mode" "$scan_target" "$@" \
    --config "$repo_root/.gitleaks.toml" \
    --redact=100 \
    --no-banner \
    --report-format json \
    --report-path "$report_path" \
    >"$workdir/scanner-output.log" 2>&1
  local scan_status=$?
  set -e

  python3 - "$report_path" "$scan_status" "$mode" "$scan_target" <<'PY'
import json
import sys
from pathlib import Path

report_path = Path(sys.argv[1])
scan_status = int(sys.argv[2])
mode = sys.argv[3]
scan_root = Path(sys.argv[4]).resolve()
try:
    findings = json.loads(report_path.read_text())
except (OSError, json.JSONDecodeError):
    print(f"Gitleaks {mode} scan failed: no valid report; scanner output suppressed.", file=sys.stderr)
    raise SystemExit(2)

if scan_status == 0 and not findings:
    scope = "tracked and non-ignored working-tree files" if mode == "dir" else "all fetched Git refs"
    print(f"Gitleaks {mode} scan: clean across {scope}; secret output redaction enabled.")
    raise SystemExit(0)
if scan_status == 1 and findings:
    print(f"Gitleaks {mode} scan found {len(findings)} finding(s); secret values are suppressed:")
    for finding in findings:
        path = str(finding.get("File", "<unknown>"))
        line = finding.get("StartLine", "?")
        rule = str(finding.get("RuleID", "unknown"))
        candidate = Path(path)
        if candidate.is_absolute():
            try:
                path = candidate.resolve().relative_to(scan_root).as_posix()
            except (OSError, ValueError):
                path = candidate.name
        elif candidate.parts and candidate.parts[0] == scan_root.name:
            path = Path(*candidate.parts[1:]).as_posix()
        else:
            path = candidate.as_posix()
        print(f"- {path}:{line} [{rule}]")
    raise SystemExit(1)

print(f"Gitleaks {mode} scanner failed or returned an inconsistent result (exit {scan_status}); output suppressed.", file=sys.stderr)
raise SystemExit(scan_status or 2)
PY
}

python3 "$repo_root/scripts/create-gitleaks-worktree-snapshot.py" \
  "$repo_root" "$workdir/worktree"
scan_report dir "$workdir/gitleaks-worktree-report.json" "$workdir/worktree"
scan_report git "$workdir/gitleaks-history-report.json" "$repo_root" --log-opts=--all
