#!/usr/bin/env python3
"""Verify the reviewed crates.io source and narrow local manifest patch."""

from __future__ import annotations

import hashlib
import sys
import tarfile
from pathlib import Path, PurePosixPath

EXPECTED_SHA256 = "a8d43fa61c285421813524f466b5ad9a81fb97a03d1a8bf02931588a2aa47434"
ARCHIVE_ROOT = "symbolic-debuginfo-13.9.0"
OMITTED_FILES = {"Cargo.lock"}
NOTES_FILE = "README.kokonut.md"
ZIP_DEPENDENCY_OLD = b"[dependencies.zip]\nversion = \"7.2.0\""
ZIP_DEPENDENCY_NEW = b"[dependencies.zip]\nversion = \"9.6.0\""


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: verify-vendored-symbolic-debuginfo.py ARCHIVE VENDOR_DIR")
    archive_path = Path(sys.argv[1])
    vendor_dir = Path(sys.argv[2])
    archive_bytes = archive_path.read_bytes()
    actual_sha256 = hashlib.sha256(archive_bytes).hexdigest()
    if actual_sha256 != EXPECTED_SHA256:
        raise SystemExit(f"symbolic-debuginfo archive checksum mismatch: {actual_sha256}")

    upstream: dict[str, bytes] = {}
    with tarfile.open(archive_path, mode="r:gz") as archive:
        for member in archive.getmembers():
            if not member.isfile():
                continue
            path = PurePosixPath(member.name)
            if not path.parts or path.parts[0] != ARCHIVE_ROOT:
                raise SystemExit(f"unexpected crate archive path: {member.name}")
            relative = PurePosixPath(*path.parts[1:]).as_posix()
            stream = archive.extractfile(member)
            if stream is None:
                raise SystemExit(f"unable to read crate archive file: {member.name}")
            upstream[relative] = stream.read()

    vendored = {path.relative_to(vendor_dir).as_posix(): path.read_bytes() for path in vendor_dir.rglob("*") if path.is_file()}
    expected_files = (set(upstream) - OMITTED_FILES) | {NOTES_FILE}
    if set(vendored) != expected_files:
        missing = sorted(expected_files - set(vendored))
        extra = sorted(set(vendored) - expected_files)
        raise SystemExit(f"vendored file set differs; missing={missing}, extra={extra}")

    for relative, original in upstream.items():
        if relative in OMITTED_FILES:
            continue
        expected = original
        if relative == "Cargo.toml":
            if original.count(ZIP_DEPENDENCY_OLD) != 1:
                raise SystemExit("upstream ZIP dependency stanza changed; review the vendor patch")
            expected = original.replace(ZIP_DEPENDENCY_OLD, ZIP_DEPENDENCY_NEW)
        if vendored[relative] != expected:
            raise SystemExit(f"unexpected vendor change in {relative}")

    print("symbolic-debuginfo 13.9.0 archive verified; only zip 7.2.0 -> 9.6.0 is patched")


if __name__ == "__main__":
    main()
