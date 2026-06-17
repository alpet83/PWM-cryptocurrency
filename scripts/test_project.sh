#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
export PROJECT_ROOT

source "$PROJECT_ROOT/.build.env"
cd "$PROJECT_ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[test_project] cargo not found in PATH." >&2
  exit 1
fi

if ! command -v dlltool >/dev/null 2>&1; then
  echo "[test_project] ERROR: dlltool not found after PATH refresh." >&2
  echo "[test_project] Expected under: /ucrt64/bin" >&2
  exit 1
fi

echo "[test_project] dlltool=$(command -v dlltool)"

if [[ "$#" -eq 0 ]]; then
  echo "[test_project] running default V6-4 matrix:"
  echo "  cargo test -p pwm-core --lib"
  cargo test -p pwm-core --lib
  echo "  cargo test -p pwmd cluster_prop_"
  cargo test -p pwmd cluster_prop_
else
  echo "[test_project] running: cargo $*"
  cargo "$@"
fi
