#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
export PROJECT_ROOT

source "$PROJECT_ROOT/.build.env"
cd "$PROJECT_ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[build_project] cargo not found in PATH." >&2
  echo "[build_project] PATH begins with: ${PATH%%:*}" >&2
  exit 1
fi

if ! command -v dlltool >/dev/null 2>&1; then
  echo "[build_project] warning: dlltool not found in PATH; GNU target builds may fail." >&2
fi

if [[ "$#" -eq 0 ]]; then
  echo "[build_project] running: cargo build --workspace"
  cargo build --workspace
else
  echo "[build_project] running: cargo $*"
  cargo "$@"
fi
