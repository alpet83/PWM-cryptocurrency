//! pwm-cli scenario tests spanning wallet flows and RPC helpers.

use super::{
    ensure_import_sender, get_roaming_intent_status, is_terminal_intent_status, parse_export_hex,
    post_export_handoff, post_import_retry, post_roaming_intent, roaming_intent_err,
    tx_import_contract_note, wallet_account_add, wallet_account_list, Cli, Cmd, WalletAccountCmd,
    WalletCmd,
};
use crate::bruteforce::DomainMatchMode;
use crate::cli_config::{is_rpc_offline, DEFAULT_WALLET_OUT_REL};
use crate::cli_parse::{parse_address_arg, parse_address_input, resolve_tx_send_amount};
use crate::cmd_account::{calc_eff_marks, calc_sat_pct, parse_acct_body, parse_head_h};
use crate::cmd_addr::{
    bf_attempt_budget, bf_end_index, bf_no_match_msg, bruteforce_resume_index,
    fmt_addr_bruteforce_results, is_rpc_unavailable_error, persist_wallet_account_output,
    resolve_master_seed,
};
use crate::cmd_genesis::{build_genesis_v4_wallet, GENESIS_SCHEMA_VERSION};
use crate::rpc_helpers::{
    nonce_404_account_hint, parse_account_lookup_meta, parse_nonce_acct_json,
    parse_nonce_init_response, post_signed_tx, preflight_recipient_init,
};
use crate::signer::{load_tx_signer_source, TxSignerSource};
use crate::wallet::{
    build_wallet_yaml, save_wallet_v3_new, wallet_account_add_seed, WalletAccountEntry,
    WalletProtection, WalletSecrets, WalletYaml,
};
use crate::wallet_shell::{
    derive_user_profile_hit, parse_domain_label_only, resolve_bruteforce_wallet_protection,
    resolve_explicit_derivation_index, resolve_wallet_protection,
    validate_explicit_derivation_account, validate_user_profile_flags, wallet_show_lines,
};
use clap::{CommandFactory, Parser};
use pwm_core::domain_index::{lookup_by_raw, DomainCategory};
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::{account_id_to_human, parse_account_id, validate_recipient_domain_policy};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

fn spawn_mock_http_server(script: Vec<(&'static str, u16, &'static str)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    thread::spawn(move || {
        for (expected_request_line, status, body) in script {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).expect("read request");
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            assert!(
                req.starts_with(expected_request_line),
                "unexpected request: {req}"
            );
            let reason = match status {
                200 => "OK",
                204 => "No Content",
                400 => "Bad Request",
                409 => "Conflict",
                _ => "OK",
            };
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(resp.as_bytes()).expect("write response");
        }
    });
    format!("http://{addr}")
}

#[test]
fn label_only_accepts_sector_label() {
    let entry = parse_domain_label_only("FIN").expect("must parse");
    assert_eq!(entry.raw, 0xD00C);
    assert_eq!(entry.category, DomainCategory::Sector);
}

#[test]
/// `parse_nonce_acct_json` accepts numeric JSON values or decimal digit strings.
fn acct_json_nonce_parse_formats() {
    assert_eq!(parse_nonce_acct_json(r#"{"nonce":7}"#), Some(7));
    assert_eq!(parse_nonce_acct_json(r#"{"nonce":"12"}"#), Some(12));
    assert_eq!(parse_nonce_acct_json(r#"{"nonce":"bad"}"#), None);
    assert_eq!(parse_nonce_acct_json(r#"{"height":1}"#), None);
    assert_eq!(parse_nonce_acct_json("not-json"), None);
}

#[test]
/// `nonce_404_account_hint` matches only HTTP 404 with account-not-found style body.
fn nonce_hint_only_acct_nf() {
    let hit = nonce_404_account_hint(404, "account not found");
    assert!(hit.is_some());
    let miss_status = nonce_404_account_hint(500, "account not found");
    assert!(miss_status.is_none());
    let miss_body = nonce_404_account_hint(404, "other error");
    assert!(miss_body.is_none());
}

#[test]
/// `parse_nonce_init_response` maps account-not-found 404 to absent account (init nonce 0).
fn init_nonce_404_none() {
    let out = parse_nonce_init_response(
        reqwest::StatusCode::NOT_FOUND,
        "account not found",
        "http://127.0.0.1:3030/v1/account/dead",
    )
    .expect("parse");
    assert!(out.is_none());
}

#[test]
/// `parse_account_lookup_meta` reads foreign authoritative lookup fields.
fn acct_lookup_meta_foreign() {
    let meta = parse_account_lookup_meta(
            r#"{"local_view_only":true,"home_lookup_status":"ok","authoritative_home_initialized":false}"#,
        )
        .expect("must parse");
    assert!(meta.local_view_only);
    assert_eq!(meta.home_lookup_status.as_deref(), Some("ok"));
    assert_eq!(meta.authoritative_home_initialized, Some(false));
}

#[test]
/// Optional lookup-meta fields default when omitted from JSON.
fn acct_lookup_meta_defaults() {
    let meta = parse_account_lookup_meta(r#"{"initialized":true,"nonce":1}"#).expect("parse");
    assert!(!meta.local_view_only);
    assert_eq!(meta.home_lookup_status, None);
    assert_eq!(meta.authoritative_home_initialized, None);
}

#[test]
fn label_only_rejects_decimal_numeric() {
    let err = parse_domain_label_only("17241").expect_err("must reject numeric");
    assert!(err.contains("numeric domain input is not allowed"));
}

#[test]
fn label_only_rejects_hex_numeric() {
    let err = parse_domain_label_only("0x4359").expect_err("must reject numeric");
    assert!(err.contains("numeric domain input is not allowed"));
}

#[test]
/// `validate_user_profile_flags` rejects flags outside low 10 bits.
fn user_prof_flags_hi_bits() {
    let err = validate_user_profile_flags(0x0400, 0).expect_err("must reject high bits");
    assert!(err.contains("low 10 bits"));
}

#[test]
/// `validate_user_profile_flags` rejects expected flags outside mask.
fn user_prof_exp_flags_mask() {
    let err = validate_user_profile_flags(0x0003, 0x0004).expect_err("must reject mismatch");
    assert!(err.contains("outside flags_mask"));
}

#[test]
fn sector_label_is_not_regulatory() {
    let entry = parse_domain_label_only("FIN").expect("must parse");
    assert_ne!(entry.category, DomainCategory::Regulatory);
}

#[test]
fn wallet_init_cli_parsing() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "init",
        "--country",
        "CY",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse wallet init");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Init {
                country,
                master,
                max_try,
                derivation_index,
                derivation_path,
                plaintext_dev,
                ..
            } => {
                assert_eq!(country.as_deref(), Some("CY"));
                assert!(master.is_none());
                assert_eq!(max_try, 500_000);
                assert!(derivation_index.is_none());
                assert!(derivation_path.is_none());
                assert!(!plaintext_dev);
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn wallet_import_seed_cli_parsing() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "import-seed",
        "--country",
        "CY",
        "--master",
        &"11".repeat(32),
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse wallet import-seed");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::ImportSeed {
                country,
                master,
                max_try,
                derivation_index,
                derivation_path,
                plaintext_dev,
                ..
            } => {
                assert_eq!(country.as_deref(), Some("CY"));
                assert_eq!(master, "11".repeat(32));
                assert_eq!(max_try, 500_000);
                assert!(derivation_index.is_none());
                assert!(derivation_path.is_none());
                assert!(!plaintext_dev);
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn account_info_cli_acct() {
    let cli = Cli::try_parse_from([
        "pwm",
        "account-info",
        "--account",
        "ff00aa00bb00cc00dd00ee00ff00aa00bb00cc00dd00ee00ff00aa00bb00cc00",
    ])
    .expect("must parse account-info --account");
    match cli.cmd {
        Cmd::AccountInfo { account, wallet } => {
            assert!(account.is_some());
            assert!(wallet.is_none());
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn account_info_cli_wallet() {
    let cli = Cli::try_parse_from(["pwm", "account-info", "--wallet", "wallet.yaml"])
        .expect("must parse account-info --wallet");
    match cli.cmd {
        Cmd::AccountInfo { account, wallet } => {
            assert!(account.is_none());
            assert_eq!(wallet, Some(PathBuf::from("wallet.yaml")));
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn account_info_marks_zero_stake() {
    let eff = calc_eff_marks(10, 0, 0, 9999);
    assert_eq!(eff, 10);
    assert_eq!(calc_sat_pct(eff), 0);
}

#[test]
fn account_info_effective_at_head() {
    let eff = calc_eff_marks(10, 0, 5_000_000, 3600);
    assert_eq!(eff, 15);
}

#[test]
fn account_info_parse_fields() {
    let head = parse_head_h(r#"{"height":123,"tip":"ab"}"#).expect("head parse");
    assert_eq!(head, 123);

    let acct = parse_acct_body(r#"{"marks":9,"marks_last_block":7,"staked":"11"}"#)
        .expect("account parse");
    assert_eq!(acct.marks, 9);
    assert_eq!(acct.marks_last_block, 7);
    assert_eq!(acct.staked, 11);
}

#[test]
/// `addr-bruteforce` CLI defaults `flags-mask` to 1023.
fn bf_cli_flags_mask_default() {
    let cli = Cli::try_parse_from([
        "pwm",
        "addr-bruteforce",
        "--master",
        &"11".repeat(32),
        "--domain",
        "CY",
        "--expected-flags",
        "0",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse addr-bruteforce with default flags-mask");
    match cli.cmd {
        Cmd::AddrBruteforce {
            flags_mask,
            expected_flags,
            ..
        } => {
            assert_eq!(flags_mask, 1023);
            assert_eq!(expected_flags, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `is_rpc_offline` accepts trimmed case-insensitive `offline`.
fn rpc_offline_true() {
    assert!(is_rpc_offline("offline"));
    assert!(is_rpc_offline(" OffLine "));
}

#[test]
/// `is_rpc_offline` rejects regular RPC URLs.
fn rpc_offline_false() {
    assert!(!is_rpc_offline("http://127.0.0.1:3030"));
    assert!(!is_rpc_offline("https://node.example"));
}

#[test]
/// Global `--rpc offline` parses together with `addr-bruteforce`.
fn bf_cli_rpc_offline_parse() {
    let cli = Cli::try_parse_from([
        "pwm",
        "--rpc",
        "offline",
        "addr-bruteforce",
        "--master",
        &"11".repeat(32),
        "--domain",
        "CY",
        "--expected-flags",
        "0",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse addr-bruteforce with --rpc offline");
    assert_eq!(cli.rpc, "offline");
    match cli.cmd {
        Cmd::AddrBruteforce { .. } => {}
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `addr-bruteforce` parses without explicit `--master`.
fn bf_cli_master_opt() {
    let cli = Cli::try_parse_from([
        "pwm",
        "addr-bruteforce",
        "--domain",
        "CY",
        "--expected-flags",
        "0",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse addr-bruteforce without explicit master");
    match cli.cmd {
        Cmd::AddrBruteforce { master, .. } => assert!(master.is_none()),
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `addr-derive` without `--wallet-out` stays stateless (no wallet output path).
fn addr_der_cli_stateless() {
    let cli = Cli::try_parse_from(["pwm", "addr-derive", "--domain", "0x2C00"])
        .expect("must parse addr-derive without wallet-out");
    match cli.cmd {
        Cmd::AddrDer {
            master, wallet_out, ..
        } => {
            assert!(master.is_none());
            assert!(wallet_out.is_none());
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `addr-derive` parses optional `--wallet-out`.
fn addr_der_cli_wallet_out() {
    let cli = Cli::try_parse_from([
        "pwm",
        "addr-derive",
        "--domain",
        "0x2C00",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse addr-derive with wallet-out");
    match cli.cmd {
        Cmd::AddrDer {
            master, wallet_out, ..
        } => {
            assert!(master.is_none());
            assert_eq!(wallet_out, Some(PathBuf::from("wallet.yaml")));
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `addr-derive` still accepts explicit `--master`.
fn addr_der_cli_master_opt() {
    let cli = Cli::try_parse_from([
        "pwm",
        "addr-derive",
        "--master",
        &"11".repeat(32),
        "--domain",
        "0x2C00",
    ])
    .expect("must parse addr-derive with explicit master");
    match cli.cmd {
        Cmd::AddrDer { master, .. } => assert_eq!(master, Some("11".repeat(32))),
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `genesis-build` parses wallet and output paths.
fn gen_build_cli_required() {
    let cli = Cli::try_parse_from([
        "pwm",
        "genesis-build",
        "--wallet",
        "wallet.yaml",
        "--out",
        "genesis.json",
    ])
    .expect("must parse genesis-build");
    match cli.cmd {
        Cmd::GenesisBuild { wallet, out, .. } => {
            assert_eq!(wallet, PathBuf::from("wallet.yaml"));
            assert_eq!(out, PathBuf::from("genesis.json"));
        }
        _ => panic!("expected genesis-build"),
    }
}

#[test]
/// `build_genesis_v4_wallet` emits bundle/config with current genesis schema.
fn gen_build_schema5_bundle() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_genesis_build_wallet_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [0x2Au8; 32];
    let passphrase = "slice9-pass";
    let (sk, pk, idx, id) =
        pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
    let wallet = crate::wallet::build_wallet_yaml(
        seed,
        sk.to_bytes(),
        pk,
        idx,
        0x2C00,
        0x03FF,
        0,
        0,
        hex::encode(id),
        account_id_to_human(&id),
        Some("CY".to_string()),
        crate::wallet::WalletProtection::Encrypted {
            passphrase: passphrase.to_string(),
        },
    )
    .expect("wallet");
    crate::wallet::save_wallet_v3_new(&wallet_path, &wallet).expect("save wallet");
    crate::wallet::wallet_account_add(&wallet_path, idx.saturating_add(1), Some(passphrase))
        .expect("add account");

    let (bundle, cfg) = build_genesis_v4_wallet(
        &wallet_path,
        Some(passphrase),
        "slice10-genesis-pass",
        true,
        777,
        9,
        10,
        None,
    )
    .expect("build genesis");
    // Slice 0 schema prep moved emitted genesis schema to v5.
    assert_eq!(bundle.schema_version, GENESIS_SCHEMA_VERSION);
    assert_eq!(bundle.gen_cfg.funding.accounts.len(), 3);
    assert_eq!(bundle.gen_cfg.validators.set.len(), 1);
    assert_eq!(bundle.validator_keys.len(), 1);
    assert!(bundle
        .validator_keys
        .iter()
        .all(|k| k.derivation_path == crate::cmd_genesis::GENESIS_VALIDATOR_DER_PATH));
    assert_eq!(cfg.funding.accounts.len(), 3);
    assert_eq!(cfg.vals.set.len(), 1);
    assert_eq!(cfg.rew, pwm_core::RewPol::ToProducerAccount);
    let val = &bundle.gen_cfg.validators.set[0];
    let funding_val = bundle
        .gen_cfg
        .funding
        .accounts
        .iter()
        .find(|r| r.acct_hex.eq_ignore_ascii_case(&val.acct_hex))
        .expect("validator account must exist in funding rows");
    assert_eq!(funding_val.pubkey_hex, val.pubkey_hex);
    assert_eq!(funding_val.der_idx, val.der_idx);
    assert_eq!(funding_val.bal, "0");
    let _ = std::fs::remove_file(&wallet_path);
}

#[test]
/// Genesis build inserts a zero-balance funding row when the validator account is missing from funding.
fn gen_build_val_zero_row() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_genesis_build_wallet_missing_validator_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [0x3Bu8; 32];
    let passphrase = "slice11-pass";
    let (sk, pk, idx, id) =
        pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
    let wallet = crate::wallet::build_wallet_yaml(
        seed,
        sk.to_bytes(),
        pk,
        idx,
        0x2C00,
        0x03FF,
        0,
        0,
        hex::encode(id),
        account_id_to_human(&id),
        Some("CY".to_string()),
        crate::wallet::WalletProtection::Encrypted {
            passphrase: passphrase.to_string(),
        },
    )
    .expect("wallet");
    crate::wallet::save_wallet_v3_new(&wallet_path, &wallet).expect("save wallet");

    let (bundle, cfg) = build_genesis_v4_wallet(
        &wallet_path,
        Some(passphrase),
        "slice11-genesis-pass",
        true,
        777,
        9,
        10,
        None,
    )
    .expect("build genesis");
    assert_eq!(bundle.gen_cfg.validators.set.len(), 1);
    assert_eq!(bundle.gen_cfg.funding.accounts.len(), 2);
    assert_eq!(cfg.vals.set.len(), 1);
    assert_eq!(cfg.funding.accounts.len(), 2);
    let val = &bundle.gen_cfg.validators.set[0];
    let funding_val = bundle
        .gen_cfg
        .funding
        .accounts
        .iter()
        .find(|r| r.acct_hex.eq_ignore_ascii_case(&val.acct_hex))
        .expect("validator account must exist in funding rows");
    assert_eq!(funding_val.pubkey_hex, val.pubkey_hex);
    assert_eq!(funding_val.der_idx, val.der_idx);
    assert_eq!(funding_val.bal, "0");
    let _ = std::fs::remove_file(&wallet_path);
}

#[test]
/// Parses `wallet init` with explicit `--derivation-index` and `--derivation-path`.
fn wal_init_cli_der_path() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "init",
        "--country",
        "CY",
        "--derivation-index",
        "42",
        "--derivation-path",
        "m/0/42",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse wallet init with explicit derivation selector");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Init {
                derivation_index,
                derivation_path,
                ..
            } => {
                assert_eq!(derivation_index, Some(42));
                assert_eq!(derivation_path.as_deref(), Some("m/0/42"));
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// Parses `wallet import-seed` with explicit `--derivation-index` and `--derivation-path`.
fn wal_impseed_cli_der_path() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "import-seed",
        "--country",
        "CY",
        "--master",
        &"22".repeat(32),
        "--derivation-index",
        "7",
        "--derivation-path",
        "m/0/7",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse wallet import-seed with explicit derivation selector");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::ImportSeed {
                derivation_index,
                derivation_path,
                ..
            } => {
                assert_eq!(derivation_index, Some(7));
                assert_eq!(derivation_path.as_deref(), Some("m/0/7"));
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `resolve_explicit_derivation_index` errors when index conflicts with path.
fn resolve_exp_der_conflict() {
    let err = resolve_explicit_derivation_index(Some(3), Some("m/0/9"))
        .expect_err("must reject conflicting index and path");
    assert!(err.contains("conflicting derivation selectors"));
}

#[test]
/// `validate_explicit_derivation_account` rejects non-cluster domain profiles.
fn val_exp_der_bad_cluster() {
    let seed = [0x42u8; 32];
    let bad = (0..8192u32).find_map(|i| {
        let hit = derive_user_profile_hit(&seed, i);
        validate_explicit_derivation_account(&hit).err().map(|e| e)
    });
    assert!(bad.is_some(), "expected at least one rejected index");
    let msg = bad.unwrap();
    assert!(
        msg.contains("reserve") || msg.contains("witness") || msg.contains("not recognized"),
        "unexpected msg: {msg}"
    );
}

#[test]
/// First 256 explicit derivation indices split between accepted and rejected recipient-domain policy.
fn exp_der_256_policy_split() {
    const SEED: [u8; 32] = [0xAB; 32];
    let mut ok = 0u32;
    let mut bad = 0u32;
    for i in 0..256u32 {
        let hit = derive_user_profile_hit(&SEED, i);
        match validate_explicit_derivation_account(&hit) {
            Ok(()) => ok += 1,
            Err(_) => bad += 1,
        }
    }
    assert_eq!(ok + bad, 256);
    assert!(ok >= 1, "expect at least one policy-ok index in 0..256");
    assert!(
        bad >= 1,
        "expect at least one policy-rejected index in 0..256 for spread (ok={ok}, bad={bad})"
    );
}

#[test]
/// `wallet init` allows derivation selectors without `--country`.
fn wal_init_der_no_cc() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "init",
        "--derivation-index",
        "0",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Init {
                country,
                derivation_index,
                ..
            } => {
                assert!(country.is_none());
                assert_eq!(derivation_index, Some(0));
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `resolve_explicit_derivation_index(None, None)` yields None (legacy path).
fn resolve_exp_der_none_ok() {
    let idx = resolve_explicit_derivation_index(None, None).expect("must resolve");
    assert!(idx.is_none());
}

#[test]
/// `addr-bruteforce` honors global `--wallet-passphrase` without env fallback.
fn bf_cli_pp_no_env() {
    let cli = Cli::try_parse_from([
        "pwm",
        "--wallet-passphrase",
        "flag-secret",
        "addr-bruteforce",
        "--master",
        &"11".repeat(32),
        "--domain",
        "CY",
        "--expected-flags",
        "0",
        "--wallet-out",
        "wallet.yaml",
    ])
    .expect("must parse addr-bruteforce with cli passphrase");
    assert_eq!(cli.wallet_passphrase.as_deref(), Some("flag-secret"));
}

#[test]
/// `addr-bruteforce` parses `--overwrite-wallet`.
fn bf_cli_overwrite_wallet() {
    let cli = Cli::try_parse_from([
        "pwm",
        "addr-bruteforce",
        "--master",
        &"11".repeat(32),
        "--domain",
        "CY",
        "--expected-flags",
        "0",
        "--wallet-out",
        "wallet.yaml",
        "--overwrite-wallet",
    ])
    .expect("must parse addr-bruteforce with overwrite-wallet");
    match cli.cmd {
        Cmd::AddrBruteforce {
            overwrite_wallet, ..
        } => assert!(overwrite_wallet),
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `tx-send` honors global `--upgrade-wallet`.
fn tx_send_cli_upg_wallet() {
    let cli = Cli::try_parse_from([
        "pwm",
        "--upgrade-wallet",
        "tx-send",
        "--wallet",
        "wallet.yaml",
        "--to",
        &"11".repeat(32),
        "--amount",
        "1",
    ])
    .expect("must parse tx-send with --upgrade-wallet");
    assert!(cli.upgrade_wallet);
}

#[test]
fn wallet_show_cli_parsing() {
    let cli = Cli::try_parse_from(["pwm", "wallet", "show", "--wallet", "wallet.yaml"])
        .expect("must parse wallet show");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Show {
                wallet,
                unsafe_show_secrets,
            } => {
                assert_eq!(wallet, PathBuf::from("wallet.yaml"));
                assert!(!unsafe_show_secrets);
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `wallet show` parses `--unsafe-show-secrets`.
fn wal_show_cli_unsafe() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "show",
        "--wallet",
        "wallet.yaml",
        "--unsafe-show-secrets",
    ])
    .expect("must parse wallet show with unsafe flag");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Show {
                wallet,
                unsafe_show_secrets,
            } => {
                assert_eq!(wallet, PathBuf::from("wallet.yaml"));
                assert!(unsafe_show_secrets);
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn wallet_backup_cli_parsing() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "backup",
        "--wallet",
        "wallet.yaml",
        "--out",
        "wallet.backup.yaml",
    ])
    .expect("must parse wallet backup");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Backup { wallet, out } => {
                assert_eq!(wallet, PathBuf::from("wallet.yaml"));
                assert_eq!(out, PathBuf::from("wallet.backup.yaml"));
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn wallet_recover_cli_parsing() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "recover",
        "--backup",
        "wallet.backup.yaml",
        "--out",
        "wallet-restored.yaml",
    ])
    .expect("must parse wallet recover");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Recover { backup, out } => {
                assert_eq!(backup, PathBuf::from("wallet.backup.yaml"));
                assert_eq!(out, PathBuf::from("wallet-restored.yaml"));
            }
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn wallet_account_list_cli_parsing() {
    let cli = Cli::try_parse_from(["pwm", "wallet", "account", "list", "--wallet", "w.yaml"])
        .expect("must parse wallet account list");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Account { cmd } => match cmd {
                WalletAccountCmd::List { wallet } => {
                    assert_eq!(wallet, PathBuf::from("w.yaml"))
                }
                _ => panic!("unexpected wallet account subcmd"),
            },
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected root cmd"),
    }
}

#[test]
fn wallet_account_add_cli_parsing() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "account",
        "add",
        "--wallet",
        "w.yaml",
        "--derivation-index",
        "42",
    ])
    .expect("must parse wallet account add");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Account { cmd } => match cmd {
                WalletAccountCmd::Add {
                    wallet,
                    derivation_index,
                } => {
                    assert_eq!(wallet, PathBuf::from("w.yaml"));
                    assert_eq!(derivation_index, 42);
                }
                _ => panic!("unexpected wallet account subcmd"),
            },
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected root cmd"),
    }
}

#[test]
/// `wallet account add` with global `--wallet-passphrase`.
fn wal_acct_add_cli_pass() {
    let cli = Cli::try_parse_from([
        "pwm",
        "--wallet-passphrase",
        "secret-pass",
        "wallet",
        "account",
        "add",
        "--wallet",
        "w.yaml",
        "--derivation-index",
        "42",
    ])
    .expect("must parse wallet account add with passphrase");
    assert_eq!(cli.wallet_passphrase.as_deref(), Some("secret-pass"));
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Account { cmd } => match cmd {
                WalletAccountCmd::Add {
                    wallet,
                    derivation_index,
                } => {
                    assert_eq!(wallet, PathBuf::from("w.yaml"));
                    assert_eq!(derivation_index, 42);
                }
                _ => panic!("unexpected wallet account subcmd"),
            },
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected root cmd"),
    }
}

#[test]
fn wallet_account_use_cli_parsing() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "account",
        "use",
        "--wallet",
        "w.yaml",
        "--id-hex",
        &"aa".repeat(32),
    ])
    .expect("must parse wallet account use");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Account { cmd } => match cmd {
                WalletAccountCmd::Use { wallet, id_hex } => {
                    assert_eq!(wallet, PathBuf::from("w.yaml"));
                    assert_eq!(id_hex, "aa".repeat(32));
                }
                _ => panic!("unexpected wallet account subcmd"),
            },
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected root cmd"),
    }
}

#[test]
fn wallet_account_remove_cli_parsing() {
    let cli = Cli::try_parse_from([
        "pwm",
        "wallet",
        "account",
        "remove",
        "--wallet",
        "w.yaml",
        "--id-hex",
        &"aa".repeat(32),
    ])
    .expect("must parse wallet account remove");
    match cli.cmd {
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Account { cmd } => match cmd {
                WalletAccountCmd::Remove { wallet, id_hex } => {
                    assert_eq!(wallet, PathBuf::from("w.yaml"));
                    assert_eq!(id_hex, "aa".repeat(32));
                }
                _ => panic!("unexpected wallet account subcmd"),
            },
            _ => panic!("unexpected wallet cmd"),
        },
        _ => panic!("unexpected root cmd"),
    }
}

#[test]
/// Wallet account list lines prefix active entries with `* id_hex=`.
fn wal_acct_list_active() {
    let active = WalletAccountEntry {
        derivation_index: 3,
        derivation_path: "m/0/3".to_string(),
        id_hex: "aa".repeat(32),
        id_pretty: "pwm1-CY/00-f00000003-tdeadbeef".to_string(),
        is_active: true,
    };
    let inactive = WalletAccountEntry {
        is_active: false,
        ..active.clone()
    };

    let active_line = crate::rpc_helpers::fmt_wallet_acct_line(&active);
    let inactive_line = crate::rpc_helpers::fmt_wallet_acct_line(&inactive);

    assert!(active_line.starts_with("* id_hex="));
    assert!(inactive_line.starts_with("  id_hex="));
}

#[test]
/// `wallet_show_lines` hides sensitive fields unless unsafe.
fn wal_show_redact_default() {
    let doc = WalletYaml {
        schema_version: 2,
        mode: "encrypted".to_string(),
        created_at_unix_sec: 1,
        country_code_label: Some("CY".to_string()),
        derivation_index: 0,
        derivation_path: Some("m/0/0".to_string()),
        domain_u16: 0x4359,
        flags_mask_u32: 0x03FF,
        expected_flags_u32: 0,
        flags_derived_u32: 0,
        account_id_hex: "aa".repeat(32),
        account_id_human: "pwm1-CY-f00000000-t0000000000000".to_string(),
        master_seed_hex: None,
        master_seed_b64: None,
        signing_key_hex: None,
        signing_key_b64: None,
        verifying_key_hex: None,
        verifying_key_b64: None,
        encrypted_payload_b64: None,
        kdf_salt_b64: None,
        aead_nonce_b64: None,
        kdf: None,
        kdf_iters: None,
        address_book: Vec::new(),
        ignored_legacy_pretty_entries: 0,
    };
    let out = wallet_show_lines(&doc, &PathBuf::from("wallet.yaml"), None);
    let joined = out.join("\n");
    assert!(joined.contains("id_pretty "));
    assert!(!joined.contains("account_id_human "));
    assert!(!joined.contains("master_seed_hex"));
    assert!(!joined.contains("signing_key_hex"));
    assert!(!joined.contains("verifying_key_hex"));
}

#[test]
/// `wallet_show_lines` prints secrets when unsafe secrets struct is supplied.
fn wal_show_reveal_unsafe() {
    let doc = WalletYaml {
        schema_version: 2,
        mode: "encrypted".to_string(),
        created_at_unix_sec: 1,
        country_code_label: Some("CY".to_string()),
        derivation_index: 0,
        derivation_path: Some("m/0/0".to_string()),
        domain_u16: 0x4359,
        flags_mask_u32: 0x03FF,
        expected_flags_u32: 0,
        flags_derived_u32: 0,
        account_id_hex: "aa".repeat(32),
        account_id_human: "pwm1-CY-f00000000-t0000000000000".to_string(),
        master_seed_hex: None,
        master_seed_b64: None,
        signing_key_hex: None,
        signing_key_b64: None,
        verifying_key_hex: None,
        verifying_key_b64: None,
        encrypted_payload_b64: None,
        kdf_salt_b64: None,
        aead_nonce_b64: None,
        kdf: None,
        kdf_iters: None,
        address_book: Vec::new(),
        ignored_legacy_pretty_entries: 0,
    };
    let secrets = WalletSecrets {
        master_seed_hex: "11".repeat(32),
        signing_key_hex: "22".repeat(32),
        verifying_key_hex: "33".repeat(32),
    };
    let out = wallet_show_lines(&doc, &PathBuf::from("wallet.yaml"), Some(&secrets));
    let joined = out.join("\n");
    assert!(joined.contains("master_seed_hex"));
    assert!(joined.contains("signing_key_hex"));
    assert!(joined.contains("verifying_key_hex"));
}

#[test]
/// `bruteforce_resume_index` resumes after highest matching cluster index.
fn bf_resume_pref_cluster() {
    let path = std::env::temp_dir().join(format!(
        "pwm_cli_addr_bruteforce_resume_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [6u8; 32];
    let hit = derive_user_profile_hit(&seed, 4);
    let wallet = build_wallet_yaml(
        seed,
        hit.signing_key,
        hit.verifying_key,
        hit.derivation_index,
        hit.domain,
        0x03FF,
        0,
        hit.derived_flags,
        hex::encode(hit.account_id),
        account_id_to_human(&hit.account_id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&path, &wallet).expect("save");
    wallet_account_add(&path, 9, None).expect("add account");

    let start = bruteforce_resume_index(
        &path,
        false,
        false,
        50,
        hit.domain,
        DomainMatchMode::HighByteOnly,
    )
    .expect("resume index");
    assert_eq!(start, 5);

    let _ = std::fs::remove_file(&path);
}

#[test]
/// Bruteforce attempt budget equals requested attempt count.
fn bf_budget_exhausted_ok() {
    assert_eq!(bf_attempt_budget(10, 0), 0);
    assert_eq!(bf_attempt_budget(10, 1), 1);
    assert_eq!(bf_attempt_budget(10, 6), 6);
}

#[test]
/// No-match message explains attempt-count `--max-try` semantics and recovery hints.
fn bf_no_match_msg_hints() {
    let msg = bf_no_match_msg(120, Some(125), 6, 6, 1023, 2);
    assert!(msg.contains("120..=125"));
    assert!(msg.contains("attempt_budget=6"));
    assert!(msg.contains("checked 6 derivations"));
    assert!(msg.contains("--max-try is attempt count"));
    assert!(msg.contains("--flags-mask 2 --expected-flags 2"));
    assert!(msg.contains("V7-2 backlog"));
}

#[test]
/// Non-zero resume index with small attempt count still runs bounded search.
fn bf_resume_small_try_ok() {
    let seed = [0x5Au8; 32];
    let resume_start_index = 17u32;
    let hit = derive_user_profile_hit(&seed, resume_start_index);
    let end_index = bf_end_index(resume_start_index, 1).expect("must derive end index");
    let found = crate::bruteforce::brute_force_from_index(
        &seed,
        hit.domain,
        DomainMatchMode::HighByteOnly,
        0x03FF,
        hit.derived_flags & 0x03FF,
        resume_start_index,
        end_index,
        0,
        |_| {},
    )
    .expect("must evaluate at least the resumed derivation index");
    assert_eq!(found.derivation_index, resume_start_index);
}

#[test]
/// Resume index is zero when no wallet row matches target domain/cluster.
fn bf_resume_zero_absent_dom() {
    let path = std::env::temp_dir().join(format!(
        "pwm_cli_addr_bruteforce_resume_absent_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [7u8; 32];
    let hit = derive_user_profile_hit(&seed, 5);
    let wallet = build_wallet_yaml(
        seed,
        hit.signing_key,
        hit.verifying_key,
        hit.derivation_index,
        hit.domain,
        0x03FF,
        0,
        hit.derived_flags,
        hex::encode(hit.account_id),
        account_id_to_human(&hit.account_id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&path, &wallet).expect("save");
    wallet_account_add(&path, 12, None).expect("add account");
    let absent_domain = hit.domain ^ 0x0001;
    let start = bruteforce_resume_index(
        &path,
        false,
        false,
        50,
        absent_domain,
        DomainMatchMode::FullU16,
    )
    .expect("resume index");
    assert_eq!(start, 0);
    let _ = std::fs::remove_file(&path);
}

#[test]
/// Overwrite wallet restart forces resume index zero.
fn bf_resume_zero_overwrite() {
    let path = std::env::temp_dir().join(format!(
        "pwm_cli_addr_bruteforce_resume_overwrite_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [9u8; 32];
    let hit = derive_user_profile_hit(&seed, 12);
    let wallet = build_wallet_yaml(
        seed,
        hit.signing_key,
        hit.verifying_key,
        hit.derivation_index,
        hit.domain,
        0x03FF,
        0,
        hit.derived_flags,
        hex::encode(hit.account_id),
        account_id_to_human(&hit.account_id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&path, &wallet).expect("save");

    let start = bruteforce_resume_index(
        &path,
        false,
        true,
        500,
        hit.domain,
        DomainMatchMode::HighByteOnly,
    )
    .expect("overwrite mode must start from zero");
    assert_eq!(start, 0);

    let _ = std::fs::remove_file(&path);
}

#[test]
/// Addr-bruteforce output lines use separator rows, indentation, and `id_hex` label.
fn bf_out_lines_indent_id() {
    let lines = fmt_addr_bruteforce_results(
        7,
        "ab".repeat(32).as_str(),
        "pwm1-test",
        "bech32dx-test",
        11,
        11264,
        "CY",
        1023,
        0,
        12,
        std::path::Path::new("wallet.yaml"),
        "created",
        5,
        12.345,
        400.1,
    );
    assert_eq!(lines.first().map(String::as_str), Some("-------------"));
    assert!(lines
        .iter()
        .all(|line| { line == "-------------" || line.starts_with("    ") }));
    assert!(lines.iter().any(|line| line.starts_with("    id_hex ")));
    assert!(!lines.iter().any(|line| line.contains("account_id_hex ")));
}

#[test]
/// `persist_wallet_account_output` appends to an existing wallet file unless overwrite is set.
fn bf_persist_append_default() {
    let path = std::env::temp_dir().join(format!(
        "pwm_cli_addr_bruteforce_append_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [6u8; 32];
    let first = derive_user_profile_hit(&seed, 3);
    let wallet = build_wallet_yaml(
        seed,
        first.signing_key,
        first.verifying_key,
        first.derivation_index,
        first.domain,
        0x03FF,
        0,
        first.derived_flags,
        hex::encode(first.account_id),
        account_id_to_human(&first.account_id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&path, &wallet).expect("save");

    let second = derive_user_profile_hit(&seed, 9);
    let mode = persist_wallet_account_output(
        &path,
        seed,
        second.signing_key,
        second.verifying_key,
        second.derivation_index,
        second.domain,
        0x03FF,
        0,
        second.derived_flags,
        hex::encode(second.account_id),
        account_id_to_human(&second.account_id),
        None,
        WalletProtection::PlaintextDev,
        false,
    )
    .expect("append");
    assert_eq!(mode, "appended");
    let accounts = wallet_account_list(&path).expect("list");
    assert_eq!(accounts.len(), 2);
    assert!(accounts.iter().any(|a| a.derivation_index == 9));
    let _ = std::fs::remove_file(&path);
}

#[test]
/// `persist_wallet_account_output` creates a new wallet file when path is absent.
fn addr_der_out_create_miss() {
    let path = std::env::temp_dir().join(format!(
        "pwm_cli_addr_derive_wallet_out_create_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [8u8; 32];
    let hit = derive_user_profile_hit(&seed, 0);
    let mode = persist_wallet_account_output(
        &path,
        seed,
        hit.signing_key,
        hit.verifying_key,
        hit.derivation_index,
        hit.domain,
        0x03FF,
        0,
        hit.derived_flags,
        hex::encode(hit.account_id),
        account_id_to_human(&hit.account_id),
        None,
        WalletProtection::PlaintextDev,
        false,
    )
    .expect("create");
    assert_eq!(mode, "created");
    let accounts = wallet_account_list(&path).expect("list");
    assert_eq!(accounts.len(), 1);
    let _ = std::fs::remove_file(&path);
}

#[test]
/// Default relative wallet-out expands under home `.pwm-crypto/default-wallet.yaml`.
fn wal_out_path_home_def() {
    let home = PathBuf::from(if cfg!(windows) {
        r"C:\Users\qa"
    } else {
        "/tmp/qa-home"
    });
    let out = pwm_core::expand_tilde_path(&PathBuf::from(DEFAULT_WALLET_OUT_REL), &home);
    assert_eq!(out, home.join(".pwm-crypto").join("default-wallet.yaml"));
}

#[test]
/// `expand_tilde_path` expands leading `~` in wallet-out.
fn wal_out_path_tilde() {
    let home = PathBuf::from(if cfg!(windows) {
        r"C:\Users\qa"
    } else {
        "/tmp/qa-home"
    });
    let out = pwm_core::expand_tilde_path(&PathBuf::from("~/.pwm-crypto/custom.yaml"), &home);
    assert_eq!(out, home.join(".pwm-crypto").join("custom.yaml"));
}

#[test]
/// Explicit non-tilde `wallet-out` paths stay unchanged when home expansion is unavailable.
fn wal_out_path_explicit() {
    let out = crate::cli_config::resolve_wallet_out_path(Some(PathBuf::from("wallet.yaml")))
        .expect("must keep explicit non-tilde path");
    assert_eq!(out, PathBuf::from("wallet.yaml"));
}

#[test]
/// Encrypted wallet mode requires passphrase unless plaintext dev is explicitly enabled.
fn wal_prot_need_pass_enc() {
    let err = resolve_wallet_protection(None, false).expect_err("must require passphrase");
    assert!(err.contains("encrypted wallet mode is default"));
}

#[test]
/// `resolve_wallet_protection` allows plaintext dev mode only with explicit opt-in.
fn wal_prot_plain_opt_in() {
    let mode = resolve_wallet_protection(Some("ignored"), true).expect("must allow plaintext");
    match mode {
        WalletProtection::PlaintextDev => {}
        _ => panic!("unexpected mode"),
    }
}

#[test]
/// `resolve_bruteforce_wallet_protection` rejects whitespace-only passphrase.
fn bf_preflight_empty_pass() {
    let err = resolve_bruteforce_wallet_protection(Some("   ")).expect_err("must reject empty");
    assert!(err.contains("must not be empty"));
}

#[test]
/// Missing passphrase falls back to plaintext dev with warning.
fn bf_plain_fallback_no_pass() {
    let (mode, warn_plaintext) =
        resolve_bruteforce_wallet_protection(None).expect("must allow fallback");
    assert!(warn_plaintext);
    match mode {
        WalletProtection::PlaintextDev => {}
        _ => panic!("unexpected mode"),
    }
}

#[test]
/// CLI passphrase selects encrypted wallet protection.
fn bf_cli_pass_enc_mode() {
    let (mode, warn_plaintext) = resolve_bruteforce_wallet_protection(Some("cli-pass"))
        .expect("must resolve encrypted mode");
    assert!(!warn_plaintext);
    match mode {
        WalletProtection::Encrypted { passphrase } => {
            assert_eq!(passphrase, "cli-pass");
        }
        _ => panic!("unexpected mode"),
    }
}

#[test]
/// `resolve_master_seed` uses explicit `--master` when wallet is not authoritative.
fn seed_resolve_cli_wins() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_seed_cli_wins_{}.yaml",
        rand::random::<u128>()
    ));
    let got = resolve_master_seed(
        Some("11".repeat(32)),
        true,
        wallet_path.as_path(),
        false,
        false,
        None,
    )
    .expect("master from cli/env");
    assert_eq!(got, [0x11u8; 32]);
}

#[test]
/// Plaintext wallet fallback uses `master_seed_hex` when `--wallet-out` is explicit.
fn seed_resolve_wallet_plain() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_seed_wallet_plain_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [0x41u8; 32];
    let (sk, pk, idx, id) =
        pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
    let wallet = build_wallet_yaml(
        seed,
        sk.to_bytes(),
        pk,
        idx,
        0x2C00,
        0x03FF,
        0,
        0,
        hex::encode(id),
        account_id_to_human(&id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&wallet_path, &wallet).expect("save wallet");

    let got = resolve_master_seed(None, true, wallet_path.as_path(), false, false, None)
        .expect("master from wallet");
    assert_eq!(got, seed);
    let _ = std::fs::remove_file(&wallet_path);
}

#[test]
/// Encrypted wallet fallback requires passphrase and can decrypt with it.
fn seed_resolve_wallet_enc() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_seed_wallet_enc_{}.yaml",
        rand::random::<u128>()
    ));
    let seed = [0x52u8; 32];
    let passphrase = "seed-pass";
    let (sk, pk, idx, id) =
        pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
    let wallet = build_wallet_yaml(
        seed,
        sk.to_bytes(),
        pk,
        idx,
        0x2C00,
        0x03FF,
        0,
        0,
        hex::encode(id),
        account_id_to_human(&id),
        Some("CY".to_string()),
        WalletProtection::Encrypted {
            passphrase: passphrase.to_string(),
        },
    )
    .expect("wallet");
    save_wallet_v3_new(&wallet_path, &wallet).expect("save wallet");

    let err = resolve_master_seed(None, true, wallet_path.as_path(), false, false, None)
        .expect_err("encrypted wallet needs passphrase");
    assert!(err.contains("failed to decode wallet secrets"));
    let got = resolve_master_seed(
        None,
        true,
        wallet_path.as_path(),
        false,
        false,
        Some(passphrase),
    )
    .expect("master from encrypted wallet");
    assert_eq!(got, seed);
    let _ = std::fs::remove_file(&wallet_path);
}

#[test]
/// Missing explicit wallet fallback path returns actionable error.
fn seed_resolve_wallet_miss() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_seed_wallet_missing_{}.yaml",
        rand::random::<u128>()
    ));
    let err = resolve_master_seed(None, true, wallet_path.as_path(), false, false, None)
        .expect_err("missing wallet");
    assert!(err.contains("requires existing --wallet-out file"));
    let err = resolve_master_seed(None, false, wallet_path.as_path(), false, false, None)
        .expect_err("no stateless fallback");
    assert!(
        err.contains("master seed is required") && err.contains("PWM_MASTER_SEED"),
        "{err}"
    );
}

#[test]
/// Existing wallet rejects mismatched external master candidate.
fn seed_resolve_wallet_conflict() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_seed_wallet_conflict_{}.yaml",
        rand::random::<u128>()
    ));
    let wallet_seed = [0x61u8; 32];
    let (sk, pk, idx, id) =
        pwm_core::hd::brute_cluster_address(&wallet_seed, 0x2C00, 500_000).expect("fixture hit");
    let wallet = build_wallet_yaml(
        wallet_seed,
        sk.to_bytes(),
        pk,
        idx,
        0x2C00,
        0x03FF,
        0,
        0,
        hex::encode(id),
        account_id_to_human(&id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&wallet_path, &wallet).expect("save wallet");
    let cli_seed = "62".repeat(32);
    let err = resolve_master_seed(
        Some(cli_seed),
        true,
        wallet_path.as_path(),
        false,
        false,
        None,
    )
    .expect_err("must reject conflicting master");
    assert!(
        err.contains("master seed conflict")
            && err.contains("--master/PWM_MASTER_SEED/MASTER_SEED")
            && err.contains("--wallet-out"),
        "{err}"
    );
    let _ = std::fs::remove_file(&wallet_path);
}

#[test]
/// Existing wallet accepts matching external master candidate.
fn seed_resolve_wallet_match() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_seed_wallet_match_{}.yaml",
        rand::random::<u128>()
    ));
    let wallet_seed = [0x71u8; 32];
    let (sk, pk, idx, id) =
        pwm_core::hd::brute_cluster_address(&wallet_seed, 0x2C00, 500_000).expect("fixture hit");
    let wallet = build_wallet_yaml(
        wallet_seed,
        sk.to_bytes(),
        pk,
        idx,
        0x2C00,
        0x03FF,
        0,
        0,
        hex::encode(id),
        account_id_to_human(&id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&wallet_path, &wallet).expect("save wallet");
    let got = resolve_master_seed(
        Some("71".repeat(32)),
        true,
        wallet_path.as_path(),
        false,
        false,
        None,
    )
    .expect("must accept matching master");
    assert_eq!(got, wallet_seed);
    let _ = std::fs::remove_file(&wallet_path);
}

#[test]
/// Overwrite mode ignores wallet seed and takes external candidate.
fn seed_resolve_overwrite_wins() {
    let wallet_path = std::env::temp_dir().join(format!(
        "pwm_cli_seed_overwrite_wins_{}.yaml",
        rand::random::<u128>()
    ));
    let wallet_seed = [0x81u8; 32];
    let (sk, pk, idx, id) =
        pwm_core::hd::brute_cluster_address(&wallet_seed, 0x2C00, 500_000).expect("fixture hit");
    let wallet = build_wallet_yaml(
        wallet_seed,
        sk.to_bytes(),
        pk,
        idx,
        0x2C00,
        0x03FF,
        0,
        0,
        hex::encode(id),
        account_id_to_human(&id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&wallet_path, &wallet).expect("save wallet");
    let got = resolve_master_seed(
        Some("82".repeat(32)),
        true,
        wallet_path.as_path(),
        true,
        false,
        None,
    )
    .expect("overwrite uses external seed");
    assert_eq!(got, [0x82u8; 32]);
    let _ = std::fs::remove_file(&wallet_path);
}

#[test]
/// `is_rpc_unavailable_error` detects connect failures.
fn rpc_err_detect_connect() {
    assert!(is_rpc_unavailable_error(
        "addr-bruteforce auto tx-init: cannot connect (is pwmd running? check --rpc/PWM_RPC)"
    ));
}

#[test]
/// `is_rpc_unavailable_error` detects RPC timeouts.
fn rpc_err_detect_timeout() {
    assert!(is_rpc_unavailable_error(
        "addr-bruteforce auto tx-init: RPC timeout after 10s"
    ));
}

#[test]
/// `tx-send --to` accepts pretty human account id.
fn tx_send_cli_to_pretty() {
    let mut recipient_id = [3u8; 32];
    recipient_id[0] = 0xBF;
    recipient_id[1] = 0x10;
    let recipient = account_id_to_human(&recipient_id);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-send",
        "--wallet",
        "wallet.yaml",
        "--to",
        &recipient,
        "--amount",
        "7",
    ])
    .expect("must parse tx-send with pretty recipient");
    match cli.cmd {
        Cmd::TxSend {
            wallet,
            master,
            domain,
            index,
            to,
            amount,
            fee,
        } => {
            assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
            assert!(master.is_none());
            assert!(domain.is_none());
            assert_eq!(index, 0);
            assert_eq!(parse_account_id(&to).unwrap(), recipient_id);
            assert_eq!(amount, Some(7));
            assert_eq!(fee, 1);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `tx-burn-mark --beneficiary` accepts pretty form.
fn tx_burn_cli_ben_pretty() {
    let mut beneficiary_id = [5u8; 32];
    beneficiary_id[0] = 0xBF;
    beneficiary_id[1] = 0x11;
    let beneficiary = account_id_to_human(&beneficiary_id);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-burn-mark",
        "--wallet",
        "wallet.yaml",
        "--mark-amount",
        "12",
        "--beneficiary",
        &beneficiary,
    ])
    .expect("must parse tx-burn-mark with pretty beneficiary");
    match cli.cmd {
        Cmd::TxBurnMark {
            beneficiary: got,
            mark_amount,
            purpose,
            index,
            ..
        } => {
            assert_eq!(mark_amount, 12);
            assert!(purpose.is_none());
            assert_eq!(index, 0);
            assert_eq!(
                parse_account_id(got.as_deref().unwrap()).unwrap(),
                beneficiary_id
            );
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `tx-send --to` accepts canonical bech32dx.
fn tx_send_cli_to_canon() {
    let recipient = pwm_core::account_id_to_bech32dx(&[4u8; 32]);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-send",
        "--wallet",
        "wallet.yaml",
        "--to",
        &recipient,
        "--amount",
        "6",
    ])
    .expect("must parse tx-send with canonical recipient");
    match cli.cmd {
        Cmd::TxSend { to, .. } => {
            assert_eq!(parse_account_id(&to).unwrap(), [4u8; 32]);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `tx-burn-mark --beneficiary` accepts canonical.
fn tx_burn_cli_ben_canon() {
    let beneficiary = pwm_core::account_id_to_bech32dx(&[6u8; 32]);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-burn-mark",
        "--wallet",
        "wallet.yaml",
        "--mark-amount",
        "13",
        "--beneficiary",
        &beneficiary,
    ])
    .expect("must parse tx-burn-mark with canonical beneficiary");
    match cli.cmd {
        Cmd::TxBurnMark {
            beneficiary: got,
            purpose,
            index,
            ..
        } => {
            assert!(purpose.is_none());
            assert_eq!(index, 0);
            assert_eq!(
                parse_account_id(got.as_deref().unwrap()).unwrap(),
                [6u8; 32]
            );
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_burn_cli_purpose_flag() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-burn-mark",
        "--wallet",
        "wallet.yaml",
        "--mark-amount",
        "3",
        "--purpose",
        "salted-email-proof-v1",
    ])
    .expect("parse");
    match cli.cmd {
        Cmd::TxBurnMark {
            purpose,
            mark_amount,
            index,
            ..
        } => {
            assert_eq!(mark_amount, 3);
            assert_eq!(purpose.as_deref(), Some("salted-email-proof-v1"));
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_claim_cli_parse() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-claim",
        "--wallet",
        "wallet.yaml",
        "--claim-mode",
        "free",
        "--claim-units",
        "10",
        "--anchor-ref",
        "42",
        "--fee",
        "0",
    ])
    .expect("parse");
    match cli.cmd {
        Cmd::TxClaim {
            claim_mode,
            claim_units,
            anchor_ref,
            fee,
            ..
        } => {
            assert_eq!(claim_mode, "free");
            assert_eq!(claim_units, 10);
            assert_eq!(anchor_ref, 42);
            assert_eq!(fee, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_claim_cli_parse_paid() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-claim",
        "--wallet",
        "wallet.yaml",
        "--claim-mode",
        "paid",
        "--claim-units",
        "11",
        "--anchor-ref",
        "43",
        "--fee",
        "5",
    ])
    .expect("parse");
    match cli.cmd {
        Cmd::TxClaim {
            claim_mode,
            claim_units,
            anchor_ref,
            fee,
            ..
        } => {
            assert_eq!(claim_mode, "paid");
            assert_eq!(claim_units, 11);
            assert_eq!(anchor_ref, 43);
            assert_eq!(fee, 5);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
// Invalid --claim-mode must return an error mentioning --claim-mode.
fn tx_claim_mode_cli_err() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-claim",
        "--wallet",
        "wallet.yaml",
        "--claim-mode",
        "weird",
        "--claim-units",
        "10",
        "--anchor-ref",
        "42",
        "--fee",
        "0",
    ])
    .expect("parse");
    let raw_mode = match cli.cmd {
        Cmd::TxClaim { claim_mode, .. } => claim_mode,
        _ => panic!("unexpected cmd"),
    };
    let err = crate::cmd_tx::parse_claim_mode_cli(&raw_mode).expect_err("must reject mode");
    assert!(err.contains("--claim-mode"));
    assert!(!err.contains("invalid --mode "));
}

#[test]
/// `tx-send` allows `--master` and `--domain` over wallet file.
fn tx_send_cli_master_ovr() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-send",
        "--wallet",
        "wallet.yaml",
        "--master",
        &"11".repeat(32),
        "--domain",
        "CY",
        "--to",
        &account_id_to_human(&[7u8; 32]),
        "--amount",
        "9",
    ])
    .expect("must parse tx-send with override");
    match cli.cmd {
        Cmd::TxSend {
            wallet,
            master,
            domain,
            index,
            ..
        } => {
            assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
            assert_eq!(master.unwrap(), "11".repeat(32));
            assert_eq!(domain.as_deref(), Some("CY"));
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `parse_address_input` accepts `pwm:` URI with amount query.
fn parse_addr_in_uri_amt() {
    let mut id = [0u8; 32];
    id[0] = 0x2C;
    id[31] = 1;
    let pretty = account_id_to_human(&id);
    let (parsed, amount) =
        parse_address_input("--to", &format!("pwm:{pretty}?amount=42")).expect("uri");
    assert_eq!(parsed, id);
    assert_eq!(amount, Some(42));
}

#[test]
/// Unknown PWM URI query keys are rejected.
fn parse_addr_in_bad_query() {
    let id = pwm_core::account_id_to_bech32dx(&[4u8; 32]);
    let err = parse_address_input("--to", &format!("pwm:{id}?memo=abc")).expect_err("reject");
    assert!(err.contains("unsupported pwm URI query parameter"));
}

#[test]
/// PWM URI without address body is rejected.
fn parse_uri_missing_addr() {
    let err = parse_address_input("--to", "pwm:?amount=1").expect_err("reject");
    assert!(err.contains("missing address"));
}

#[test]
/// `resolve_tx_send_amount` errors when CLI and URI amounts disagree.
fn tx_amt_resolve_conflict() {
    let err = resolve_tx_send_amount(Some(10), Some(11)).expect_err("must conflict");
    assert!(err.contains("amount conflict"));
}

#[test]
/// URI-provided amount used when CLI `--amount` omitted.
fn tx_amt_from_uri_only() {
    let amount = resolve_tx_send_amount(None, Some(15)).expect("uri amount");
    assert_eq!(amount, 15);
}

#[test]
/// `tx-send` accepts PWM URI with amount while omitting `--amount`.
fn tx_send_uri_amt_only() {
    let recipient = account_id_to_human(&[7u8; 32]);
    let uri = format!("pwm:{recipient}?amount=9");
    let cli = Cli::try_parse_from(["pwm", "tx-send", "--wallet", "wallet.yaml", "--to", &uri])
        .expect("must parse tx-send with uri amount only");
    match cli.cmd {
        Cmd::TxSend { amount, to, .. } => {
            assert!(amount.is_none());
            assert_eq!(to, uri);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// PWM URI with invalid embedded address fails.
fn parse_uri_bad_embed_addr() {
    let err = parse_address_input("--to", "pwm:not-an-address?amount=1").expect_err("reject");
    assert!(err.contains("Invalid value for --to"));
}

#[test]
/// `tx-send` requires `--domain` when `--master` override is used.
fn tx_send_master_no_dom() {
    let err = match Cli::try_parse_from([
        "pwm",
        "tx-send",
        "--master",
        &"11".repeat(32),
        "--to",
        &account_id_to_human(&[8u8; 32]),
        "--amount",
        "1",
    ]) {
        Ok(_) => panic!("must reject missing domain"),
        Err(err) => err.to_string(),
    };
    assert!(err.contains("--domain"));
}

#[test]
/// `tx-init` fails when neither `--wallet` nor `--master` signing source is given.
fn tx_init_no_sign_src() {
    let err = match Cli::try_parse_from(["pwm", "tx-init", "--index", "0", "--flags", "0"]) {
        Ok(_) => panic!("must reject missing signing source"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("--wallet"));
}

#[test]
fn tx_init_v4_cli_parse() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-init",
        "--wallet",
        "wallet.yaml",
        "--index",
        "1",
        "--flags",
        "7",
        "--owner-kind",
        "company",
        "--owner-name",
        "Acme",
        "--owner-country",
        "CY",
        "--metadata-commitment",
        &"aa".repeat(32),
        "--verification-ref",
        "kyc:acme",
        "--requested-domain-lo",
        "5",
        "--rescue-address",
        &account_id_to_human(&[4u8; 32]),
        "--initial-policy",
        "sender_filter:dormant",
        "--save-activation-tx",
        "activation.json",
    ])
    .expect("must parse tx-init v4 args");
    match cli.cmd {
        Cmd::TxInit {
            owner_kind,
            owner_name,
            owner_country,
            metadata_commitment,
            verification_ref,
            requested_domain_lo,
            rescue_address,
            initial_policy,
            save_activation_tx,
            ..
        } => {
            let expected_commitment = "aa".repeat(32);
            assert_eq!(owner_kind.as_deref(), Some("company"));
            assert_eq!(owner_name.as_deref(), Some("Acme"));
            assert_eq!(owner_country.as_deref(), Some("CY"));
            assert_eq!(
                metadata_commitment.as_deref(),
                Some(expected_commitment.as_str())
            );
            assert_eq!(verification_ref.as_deref(), Some("kyc:acme"));
            assert_eq!(requested_domain_lo, Some(5));
            assert!(rescue_address.is_some());
            assert_eq!(initial_policy, vec!["sender_filter:dormant".to_string()]);
            assert_eq!(save_activation_tx, Some(PathBuf::from("activation.json")));
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_policy_set_cli_parse() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-set",
        "--wallet",
        "wallet.yaml",
        "--policy",
        "sender_filter",
        "--activation",
        "dormant",
        "--fee",
        "10",
    ])
    .expect("must parse tx-policy-set");
    match cli.cmd {
        Cmd::TxPolicySet {
            policy,
            activation,
            fee,
            index,
            ..
        } => {
            assert_eq!(policy, "sender_filter");
            assert_eq!(activation, "dormant");
            assert_eq!(fee, 10);
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_policy_set_deferred_parse() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-set",
        "--wallet",
        "wallet.yaml",
        "--policy",
        "sender_filter",
        "--activation",
        "deferred",
        "--activate-at-height",
        "77",
        "--fee",
        "10",
    ])
    .expect("must parse tx-policy-set deferred");
    match cli.cmd {
        Cmd::TxPolicySet {
            activation,
            activate_at_height,
            index,
            ..
        } => {
            assert_eq!(activation, "deferred");
            assert_eq!(activate_at_height, Some(77));
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_policy_set_no_height() {
    let err = crate::cmd_tx::parse_policy_act_cli("deferred", None)
        .expect_err("must reject deferred without height");
    assert!(err.contains("--activate-at-height"), "{err}");
}

#[test]
fn tx_pol_act_parse() {
    let target = account_id_to_human(&[7u8; 32]);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-activate",
        "--wallet",
        "wallet.yaml",
        "--policy-id",
        "1",
        "--activation-target",
        &target,
        "--rescue-account-index",
        "7",
        "--fee",
        "99",
    ])
    .expect("must parse tx-policy-activate");
    match cli.cmd {
        Cmd::TxPolicyActivate {
            policy_id,
            rescue_account_index,
            fee,
            activation_target,
            index,
            ..
        } => {
            assert_eq!(policy_id, Some(1));
            assert_eq!(rescue_account_index, Some(7));
            assert_eq!(fee, 99);
            assert_eq!(activation_target.as_deref(), Some(target.as_str()));
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_pol_act_sw_idx() {
    let target = account_id_to_human(&[8u8; 32]);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-activate",
        "--wallet",
        "wallet.yaml",
        "--index",
        "3",
        "--policy",
        "routing.emergency_redirect",
        "--activation-target",
        &target,
        "--rescue-account-index",
        "9",
    ])
    .expect("must parse same-wallet emergency activate");
    match cli.cmd {
        Cmd::TxPolicyActivate {
            index,
            rescue_account_index,
            policy,
            ..
        } => {
            assert_eq!(index, 3);
            assert_eq!(rescue_account_index, Some(9));
            assert_eq!(policy.as_deref(), Some("routing.emergency_redirect"));
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_pol_act_prepared_parse() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-activate",
        "--activation-tx",
        "activation.json",
    ])
    .expect("must parse tx-policy-activate prepared tx path without wallet");
    match cli.cmd {
        Cmd::TxPolicyActivate {
            wallet,
            activation_tx,
            fee,
            index,
            ..
        } => {
            assert!(wallet.is_none());
            assert_eq!(activation_tx, Some(PathBuf::from("activation.json")));
            assert_eq!(fee, 0);
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_pol_act_rescue_wallet() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-activate",
        "--wallet",
        "wallet.yaml",
        "--policy",
        "sender_filter",
        "--rescue-wallet",
        "rescue.yaml",
    ])
    .expect("must parse tx-policy-activate with rescue wallet path");
    match cli.cmd {
        Cmd::TxPolicyActivate {
            policy,
            rescue_wallet,
            rescue_master,
            rescue_domain,
            index,
            ..
        } => {
            assert_eq!(policy.as_deref(), Some("sender_filter"));
            assert_eq!(rescue_wallet, Some(PathBuf::from("rescue.yaml")));
            assert!(rescue_master.is_none());
            assert!(rescue_domain.is_none());
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_pol_act_rescue_seed() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-activate",
        "--wallet",
        "wallet.yaml",
        "--policy-id",
        "2",
        "--rescue-master",
        &"11".repeat(32),
        "--rescue-domain",
        "CY",
    ])
    .expect("must parse tx-policy-activate with rescue master+domain");
    match cli.cmd {
        Cmd::TxPolicyActivate {
            policy_id,
            rescue_master,
            rescue_domain,
            rescue_wallet,
            index,
            ..
        } => {
            assert_eq!(policy_id, Some(2));
            assert_eq!(rescue_master, Some("11".repeat(32)));
            assert_eq!(rescue_domain.as_deref(), Some("CY"));
            assert!(rescue_wallet.is_none());
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_pol_deact_parse() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-policy-deactivate",
        "--wallet",
        "wallet.yaml",
        "--policy-id",
        "1",
        "--fee",
        "33",
    ])
    .expect("must parse tx-policy-deactivate");
    match cli.cmd {
        Cmd::TxPolicyDeactivate {
            policy_id,
            policy,
            fee,
            index,
            ..
        } => {
            assert_eq!(policy_id, Some(1));
            assert!(policy.is_none());
            assert_eq!(fee, 33);
            assert_eq!(index, 0);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `tx-stake` parses wallet-only signing.
fn tx_stake_cli_wallet_only() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-stake",
        "--wallet",
        "wallet.yaml",
        "--amount",
        "15",
    ])
    .expect("must parse tx-stake wallet-first");
    match cli.cmd {
        Cmd::TxStake {
            wallet,
            master,
            domain,
            index,
            amount,
        } => {
            assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
            assert!(master.is_none());
            assert!(domain.is_none());
            assert_eq!(index, 0);
            assert_eq!(amount, 15);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
fn tx_cmd_idx_parse() {
    let to = account_id_to_human(&[7u8; 32]);
    let send = Cli::try_parse_from([
        "pwm",
        "tx-send",
        "--wallet",
        "wallet.yaml",
        "--index",
        "2",
        "--to",
        &to,
        "--amount",
        "1",
    ])
    .expect("tx-send parse");
    let burn = Cli::try_parse_from([
        "pwm",
        "tx-burn-mark",
        "--wallet",
        "wallet.yaml",
        "--index",
        "4",
        "--mark-amount",
        "1",
    ])
    .expect("tx-burn-mark parse");
    let stake = Cli::try_parse_from([
        "pwm",
        "tx-stake",
        "--wallet",
        "wallet.yaml",
        "--index",
        "5",
        "--amount",
        "2",
    ])
    .expect("tx-stake parse");
    let unstake = Cli::try_parse_from([
        "pwm",
        "tx-unstake",
        "--wallet",
        "wallet.yaml",
        "--index",
        "6",
        "--amount",
        "3",
    ])
    .expect("tx-unstake parse");
    let pset = Cli::try_parse_from([
        "pwm",
        "tx-policy-set",
        "--wallet",
        "wallet.yaml",
        "--index",
        "7",
        "--policy",
        "sender_filter",
        "--activation",
        "dormant",
    ])
    .expect("tx-policy-set parse");
    let pdeact = Cli::try_parse_from([
        "pwm",
        "tx-policy-deactivate",
        "--wallet",
        "wallet.yaml",
        "--index",
        "8",
        "--policy-id",
        "1",
    ])
    .expect("tx-policy-deactivate parse");

    match send.cmd {
        Cmd::TxSend { index, .. } => assert_eq!(index, 2),
        _ => panic!("unexpected send"),
    }
    match burn.cmd {
        Cmd::TxBurnMark { index, .. } => assert_eq!(index, 4),
        _ => panic!("unexpected burn"),
    }
    match stake.cmd {
        Cmd::TxStake { index, .. } => assert_eq!(index, 5),
        _ => panic!("unexpected stake"),
    }
    match unstake.cmd {
        Cmd::TxUnstake { index, .. } => assert_eq!(index, 6),
        _ => panic!("unexpected unstake"),
    }
    match pset.cmd {
        Cmd::TxPolicySet { index, .. } => assert_eq!(index, 7),
        _ => panic!("unexpected policy-set"),
    }
    match pdeact.cmd {
        Cmd::TxPolicyDeactivate { index, .. } => assert_eq!(index, 8),
        _ => panic!("unexpected policy-deactivate"),
    }
}

#[test]
/// V3 wallet without `active_account_id_hex` picks deterministic lowest derivation index.
fn tx_signer_v3_no_active() {
    let path = std::env::temp_dir().join(format!(
        "pwm-cli-v3-signing-no-active-{}.yml",
        rand::random::<u128>()
    ));
    let seed = [44u8; 32];
    let first_idx = 21u32;
    let default_idx = 3u32;
    let first_key = slip10_ed25519::derive_ed25519_private_key(&seed, &[0, first_idx]);
    let first_sk = ed25519_dalek::SigningKey::from_bytes(&first_key);
    let first_pk = first_sk.verifying_key().to_bytes();
    let first_id = pwm_core::hd::account_id_from_parts(&first_pk, first_idx);
    let wallet = build_wallet_yaml(
        seed,
        first_sk.to_bytes(),
        first_pk,
        first_idx,
        u16::from_be_bytes([first_id[0], first_id[1]]),
        0x03FF,
        0,
        u32::from_be_bytes([first_id[2], first_id[3], first_id[4], first_id[5]]),
        hex::encode(first_id),
        account_id_to_human(&first_id),
        Some("CY".to_string()),
        WalletProtection::PlaintextDev,
    )
    .expect("wallet");
    save_wallet_v3_new(&path, &wallet).expect("save v3");
    wallet_account_add_seed(&path, default_idx, &seed).expect("add lower-index account");

    let raw = std::fs::read_to_string(&path).expect("read");
    assert!(!raw.contains("active_account_id_hex"));

    let expected_key = slip10_ed25519::derive_ed25519_private_key(&seed, &[0, default_idx]);
    let expected_sk = ed25519_dalek::SigningKey::from_bytes(&expected_key);
    let expected_id =
        pwm_core::hd::account_id_from_parts(&expected_sk.verifying_key().to_bytes(), default_idx);
    let source =
        load_tx_signer_source(Some(path.clone()), None, None, None, false).expect("signer");
    assert_eq!(source.idx, default_idx);
    assert_eq!(source.from, expected_id);
    assert_eq!(
        pwm_core::hd::account_id_from_parts(&source.sk.verifying_key().to_bytes(), source.idx),
        expected_id
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
/// `tx-export` happy-path CLI parse.
fn tx_export_cli_parse_ok() {
    let recipient = pwm_core::account_id_to_bech32dx(&[9u8; 32]);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-export",
        "--wallet",
        "wallet.yaml",
        "--to",
        &recipient,
        "--target-domain",
        "DO",
        "--amount",
        "25",
        "--fee",
        "2",
    ])
    .expect("must parse tx-export");
    match cli.cmd {
        Cmd::TxExport {
            wallet,
            master,
            domain,
            to,
            target_domain,
            amount,
            fee,
        } => {
            assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
            assert!(master.is_none());
            assert!(domain.is_none());
            assert_eq!(parse_account_id(&to).unwrap(), [9u8; 32]);
            assert_eq!(target_domain, "DO");
            assert_eq!(amount, 25);
            assert_eq!(fee, 2);
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `tx-import` happy-path CLI parse.
fn tx_import_cli_parse_ok() {
    let recipient = pwm_core::account_id_to_bech32dx(&[10u8; 32]);
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-import",
        "--wallet",
        "wallet.yaml",
        "--to",
        &recipient,
        "--amount",
        "25",
        "--export-id",
        &"ab".repeat(32),
    ])
    .expect("must parse tx-import");
    match cli.cmd {
        Cmd::TxImport {
            wallet,
            master,
            domain,
            to,
            amount,
            export_id,
        } => {
            assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
            assert!(master.is_none());
            assert!(domain.is_none());
            assert_eq!(parse_account_id(&to).unwrap(), [10u8; 32]);
            assert_eq!(amount, 25);
            assert_eq!(export_id, "ab".repeat(32));
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// `tx-handoff-register` happy-path CLI parse.
fn tx_handoff_reg_cli_ok() {
    let cli = Cli::try_parse_from([
        "pwm",
        "tx-handoff-register",
        "--handoff-json",
        "handoff.json",
    ])
    .expect("must parse tx-handoff-register");
    match cli.cmd {
        Cmd::TxHandoffRegister { handoff_json } => {
            assert_eq!(handoff_json, PathBuf::from("handoff.json"));
        }
        _ => panic!("unexpected cmd"),
    }
}

#[test]
/// Long help mentions raw PWM scaling for relevant subcommands.
fn cli_help_raw_pwm_unit() {
    let mut cmd = Cli::command();
    let tx_send = cmd
        .find_subcommand_mut("tx-send")
        .expect("tx-send subcommand")
        .render_long_help()
        .to_string();
    assert!(tx_send.contains("1 PWM = 1_000_000 raw"), "{tx_send}");

    let tx_import = cmd
        .find_subcommand_mut("tx-import")
        .expect("tx-import subcommand")
        .render_long_help()
        .to_string();
    assert!(
        tx_import.contains("Must already be initialized"),
        "{tx_import}"
    );

    let handoff_register = cmd
        .find_subcommand_mut("tx-handoff-register")
        .expect("tx-handoff-register subcommand")
        .render_long_help()
        .to_string();
    assert!(
        handoff_register.contains("handoff JSON"),
        "{handoff_register}"
    );
}

#[test]
/// `tx_import_contract_note` documents initialized recipient requirement.
fn tx_imp_note_need_init() {
    let note = tx_import_contract_note();
    assert!(note.contains("target --to"), "{note}");
    assert!(note.contains("already be initialized"), "{note}");
    assert!(note.contains("target shard"), "{note}");
    assert!(note.contains("tx-init"), "{note}");
    assert!(note.contains("configured seed context"), "{note}");
    assert!(!note.contains("stub"), "{note}");
    assert!(!note.contains("missing/uninitialized"), "{note}");
}

#[test]
/// `parse_export_hex` rejects non-hex input and wrong-length hex.
fn parse_exp_id_hex_bad() {
    let err = parse_export_hex("xyz").expect_err("must reject malformed export id");
    assert!(err.contains("Invalid value for --export-id"));
    let err_short = parse_export_hex("aa").expect_err("must reject short export id");
    assert!(err_short.contains("32-byte hex"));
}

#[test]
/// `roaming_intent_err` maps HTTP 409 duplicate intent.
fn roam_err_map_dup() {
    let msg = roaming_intent_err(
        reqwest::StatusCode::CONFLICT,
        "duplicate roaming intent already exists",
    );
    assert!(msg.contains("already started earlier"), "{msg}");
}

#[test]
/// `roaming_intent_err` maps HTTP 400 bodies.
fn roam_err_map_bad_req() {
    let msg = roaming_intent_err(reqwest::StatusCode::BAD_REQUEST, "invalid target domain");
    assert!(msg.contains("invalid"), "{msg}");
}

#[test]
/// Roaming intent POST plus polling until terminal status in one mocked window.
fn tx_send_roam_status_flow() {
    let rpc = spawn_mock_http_server(vec![
        (
            "POST /v1/roaming-intents HTTP/1.1",
            200,
            r#"{"intent_id":"intent-1","export_id":"export-1","status":"queued","duplicate":true}"#,
        ),
        (
            "GET /v1/roaming-intents/intent-1 HTTP/1.1",
            200,
            r#"{"status":"queued"}"#,
        ),
        (
            "GET /v1/roaming-intents/intent-1 HTTP/1.1",
            200,
            r#"{"status":"imported"}"#,
        ),
    ]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&[11u8; 32]);
    let export_tx = SignedTx::sign_body(
        &sk,
        0x2C00,
        0,
        7,
        TxBody::Export {
            to: [12u8; 32],
            target_domain: 0x4359,
            amount: 100,
            fee: 1,
        },
    );
    let client = reqwest::blocking::Client::new();
    let created = post_roaming_intent(&client, &rpc, &export_tx).expect("create must succeed");
    assert_eq!(created.intent_id, "intent-1");
    assert!(created.duplicate);
    let mut statuses = Vec::new();
    for _ in 0..2 {
        let st = get_roaming_intent_status(&client, &rpc, &created.intent_id)
            .expect("status must be readable");
        statuses.push(st.status.clone());
        if is_terminal_intent_status(&st.status) {
            break;
        }
    }
    assert_eq!(statuses, vec!["queued".to_string(), "imported".to_string()]);
}

#[test]
/// Local tx send path still POSTs to `/v1/tx` with 204 response.
fn tx_send_local_v1_tx() {
    let rpc = spawn_mock_http_server(vec![("POST /v1/tx HTTP/1.1", 204, "")]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&[21u8; 32]);
    let tx = SignedTx::sign_body(
        &sk,
        0x2C00,
        1,
        3,
        TxBody::Transfer {
            to: [22u8; 32],
            amount: 5,
            fee: 0,
        },
    );
    let client = reqwest::blocking::Client::new();
    post_signed_tx(&client, &rpc, &tx).expect("local tx send path must stay valid");
}

#[test]
/// `post_signed_tx` surfaces InsufficientMarks from RPC JSON error body.
fn tx_burn_err_insufficient_marks() {
    let rpc = spawn_mock_http_server(vec![("POST /v1/tx HTTP/1.1", 400, "InsufficientMarks")]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&[31u8; 32]);
    let tx = SignedTx::sign_body(
        &sk,
        0x2C00,
        1,
        9,
        TxBody::BurnMark {
            mark_amount: 99,
            beneficiary: None,
        },
    );
    let client = reqwest::blocking::Client::new();
    let err = post_signed_tx(&client, &rpc, &tx).expect_err("burn must surface rejection");
    assert!(err.contains("InsufficientMarks"), "{err}");
}

#[test]
/// `post_export_handoff` hits `/v1/export-provenance`.
fn tx_handoff_reg_prov_post() {
    let rpc = spawn_mock_http_server(vec![(
        "POST /v1/export-provenance HTTP/1.1",
        200,
        r#"{"export_id":"aa","registered":true,"duplicate":false}"#,
    )]);
    let client = reqwest::blocking::Client::new();
    let handoff = serde_json::json!({
        "proof_version": 1,
        "export_id": "aa"
    });
    post_export_handoff(&client, &rpc, &handoff).expect("handoff post must succeed");
}

#[test]
/// `parse_address_arg` error lists accepted pretty/canonical formats for bad pretty input.
fn parse_addr_arg_fmt_help() {
    let err = parse_address_arg("--to", "pwm1-CY-f00000003-tABCDEF")
        .expect_err("must reject malformed pretty input");
    assert!(err.contains("Invalid value for --to"));
    assert!(
        err.contains("Accepted formats: pretty pwm1-<label_or_$hex!>-f<flags8hex>-t<tail52hex>")
    );
    assert!(err.contains("canonical pwm1..."));
    assert!(err.contains("legacy PWMv0-... / hex"));
}

#[test]
/// Rejects legacy pretty addresses missing required `/LO` segment.
fn parse_addr_arg_lo_strict() {
    let legacy = "pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000";
    let err = parse_address_arg("--to", legacy).expect_err("must reject ambiguous pretty");
    assert!(err.contains("Invalid value for --to"));
    assert!(err.contains("missing '/LO'"));
    assert!(err.contains("strict pretty"));
    assert!(err.contains("canonical bech32dx"));
}

#[test]
/// Canonical regulatory address with `/00` LO parses.
fn parse_addr_arg_canon_lo0() {
    let mut id = [0u8; 32];
    id[0] = 0x2C;
    id[1] = 0x00;
    let canonical = pwm_core::account_id_to_bech32dx(&id);
    let parsed = parse_address_arg("--to", &canonical).expect("must accept canonical /00");
    assert_eq!(parsed, id);
}

#[test]
/// Canonical form with corrupted checksum is rejected.
fn parse_addr_arg_bad_cs() {
    let canonical = pwm_core::account_id_to_bech32dx(&[8u8; 32]);
    let mut bad = canonical.clone();
    let last = bad.pop().expect("non-empty");
    let replacement = if last == 'q' { 'p' } else { 'q' };
    bad.push(replacement);
    let err = parse_address_arg("--to", &bad).expect_err("must reject bad checksum");
    assert!(err.contains("Invalid value for --to"));
    assert!(err.contains("canonical pwm1..."));
}

#[test]
/// `validate_recipient_domain_policy` rejects unknown regulatory prefix.
fn tx_recv_unknown_reg_dom() {
    let mut id = [0u8; 32];
    id[0] = 0xC6;
    id[1] = 0x00;
    let err = validate_recipient_domain_policy(&id, Some("--to")).expect_err("must reject unknown");
    assert!(err.contains("not recognized by domain index"));
}

#[test]
fn tx_recipient_rejects_reserve_domain() {
    let mut id = [0u8; 32];
    id[0] = 0xE0;
    id[1] = 0x03;
    let err = validate_recipient_domain_policy(&id, Some("--to")).expect_err("must reject reserve");
    assert!(err.contains("reserve"));
    assert!(err.contains("cannot be used as transaction recipient"));
}

#[test]
fn tx_recipient_rejects_witness_domain() {
    let witness = lookup_by_raw(0xF003).expect("witness entry");
    assert_eq!(witness.category, DomainCategory::Witness);
    let mut id = [0u8; 32];
    id[0] = 0xF0;
    id[1] = 0x03;
    let err = validate_recipient_domain_policy(&id, Some("--beneficiary"))
        .expect_err("must reject witness");
    assert!(err.contains("witness-only"));
}

#[test]
/// Pretty reserve/witness IDs fail recipient policy after parse.
fn tx_path_recv_pol_reject() {
    let cases = [
        (
            "--to",
            "pwm1-$C600!-f00000000-t0000000000000000000000000000000000000000000000000000",
            "not recognized by domain index",
        ),
        (
            "--to",
            "pwm1-$E003!-f00000000-t0000000000000000000000000000000000000000000000000000",
            "reserve",
        ),
        (
            "--beneficiary",
            "pwm1-$F003!-f00000000-t0000000000000000000000000000000000000000000000000000",
            "witness-only",
        ),
    ];
    for (field, addr, expected) in cases {
        let parsed = parse_address_arg(field, addr).expect("pretty parse must succeed");
        let err = validate_recipient_domain_policy(&parsed, Some(field)).expect_err("must reject");
        assert!(err.contains(expected), "expected '{expected}' in '{err}'");
    }
}

#[test]
/// Country-code domain label parses as regulatory profile without `/LO` tail.
fn wal_prof_label_no_lo() {
    let entry = parse_domain_label_only("CY").expect("must parse label");
    assert_eq!(entry.category, DomainCategory::Regulatory);
}

#[test]
/// `post_import_retry` succeeds after export id becomes known.
fn tx_imp_retry_exp_known() {
    let rpc = spawn_mock_http_server(vec![
        (
            "POST /v1/tx HTTP/1.1",
            400,
            "invalid import: export_id is not known",
        ),
        ("POST /v1/tx HTTP/1.1", 204, ""),
    ]);
    let client = reqwest::blocking::Client::new();
    let sk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    let to = [10u8; 32];
    let export_id = [3u8; 32];
    let tx = SignedTx::sign_body(
        &sk,
        0x3200,
        0,
        0,
        TxBody::Import {
            to,
            amount: 25,
            export_id,
        },
    );

    post_import_retry(&client, &rpc, &tx, 2, Duration::from_millis(0))
        .expect("retry should succeed once export provenance becomes known");
}

#[test]
/// `ensure_import_sender` runs tx-init path when GET account returns 404.
fn tx_imp_auto_init_miss() {
    let from_pk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32])
        .verifying_key()
        .to_bytes();
    let from = pwm_core::hd::account_id_from_parts(&from_pk, 0);
    let from_hex = hex::encode(from);

    let get_from = Box::leak(format!("GET /v1/account/{from_hex} HTTP/1.1").into_boxed_str());
    let rpc = spawn_mock_http_server(vec![
        (get_from, 404, "account not found"),
        ("POST /v1/tx HTTP/1.1", 204, ""),
        (get_from, 200, r#"{"nonce":1,"initialized":true}"#),
    ]);

    let client = reqwest::blocking::Client::new();
    let source = TxSignerSource {
        sk: ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]),
        dom: 0x3200,
        idx: 0,
        from,
    };

    let nonce = ensure_import_sender(&client, &rpc, &source)
        .expect("auto tx-init should initialize sender");
    assert_eq!(nonce, 1);
}

#[test]
/// `ensure_import_sender` tx-init path when account exists but `initialized` is false.
fn tx_imp_auto_init_uninit() {
    let from_pk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32])
        .verifying_key()
        .to_bytes();
    let from = pwm_core::hd::account_id_from_parts(&from_pk, 0);
    let from_hex = hex::encode(from);

    let get_from = Box::leak(format!("GET /v1/account/{from_hex} HTTP/1.1").into_boxed_str());
    let rpc = spawn_mock_http_server(vec![
        (get_from, 200, r#"{"nonce":0,"initialized":false}"#),
        ("POST /v1/tx HTTP/1.1", 204, ""),
        (get_from, 200, r#"{"nonce":1,"initialized":true}"#),
    ]);

    let client = reqwest::blocking::Client::new();
    let source = TxSignerSource {
        sk: ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]),
        dom: 0x3200,
        idx: 0,
        from,
    };

    let nonce = ensure_import_sender(&client, &rpc, &source)
        .expect("auto tx-init should initialize uninitialized sender");
    assert_eq!(nonce, 1);
}

#[test]
/// Auto tx-init completes but unknown `export_id` on import still surfaces as error.
fn tx_imp_bad_exp_visible() {
    let sk = ed25519_dalek::SigningKey::from_bytes(&[9u8; 32]);
    let from_pk = sk.verifying_key().to_bytes();
    let from = pwm_core::hd::account_id_from_parts(&from_pk, 0);
    let from_hex = hex::encode(from);

    let get_from = Box::leak(format!("GET /v1/account/{from_hex} HTTP/1.1").into_boxed_str());
    let rpc = spawn_mock_http_server(vec![
        (get_from, 404, "account not found"),
        ("POST /v1/tx HTTP/1.1", 204, ""),
        (get_from, 200, r#"{"nonce":1,"initialized":true}"#),
        (
            "POST /v1/tx HTTP/1.1",
            400,
            "invalid import: export_id is not known",
        ),
    ]);

    let client = reqwest::blocking::Client::new();
    let source = TxSignerSource {
        sk,
        dom: 0x3200,
        idx: 0,
        from,
    };
    let nonce =
        ensure_import_sender(&client, &rpc, &source).expect("auto tx-init should complete first");
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Import {
            to: from,
            amount: 25,
            export_id: [0xEE; 32],
        },
    );

    let err = post_import_retry(&client, &rpc, &tx, 1, Duration::from_millis(0))
        .expect_err("invalid import must remain rejected after auto-init");
    assert!(err.contains("export_id is not known"));
}

#[test]
fn recipient_preflight_blocks_missing_account() {
    let to = [0x42u8; 32];
    let to_hex = hex::encode(to);
    let get_to = Box::leak(format!("GET /v1/account/{to_hex} HTTP/1.1").into_boxed_str());
    let rpc = spawn_mock_http_server(vec![(get_to, 404, "account not found")]);

    let client = reqwest::blocking::Client::new();
    let err = preflight_recipient_init(&client, &rpc, to, "tx-import")
        .expect_err("missing recipient must block");
    assert!(err.contains("recipient account not found"), "{err}");
    assert!(err.contains("tx-init"), "{err}");
}

#[test]
fn recipient_preflight_blocks_uninitialized_account() {
    let to = [0x43u8; 32];
    let to_hex = hex::encode(to);
    let get_to = Box::leak(format!("GET /v1/account/{to_hex} HTTP/1.1").into_boxed_str());
    let rpc = spawn_mock_http_server(vec![(get_to, 200, r#"{"nonce":0,"initialized":false}"#)]);

    let client = reqwest::blocking::Client::new();
    let err = preflight_recipient_init(&client, &rpc, to, "tx-send")
        .expect_err("uninitialized recipient must block");
    assert!(
        err.contains("recipient account is not initialized"),
        "{err}"
    );
    assert!(err.contains("tx-init"), "{err}");
}
