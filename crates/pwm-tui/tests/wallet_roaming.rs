mod common;
use common::*;

/// Schema v3 without explicit active account still loads the single listed account.
#[test]
fn load_v3_no_active() {
    let seed = [29u8; 32];
    let idx = 0u32;
    let sk_bytes = slip10_ed25519::derive_ed25519_private_key(&seed, &[0, idx]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pk = sk.verifying_key().to_bytes();
    let id = pwm_core::hd::account_id_from_parts(&pk, idx);
    let id_hex = hex::encode(id);
    let id_pretty = account_id_to_human(&id);
    let domain = u16::from_be_bytes([id[0], id[1]]);
    let flags = u32::from_be_bytes([id[2], id[3], id[4], id[5]]);
    let yaml = format!(
        r#"schema_version: 3
mode: plaintext_dev
accounts:
  - derivation_path: "m/0/{idx}"
    derivation_index: {idx}
    domain_u16: {domain}
    flags_mask_u32: 1023
    expected_flags_u32: 1
    flags_derived_u32: {flags}
    id_hex: "{id_hex}"
    id_pretty: "{id_pretty}"
master_seed_hex: "{seed_hex}"
signing_key_hex: "{signing_key_hex}"
address_book: []
"#,
        seed_hex = hex::encode(seed),
        signing_key_hex = hex::encode(sk.to_bytes())
    );
    let uniq = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("pwm-tui-v3-no-active-{uniq}.yml"));
    std::fs::write(&path, yaml).unwrap();
    let identity = load_wallet_identity(&path, None, 300, false).expect("load v3 no active");
    assert_eq!(identity.account_id, id);
    assert_eq!(identity.owned_accounts.len(), 1);
    let _ = std::fs::remove_file(&path);
}

/// Legacy `active_account_id_hex` does not override runtime; F6 uses UI-selected owner row.
#[test]
fn load_v3_ignore_legacy_active() {
    let seed = [30u8; 32];
    let default_idx = 2u32;
    let legacy_active_idx = 9u32;
    let (default_sk, default_id) = derived_account(&seed, default_idx);
    let (_legacy_sk, legacy_id) = derived_account(&seed, legacy_active_idx);
    let default_hex = hex::encode(default_id);
    let legacy_hex = hex::encode(legacy_id);
    let default_pretty = account_id_to_human(&default_id);
    let legacy_pretty = account_id_to_human(&legacy_id);
    let default_domain = u16::from_be_bytes([default_id[0], default_id[1]]);
    let legacy_domain = u16::from_be_bytes([legacy_id[0], legacy_id[1]]);
    let default_flags =
        u32::from_be_bytes([default_id[2], default_id[3], default_id[4], default_id[5]]);
    let legacy_flags = u32::from_be_bytes([legacy_id[2], legacy_id[3], legacy_id[4], legacy_id[5]]);
    let yaml = format!(
        r#"schema_version: 3
mode: plaintext_dev
active_account_id_hex: "{legacy_hex}"
accounts:
  - derivation_path: "m/0/{legacy_active_idx}"
    derivation_index: {legacy_active_idx}
    domain_u16: {legacy_domain}
    flags_mask_u32: 1023
    expected_flags_u32: 0
    flags_derived_u32: {legacy_flags}
    id_hex: "{legacy_hex}"
    id_pretty: "{legacy_pretty}"
  - derivation_path: "m/0/{default_idx}"
    derivation_index: {default_idx}
    domain_u16: {default_domain}
    flags_mask_u32: 1023
    expected_flags_u32: 0
    flags_derived_u32: {default_flags}
    id_hex: "{default_hex}"
    id_pretty: "{default_pretty}"
master_seed_hex: "{seed_hex}"
signing_key_hex: "{default_signing_key_hex}"
address_book: []
"#,
        seed_hex = hex::encode(seed),
        default_signing_key_hex = hex::encode(default_sk.to_bytes())
    );
    let uniq = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("pwm-tui-v3-legacy-active-{uniq}.yml"));
    std::fs::write(&path, yaml).unwrap();
    let identity = load_wallet_identity(&path, None, 300, false).expect("load v3");
    assert_eq!(identity.account_id, default_id);
    assert!(identity
        .owned_accounts
        .iter()
        .any(|a| a.id == default_id && a.is_active));
    assert!(identity
        .owned_accounts
        .iter()
        .any(|a| a.id == legacy_id && !a.is_active));
    let owner_rows = vec![
        AcctRow {
            balance_pwm: 1,
            id_hex: legacy_hex,
            ..mk_acct_row(legacy_id)
        },
        AcctRow {
            balance_pwm: 2,
            id_hex: default_hex,
            ..mk_acct_row(default_id)
        },
    ];
    let form = f6_build_send_form(&IdentitySource::Wallet(identity), &owner_rows, 0, &[], 0)
        .expect("selected Owner remains sender source");
    assert_eq!(form.from, account_id_to_human(&legacy_id));
    let _ = std::fs::remove_file(&path);
}

/// Plaintext unlocked wallet has empty lock suffix in status line.
#[test]
fn lock_suffix_plain_empty() {
    let owner = [9u8; 32];
    let w = WalletIdentity {
        account_id: owner,
        account_id_human: account_id_to_human(&owner),
        domain: 0x4359,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("x.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    };
    assert!(identity_lock_status_suffix(&IdentitySource::Wallet(w)).is_empty());
}

/// Short ASCII strings pass through `ellipsis_middle_ascii` unchanged.
#[test]
fn ellipsis_mid_short_ok() {
    assert_eq!(pwm_tui::ellipsis_middle_ascii("abc", 2, 2), "abc");
}

/// Long ASCII gets middle ellipsis with requested head/tail widths.
#[test]
fn ellipsis_mid_long() {
    let s = "012345678901234567890";
    assert_eq!(pwm_tui::ellipsis_middle_ascii(s, 4, 4), "0123...7890");
}

/// Footer head line keeps short tips verbatim.
#[test]
fn footer_head_short() {
    let s = "height=2 tip=deadbeef";
    assert_eq!(pwm_tui::format_footer_head_line(s), s);
}

/// Long chain tips are truncated with ellipsis while keeping prefix.
#[test]
fn footer_head_long_trunc() {
    let tip = "ab".repeat(40);
    let head = format!("height=9 tip={tip}");
    let got = pwm_tui::format_footer_head_line(&head);
    assert!(got.contains("..."));
    assert!(got.starts_with("height=9 tip="));
    assert!(got.len() < head.len());
}

/// Offline RPC is highlighted first; order includes accounts, shard hint, F-keys, debug.
#[test]
fn footer_rpc_offline_order() {
    use ratatui::style::Color;
    let line = pwm_tui::status_footer_line(
        "height=1 tip=x",
        "accounts: offline",
        "wallet: ok",
        "lock",
        pwm_tui::RpcHealth::Offline,
        true,
        "http://example:3030",
        None,
        None,
        false,
    );
    assert_eq!(line.spans[0].content, "RPC offline");
    assert_eq!(line.spans[0].style.fg, Some(Color::Red));
    let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        flat.starts_with(
            "RPC offline | accounts: offline | height=1 tip=x | RPC=http://example:3030 (shard A?) | Tab switch"
        ),
        "unexpected order/prefix: {flat}"
    );
    assert!(
        flat.contains("F3 lock"),
        "footer should advertise F3: {flat}"
    );
    assert!(flat.contains("F4 encrypt"), "{flat}");
    assert!(flat.contains("F5 burn"), "{flat}");
    assert!(!flat.contains("F7 inter-shard->CLI"), "{flat}");
    assert!(flat.contains("PWM_TUI_DEBUG=1"));
    assert!(flat.contains("wallet: ok"));
}

/// Online healthy RPC: no red health prefix; unknown shard hint in rpc context.
#[test]
fn footer_rpc_online_one() {
    use ratatui::style::Color;
    let line = pwm_tui::status_footer_line(
        "…",
        "",
        "",
        "unlock",
        pwm_tui::RpcHealth::Online,
        false,
        "http://127.0.0.1:4040",
        None,
        None,
        false,
    );
    assert!(
        line.spans.iter().all(|s| s.style.fg != Some(Color::Red)),
        "online RPC should not emit red health spans"
    );
    let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(
        flat.starts_with("… | RPC=http://127.0.0.1:4040 (unknown shard) | Tab switch"),
        "{flat}"
    );
    assert!(flat.contains("unknown shard"), "{flat}");
}

/// Demo RPC ports map to human shard hints (A/B vs unknown).
#[test]
fn shard_hint_demo_ports() {
    assert_eq!(pwm_tui::shard_hint_rpc("http://127.0.0.1:3030"), "shard A?");
    assert_eq!(pwm_tui::shard_hint_rpc("http://127.0.0.1:3031"), "shard B?");
    assert_eq!(
        pwm_tui::shard_hint_rpc("http://127.0.0.1:4040"),
        "unknown shard"
    );
}

/// Status JSON shard labels accept CY/DO/A/neutral spellings.
#[test]
fn parse_shard_label_vals() {
    let cy = serde_json::json!({ "shard": "CY" });
    let do_alias = serde_json::json!({ "shard": "DO" });
    let compat = serde_json::json!({ "shard": "A" });
    let neutral = serde_json::json!({ "shard": "neutral" });
    assert_eq!(
        pwm_tui::parse_status_shard_label(&cy).as_deref(),
        Some("CY")
    );
    assert_eq!(
        pwm_tui::parse_status_shard_label(&do_alias).as_deref(),
        Some("DO")
    );
    assert_eq!(
        pwm_tui::parse_status_shard_label(&compat).as_deref(),
        Some("A")
    );
    assert_eq!(
        pwm_tui::parse_status_shard_label(&neutral).as_deref(),
        Some("neutral")
    );
}

/// RPC context prefers `/status` shard label over port-only heuristic.
#[test]
fn rpc_ctx_prefers_status() {
    let ctx = pwm_tui::rpc_context_label("http://127.0.0.1:3130", Some("CY"));
    assert_eq!(ctx, "RPC=http://127.0.0.1:3130 (CY)");
}

/// Inter-shard helper text references handoff/import CLI steps.
#[test]
fn xshard_cli_hint_text() {
    let msg = pwm_tui::shard_cli_hint("http://127.0.0.1:3030");
    assert!(msg.contains("roaming-intent relay"), "{msg}");
    assert!(msg.contains("trusted"), "{msg}");
    assert!(msg.contains("tx-handoff-register"), "{msg}");
    assert!(msg.contains("tx-import"), "{msg}");
}

/// Same high-byte domain stays local; different domain prefix is roaming.
#[test]
fn route_same_hi_local() {
    let mut from = [0u8; 32];
    let mut to = [0u8; 32];
    from[0] = 0x2C;
    from[1] = 0x01;
    to[0] = 0x2C;
    to[1] = 0xF0;
    assert!(!pwm_tui::is_cross_domain_route(&from, &to));
    to[0] = 0x32;
    assert!(pwm_tui::is_cross_domain_route(&from, &to));
}

/// Short inter-shard status blurb is one line and points at F7.
#[test]
fn xshard_status_short_f7() {
    let msg = pwm_tui::inter_shard_status_short();
    assert!(!msg.contains('\n'), "{msg}");
    assert!(msg.contains("roaming-intent lifecycle"), "{msg}");
    assert!(msg.contains("queued"), "{msg}");
    assert!(msg.contains("failed"), "{msg}");
}

/// 409 cross-domain policy adds roaming relay / CLI handoff wording.
#[test]
fn xfer_err_xshard_hint() {
    let body =
        "cross-domain transfer is disabled on local tx path; use explicit EXPORT/IMPORT flow";
    let msg = pwm_tui::format_submit_transfer_error(
        reqwest::StatusCode::CONFLICT,
        body,
        "http://127.0.0.1:3030",
    );
    assert!(msg.contains("submit failed: 409 Conflict"), "{msg}");
    assert!(msg.contains("roaming-intent relay"), "{msg}");
    assert!(msg.contains("tx-handoff-register"), "{msg}");
    assert!(msg.contains("tx-import"), "{msg}");
}

/// Non-policy submit failures keep the generic error string.
#[test]
fn xfer_err_generic() {
    let msg = pwm_tui::format_submit_transfer_error(
        reqwest::StatusCode::BAD_REQUEST,
        "bad amount",
        "http://127.0.0.1:3030",
    );
    assert_eq!(msg, "submit failed: 400 Bad Request bad amount");
}

/// Roaming submit happy path: duplicate intent then polls to imported.
#[test]
fn roam_dup_to_imported() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    let rpc = spawn_mock_http_server(vec![
        (
            "GET /v1/account/",
            200,
            r#"{"nonce":7,"balance":0,"initialized":true}"#,
        ),
        (
            "POST /v1/export-readiness HTTP/1.1",
            200,
            r#"{"ready":true,"export_id":"export-1","expires_at_unix_ms":123456,"reason_code":"ready","recovery_hint":"ok"}"#,
        ),
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
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::set_var("PWM_RPC", &rpc) };
    let sk = ed25519_dalek::SigningKey::from_bytes(&[77u8; 32]);
    let from = account_id_from_parts(&sk.verifying_key().to_bytes(), 0);
    let domain = domain_of_account_id(&from);
    let to = [
        0x43u8, 0x59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 2,
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: from,
        account_id_human: account_id_to_human(&from),
        domain,
        derivation_index: 0,
        signing_key: Some(sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("unused.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let out = submit_roaming_intent(&from, &to, 1_000_000, 0, &identity).expect("must pass");
    assert!(out.contains("duplicate"), "{out}");
    assert!(out.contains("1) preflight"), "{out}");
    assert!(out.contains("2) export submit"), "{out}");
    assert!(out.contains("3) handoff/provenance"), "{out}");
    assert!(out.contains("4) import submit"), "{out}");
    assert!(out.contains("status=imported"), "{out}");
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::remove_var("PWM_RPC") };
}

/// Roaming flow surfaces invalid roaming request after export readiness.
#[test]
fn roam_reject_bad_req() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    let rpc = spawn_mock_http_server(vec![
        (
            "GET /v1/account/",
            200,
            r#"{"nonce":1,"balance":0,"initialized":true}"#,
        ),
        (
            "POST /v1/export-readiness HTTP/1.1",
            200,
            r#"{"ready":true,"export_id":"export-2","expires_at_unix_ms":123456,"reason_code":"ready","recovery_hint":"ok"}"#,
        ),
        (
            "POST /v1/roaming-intents HTTP/1.1",
            400,
            "invalid target domain",
        ),
    ]);
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::set_var("PWM_RPC", &rpc) };
    let sk = ed25519_dalek::SigningKey::from_bytes(&[78u8; 32]);
    let from = account_id_from_parts(&sk.verifying_key().to_bytes(), 0);
    let domain = domain_of_account_id(&from);
    let to = [
        0x43u8, 0x59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 4,
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: from,
        account_id_human: account_id_to_human(&from),
        domain,
        derivation_index: 0,
        signing_key: Some(sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("unused.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let err = submit_roaming_intent(&from, &to, 1_000_000, 0, &identity).unwrap_err();
    assert!(err.contains("2) export submit"), "{err}");
    assert!(err.contains("FAIL"), "{err}");
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::remove_var("PWM_RPC") };
}

/// Roaming submit includes missing-preflight server hint in the chained error text.
#[test]
fn roam_miss_preflight_hint() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    let rpc = spawn_mock_http_server(vec![
        (
            "GET /v1/account/",
            200,
            r#"{"nonce":2,"balance":0,"initialized":true}"#,
        ),
        (
            "POST /v1/export-readiness HTTP/1.1",
            200,
            r#"{"ready":true,"export_id":"export-3","expires_at_unix_ms":123456,"reason_code":"ready","recovery_hint":"ok"}"#,
        ),
        (
            "POST /v1/roaming-intents HTTP/1.1",
            409,
            r#"{"code":"missing_preflight","hint":"Run /v1/export-readiness for this exact EXPORT payload before submit.","message":"export readiness reject: code=missing_preflight; hint=Run /v1/export-readiness for this exact EXPORT payload before submit."}"#,
        ),
    ]);
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::set_var("PWM_RPC", &rpc) };
    let sk = ed25519_dalek::SigningKey::from_bytes(&[80u8; 32]);
    let from = account_id_from_parts(&sk.verifying_key().to_bytes(), 0);
    let domain = domain_of_account_id(&from);
    let to = [
        0x43u8, 0x59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 8,
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: from,
        account_id_human: account_id_to_human(&from),
        domain,
        derivation_index: 0,
        signing_key: Some(sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("unused.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let err = submit_roaming_intent(&from, &to, 1_000_000, 0, &identity).unwrap_err();
    assert!(err.contains("1) preflight"), "{err}");
    assert!(err.contains("2) export submit"), "{err}");
    assert!(err.contains("missing_preflight"), "{err}");
    assert!(err.contains("Run /v1/export-readiness"), "{err}");
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::remove_var("PWM_RPC") };
}

/// Poll stops on expired intent with import-stage failure wording.
#[test]
fn roam_expired_import() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    let rpc = spawn_mock_http_server(vec![
        (
            "GET /v1/account/",
            200,
            r#"{"nonce":9,"balance":0,"initialized":true}"#,
        ),
        (
            "POST /v1/export-readiness HTTP/1.1",
            200,
            r#"{"ready":true,"export_id":"export-exp","expires_at_unix_ms":123456,"reason_code":"ready","recovery_hint":"ok"}"#,
        ),
        (
            "POST /v1/roaming-intents HTTP/1.1",
            200,
            r#"{"intent_id":"intent-exp","export_id":"export-exp","status":"queued","duplicate":false}"#,
        ),
        (
            "GET /v1/roaming-intents/intent-exp HTTP/1.1",
            200,
            r#"{"status":"expired"}"#,
        ),
    ]);
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::set_var("PWM_RPC", &rpc) };
    let sk = ed25519_dalek::SigningKey::from_bytes(&[79u8; 32]);
    let from = account_id_from_parts(&sk.verifying_key().to_bytes(), 0);
    let domain = domain_of_account_id(&from);
    let to = [
        0x43u8, 0x59, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 6,
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: from,
        account_id_human: account_id_to_human(&from),
        domain,
        derivation_index: 0,
        signing_key: Some(sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("unused.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let err = submit_roaming_intent(&from, &to, 1_000_000, 0, &identity).unwrap_err();
    assert!(err.contains("4) import submit: FAIL"), "{err}");
    assert!(err.contains("expired"), "{err}");
    // SAFETY: serialized by TEST_ENV_LOCK.
    unsafe { std::env::remove_var("PWM_RPC") };
}

/// Locked wallet cannot satisfy signing material for auto-init path.
#[test]
fn preflight_sel_init_blocks_locked() {
    let row = AcctRow {
        id_hex: "09".repeat(32),
        initialized: false,
        ..mk_acct_row([9u8; 32])
    };
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: [9u8; 32],
        account_id_human: account_id_to_human(&[9u8; 32]),
        domain: 0x2C00,
        derivation_index: 0,
        signing_key: None,
        unlock_expires_at: None,
        wallet_is_encrypted: true,
        wallet_path: PathBuf::from("locked.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let err = preflight_sel_init_auto(Some(&row), "F6 send", &identity).unwrap_err();
    assert!(err.contains("F6 send blocked"), "{err}");
    assert!(err.contains("auto-init"), "{err}");
    assert!(err.contains("tx-init"), "{err}");
}

/// Initialized or absent row yields Ok(None) / Ok(Some(auto-init msg)); ready row allows preflight.
#[test]
fn preflight_sel_ready_ok() {
    let row = AcctRow {
        balance_pwm: 1,
        nonce: 3,
        id_hex: "07".repeat(32),
        label: Some("ok".into()),
        ..mk_acct_row([7u8; 32])
    };
    assert!(matches!(
        preflight_sel_init_auto(Some(&row), "F5 burn", &IdentitySource::SeedFallback),
        Ok(None)
    ));
    assert!(matches!(
        preflight_sel_init_auto(None, "F6 send", &IdentitySource::SeedFallback),
        Ok(None)
    ));
}

/// Same-shard send preflight errors when the recipient row is missing from cache.
#[test]
fn xfer_dst_miss_same_shard() {
    let from = [
        0x2Cu8, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 1,
    ];
    let to = [
        0x2Cu8, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 2,
    ];
    let err = preflight_xfer_dst(&from, &to, &[], &[])
        .expect_err("same-shard missing recipient must block");
    assert!(err.contains("recipient is missing"), "{err}");
}

/// Known recipient row that is not initialized blocks the transfer preflight.
#[test]
fn xfer_dst_uninit_row() {
    let from = [0x2Cu8; 32];
    let mut to = [0x43u8; 32];
    to[1] = 0x59;
    let row = AcctRow {
        id_hex: hex::encode(to),
        initialized: false,
        ..mk_acct_row(to)
    };
    let err = preflight_xfer_dst(&from, &to, &[], &[row])
        .expect_err("known uninitialized recipient must block");
    assert!(err.contains("recipient is not initialized"), "{err}");
}

/// Recipient account RPC preflight maps 404 missing account to operator text.
#[test]
fn recv_rpc_preflight_404() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    let to = [0x44u8; 32];
    let to_hex = hex::encode(to);
    let get_to = Box::leak(format!("GET /v1/account/{to_hex} HTTP/1.1").into_boxed_str());
    let rpc = spawn_mock_http_server(vec![(get_to, 404, "account not found")]);
    let client = reqwest::blocking::Client::new();
    let err = preflight_recipient_rpc(&client, &rpc, to).expect_err("missing recipient must block");
    assert!(err.contains("recipient account not found"), "{err}");
}

/// On-disk re-encrypt without unlocked JSON payload is rejected (need passphrase/unlock).
#[test]
fn enc_disk_rekey_denied() {
    use pwm_core::hd::account_id_from_parts;
    let raw_key = [21u8; 32];
    let sk = ed25519_dalek::SigningKey::from_bytes(&raw_key);
    let pk = sk.verifying_key().to_bytes();
    let di = 0u32;
    let account_id = account_id_from_parts(&pk, di);
    let human = account_id_to_human(&account_id);
    let domain = u16::from_be_bytes([account_id[0], account_id[1]]);
    let payload = serde_json::json!({"signing_key_hex": hex::encode(sk.to_bytes())});
    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    let (salt_b64, nonce_b64, enc_b64) = seal_test_enc_wallet("sec", &payload_bytes);
    let iters = pwm_core::WALLET_KDF_ITERS;
    let yaml = format!(
        "mode: encrypted\nderivation_index: {di}\nderivation_path: m/0/{di}\ndomain_u16: {domain}\naccount_id_human: {human}\nkdf: pbkdf2_sha256\nkdf_iters: {iters}\nkdf_salt_b64: {salt_b64}\naead_nonce_b64: {nonce_b64}\nencrypted_payload_b64: {enc_b64}\naddress_book: []\n"
    );
    let path =
        std::env::temp_dir().join(format!("pwm-tui-rekey-denied-{}.yml", std::process::id()));
    std::fs::write(&path, yaml).unwrap();
    let err = pwm_tui::wallet_rekey(&path, "newpw", None).expect_err("must fail");
    assert!(
        err.contains("unlock") || err.contains("Passphrase"),
        "err={err}"
    );
    let _ = std::fs::remove_file(&path);
}

/// submit_transfer bails early when encrypted wallet has no signing key.
#[test]
fn xfer_locked_fast_fail() {
    let from = [42u8; 32];
    let to = [43u8; 32];
    let id = IdentitySource::Wallet(WalletIdentity {
        account_id: from,
        account_id_human: account_id_to_human(&from),
        domain: 0x2C00,
        derivation_index: 0,
        signing_key: None,
        unlock_expires_at: None,
        wallet_is_encrypted: true,
        wallet_path: PathBuf::from("unused.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let err = pwm_tui::submit_transfer(&from, &to, 1_000_000, 0, &id).expect_err("must be locked");
    assert!(err.contains("wallet is locked"), "err={err}");
}

/// After unlock timeout + auto-lock, F6 surfaces the locked-wallet modal error.
#[test]
fn f6_locked_after_timeout() {
    let mut id = IdentitySource::Wallet(WalletIdentity {
        account_id: [50u8; 32],
        account_id_human: account_id_to_human(&[50u8; 32]),
        domain: 0x2C00,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[51u8; 32])),
        unlock_expires_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(1)),
        wallet_is_encrypted: true,
        wallet_path: PathBuf::from("unused.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: Some(vec![1, 2, 3]),
    });
    wallet_apply_auto_lock(&mut id);
    let err = match f6_build_send_form(&id, &[], 0, &[], 0) {
        Ok(_) => panic!("must stay locked"),
        Err(e) => e,
    };
    assert!(
        err.contains("Wallet is locked"),
        "F6 must use locked-wallet path, err={err}"
    );
}

/// Encrypt plaintext YAML wallet on disk and reload as encrypted + unlocked.
#[test]
fn enc_plain_yml_roundtrip() {
    use base64::Engine;
    use pwm_core::hd::account_id_from_parts;
    use slip10_ed25519::derive_ed25519_private_key;
    let seed = [8u8; 32];
    let idx = 1u32;
    let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pk = sk.verifying_key().to_bytes();
    let account_id = account_id_from_parts(&pk, idx);
    let human = account_id_to_human(&account_id);
    let domain = u16::from_be_bytes([account_id[0], account_id[1]]);
    let b64 = base64::engine::general_purpose::STANDARD;
    let yaml = format!(
        r#"schema_version: 1
mode: plaintext_dev
created_at_unix_sec: 1
derivation_index: {idx}
derivation_path: m/0/{idx}
domain_u16: {domain}
account_id_hex: "{}"
account_id_human: "{human}"
flags_mask_u32: 0
expected_flags_u32: 0
flags_derived_u32: 0
master_seed_hex: "{}"
master_seed_b64: "{}"
signing_key_hex: "{}"
signing_key_b64: "{}"
verifying_key_hex: "{}"
verifying_key_b64: "{}"
address_book: []
"#,
        hex::encode(account_id),
        hex::encode(seed),
        b64.encode(seed),
        hex::encode(sk.to_bytes()),
        b64.encode(sk.to_bytes()),
        hex::encode(pk),
        b64.encode(pk),
    );
    let path = std::env::temp_dir().join(format!("pwm-tui-plain-enc-{}.yml", std::process::id()));
    std::fs::write(&path, yaml).unwrap();
    pwm_tui::wallet_rekey(&path, "encrypt-me", None).expect("encrypt");
    let id = load_wallet_identity(&path, Some("encrypt-me"), 300, false).expect("reload encrypted");
    assert!(id.wallet_is_encrypted);
    assert!(id.signing_key.is_some());
    let _ = std::fs::remove_file(&path);
}

/// Re-key to new passphrase invalidates old password on subsequent loads.
#[test]
fn wallet_rekey_new_pw() {
    use pwm_core::hd::account_id_from_parts;
    let raw_key = [31u8; 32];
    let sk = ed25519_dalek::SigningKey::from_bytes(&raw_key);
    let pk = sk.verifying_key().to_bytes();
    let di = 0u32;
    let account_id = account_id_from_parts(&pk, di);
    let human = account_id_to_human(&account_id);
    let domain = u16::from_be_bytes([account_id[0], account_id[1]]);
    let payload = serde_json::json!({"signing_key_hex": hex::encode(sk.to_bytes())});
    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    let iters = pwm_core::WALLET_KDF_ITERS;
    let (salt_b64, nonce_b64, enc_b64) = seal_test_enc_wallet("old-pass", &payload_bytes);
    let yaml = format!(
        "mode: encrypted\nderivation_index: {di}\nderivation_path: m/0/{di}\ndomain_u16: {domain}\naccount_id_human: {human}\nkdf: pbkdf2_sha256\nkdf_iters: {iters}\nkdf_salt_b64: {salt_b64}\naead_nonce_b64: {nonce_b64}\nencrypted_payload_b64: {enc_b64}\naddress_book: []\n"
    );
    let uniq = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("pwm-tui-rekey-roundtrip-{uniq}.yml"));
    std::fs::write(&path, yaml).unwrap();

    let unlocked = load_wallet_identity(&path, Some("old-pass"), 300, false).expect("unlock old");
    let decrypted = unlocked
        .secret_payload_plaintext
        .clone()
        .expect("decrypted payload must be cached after unlock");
    pwm_tui::wallet_rekey(&path, "new-pass", Some(decrypted.as_slice())).expect("rekey");

    let old_err = match load_wallet_identity(&path, Some("old-pass"), 300, false) {
        Ok(_) => panic!("old passphrase must fail after re-key"),
        Err(e) => e,
    };
    assert!(old_err.contains("failed to decrypt"), "old_err={old_err}");
    let reloaded =
        load_wallet_identity(&path, Some("new-pass"), 300, false).expect("new must load");
    assert!(reloaded.wallet_is_encrypted);
    assert!(reloaded.signing_key.is_some());
    let _ = std::fs::remove_file(&path);
}

/// Corrupt ciphertext after re-key fails load without panicking.
#[test]
fn wallet_rekey_corrupt_fail() {
    use pwm_core::hd::account_id_from_parts;
    let raw_key = [41u8; 32];
    let sk = ed25519_dalek::SigningKey::from_bytes(&raw_key);
    let pk = sk.verifying_key().to_bytes();
    let di = 0u32;
    let account_id = account_id_from_parts(&pk, di);
    let human = account_id_to_human(&account_id);
    let domain = u16::from_be_bytes([account_id[0], account_id[1]]);
    let payload = serde_json::json!({"signing_key_hex": hex::encode(sk.to_bytes())});
    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    let iters = pwm_core::WALLET_KDF_ITERS;
    let (salt_b64, nonce_b64, enc_b64) = seal_test_enc_wallet("old-pass", &payload_bytes);
    let yaml = format!(
        "mode: encrypted\nderivation_index: {di}\nderivation_path: m/0/{di}\ndomain_u16: {domain}\naccount_id_human: {human}\nkdf: pbkdf2_sha256\nkdf_iters: {iters}\nkdf_salt_b64: {salt_b64}\naead_nonce_b64: {nonce_b64}\nencrypted_payload_b64: {enc_b64}\naddress_book: []\n"
    );
    let uniq = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("pwm-tui-rekey-corrupt-{uniq}.yml"));
    std::fs::write(&path, yaml).unwrap();

    let unlocked = load_wallet_identity(&path, Some("old-pass"), 300, false).expect("unlock old");
    let decrypted = unlocked
        .secret_payload_plaintext
        .clone()
        .expect("decrypted payload must be cached after unlock");
    pwm_tui::wallet_rekey(&path, "new-pass", Some(decrypted.as_slice())).expect("rekey");

    let after_rekey = std::fs::read_to_string(&path).expect("read yaml");
    let marker = "encrypted_payload_b64:";
    let marker_pos = after_rekey.find(marker).expect("encrypted payload field");
    let value_start = marker_pos + marker.len();
    let line_end = after_rekey[value_start..]
        .find('\n')
        .map(|p| value_start + p)
        .unwrap_or(after_rekey.len());
    let mut corrupted = after_rekey.clone();
    corrupted.replace_range(value_start..line_end, " not-base64-ciphertext ");
    std::fs::write(&path, corrupted).expect("write corrupted yaml");

    let err = match load_wallet_identity(&path, Some("new-pass"), 300, false) {
        Ok(_) => panic!("corrupted encrypted payload must fail safely"),
        Err(e) => e,
    };
    assert!(
        err.contains("encrypted_payload_b64:") || err.contains("failed to decrypt"),
        "err={err}"
    );
    let _ = std::fs::remove_file(&path);
}

/// `OpStatus` string labels stay stable for UI/history consumers.
#[test]
fn op_status_lbl_stable() {
    assert_eq!(pwm_tui::OpStatus::Pending.as_str(), "pending");
    assert_eq!(pwm_tui::OpStatus::Ok.as_str(), "ok");
    assert_eq!(pwm_tui::OpStatus::Error.as_str(), "error");
}

/// Operation history ring buffer drops oldest items beyond max.
#[test]
fn op_hist_cap_max() {
    let mut hist = Vec::new();
    for i in 0..(pwm_tui::OP_HISTORY_MAX_ITEMS + 5) {
        pwm_tui::push_op_history(
            &mut hist,
            pwm_tui::OperationHistoryEntry {
                req_id: i as u64,
                created_unix_secs: i as u64,
                from_human: "from".into(),
                to_human: "to".into(),
                amount_units: 1,
                fee_units: 0,
                nonce: i as u64,
                status: pwm_tui::OpStatus::Pending,
                note: "queued".into(),
            },
        );
    }
    assert_eq!(hist.len(), pwm_tui::OP_HISTORY_MAX_ITEMS);
    assert_eq!(hist[0].req_id, (pwm_tui::OP_HISTORY_MAX_ITEMS + 4) as u64);
}

/// Status update finds matching `req_id` and mutates note/status.
#[test]
fn op_hist_set_req() {
    let mut hist = vec![pwm_tui::OperationHistoryEntry {
        req_id: 17,
        created_unix_secs: 1,
        from_human: "f".into(),
        to_human: "t".into(),
        amount_units: 7,
        fee_units: 1,
        nonce: 17,
        status: pwm_tui::OpStatus::Pending,
        note: "submitting".into(),
    }];
    let changed =
        pwm_tui::set_op_history_status(&mut hist, 17, pwm_tui::OpStatus::Ok, "sent".into());
    assert!(changed);
    assert_eq!(hist[0].status, pwm_tui::OpStatus::Ok);
    assert_eq!(hist[0].note, "sent");
}

/// Submit completion updates history even if the send form UI is already closed.
#[test]
fn submit_done_hist_closed() {
    let mut inflight_send_req_id = Some(42_u64);
    let mut hist = vec![pwm_tui::OperationHistoryEntry {
        req_id: 42,
        created_unix_secs: 1,
        from_human: "f".into(),
        to_human: "t".into(),
        amount_units: 7,
        fee_units: 1,
        nonce: 42,
        status: pwm_tui::OpStatus::Pending,
        note: "submitting".into(),
    }];
    // Form is closed (None), but SubmitDone must still resolve op_history.
    let changed = pwm_tui::handle_submit_done_history(
        &mut inflight_send_req_id,
        &mut hist,
        42,
        &Ok("sent".into()),
    );
    assert!(changed);
    assert_eq!(inflight_send_req_id, None);
    assert_eq!(hist[0].status, pwm_tui::OpStatus::Ok);
    assert_eq!(hist[0].note, "sent");
}

/// Replay guard blocks a second submit while multi-step flow diagnostics are active.
#[test]
fn send_replay_guard_flow() {
    let mut form = pwm_tui::SendForm::new("from".into(), "to".into(), true);
    form.apply_submit_result(
        &Ok("Cross-shard flow diagnostics:\n1) preflight OK\n2) export pending".into()),
        None,
    );
    let msg = pwm_tui::send_replay_guard_status(&form, None).expect("must block replay");
    assert!(msg.contains("step flow is active"), "{msg}");
}

/// Step flow auto-advances after the idle timeout elapses.
#[test]
fn send_flow_auto_step() {
    let now = Instant::now();
    let mut flow = pwm_tui::SendStepFlow::from_submit_message(
        "Cross-shard flow diagnostics:\n1) preflight OK\n2) export OK",
        false,
        now - pwm_tui::SEND_FLOW_STEP_TIMEOUT,
    );
    assert!(flow.is_active());
    let changed = flow.auto_advance_if_due(now, pwm_tui::SEND_FLOW_STEP_TIMEOUT);
    assert!(changed);
    assert!(!flow.is_active());
    assert_eq!(flow.shown_steps, 2);
}

/// Enter advances staged send flow without restarting an HTTP submit.
#[test]
fn send_flow_enter_done() {
    let mut form = pwm_tui::SendForm::new("from".into(), "to".into(), true);
    form.apply_submit_result(
        &Ok("Cross-shard flow diagnostics:\n1) preflight OK\n2) export submit: OK".into()),
        None,
    );
    assert!(form.flow_is_active());
    let advanced = form.try_advance_flow(Instant::now());
    assert!(advanced, "enter-like advance must move to next step");
    assert!(
        !form.flow_is_active(),
        "flow should complete after second step"
    );
    assert!(
        form.status.contains("Flow completed"),
        "status must describe completed flow: {}",
        form.status
    );
}

/// Submit error leaves error UI open until the operator dismisses with Esc.
#[test]
fn send_err_keeps_form() {
    let mut form = pwm_tui::SendForm::new("from".into(), "to".into(), true);
    form.apply_submit_result(
        &Err("Cross-shard flow diagnostics:\n1) preflight OK\n2) export submit: FAIL".into()),
        None,
    );
    assert!(form.status_is_error);
    assert!(form.flow.as_ref().map(|f| f.failed).unwrap_or(false));
    assert!(form.flow.is_some());
}

/// Failed flow must be acknowledged (Esc) before another send attempt.
#[test]
fn send_flow_fail_esc() {
    let mut form = pwm_tui::SendForm::new("from".into(), "to".into(), true);
    form.apply_submit_result(
        &Err("Cross-shard flow diagnostics:\n1) preflight OK\n2) export submit: FAIL".into()),
        None,
    );
    assert!(form.flow_is_active());
    let _ = form.try_advance_flow(Instant::now());
    assert!(!form.flow_is_active());
    let msg = pwm_tui::send_replay_guard_status(&form, None).expect("failed flow must lock");
    assert!(msg.contains("press Esc"), "{msg}");
}

/// Enter does not resubmit immediately after a hard send failure path.
#[test]
fn send_enter_blk_fail() {
    let mut form = pwm_tui::SendForm::new("from".into(), "to".into(), true);
    form.apply_submit_result(&Err("send failed hard".into()), None);
    assert!(!form.flow_is_active());
    let msg = pwm_tui::send_replay_guard_status(&form, None).expect("must block enter replay");
    assert!(msg.contains("send failed"), "{msg}");
}

/// Pending address-book prompt survives error state until explicit close handling.
#[test]
fn send_pending_book_esc() {
    let mut form = pwm_tui::SendForm::new("from".into(), "to".into(), true);
    form.apply_submit_result(&Ok("sent".into()), Some("pwm1-first".into()));
    assert_eq!(form.pending_book_prompt_to.as_deref(), Some("pwm1-first"));
    form.apply_submit_result(&Err("send failed hard".into()), None);
    let msg = pwm_tui::send_replay_guard_status(&form, None).expect("must stay blocked");
    assert!(msg.contains("press Esc"), "{msg}");
    assert_eq!(form.pending_book_prompt_to.as_deref(), Some("pwm1-first"));
    let deferred = form.take_book_prompt();
    assert_eq!(deferred.as_deref(), Some("pwm1-first"));
    assert!(form.pending_book_prompt_to.is_none());
}

/// Roaming HTTP errors map duplicate/invalid/expired cases to operator strings.
#[test]
fn roam_err_map_cases() {
    let duplicate = pwm_tui::format_roaming_error(
        reqwest::StatusCode::CONFLICT,
        "duplicate roaming intent already exists",
    );
    assert!(duplicate.contains("already exists"), "{duplicate}");
    let invalid =
        pwm_tui::format_roaming_error(reqwest::StatusCode::BAD_REQUEST, "invalid target_domain");
    assert!(invalid.contains("invalid request"), "{invalid}");
    let expired = pwm_tui::format_roaming_error(
        reqwest::StatusCode::CONFLICT,
        "intent expired at current height",
    );
    assert!(expired.contains("expired"), "{expired}");
}

/// Encrypt modal passphrase validation rejects empty or mismatched repeats.
#[test]
fn val_enc_pw_match() {
    let empty = validate_encrypt_passphrase_inputs("", "x").expect_err("must reject empty");
    assert_eq!(empty, "passphrase must not be empty");
    let mismatch = validate_encrypt_passphrase_inputs("abc", "xyz").expect_err("must reject");
    assert_eq!(mismatch, "passphrases do not match");
    assert!(validate_encrypt_passphrase_inputs("ok", "ok").is_ok());
}
