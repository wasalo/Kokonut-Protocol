#!/usr/bin/env python3
"""Verify email-address-parser provenance and its fixed quick-xml requirement."""

from __future__ import annotations

import hashlib
import sys
import tarfile
from pathlib import Path, PurePosixPath

EXPECTED_SHA256 = "e981c3b50d728bb498dd0f860a7228ef17e19efef5cc2c6e30d78ebce13bcaa7"
ARCHIVE_ROOT = "email-address-parser-2.0.2"
OMITTED_FILES = {"Cargo.lock"}
NOTES_FILE = "README.kokonut.md"
OLD_DEP = b'[build-dependencies.quick-xml]\nversion = "^0.39.2"'
NEW_DEP = b'[build-dependencies.quick-xml]\nversion = "=0.41.0"'
OLD_ORIG_DEP = b'quick-xml = "^0.39.2"'
NEW_ORIG_DEP = b'quick-xml = "=0.41.0"'


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: verify-vendored-email-address-parser.py ARCHIVE VENDOR_DIR")
    archive_path = Path(sys.argv[1])
    vendor_dir = Path(sys.argv[2])
    archive_bytes = archive_path.read_bytes()
    actual_sha256 = hashlib.sha256(archive_bytes).hexdigest()
    if actual_sha256 != EXPECTED_SHA256:
        raise SystemExit(f"email-address-parser archive checksum mismatch: {actual_sha256}")

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

    vendored = {
        path.relative_to(vendor_dir).as_posix(): path.read_bytes()
        for path in vendor_dir.rglob("*")
        if path.is_file()
    }
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
            if original.count(OLD_DEP) != 1:
                raise SystemExit("upstream quick-xml dependency stanza changed; review the vendor patch")
            expected = original.replace(OLD_DEP, NEW_DEP)
        elif relative == "Cargo.toml.orig":
            if original.count(OLD_ORIG_DEP) != 1:
                raise SystemExit("upstream quick-xml requirement changed; review the vendor patch")
            expected = original.replace(OLD_ORIG_DEP, NEW_ORIG_DEP)
        if vendored[relative] != expected:
            raise SystemExit(f"unexpected email-address-parser vendor change in {relative}")

    print("email-address-parser 2.0.2 archive verified; only quick-xml 0.39.2 -> 0.41.0 is patched")


if __name__ == "__main__":
    main()
