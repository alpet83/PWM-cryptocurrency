#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

DRY_RUN=0
if [[ "${1:-}" == "--dry-run" ]]; then
  DRY_RUN=1
fi

trim_cr() {
  local value="$1"
  printf "%s" "${value%$'\r'}"
}

prompt_with_default() {
  local prompt="$1"
  local default_value="$2"
  local answer
  if [[ -n "${default_value}" ]]; then
    read -r -p "${prompt} [${default_value}]: " answer || true
    if [[ -z "${answer}" ]]; then
      answer="${default_value}"
    fi
  else
    read -r -p "${prompt}: " answer || true
  fi
  trim_cr "${answer}"
}

echo "=== PWM addr-bruteforce interactive runner ==="
echo

read -r -p "Master seed hex (32-byte, leave empty to auto-generate): " master_seed || true
master_seed="$(trim_cr "${master_seed}")"
if [[ -z "${master_seed}" ]]; then
  echo "Generating seed via pwm key-gen..."
  master_seed="$(
    cd "${REPO_ROOT}"
    cargo run -q -p pwm-cli --bin pwm -- key-gen
  )"
  echo "Generated master seed: ${master_seed}"
fi

domain_label="$(prompt_with_default "Domain label/code (e.g. AD, CY, US)" "")"
if [[ -z "${domain_label}" ]]; then
  echo "error: domain label is required"
  exit 2
fi

wallet_default="tmp/wallet-${domain_label,,}.yaml"
wallet_out="$(prompt_with_default "Wallet output path" "${wallet_default}")"
if [[ -z "${wallet_out}" ]]; then
  echo "error: wallet output path is required"
  exit 2
fi

flags_mask="$(prompt_with_default "Flags mask" "1023")"
expected_flags="$(prompt_with_default "Expected flags" "0")"
max_try="$(prompt_with_default "Max try" "500000")"
read -r -p "Wallet passphrase (optional, empty = plaintext_dev fallback): " wallet_passphrase || true
wallet_passphrase="$(trim_cr "${wallet_passphrase}")"

cmd=(cargo run -p pwm-cli --bin pwm --)
if [[ -n "${wallet_passphrase}" ]]; then
  cmd+=(--wallet-passphrase "${wallet_passphrase}")
fi
cmd+=(
  addr-bruteforce
  --master "${master_seed}"
  --domain "${domain_label}"
  --flags-mask "${flags_mask}"
  --expected-flags "${expected_flags}"
  --max-try "${max_try}"
  --wallet-out "${wallet_out}"
)

echo
echo "Command:"
if [[ -n "${wallet_passphrase}" ]]; then
  echo "  cargo run -p pwm-cli --bin pwm -- --wallet-passphrase ****** addr-bruteforce --master ${master_seed} --domain ${domain_label} --flags-mask ${flags_mask} --expected-flags ${expected_flags} --max-try ${max_try} --wallet-out ${wallet_out}"
else
  echo "  cargo run -p pwm-cli --bin pwm -- addr-bruteforce --master ${master_seed} --domain ${domain_label} --flags-mask ${flags_mask} --expected-flags ${expected_flags} --max-try ${max_try} --wallet-out ${wallet_out}"
fi

if [[ "${DRY_RUN}" -eq 1 ]]; then
  echo
  echo "Dry run: command is prepared and not executed."
  exit 0
fi

echo
read -r -p "Run command now? [Y/n]: " run_now || true
run_now="$(trim_cr "${run_now}")"
if [[ -n "${run_now}" && "${run_now}" != "y" && "${run_now}" != "Y" ]]; then
  echo "Cancelled."
  exit 0
fi

(
  cd "${REPO_ROOT}"
  "${cmd[@]}"
)
