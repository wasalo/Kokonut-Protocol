# Kokonut patch: self_update 0.42.0

This vendored crate is copied from the crates.io archive `self_update-0.42.0.crate`; SHA-256 `d832c086ece0dacc29fb2947bb4219b8f6e12fe9e40b7108f9e57c4224e47b5c` matches its registry checksum.

The patch raises the `quick-xml` dependency from 0.37 to 0.41.0 and adapts the S3 release text parser to the 0.41 API while preserving XML entity unescaping. A focused regression test covers that behavior. All retained upstream files are checked against the archive by `scripts/verify-vendored-self-update.py`.

The upstream GitHub example includes `github-private.key` and `github-public.key` as example signing fixtures. They are only referenced by `examples/github.rs`; Protocol and CI code do not load them. The verifier checks these files byte-for-byte against the checksum-pinned crate archive, and the repository secret scan covers tracked history and the worktree.

The upstream `Cargo.lock` is omitted because this is a path dependency and its development lock does not control the cargo-near workspace lock.
