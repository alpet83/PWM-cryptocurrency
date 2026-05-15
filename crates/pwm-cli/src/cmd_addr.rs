//! `addr-derive` / `addr-bruteforce` (`CODEBASE_REFACTORING.md` §2.3 row 4).

use crate::bruteforce::{
    brute_force_from_index, flags_from_account_id, format_eta_human, BruteforceProgress,
    DomainMatchMode,
};
use crate::cli_config::{http_client_for_rpc, resolve_wallet_out_path};
use crate::cli_parse::{master_seed, parse_domain};
use crate::exit_user_error;
use crate::rpc_helpers::{map_reqwest_err, truncate_rpc_body_hint};
use crate::wallet::{
    build_wallet_yaml, detect_resume_der_index, load_wallet_yaml_upgrade, save_wallet_v3_new,
    wallet_account_add_seed, wallet_secrets, WalletProtection,
};
use crate::wallet_shell::{
    parse_domain_label_only, resolve_bruteforce_wallet_protection, validate_user_profile_flags,
};
use ed25519_dalek::SigningKey;
use pwm_core::domain_index::DomainCategory;
use pwm_core::hd::brute_cluster_address;
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::{account_id_to_bech32dx, account_id_to_human, format_domain_for_display};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub(crate) fn bruteforce_resume_index(
    wallet_out: &PathBuf,
    upgrade_wallet: bool,
    overwrite_wallet: bool,
    max_try: u32,
    target_domain: u16,
    domain_mode: DomainMatchMode,
) -> Result<u32, String> {
    if overwrite_wallet {
        return Ok(0);
    }
    if !wallet_out.exists() {
        return Ok(0);
    }
    let start = detect_resume_der_index(wallet_out, upgrade_wallet, target_domain, domain_mode)
        .map_err(|e| format!("failed to read wallet resume metadata: {e}"))?;
    if start > max_try {
        return Err(format!(
            "addr-bruteforce resume start index {start} is greater than --max-try {max_try}"
        ));
    }
    Ok(start)
}

fn fmt_addr_bruteforce_progress(p: BruteforceProgress) -> String {
    format!(
        "    progress checked_derivations={:.4}M rate_per_sec={:.0} eta={}",
        p.checked as f64 / 1_000_000.0,
        p.attempts_per_sec,
        format_eta_human(p.eta_sec)
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fmt_addr_bruteforce_results(
    resume_start_index: u32,
    id_hex: &str,
    id_pretty: &str,
    account_id_bech32dx: &str,
    derivation_index: u32,
    domain_u16: u16,
    domain_label: &str,
    flags_mask_u32: u32,
    expected_flags_u32: u32,
    flags_derived_u32: u32,
    wallet_out: &Path,
    wallet_write: &str,
    attempts: u64,
    elapsed_ms: f64,
    attempts_per_sec: f64,
) -> Vec<String> {
    vec![
        "-------------".to_string(),
        "    mode single_thread_linear".to_string(),
        "    profile phase1_user_country_hi8".to_string(),
        "    domain_match_mode high_byte_only".to_string(),
        format!("    resume_start_index {}", resume_start_index),
        format!("    id_hex {}", id_hex),
        format!("    id_pretty {}", id_pretty),
        format!("    account_id_bech32dx {}", account_id_bech32dx),
        format!("    derivation_index {}", derivation_index),
        format!("    domain_u16 {}", domain_u16),
        format!("    domain_label {}", domain_label),
        format!("    flags_mask_u32 {}", flags_mask_u32),
        format!("    expected_flags_u32 {}", expected_flags_u32),
        format!("    flags_derived_u32 {}", flags_derived_u32),
        format!("    wallet_path {}", wallet_out.display()),
        format!("    wallet_write_mode {}", wallet_write),
        format!("    benchmark_attempts {}", attempts),
        format!("    benchmark_elapsed_ms {:.3}", elapsed_ms),
        format!("    benchmark_attempts_per_sec {:.3}", attempts_per_sec),
    ]
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_wallet_account_output(
    wallet_out: &PathBuf,
    seed: [u8; 32],
    signing_key: [u8; 32],
    verifying_key: [u8; 32],
    derivation_index: u32,
    domain_u16: u16,
    flags_mask_u32: u32,
    expected_flags_u32: u32,
    flags_derived_u32: u32,
    account_id_hex: String,
    id_pretty: String,
    country_code_label: Option<String>,
    protection: WalletProtection,
    overwrite_wallet: bool,
) -> Result<&'static str, String> {
    if wallet_out.exists() && !overwrite_wallet {
        wallet_account_add_seed(wallet_out, derivation_index, &seed)
            .map_err(|e| format!("failed to append account to existing wallet: {e}"))?;
        return Ok("appended");
    }
    let wallet = build_wallet_yaml(
        seed,
        signing_key,
        verifying_key,
        derivation_index,
        domain_u16,
        flags_mask_u32,
        expected_flags_u32,
        flags_derived_u32,
        account_id_hex,
        id_pretty,
        country_code_label,
        protection,
    )
    .map_err(|e| format!("failed to build wallet file: {e}"))?;
    save_wallet_v3_new(wallet_out, &wallet).map_err(|e| format!("save wallet failed: {e}"))?;
    if overwrite_wallet {
        Ok("overwritten")
    } else {
        Ok("created")
    }
}

/// Attempts automatic INIT tx submission after successful brute-force key derivation.
fn try_auto_init(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    sk: &SigningKey,
    dom: u16,
    derivation_index: u32,
    derived_flags: u32,
) -> Result<reqwest::StatusCode, String> {
    let tx = SignedTx::sign_body(
        sk,
        dom,
        derivation_index,
        0,
        TxBody::Init {
            index: derivation_index,
            flags: derived_flags,
        },
    );
    let url = format!("{}/v1/tx", rpc_base);
    let r = c
        .post(&url)
        .json(&tx)
        .send()
        .map_err(|e| map_reqwest_err(&e, "addr-bruteforce auto tx-init"))?;
    let status = r.status();
    if status.is_success() {
        return Ok(status);
    }
    let body = r.text().unwrap_or_default();
    let hint = truncate_rpc_body_hint(&body, 400);
    Err(if hint.is_empty() {
        format!("addr-bruteforce auto tx-init: HTTP {status} ({url})")
    } else {
        format!("addr-bruteforce auto tx-init: HTTP {status} ({url}): {hint}")
    })
}

pub(crate) fn is_rpc_unavailable_error(err: &str) -> bool {
    err.contains("cannot connect") || err.contains("RPC timeout")
}

pub(crate) fn resolve_master_seed(
    cli_master: Option<String>,
    wal_out_explicit: bool,
    wal_path: &Path,
    overwrite_wallet: bool,
    upgrade_wallet: bool,
    wallet_passphrase: Option<&str>,
) -> Result<[u8; 32], String> {
    if overwrite_wallet {
        let Some(seed) = resolve_seed_candidate(cli_master.as_deref())? else {
            return Err(
                "master seed is required with --overwrite-wallet: provide non-empty --master (or PWM_MASTER_SEED) or MASTER_SEED"
                    .to_string(),
            );
        };
        return Ok(seed);
    }

    if wal_out_explicit && wal_path.exists() {
        let wallet = load_wallet_yaml_upgrade(wal_path, upgrade_wallet).map_err(|e| {
            format!("failed to read wallet file for wallet-authoritative master seed: {e}")
        })?;
        let secrets = wallet_secrets(&wallet, wallet_passphrase).map_err(|e| {
            format!("failed to decode wallet secrets for wallet-authoritative master seed: {e}")
        })?;
        let wallet_seed = master_seed(&secrets.master_seed_hex)
            .map_err(|e| format!("wallet master_seed_hex is invalid: {e}"))?;
        if let Some(candidate_seed) = resolve_seed_candidate(cli_master.as_deref())? {
            if candidate_seed != wallet_seed {
                return Err(
                    "master seed conflict: provided --master/PWM_MASTER_SEED/MASTER_SEED does not match existing --wallet-out master seed"
                        .to_string(),
                );
            }
        }
        return Ok(wallet_seed);
    }

    if let Some(seed) = resolve_seed_candidate(cli_master.as_deref())? {
        return Ok(seed);
    }

    if !wal_out_explicit {
        return Err(
            "master seed is required: provide --master value, PWM_MASTER_SEED, MASTER_SEED, or explicit --wallet-out with existing wallet"
                .to_string(),
        );
    }
    Err(format!(
        "master seed fallback requires existing --wallet-out file: '{}' not found",
        wal_path.display()
    ))
}

fn resolve_seed_candidate(cli_master: Option<&str>) -> Result<Option<[u8; 32]>, String> {
    let cli_or_pwm = cli_master.map(str::trim).filter(|s| !s.is_empty());
    if let Some(master) = cli_or_pwm {
        let seed = master_seed(master).map_err(|e| format!("invalid --master seed: {e}"))?;
        return Ok(Some(seed));
    }
    if let Ok(s) = std::env::var("MASTER_SEED") {
        let t = s.trim();
        if !t.is_empty() {
            let seed = master_seed(t).map_err(|e| format!("invalid MASTER_SEED env: {e}"))?;
            return Ok(Some(seed));
        }
    }
    Ok(None)
}

pub(crate) fn run_addr_derive(
    master: Option<String>,
    domain: String,
    max_try: u32,
    wallet_out: Option<PathBuf>,
    wal_out_explicit: bool,
    wallet_passphrase: Option<String>,
    upgrade_wallet: bool,
) {
    eprintln!(
        "warning: `addr-derive` is deprecated and will be removed in a future release; use `addr-bruteforce` instead"
    );
    let wallet_path = resolve_wallet_out_path(wallet_out.clone())
        .unwrap_or_else(|e| exit_user_error(&format!("failed to resolve wallet output path: {e}")));
    let seed = resolve_master_seed(
        master,
        wal_out_explicit,
        wallet_path.as_path(),
        false,
        upgrade_wallet,
        wallet_passphrase.as_deref(),
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let dom = parse_domain(&domain).expect("domain");
    let r = brute_cluster_address(&seed, dom, max_try).expect("no match");
    let (domain_display, domain_ok) = format_domain_for_display(dom as u32);
    let account_id_pretty = account_id_to_human(&r.3);
    let account_id_bech32dx = account_id_to_bech32dx(&r.3);
    let wallet_write = if wal_out_explicit {
        let (wallet_protection, warn_plaintext) =
            resolve_bruteforce_wallet_protection(wallet_passphrase.as_deref())
                .unwrap_or_else(|e| exit_user_error(&e));
        let write_mode = persist_wallet_account_output(
            &wallet_path,
            seed,
            r.0.to_bytes(),
            r.1,
            r.2,
            dom,
            0x03FF,
            0,
            flags_from_account_id(&r.3),
            hex::encode(r.3),
            account_id_pretty.clone(),
            None,
            wallet_protection,
            false,
        )
        .unwrap_or_else(|e| exit_user_error(&e));
        if warn_plaintext && write_mode != "appended" {
            eprintln!(
                "warning: --wallet-out without passphrase (no --wallet-passphrase and no PWM_WALLET_PASSPHRASE): wallet will be saved in plaintext-dev mode"
            );
        }
        write_mode.to_string()
    } else {
        "stateless".to_string()
    };
    println!("wallet_path {}", wallet_path.display());
    println!("wallet_write_mode {wallet_write}");
    println!("account_id_hex {}", hex::encode(r.3));
    println!("id_pretty {}", account_id_pretty);
    println!("account_id_bech32dx {}", account_id_bech32dx);
    println!("domain_display {}", domain_display);
    println!("domain_known {}", domain_ok);
    println!("derivation_index {}", r.2);
    println!("pubkey_hex {}", hex::encode(r.1));
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_addr_bruteforce(
    master: Option<String>,
    domain: String,
    flags_mask: u32,
    expected_flags: u32,
    max_try: u32,
    wallet_out: Option<PathBuf>,
    wal_out_explicit: bool,
    overwrite_wallet: bool,
    wallet_passphrase: Option<String>,
    upgrade_wallet: bool,
    rpc_base: &str,
) {
    let wallet_out = resolve_wallet_out_path(wallet_out)
        .unwrap_or_else(|e| exit_user_error(&format!("failed to resolve wallet output path: {e}")));
    let seed = resolve_master_seed(
        master,
        wal_out_explicit,
        wallet_out.as_path(),
        overwrite_wallet,
        upgrade_wallet,
        wallet_passphrase.as_deref(),
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let domain_entry = parse_domain_label_only(&domain).expect("domain");
    if domain_entry.category != DomainCategory::Regulatory {
        panic!(
            "Phase1 addr-bruteforce supports country/regulatory labels only. Label '{}' is sector/other and is rejected in this phase.",
            domain_entry.label
        );
    }
    validate_user_profile_flags(flags_mask, expected_flags).expect("flags policy");
    let (wallet_protection, warn_plaintext) =
        resolve_bruteforce_wallet_protection(wallet_passphrase.as_deref())
            .unwrap_or_else(|e| exit_user_error(&e));
    let dom = domain_entry.raw as u16;
    let domain_mode = DomainMatchMode::HighByteOnly;
    let resume_start_index = bruteforce_resume_index(
        &wallet_out,
        upgrade_wallet,
        overwrite_wallet,
        max_try,
        dom,
        domain_mode,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let started = Instant::now();
    let hit = brute_force_from_index(
        &seed,
        dom,
        domain_mode,
        flags_mask,
        expected_flags,
        resume_start_index,
        max_try,
        5,
        |p: BruteforceProgress| {
            println!("{}", fmt_addr_bruteforce_progress(p));
        },
    )
    .expect("no match");
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let attempts = (hit.derivation_index - resume_start_index) as u64 + 1;
    let attempts_per_sec = if elapsed_ms > 0.0 {
        (attempts as f64) / (elapsed_ms / 1000.0)
    } else {
        0.0
    };

    let id_hex = hex::encode(hit.account_id);
    let id_pretty = account_id_to_human(&hit.account_id);
    let account_id_bech32dx = account_id_to_bech32dx(&hit.account_id);
    let wallet_write = persist_wallet_account_output(
        &wallet_out,
        seed,
        hit.signing_key,
        hit.verifying_key,
        hit.derivation_index,
        hit.domain,
        flags_mask,
        expected_flags,
        hit.derived_flags,
        id_hex.clone(),
        id_pretty.clone(),
        None,
        wallet_protection,
        overwrite_wallet,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    if warn_plaintext && wallet_write != "appended" {
        eprintln!(
            "warning: --wallet-out without passphrase (no --wallet-passphrase and no PWM_WALLET_PASSPHRASE): wallet will be saved in plaintext-dev mode"
        );
    }

    for line in fmt_addr_bruteforce_results(
        resume_start_index,
        &id_hex,
        &id_pretty,
        &account_id_bech32dx,
        hit.derivation_index,
        hit.domain,
        domain_entry.label,
        flags_mask,
        expected_flags,
        hit.derived_flags,
        wallet_out.as_path(),
        wallet_write,
        attempts,
        elapsed_ms,
        attempts_per_sec,
    ) {
        println!("{line}");
    }

    let c = http_client_for_rpc();
    let init_sk = SigningKey::from_bytes(&hit.signing_key);
    match try_auto_init(
        &c,
        rpc_base,
        &init_sk,
        hit.domain,
        hit.derivation_index,
        hit.derived_flags,
    ) {
        Ok(status) => {
            eprintln!("addr-bruteforce: auto tx-init succeeded on current RPC ({status})");
        }
        Err(e) => {
            eprintln!("addr-bruteforce: auto tx-init failed: {e}");
            if is_rpc_unavailable_error(&e) {
                eprintln!(
                    "hint: RPC unavailable; initialize address manually before tx-send/tx-burn-mark:"
                );
            } else {
                eprintln!(
                    "hint: brute-force result and wallet are saved; initialize this address manually before tx-send/tx-burn-mark:"
                );
            }
            eprintln!(
                "  pwm --rpc <url> tx-init --wallet {} --index {} --flags {}",
                wallet_out.display(),
                hit.derivation_index,
                hit.derived_flags
            );
        }
    }
}
