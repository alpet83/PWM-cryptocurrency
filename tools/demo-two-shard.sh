#!/usr/bin/env bash
set -euo pipefail

PORT_A="${PORT_A:-3030}"
PORT_B="${PORT_B:-3031}"
STATE_ROOT_A="${STATE_ROOT_A:-state-shard-a}"
STATE_ROOT_B="${STATE_ROOT_B:-state-shard-b}"

cat <<EOF
Start shard A in terminal #1:
  cargo run -p pwmd --bin pwmd -- --shard A --listen 127.0.0.1:${PORT_A} --state-root ${STATE_ROOT_A}

Start shard B in terminal #2:
  cargo run -p pwmd --bin pwmd -- --shard B --listen 127.0.0.1:${PORT_B} --state-root ${STATE_ROOT_B}

Health checks:
  curl -sS http://127.0.0.1:${PORT_A}/v1/head
  curl -sS http://127.0.0.1:${PORT_B}/v1/head

CLI target switch:
  export PWM_RPC="http://127.0.0.1:${PORT_A}"
  export PWM_RPC="http://127.0.0.1:${PORT_B}"
EOF
