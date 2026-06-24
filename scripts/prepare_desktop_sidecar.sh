#!/usr/bin/env bash
set -euo pipefail

PROFILE="${1:-debug}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_TARGET="$(rustc -vV | sed -n 's/^host: //p')"
SIDECAR_DIR="${ROOT_DIR}/target/sidecars"
SIDECAR_PATH="${SIDECAR_DIR}/grafiki-${HOST_TARGET}"

case "${PROFILE}" in
  debug)
    cargo build -p grafiki-cli
    CLI_PATH="${ROOT_DIR}/target/debug/grafiki"
    ;;
  release)
    cargo build -p grafiki-cli --release
    CLI_PATH="${ROOT_DIR}/target/release/grafiki"
    ;;
  *)
    echo "Usage: $0 [debug|release]" >&2
    exit 2
    ;;
esac

mkdir -p "${SIDECAR_DIR}"
cp "${CLI_PATH}" "${SIDECAR_PATH}"
chmod +x "${SIDECAR_PATH}"

echo "Prepared desktop sidecar ${SIDECAR_PATH}"
