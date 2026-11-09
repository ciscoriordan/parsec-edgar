#!/usr/bin/env bash
# Check that Cargo.toml, server.json and, in a tag build, the tag name all
# give the same version.
set -euo pipefail
cd "$(dirname "$0")/../.."
version=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name == "parsec-edgar") | .version')
if ! jq -e --arg v "$version" '.version == $v and all(.packages[]; .version == $v)' server.json >/dev/null; then
  echo "server.json versions differ from the Cargo.toml version $version" >&2
  exit 1
fi
if [[ "${GITHUB_REF:-}" == refs/tags/v* && "${GITHUB_REF_NAME#v}" != "$version" ]]; then
  echo "tag $GITHUB_REF_NAME does not match the Cargo.toml version $version" >&2
  exit 1
fi
echo "version $version"
