#!/usr/bin/env bash
# Preflight for pwm-testing: remove target/debug when du exceeds threshold (default 4096 MiB).
# Run from repository root (directory with Cargo.toml). Primary path: git_bash_exec / Git Bash.
# Override: PWM_PREFLIGHT_TARGET_DEBUG_MIB=<integer>

set -euo pipefail

THRESHOLD_MIB="${PWM_PREFLIGHT_TARGET_DEBUG_MIB:-4096}"

if [[ ! -f Cargo.toml ]]; then
  echo "preflight_target_debug.sh: run from repository root (Cargo.toml not found)" >&2
  exit 1
fi

TARGET="target/debug"
if [[ -d "$TARGET" ]]; then
  SZ=$(du -sm "$TARGET" 2>/dev/null | awk '{print $1}')
  if [[ -n "${SZ:-}" ]] && [[ "$SZ" -gt "$THRESHOLD_MIB" ]]; then
    echo "pwm-testing preflight: ${TARGET} ${SZ}MiB > ${THRESHOLD_MIB}MiB — rm -rf ${TARGET}"
    rm -rf "$TARGET"
  else
    echo "pwm-testing preflight: ${TARGET} ${SZ:-n/a}MiB (threshold ${THRESHOLD_MIB}MiB)"
  fi
else
  echo "pwm-testing preflight: no ${TARGET}"
fi
