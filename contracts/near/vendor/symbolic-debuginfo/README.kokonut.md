# Vendored symbolic-debuginfo 13.9.0

This directory is extracted from the crates.io symbolic-debuginfo 13.9.0 archive. Its SHA-256 is a8d43fa61c285421813524f466b5ad9a81fb97a03d1a8bf02931588a2aa47434, matching the registry checksum in the cargo-near source lock before vendoring.

The normalized Cargo.toml is the only modified upstream file: its optional zip dependency is raised from 7.2.0 to 9.6.0. The crate source and parser features remain unchanged. The upstream Cargo.lock is omitted because it is a development lockfile that still lists the affected ZIP 7.2.0; the isolated cargo-near workspace uses its own checked lockfile.

CI verifies the archive checksum and byte-compares the remaining upstream files before applying the cargo-near workspace patch. ZIP 9.6.0 retains the sourcebundle support used by this crate while addressing GHSA-m665-v7pm-7grj. This vendor is only for the cargo-near CI build tool, not the Protocol runtime. Recheck upstream symbolic releases and ZIP advisories before changing it.
