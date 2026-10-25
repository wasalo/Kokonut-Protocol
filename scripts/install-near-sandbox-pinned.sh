#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
version_file="$repo_root/contracts/near/.near-sandbox-version"
checksum_file="$repo_root/contracts/near/.near-sandbox-linux-x86_64-sha256"
version="$(tr -d '\r\n' < "$version_file")"
checksum="$(tr -d '\r\n' < "$checksum_file")"

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf '%s\n' 'invalid pinned NEAR sandbox version' >&2
  exit 1
fi
if [[ ! "$checksum" =~ ^[[:xdigit:]]{64}$ ]]; then
  printf '%s\n' 'invalid pinned NEAR sandbox archive SHA-256' >&2
  exit 1
fi
case "$(uname -s):$(uname -m)" in
  Linux:x86_64|Linux:amd64) artifact_platform='Linux-x86_64' ;;
  *) printf '%s\n' 'pinned NEAR sandbox installer supports Linux x86_64 only' >&2; exit 1 ;;
esac

output_dir="$repo_root/contracts/near/target/ci-tools"
mkdir -p "$output_dir"
if [[ -L "$output_dir" ]]; then
  printf '%s\n' 'refusing symlinked sandbox tool directory' >&2
  exit 1
fi
output="$output_dir/near-sandbox"
if [[ -L "$output" ]]; then
  printf '%s\n' 'refusing symlinked sandbox binary destination' >&2
  exit 1
fi

tmp_parent="${RUNNER_TEMP:-${TMPDIR:-$output_dir}}"
if [[ ! -d "$tmp_parent" || ! -w "$tmp_parent" ]]; then
  printf '%s\n' 'temporary directory must exist and be writable' >&2
  exit 1
fi
tmp_dir="$(mktemp -d "$tmp_parent/near-sandbox-install.XXXXXXXX")"
staged="$output_dir/.near-sandbox.$$"
cleanup() {
  rm -rf -- "$tmp_dir"
  rm -f -- "$staged"
}
trap cleanup EXIT

archive="$tmp_dir/near-sandbox.tar.gz"
url="https://s3-us-west-1.amazonaws.com/build.nearprotocol.com/nearcore/${artifact_platform}/${version}/near-sandbox.tar.gz"
curl --proto '=https' --tlsv1.2 -fsSL "$url" -o "$archive"
printf '%s  %s\n' "$checksum" "$archive" | sha256sum --check

tar -xzf "$archive" -C "$tmp_dir" --strip-components=1 --no-same-owner --no-same-permissions \
  "${artifact_platform}/near-sandbox"
binary="$tmp_dir/near-sandbox"
test -f "$binary" && test -x "$binary"
version_output="$("$binary" --version 2>&1)"
printf '%s\n' "$version_output"
grep -Fq "$version" <<<"$version_output" || {
  printf '%s\n' 'sandbox binary version does not match pinned version' >&2
  exit 1
}

install -m 0755 "$binary" "$staged"
mv -f -- "$staged" "$output"
printf 'Installed checksum-pinned NEAR sandbox %s at %s\n' "$version" "$output"
