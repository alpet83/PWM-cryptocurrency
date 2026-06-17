#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

PYTHON="${PYTHON:-/c/Python314/python.exe}"
COMPANION="${COMPANION:-/p/opt/docker/cqds/mcp-tools/cqds_companion.py}"
CONFIG="${CONFIG:-$PROJECT_ROOT/.cqds/cqds_companion.toml}"
LOG_DIR="${LOG_DIR:-$PROJECT_ROOT/scripts/logs/wait_peek_stats}"

cd "$PROJECT_ROOT"

RUNS="${CQDS_WAIT_PEEK_RUNS:-2}"
TRACE_DELAY_MS="${CQDS_WAIT_PEEK_TRACE_DELAY_MS:-25}"
EXIT_WAIT_MS="${CQDS_SIGNAL_EXIT_WAIT_MS:-100}"

mkdir -p "$LOG_DIR"

STAMP="${RANDOM}_${RANDOM}"
SUMMARY="$LOG_DIR/wait_peek_stats_${STAMP}.log"

export CQDS_SIGNAL_FAST_EXIT=1
export CQDS_SIGNAL_EXIT_WAIT_MS="$EXIT_WAIT_MS"
export CQDS_WAIT_PEEK_TRACE_DELAY_MS="$TRACE_DELAY_MS"
export CQDS_WAIT_PEEK_STAGE_DELAY_MS="$TRACE_DELAY_MS"
export CQDS_BASIC_LOG_PARENT_PROBE=0
export CQDS_BASIC_LOG_PARENT_TAG=companion
export CQDS_BASIC_LOG_FORCE_SIMPLE=1
export CQDS_CONSOLE_STAGE_TRACE=1
export CQDS_CONSOLE_STAGE_TRACE_HF_MS=250
export CQDS_CONSOLE_STAGE_TRACE_RING=300
export CQDS_CONSOLE_STAGE_TRACE_SCOPE=10.312-10.315
export CQDS_CTRL_C_CANARY_ENABLE=1
export CQDS_CTRL_C_CANARY_FILE="ctrl-c-canary_${STAMP}.jsonl"

{
  echo "wait_peek stats runner "
  echo "python=$PYTHON "
  echo "companion=$COMPANION "
  echo "config=$CONFIG "
  echo "runs=$RUNS "
  echo "trace_delay_ms=$TRACE_DELAY_MS "
  echo "signal_fast_exit=1 "
  echo "signal_exit_wait_ms=$EXIT_WAIT_MS "
  echo "console_stage_trace=1 "
  echo "console_stage_trace_hf_ms=250 "
  echo "console_stage_trace_ring=300 "
  echo "console_stage_trace_scope=10.312-10.315 "
  echo "ctrl_c_canary=1 "
  echo "ctrl_c_canary_file=ctrl-c-canary_${STAMP}.jsonl "
  echo " "
} > "$SUMMARY"

echo "starting wait_peek stats runner: $SUMMARY"

exit_code=0
for ((i = 1; i <= RUNS; i++)); do
  run_log="$LOG_DIR/run_${i}_${STAMP}.log"
  echo "[$i/$RUNS] start " >> "$SUMMARY"

  set +e
  "$PYTHON" -X dev "$COMPANION" \
    --config "$CONFIG" \
    --run-worker-loop \
    --asyncio-debug \
    --worker-loop-iterations 1 \
    --sigint-grace-sec 0 \
    > "$run_log" 2>&1
  exit_code=$?
  set -e

  echo '"Step done, parsing results..."'
  if (( exit_code != 0 )); then
    echo "[$i/$RUNS] sigint_marker=none" >> "$SUMMARY"
    echo "failed on run $i, see $run_log:"
    tail -n 10 "$run_log"
    break
  fi

  echo "[$i/$RUNS] exit=$exit_code log=$run_log" >> "$SUMMARY"
  grep -iE 'sigint_stage|sigint_origin' "$run_log" >> "$SUMMARY" || true
done

echo "summary=$SUMMARY"
echo "last_exit=$exit_code"
exit "$exit_code"