# Kokonut patch: email-address-parser 2.0.2

This vendored crate is copied from the crates.io archive email-address-parser-2.0.2.crate; SHA-256 e981c3b50d728bb498dd0f860a7228ef17e19efef5cc2c6e30d78ebce13bcaa7 matches its registry checksum.

The only manifest change updates its declared quick-xml build dependency from 0.39.2 to the security-fixed 0.41.0. No Rust source was changed. The upstream Cargo.lock is omitted because this is a path dependency and its development lock does not control Foundry's workspace lock.

The crate archive declares build = false and excludes build.rs; this patch still keeps the lock graph on the fixed parser release. Every retained upstream file is checksum-verified and byte-compared by scripts/verify-vendored-email-address-parser.py.
