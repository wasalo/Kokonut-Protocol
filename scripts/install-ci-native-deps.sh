#!/usr/bin/env bash
set -euo pipefail

# Reproducible Linux native prerequisites for cargo-near and Foundry builds.
# The timestamped Debian snapshot freezes the transitive package index too.
snapshot="20261023T000000Z"

python3 - "$snapshot" <<'PY'
from pathlib import Path
import sys

snapshot = sys.argv[1]
sources_dir = Path("/etc/apt/sources.list.d")
for path in [Path("/etc/apt/sources.list"), *sources_dir.glob("*.sources"), *sources_dir.glob("*.list")]:
    if path.exists():
        path.unlink()
lines = [
    f"deb [check-valid-until=no] https://snapshot.debian.org/archive/debian/{snapshot} trixie main",
    f"deb [check-valid-until=no] https://snapshot.debian.org/archive/debian-security/{snapshot} trixie-security main",
]
(sources_dir / "kokonut-snapshot.list").write_text("\n".join(lines) + "\n")
PY

apt-get -o Acquire::Check-Valid-Until=false update -qq
DEBIAN_FRONTEND=noninteractive apt-get -o Acquire::Check-Valid-Until=false install -y --no-install-recommends \
  ca-certificates curl git \
  pkg-config=1.8.1-4 \
  libudev-dev=257.13-1~deb13u1 \
  libudev1=257.13-1~deb13u1

dpkg-query -W -f='${Package}=${Version}\n' pkg-config libudev-dev libudev1
