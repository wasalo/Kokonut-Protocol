#!/usr/bin/env python3
"""Regression-test historical secret detection and output redaction."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import secrets
import string
import subprocess
import sys
import tempfile


def longest_overlap(needle: str, text: str) -> int:
    previous = [0] * (len(text) + 1)
    best = 0
    for char in needle:
        current = [0] * (len(text) + 1)
        for index, other in enumerate(text, start=1):
            if char == other:
                current[index] = previous[index - 1] + 1
                best = max(best, current[index])
        previous = current
    return best


def git(repo: Path, env: dict[str, str], *args: str) -> None:
    subprocess.run(
        ["git", *args], cwd=repo, env=env, check=True,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--gitleaks", required=True)
    parser.add_argument("--config", required=True)
    parser.add_argument("--snapshot-helper", required=True)
    parser.add_argument("--temp-dir", required=True)
    args = parser.parse_args()

    temp_parent = Path(args.temp_dir)
    if not temp_parent.is_dir():
        raise SystemExit("Gitleaks self-test requires an existing temporary directory")

    with tempfile.TemporaryDirectory(prefix="kokonut-gitleaks-test-", dir=temp_parent) as temp:
        root = Path(temp)
        repo = root / "repo"
        repo.mkdir()
        config = Path(args.config).resolve()
        (repo / ".gitleaks.toml").write_bytes(config.read_bytes())

        env = os.environ.copy()
        env.update(
            GIT_AUTHOR_NAME="Protocol secret-scan test",
            GIT_COMMITTER_NAME="Protocol secret-scan test",
            GIT_AUTHOR_EMAIL="secret-scan-test@example.invalid",
            GIT_COMMITTER_EMAIL="secret-scan-test@example.invalid",
        )
        git(repo, env, "init", "--quiet", "--initial-branch=main")
        (repo / ".gitignore").write_text("target/\n")
        git(repo, env, "add", ".gitignore")
        git(repo, env, "commit", "--quiet", "-m", "ignore generated test output")

        alphabet = string.ascii_letters + string.digits
        github_token = "ghp_" + "".join(secrets.choice(alphabet) for _ in range(36))
        json_token = "".join(secrets.choice(alphabet) for _ in range(36))
        working_token = "".join(secrets.choice(alphabet) for _ in range(36))
        ignored_token = "".join(secrets.choice(alphabet) for _ in range(36))
        fixture = {
            "service": "svc",
            "operation": "AuthService.register",
            "auth_token": json_token,
            "github_pat": github_token,
        }
        (repo / "fixture.json").write_text(json.dumps(fixture, separators=(",", ":")) + "\n")
        git(repo, env, "add", ".")
        git(repo, env, "commit", "--quiet", "-m", "synthetic scanner regression fixture")

        (repo / "fixture.json").write_text("{}\n")
        git(repo, env, "add", "fixture.json")
        git(repo, env, "commit", "--quiet", "-m", "remove synthetic scanner fixture")
        working_fixture = {
            "service": "svc",
            "operation": "AuthService.register",
            "auth_token": working_token,
        }
        (repo / "working.json").write_text(
            json.dumps(working_fixture, separators=(",", ":")) + "\n"
        )
        ignored_path = repo / "target" / "ignored.json"
        ignored_path.parent.mkdir()
        ignored_path.write_text(json.dumps({"auth_token": ignored_token}) + "\n")

        report = root / "history-report.json"
        scan = subprocess.run(
            [
                args.gitleaks, "git", str(repo), "--log-opts=--all",
                "--config", str(repo / ".gitleaks.toml"),
                "--redact=100", "--no-banner", "--report-format", "json",
                "--report-path", str(report),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        try:
            findings = json.loads(report.read_text())
        except (OSError, json.JSONDecodeError) as error:
            raise SystemExit("Gitleaks self-test produced no valid JSON report") from error

        rules = {finding.get("RuleID") for finding in findings}
        if scan.returncode != 1 or not {"github-pat", "kokonut-json-credential"}.issubset(rules):
            raise SystemExit(
                "Gitleaks self-test failed: expected historical GitHub-PAT and supplemental JSON-rule findings"
            )

        snapshot = root / "worktree"
        subprocess.run(
            [sys.executable, args.snapshot_helper, str(repo), str(snapshot)],
            check=True,
            text=True,
            capture_output=True,
        )
        working_report = root / "working-report.json"
        working_scan = subprocess.run(
            [
                args.gitleaks, "dir", str(snapshot),
                "--config", str(repo / ".gitleaks.toml"),
                "--redact=100", "--no-banner", "--report-format", "json",
                "--report-path", str(working_report),
            ],
            text=True,
            capture_output=True,
            check=False,
        )
        try:
            working_findings = json.loads(working_report.read_text())
        except (OSError, json.JSONDecodeError) as error:
            raise SystemExit("Gitleaks working-tree self-test produced no valid JSON report") from error
        working_rules = {finding.get("RuleID") for finding in working_findings}
        if working_scan.returncode != 1 or "kokonut-json-credential" not in working_rules:
            raise SystemExit("Gitleaks self-test failed: working-tree JSON credential was not detected")
        working_paths = {
            str(finding.get("File", "")).replace("\\\\", "/")
            for finding in working_findings
        }
        if not any(Path(path).name == "working.json" for path in working_paths):
            raise SystemExit("Gitleaks self-test failed: non-ignored untracked file was not scanned")
        if any("target/ignored.json" in path for path in working_paths):
            raise SystemExit("Gitleaks self-test failed: ignored build output was included in the scan")

        tokens = (github_token, json_token, working_token, ignored_token)
        rendered = "\n".join(
            (
                scan.stdout, scan.stderr, report.read_text(),
                working_scan.stdout, working_scan.stderr, working_report.read_text(),
            )
        )
        overlaps = [longest_overlap(token, rendered) for token in tokens]
        for token in tokens:
            for finding in findings + working_findings:
                overlaps.extend(longest_overlap(token, str(value)) for value in finding.values())
        if any(token in rendered for token in tokens) or max(overlaps, default=0) > 4:
            raise SystemExit("Gitleaks self-test failed: scanner output contains an unredacted secret value or long substring")

    print("Gitleaks self-test passed: working tree, history detection, and output redaction verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
