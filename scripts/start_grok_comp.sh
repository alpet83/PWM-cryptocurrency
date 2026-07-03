#!/usr/bin/env bash
# WSL2: Grok companion - pwm-review subagent.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CQDS_SRC_PATH="${CQDS_SRC_PATH:-/opt/docker/cqds}"
export WORKSPACE_ROOT="${WORKSPACE_ROOT:-$ROOT}"
export PROJECT_ROOT="${PROJECT_ROOT:-$ROOT}"
export CQ_COMPANION_API_URL="${CQ_COMPANION_API_URL:-http://172.28.211.144:8100}"

MCP_TOOLS="${CQDS_SRC_PATH}/mcp-tools"
if [[ ! -f "${MCP_TOOLS}/cqds_companion.py" ]]; then
  echo "ERROR: cqds_companion.py not found under ${MCP_TOOLS}" >&2
  echo "Set CQDS_SRC_PATH to the CQDS repo (default: /opt/docker/cqds)." >&2
  exit 2
fi

cd "$ROOT"
exec python3 "${MCP_TOOLS}/cqds_companion.py" \
  --config .cqds/grok_companion.toml \
  --run-worker-loop \
  --save-conversation
