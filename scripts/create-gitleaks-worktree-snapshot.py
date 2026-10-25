#!/usr/bin/env python3
"""Create a private snapshot of Git-relevant working-tree files for scanning."""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: create-gitleaks-worktree-snapshot.py REPO_ROOT DESTINATION")

    repo_root = Path(sys.argv[1]).resolve(strict=True)
    destination = Path(sys.argv[2])
    if destination.exists() or destination.is_symlink():
        raise SystemExit("snapshot destination must not already exist")
    destination.mkdir(parents=True)

    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=repo_root,
        check=True,
        stdout=subprocess.PIPE,
    )

    copied = 0
    for encoded_path in result.stdout.split(b"\0"):
        if not encoded_path:
            continue
        relative = Path(os.fsdecode(encoded_path))
        if relative.is_absolute() or ".." in relative.parts:
            raise SystemExit("Git returned a path outside the repository")

        source = repo_root
        parent_is_real_directory = True
        for component in relative.parts[:-1]:
            source = source / component
            try:
                parent_mode = source.lstat().st_mode
            except FileNotFoundError:
                parent_is_real_directory = False
                break
            if not stat.S_ISDIR(parent_mode):
                parent_is_real_directory = False
                break
        if not parent_is_real_directory:
            continue

        source = repo_root / relative
        try:
            source_mode = source.lstat().st_mode
        except FileNotFoundError:
            continue

        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if stat.S_ISLNK(source_mode):
            # Scan the symlink's stored target text without following it.
            target.write_bytes(os.fsencode(os.readlink(source)))
        elif stat.S_ISREG(source_mode):
            shutil.copyfile(source, target)
        else:
            continue
        copied += 1

    print(f"Snapshotted {copied} tracked and non-ignored working-tree files.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
