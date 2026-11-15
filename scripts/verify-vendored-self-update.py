#!/usr/bin/env python3
"""Verify the checksum-pinned self_update crate and its narrow quick-xml patch."""

from __future__ import annotations

import hashlib
import sys
import tarfile
from pathlib import Path, PurePosixPath

EXPECTED_SHA256 = "d832c086ece0dacc29fb2947bb4219b8f6e12fe9e40b7108f9e57c4224e47b5c"
ARCHIVE_ROOT = "self_update-0.42.0"
OMITTED_FILES = {"Cargo.lock"}
NOTES_FILE = "README.kokonut.md"
OLD_DEP = b'[dependencies.quick-xml]\nversion = "0.37"'
NEW_DEP = b'[dependencies.quick-xml]\nversion = "0.41.0"'
OLD_ORIG_DEP = b'quick-xml = "0.37"'
NEW_ORIG_DEP = b'quick-xml = "0.41.0"'
OLD_READER = b'                if let Ok(txt) = e.unescape().map(|r| r.into_owned()) {'
NEW_READER = b'                if let Some(txt) = decode_xml_text(&e) {'
HELPER = b'''fn decode_xml_text(event: &quick_xml::events::BytesText<'_>) -> Option<String> {
    event.decode().ok().and_then(|decoded| {
        quick_xml::escape::unescape(&decoded)
            .ok()
            .map(|unescaped| unescaped.into_owned())
    })
}

'''
TESTS = b'''

#[cfg(test)]
mod kokonut_quick_xml_compat_tests {
    use super::decode_xml_text;
    use quick_xml::events::BytesText;

    #[test]
    fn s3_text_keeps_xml_entity_unescaping() {
        let event = BytesText::from_escaped("release&amp;tag");
        assert_eq!(decode_xml_text(&event), Some("release&tag".to_owned()));
    }
}
'''


def read_archive(archive_path: Path) -> dict[str, bytes]:
    files: dict[str, bytes] = {}
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
            files[relative] = stream.read()
    return files


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: verify-vendored-self-update.py ARCHIVE VENDOR_DIR")
    archive_path = Path(sys.argv[1])
    vendor_dir = Path(sys.argv[2])
    archive_bytes = archive_path.read_bytes()
    actual_sha256 = hashlib.sha256(archive_bytes).hexdigest()
    if actual_sha256 != EXPECTED_SHA256:
        raise SystemExit(f"self_update archive checksum mismatch: {actual_sha256}")

    upstream = read_archive(archive_path)
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
        elif relative == "src/backends/s3.rs":
            if original.count(OLD_READER) != 1:
                raise SystemExit("upstream S3 text parser changed; review the compatibility patch")
            expected = original.replace(OLD_READER, NEW_READER)
            marker = b"fn fetch_releases_from_s3(\n"
            if expected.count(marker) != 1:
                raise SystemExit("upstream S3 parser location changed; review the compatibility patch")
            expected = expected.replace(marker, HELPER + marker)
            expected += TESTS
        if vendored[relative] != expected:
            raise SystemExit(f"unexpected self_update vendor change in {relative}")

    print("self_update 0.42.0 archive verified; quick-xml 0.41 parser patch and regression test match")


if __name__ == "__main__":
    main()
