mod common;
use common::*;

/// `validate_send_form` accepts pretty addresses and parses amount/fee units.
#[test]
fn val_send_form_pretty_ok() {
    let from = account_id_to_human(&[1u8; 32]);
    let to = account_id_to_human(&[2u8; 32]);
    let mut form = SendForm::new(from, to, true);
    text_input_set_text(&mut form.amount, "10.5");
    text_input_set_text(&mut form.fee, "0.001");
    text_input_set_text(&mut form.confirm, "yes");
    form.active = SendField::Confirm;
    let parsed = validate_send_form(&form).unwrap();
    assert_eq!(parsed.2, 10_500_000);
    assert_eq!(parsed.3, 1_000);
}

/// Confirmation field must be exactly `yes`, not other tokens.
#[test]
fn val_send_form_yes_needed() {
    let from = account_id_to_human(&[3u8; 32]);
    let to = account_id_to_human(&[4u8; 32]);
    let mut form = SendForm::new(from, to, true);
    text_input_set_text(&mut form.amount, "1");
    text_input_set_text(&mut form.fee, "0");
    text_input_set_text(&mut form.confirm, "ok");
    form.active = SendField::Confirm;
    let err = validate_send_form(&form).unwrap_err();
    assert!(err.contains("confirm"));
}

/// Rejects ambiguous legacy pretty `to` when strict pretty requires `/LO` shard label.
#[test]
fn val_send_form_ambig_to() {
    let from = account_id_to_human(&[3u8; 32]);
    let ambiguous_to =
        "pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000".to_string();
    let mut form = SendForm::new(from, ambiguous_to, true);
    text_input_set_text(&mut form.amount, "1");
    text_input_set_text(&mut form.fee, "0");
    text_input_set_text(&mut form.confirm, "yes");
    form.active = SendField::Confirm;
    let err = validate_send_form(&form).unwrap_err();
    assert!(err.contains("to:"));
    assert!(err.contains("missing '/LO'"));
    assert!(err.contains("strict pretty"));
}

/// Fixed recipient skips `To` field; navigation wraps Amount↔Confirm only.
#[test]
fn send_form_fixed_skip_to() {
    let mut form = SendForm::new("from".into(), "to".into(), false);
    assert_eq!(form.active, SendField::Amount);
    assert!(form.active_input_mut().is_some());
    form.prev_field();
    assert_eq!(form.active, SendField::Confirm);
    form.next_field();
    assert_eq!(form.active, SendField::Amount);
}

/// New-recipient flow starts on editable `To` and accepts inline typing.
#[test]
fn send_form_new_to_first() {
    let mut form = SendForm::new("from".into(), String::new(), true);
    assert_eq!(form.active, SendField::To);
    for c in "pwm1-".chars() {
        form.insert_char(c);
    }
    assert_eq!(form.to.as_str(), "pwm1-");
}

/// Inline edit: cursor moves, insert, backspace, delete, home/end on `To` field.
#[test]
fn send_form_inline_mid_ops() {
    let mut form = SendForm::new("from".into(), "abcd".into(), true);
    assert_eq!(form.active, SendField::To);
    form.move_left();
    form.move_left();
    form.insert_char('X');
    assert_eq!(form.to.as_str(), "abXcd");
    form.backspace();
    assert_eq!(form.to.as_str(), "abcd");
    form.move_left();
    form.delete();
    assert_eq!(form.to.as_str(), "acd");
    form.move_home();
    form.insert_char('0');
    assert_eq!(form.to.as_str(), "0acd");
    form.move_end();
    form.insert_char('9');
    assert_eq!(form.to.as_str(), "0acd9");
}

/// Book-prompt label editor supports same cursor/mid-string operations as send `To`.
#[test]
fn book_inline_mid_ops() {
    let mut bp = BookPromptModal::new("pwm1-test".into());
    for c in "abcd".chars() {
        bp.label.insert_char(c);
    }
    assert_eq!(bp.label.as_str(), "abcd");
    assert_eq!(bp.label.cursor(), 4);
    bp.label.move_left();
    bp.label.move_left();
    bp.label.insert_char('X');
    assert_eq!(bp.label.as_str(), "abXcd");
    bp.label.backspace();
    assert_eq!(bp.label.as_str(), "abcd");
    bp.label.move_left();
    bp.label.delete();
    assert_eq!(bp.label.as_str(), "acd");
    bp.label.move_home();
    bp.label.insert_char('0');
    assert_eq!(bp.label.as_str(), "0acd");
    bp.label.move_end();
    bp.label.insert_char('9');
    assert_eq!(bp.label.as_str(), "0acd9");
    assert_eq!(bp.label.cursor(), bp.label.as_str().len());
}

/// `parse_nonce_json` accepts JSON nonce as number or decimal string; rejects garbage.
#[test]
fn nonce_json_num_or_str() {
    assert_eq!(parse_nonce_json("{\"nonce\": 7}"), Some(7));
    assert_eq!(parse_nonce_json("{\"nonce\":\"12\"}"), Some(12));
    assert_eq!(parse_nonce_json("{\"nonce\":\"bad\"}"), None);
    assert_eq!(parse_nonce_json("{\"height\":1}"), None);
    assert_eq!(parse_nonce_json("not-json"), None);
}

/// `nonce_from_account_body` errors on non-success HTTP or invalid JSON body.
#[test]
fn nonce_http_err_bad_body() {
    let url = "http://127.0.0.1:3030/v1/account/00";
    assert!(nonce_from_account_body(false, 404, url, "{\"nonce\": 99}").is_err());
    assert!(nonce_from_account_body(true, 200, url, "not-json").is_err());
    assert_eq!(
        nonce_from_account_body(true, 200, url, "{\"nonce\": 5}").unwrap(),
        5
    );
}

/// 404 hint applies only when status and body together indicate missing account.
#[test]
fn nonce_404_acct_nf_only() {
    let hit = nonce_404_account_hint(404, "account not found");
    assert!(hit.is_some());
    let miss_status = nonce_404_account_hint(400, "account not found");
    assert!(miss_status.is_none());
    let miss_body = nonce_404_account_hint(404, "validation failed");
    assert!(miss_body.is_none());
}

/// Receiver table length includes synthetic "new recipient" row and maps selection.
#[test]
fn recv_panel_has_new_row() {
    let rows = vec![AcctRow {
        id: [1u8; 32],
        id_hex: "01".repeat(32),
        balance_pwm: 1,
        initialized: true,
        nonce: 0,
        label: None,
    }];
    assert_eq!(receiver_table_len(&rows), 2);
    assert!(selected_to_receiver(&rows, 0).is_none());
    assert_eq!(selected_to_receiver(&rows, 1).unwrap().id, [1u8; 32]);
}

/// `move_selection_down` clamps at last row and does not run past the table.
#[test]
fn recv_sel_clamps_last() {
    let rows = vec![
        AcctRow {
            id: [1u8; 32],
            id_hex: "01".repeat(32),
            balance_pwm: 1,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: [2u8; 32],
            id_hex: "02".repeat(32),
            balance_pwm: 2,
            initialized: true,
            nonce: 0,
            label: None,
        },
    ];
    let mut sel = 0usize;
    let len = receiver_table_len(&rows);
    for _ in 0..8 {
        move_selection_down(&mut sel, len);
    }
    assert_eq!(sel, len - 1);
    assert_eq!(selected_to_receiver(&rows, sel).unwrap().id, [2u8; 32]);
    move_selection_down(&mut sel, len);
    assert_eq!(sel, len - 1);
}

/// Owner list navigation: down/up symmetric at edges and empty length is a no-op.
#[test]
fn owner_sel_bounds_sym() {
    let mut sel = 0usize;
    move_selection_down(&mut sel, 1);
    assert_eq!(sel, 0);
    move_selection_up(&mut sel);
    assert_eq!(sel, 0);
    move_selection_down(&mut sel, 0);
    assert_eq!(sel, 0);
}

/// Without `--wallet`, identity is seed fallback and status note stays empty.
#[test]
fn choose_id_no_wallet_note() {
    let args = Args::parse_from(["pwm-tui"]);
    let (id, note) = choose_identity(&args, 300).unwrap();
    assert!(matches!(id, IdentitySource::SeedFallback));
    assert!(note.is_empty());
}

/// `default_wallet_candidate` picks `default.yml` when present in the directory.
#[test]
fn wallet_default_yml_found() {
    let td = std::env::temp_dir().join(format!("pwm-tui-default-yml-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&td);
    std::fs::create_dir_all(&td).unwrap();
    std::fs::write(td.join("default.yml"), b"x").unwrap();
    let got = default_wallet_candidate(&td);
    assert_eq!(got, Some(td.join("default.yml")));
    let _ = std::fs::remove_dir_all(&td);
}

/// No default wallet path when `default.yml` is absent.
#[test]
fn wallet_default_none() {
    let td = std::env::temp_dir().join(format!("pwm-tui-no-default-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&td);
    std::fs::create_dir_all(&td).unwrap();
    assert!(default_wallet_candidate(&td).is_none());
    let _ = std::fs::remove_dir_all(&td);
}

/// `FALLBACK_MODE_WARNING` string matches the operator-facing sentence exactly.
#[test]
fn fallback_warn_eq_const() {
    assert_eq!(
        FALLBACK_MODE_WARNING,
        "FALLBACK MODE: wallet not provided, owner derived from seed/default path"
    );
}

/// Fallback warning viewport uses a fixed row budget in a sensible band.
#[test]
fn fallback_chunk_fixed_len() {
    assert!(
        FALLBACK_WARN_CHUNK_ROWS >= 3 && FALLBACK_WARN_CHUNK_ROWS <= 8,
        "WARNING chunk: compact Length() slot, still room for borders/title/1–2 wrapped lines"
    );
}

/// `merge_rpc_health` monoid prefers Offline > Timeout > Online.
#[test]
fn merge_rpc_worst_wins() {
    assert_eq!(
        merge_rpc_health(RpcHealth::Online, RpcHealth::Timeout),
        RpcHealth::Timeout
    );
    assert_eq!(
        merge_rpc_health(RpcHealth::Timeout, RpcHealth::Offline),
        RpcHealth::Offline
    );
    assert_eq!(
        merge_rpc_health(RpcHealth::Offline, RpcHealth::Online),
        RpcHealth::Offline
    );
}

/// `rpc_health_from_failure` maps timeout vs other failures predictably.
#[test]
fn rpc_health_map_stable() {
    assert_eq!(
        pwm_tui::rpc_health_from_failure(JsonFetchFailure::Timeout),
        RpcHealth::Timeout
    );
    assert_eq!(
        pwm_tui::rpc_health_from_failure(JsonFetchFailure::Other),
        RpcHealth::Offline
    );
}

/// Default wallet unlock window is 300s when env override is absent.
#[test]
fn wallet_unlock_secs_def() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
    let args = Args::parse_from(["pwm-tui"]);
    assert_eq!(args.wallet_unlock_secs, 300);
    assert_eq!(wallet_unlock_secs_clamped(&args), 300);
}

/// CLI `--wallet-unlock-secs` overrides the default duration.
#[test]
fn wallet_unlock_secs_cli() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
    let args = Args::parse_from(["pwm-tui", "--wallet-unlock-secs", "42"]);
    assert_eq!(args.wallet_unlock_secs, 42);
    assert_eq!(wallet_unlock_secs_clamped(&args), 42);
}

/// Args parsing recognizes `--upgrade-wallet`.
#[test]
fn args_parse_upg_flag() {
    let args = Args::parse_from(["pwm-tui", "--upgrade-wallet"]);
    assert!(args.upgrade_wallet);
}

/// Env `PWM_TUI_WALLET_UNLOCK_SECS` overrides defaults when set.
#[test]
fn wallet_unlock_secs_env() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    // SAFETY: tests in this module do not spawn threads that depend on this env var.
    unsafe { std::env::set_var("PWM_TUI_WALLET_UNLOCK_SECS", "77") };
    let args = Args::parse_from(["pwm-tui"]);
    assert_eq!(args.wallet_unlock_secs, 77);
    assert_eq!(wallet_unlock_secs_clamped(&args), 77);
    // SAFETY: cleanup for test isolation.
    unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
}

/// `wallet_unlock_secs_clamped` pins unlock seconds into [1, 604_800].
#[test]
fn wallet_unlock_secs_clamp() {
    let _guard = TEST_ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
    let low = Args::parse_from(["pwm-tui", "--wallet-unlock-secs", "0"]);
    assert_eq!(wallet_unlock_secs_clamped(&low), 1);
    let high = Args::parse_from(["pwm-tui", "--wallet-unlock-secs", "999999999"]);
    assert_eq!(wallet_unlock_secs_clamped(&high), 604_800);
}

/// `owner_and_receivers` puts wallet `account_id` first in owner rows.
#[test]
fn owner_recv_wallet_pref() {
    let owner = [9u8; 32];
    let other = [3u8; 32];
    let rows = vec![
        AcctRow {
            id: other,
            id_hex: hex::encode(other),
            balance_pwm: 1,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: owner,
            id_hex: hex::encode(owner),
            balance_pwm: 2,
            initialized: true,
            nonce: 0,
            label: None,
        },
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: owner,
        account_id_human: account_id_to_human(&owner),
        domain: 0x4359,
        derivation_index: 42,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let (owner_rows, _active_owner_idx, receivers) = owner_and_receivers(&rows, &identity);
    assert_eq!(owner_rows[0].id, owner);
    assert_eq!(receivers.len(), 1);
    assert_eq!(receivers[0].id, other);
}

/// Receiver list is driven by wallet address book (hydrates missing accounts as zero-balance).
#[test]
fn owner_recv_book() {
    let owner = [9u8; 32];
    let book_a = [1u8; 32];
    let book_b = [2u8; 32];
    let rows = vec![
        AcctRow {
            id: owner,
            id_hex: hex::encode(owner),
            balance_pwm: 9,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: book_a,
            id_hex: hex::encode(book_a),
            balance_pwm: 3,
            initialized: true,
            nonce: 1,
            label: None,
        },
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: owner,
        account_id_human: account_id_to_human(&owner),
        domain: 0x4359,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![
            BookRecipient {
                id: book_a,
                label: None,
            },
            BookRecipient {
                id: book_b,
                label: None,
            },
        ],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let (owner_rows, _active_owner_idx, receivers) = owner_and_receivers(&rows, &identity);
    assert_eq!(owner_rows[0].id, owner);
    assert_eq!(receivers.len(), 2);
    assert_eq!(receivers[0].id, book_a);
    assert_eq!(receivers[0].balance_pwm, 3);
    assert_eq!(receivers[1].id, book_b);
    assert_eq!(receivers[1].balance_pwm, 0);
}

/// Multi-owned wallet: all owned rows surface with correct `is_active` index.
#[test]
fn owner_recv_owned_idx() {
    let owner_a = [9u8; 32];
    let owner_b = [8u8; 32];
    let peer = [1u8; 32];
    let rows = vec![
        AcctRow {
            id: owner_a,
            id_hex: hex::encode(owner_a),
            balance_pwm: 5,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: owner_b,
            id_hex: hex::encode(owner_b),
            balance_pwm: 7,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: peer,
            id_hex: hex::encode(peer),
            balance_pwm: 1,
            initialized: true,
            nonce: 0,
            label: None,
        },
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: owner_b,
        account_id_human: account_id_to_human(&owner_b),
        domain: 0x4359,
        derivation_index: 1,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![
            OwnedWalletAccount {
                id: owner_a,
                domain: 0x4359,
                derivation_index: 0,
                is_active: false,
            },
            OwnedWalletAccount {
                id: owner_b,
                domain: 0x4359,
                derivation_index: 1,
                is_active: true,
            },
        ],
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let (owner_rows, active_owner_idx, receivers) = owner_and_receivers(&rows, &identity);
    assert_eq!(owner_rows.len(), 2);
    assert_eq!(owner_rows[0].id, owner_a);
    assert_eq!(owner_rows[1].id, owner_b);
    assert_eq!(active_owner_idx, 1);
    assert_eq!(receivers.len(), 1);
    assert_eq!(receivers[0].id, peer);
}

/// Own account listed in address book remains a receiver with its label.
#[test]
fn owner_recv_book_own() {
    let owner = [9u8; 32];
    let rows = vec![AcctRow {
        id: owner,
        id_hex: hex::encode(owner),
        balance_pwm: 9,
        initialized: true,
        nonce: 0,
        label: None,
    }];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: owner,
        account_id_human: account_id_to_human(&owner),
        domain: 0x4359,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![OwnedWalletAccount {
            id: owner,
            domain: 0x4359,
            derivation_index: 0,
            is_active: false,
        }],
        address_book: vec![BookRecipient {
            id: owner,
            label: Some("self".to_string()),
        }],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let (_owner_rows, _active_owner_idx, receivers) = owner_and_receivers(&rows, &identity);
    assert_eq!(receivers.len(), 1);
    assert_eq!(receivers[0].id, owner);
    assert_eq!(receivers[0].label.as_deref(), Some("self"));
}

/// F6 send uses the highlighted owner row (not only the active wallet account) as sender.
#[test]
fn f6_row_owner_sender() {
    let seed = [26u8; 32];
    let (sel_idx, _sel_sk, selected) = find_domain_hi(&seed, 0x2C);
    let (active_idx, active_sk, active) = find_domain_hi(&seed, 0xDB);
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: active,
        account_id_human: account_id_to_human(&active),
        domain: u16::from_be_bytes([active[0], active[1]]),
        derivation_index: active_idx,
        signing_key: Some(active_sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![
            OwnedWalletAccount {
                id: selected,
                domain: u16::from_be_bytes([selected[0], selected[1]]),
                derivation_index: sel_idx,
                is_active: false,
            },
            OwnedWalletAccount {
                id: active,
                domain: u16::from_be_bytes([active[0], active[1]]),
                derivation_index: active_idx,
                is_active: true,
            },
        ],
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: Some(hex::encode(seed)),
        secret_payload_plaintext: None,
    });
    let owner_rows = vec![
        AcctRow {
            id: selected,
            id_hex: hex::encode(selected),
            balance_pwm: 1,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: active,
            id_hex: hex::encode(active),
            balance_pwm: 2,
            initialized: true,
            nonce: 0,
            label: None,
        },
    ];
    let form =
        f6_build_send_form(&identity, &owner_rows, 0, &[], 0).expect("selected owner must open F6");
    assert_eq!(form.from, account_id_to_human(&selected));
    assert_ne!(form.from, account_id_to_human(&active));
}

/// Signing helpers reject the selected owner when keys are not available for that id.
#[test]
fn sign_blk_no_owner_mat() {
    let selected = [0x2Cu8; 32];
    let active = [0xDBu8; 32];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: active,
        account_id_human: account_id_to_human(&active),
        domain: 0xDB00,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![
            OwnedWalletAccount {
                id: selected,
                domain: 0x2C00,
                derivation_index: 1,
                is_active: false,
            },
            OwnedWalletAccount {
                id: active,
                domain: 0xDB00,
                derivation_index: 0,
                is_active: true,
            },
        ],
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let err = signing_material_for_sender(&selected, &identity).expect_err("must block");
    assert!(err.contains("selected owner cannot be signed"), "{err}");
}

/// When CY row is selected but DB is active wallet, signing still uses CY material from seed.
#[test]
fn cy_sel_signs_not_db() {
    let seed = [42u8; 32];
    let (cy_idx, _cy_sk, cy) = find_domain_hi(&seed, 0x2C);
    let (db_idx, db_sk, db) = find_domain_hi(&seed, 0xDB);
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: db,
        account_id_human: account_id_to_human(&db),
        domain: u16::from_be_bytes([db[0], db[1]]),
        derivation_index: db_idx,
        signing_key: Some(db_sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![
            OwnedWalletAccount {
                id: cy,
                domain: u16::from_be_bytes([cy[0], cy[1]]),
                derivation_index: cy_idx,
                is_active: false,
            },
            OwnedWalletAccount {
                id: db,
                domain: u16::from_be_bytes([db[0], db[1]]),
                derivation_index: db_idx,
                is_active: true,
            },
        ],
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: Some(hex::encode(seed)),
        secret_payload_plaintext: None,
    });
    let (sk, dom, idx) = signing_material_for_sender(&cy, &identity).expect("sign cy");
    let signed_id = account_id_from_parts(&sk.verifying_key().to_bytes(), idx);
    assert_eq!(signed_id, cy);
    assert_ne!(signed_id, db);
    assert_eq!(dom.to_be_bytes()[0], 0x2C);
    assert_ne!(dom.to_be_bytes()[0], 0xDB);
}

/// High derivation index path re-derives from `master_seed_hex` instead of stale `signing_key`.
#[test]
fn sel_idx_seed_before_stale() {
    let seed = [44u8; 32];
    let index = 105_053u32;
    assert_ne!(index.to_le_bytes()[0], 0);
    let (_selected_sk, selected) = derived_account(&seed, index);
    let stale_sk = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: selected,
        account_id_human: account_id_to_human(&selected),
        domain: u16::from_be_bytes([selected[0], selected[1]]),
        derivation_index: index,
        signing_key: Some(stale_sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![OwnedWalletAccount {
            id: selected,
            domain: u16::from_be_bytes([selected[0], selected[1]]),
            derivation_index: index,
            is_active: true,
        }],
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: Some(hex::encode(seed)),
        secret_payload_plaintext: None,
    });

    let (sk, dom, idx) = signing_material_for_sender(&selected, &identity).expect("sign");
    let signed_id = account_id_from_parts(&sk.verifying_key().to_bytes(), idx);
    assert_eq!(idx, index);
    assert_eq!(dom, u16::from_be_bytes([selected[0], selected[1]]));
    assert_eq!(signed_id, selected);
}

/// Encrypted V3 path derives selected account via decrypted seed before stale on-disk key.
#[test]
fn enc_v3_sel_before_stale() {
    use base64::Engine as _;

    let seed = [44u8; 32];
    let selected_index = 105_053u32;
    let (_selected_sk, selected) = derived_account(&seed, selected_index);
    let stale_root_index = 0u32;
    let (stale_root_sk, stale_root) = derived_account(&[7u8; 32], stale_root_index);
    assert_ne!(selected, stale_root);

    let b64 = base64::engine::general_purpose::STANDARD;
    let payload = serde_json::json!({
        "master_seed_hex": hex::encode(seed),
        "master_seed_b64": b64.encode(seed),
        "signing_key_hex": hex::encode(stale_root_sk.to_bytes()),
        "signing_key_b64": b64.encode(stale_root_sk.to_bytes()),
        "verifying_key_hex": hex::encode(stale_root_sk.verifying_key().to_bytes()),
        "verifying_key_b64": b64.encode(stale_root_sk.verifying_key().to_bytes()),
    });
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: stale_root,
        account_id_human: account_id_to_human(&stale_root),
        domain: u16::from_be_bytes([stale_root[0], stale_root[1]]),
        derivation_index: stale_root_index,
        signing_key: Some(stale_root_sk),
        unlock_expires_at: Some(std::time::Instant::now() + std::time::Duration::from_secs(60)),
        wallet_is_encrypted: true,
        wallet_path: PathBuf::from("encrypted-v3.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![
            OwnedWalletAccount {
                id: stale_root,
                domain: u16::from_be_bytes([stale_root[0], stale_root[1]]),
                derivation_index: stale_root_index,
                is_active: false,
            },
            OwnedWalletAccount {
                id: selected,
                domain: u16::from_be_bytes([selected[0], selected[1]]),
                derivation_index: selected_index,
                is_active: true,
            },
        ],
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: Some(serde_json::to_vec(&payload).unwrap()),
    });

    let (sk, dom, idx) = signing_material_for_sender(&selected, &identity).expect("sign selected");
    let signed_id = account_id_from_parts(&sk.verifying_key().to_bytes(), idx);
    assert_eq!(idx, selected_index);
    assert_eq!(dom, u16::from_be_bytes([selected[0], selected[1]]));
    assert_eq!(signed_id, selected);
    assert_ne!(signed_id, stale_root);
}

/// Active CY owner must not sign with DB-derived key material.
#[test]
fn cy_act_reject_db_sk() {
    let seed = [43u8; 32];
    let (cy_idx, _cy_sk, cy) = find_domain_hi(&seed, 0x2C);
    let (db_idx, db_sk, db) = find_domain_hi(&seed, 0xDB);
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: cy,
        account_id_human: account_id_to_human(&cy),
        domain: u16::from_be_bytes([cy[0], cy[1]]),
        derivation_index: cy_idx,
        signing_key: Some(db_sk),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: vec![
            OwnedWalletAccount {
                id: cy,
                domain: u16::from_be_bytes([cy[0], cy[1]]),
                derivation_index: cy_idx,
                is_active: true,
            },
            OwnedWalletAccount {
                id: db,
                domain: u16::from_be_bytes([db[0], db[1]]),
                derivation_index: db_idx,
                is_active: false,
            },
        ],
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });

    let err = signing_material_for_sender(&cy, &identity).expect_err("must block");
    assert!(err.contains("selected owner cannot be signed"), "{err}");
    assert!(err.contains("signing key for m/0/"), "{err}");

    let owner_rows = vec![AcctRow {
        id: cy,
        id_hex: hex::encode(cy),
        balance_pwm: 1,
        initialized: true,
        nonce: 0,
        label: None,
    }];
    let err = match f6_build_send_form(&identity, &owner_rows, 0, &[], 0) {
        Ok(_) => panic!("F6 must block before submit"),
        Err(err) => err,
    };
    assert!(err.contains("F6 send blocked"), "{err}");
    assert!(err.contains("selected owner cannot be signed"), "{err}");
}

/// Seed fallback keeps first RPC row as sole owner and others as receivers.
#[test]
fn owner_recv_seed_v2() {
    let first = [9u8; 32];
    let second = [8u8; 32];
    let third = [7u8; 32];
    let rows = vec![
        AcctRow {
            id: first,
            id_hex: hex::encode(first),
            balance_pwm: 5,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: second,
            id_hex: hex::encode(second),
            balance_pwm: 7,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: third,
            id_hex: hex::encode(third),
            balance_pwm: 1,
            initialized: true,
            nonce: 0,
            label: None,
        },
    ];
    let (owner_rows, active_owner_idx, receivers) =
        owner_and_receivers(&rows, &IdentitySource::SeedFallback);
    assert_eq!(owner_rows.len(), 1);
    assert_eq!(owner_rows[0].id, first);
    assert_eq!(active_owner_idx, 0);
    assert_eq!(receivers.len(), 2);
    assert_eq!(receivers[0].id, second);
    assert_eq!(receivers[1].id, third);
}

/// Regulatory LO=0 pretty accounts stay visible in receivers alongside allowed prefixes.
#[test]
fn owner_recv_keep_lo0() {
    let owner = [9u8; 32];
    let mut lo_zero = [0u8; 32];
    lo_zero[0] = 0x2C;
    lo_zero[1] = 0x00;
    let mut allowed = [0u8; 32];
    allowed[0] = 0x2C;
    allowed[1] = 0x01;
    let rows = vec![
        AcctRow {
            id: owner,
            id_hex: hex::encode(owner),
            balance_pwm: 9,
            initialized: true,
            nonce: 0,
            label: None,
        },
        AcctRow {
            id: lo_zero,
            id_hex: hex::encode(lo_zero),
            balance_pwm: 3,
            initialized: true,
            nonce: 1,
            label: Some("lo_zero".into()),
        },
        AcctRow {
            id: allowed,
            id_hex: hex::encode(allowed),
            balance_pwm: 4,
            initialized: true,
            nonce: 2,
            label: Some("allowed".into()),
        },
    ];
    let identity = IdentitySource::Wallet(WalletIdentity {
        account_id: owner,
        account_id_human: account_id_to_human(&owner),
        domain: 0x4359,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])),
        unlock_expires_at: None,
        wallet_is_encrypted: false,
        wallet_path: PathBuf::from("test-wallet.yml"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: None,
    });
    let (_owner_rows, _active_owner_idx, receivers) = owner_and_receivers(&rows, &identity);
    assert_eq!(receivers.len(), 2);
    assert_eq!(receivers[0].id, lo_zero);
    assert_eq!(receivers[1].id, allowed);
}

/// Upgrade-to-encryption hook runs only for upgraded plaintext dev wallets.
#[test]
fn wallet_upg_hook_plain() {
    let plain = WalletReadHeader {
        schema_version: 2,
        mode: "plaintext_dev".into(),
        derivation_index: 1,
        derivation_path: Some("m/0/1".into()),
        domain_u16: 0x2C00,
        account_id_hex: None,
        account_id_human: account_id_to_human(&[1u8; 32]),
        owned_accounts: Vec::new(),
        address_book: vec![],
        signing_key_hex: Some("11".repeat(32)),
        master_seed_hex: None,
        encrypted_payload_b64: None,
        kdf_salt_b64: None,
        aead_nonce_b64: None,
        kdf: None,
        kdf_iters: None,
        ignored_legacy_pretty_entries: 0,
    };
    assert!(pwm_tui::wallet_upgrade_encryption_hook(&plain, true).is_some());
    assert!(pwm_tui::wallet_upgrade_encryption_hook(&plain, false).is_none());
    let mut enc = plain.clone();
    enc.mode = "encrypted".into();
    assert!(pwm_tui::wallet_upgrade_encryption_hook(&enc, true).is_none());
}

/// Auto-lock drops signing key, plaintext payload, unlock deadline for encrypted wallet.
#[test]
fn enc_wallet_autolock_clear() {
    let w = WalletIdentity {
        account_id: [1u8; 32],
        account_id_human: "pwm1-test".into(),
        domain: 1,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[2u8; 32])),
        unlock_expires_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(10)),
        wallet_is_encrypted: true,
        wallet_path: PathBuf::from("_unused_"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: Some(vec![1, 2, 3]),
    };
    let mut id = IdentitySource::Wallet(w);
    wallet_apply_auto_lock(&mut id);
    let locked_suffix = identity_lock_status_suffix(&id);
    match id {
        IdentitySource::Wallet(w) => {
            assert!(w.signing_key.is_none());
            assert!(w.unlock_expires_at.is_none());
            assert!(w.secret_payload_plaintext.is_none());
        }
        _ => panic!("expected wallet"),
    }
    assert!(locked_suffix.contains("LOCKED"));
}

/// Manual lock clears signing material and unlock timer for encrypted wallet.
#[test]
fn enc_wallet_manual_clear() {
    let mut w = WalletIdentity {
        account_id: [4u8; 32],
        account_id_human: "pwm1-test".into(),
        domain: 1,
        derivation_index: 0,
        signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[8u8; 32])),
        unlock_expires_at: Some(std::time::Instant::now() + std::time::Duration::from_secs(60)),
        wallet_is_encrypted: true,
        wallet_path: PathBuf::from("_unused_"),
        upgrade_wallet: false,
        owned_accounts: Vec::new(),
        address_book: vec![],
        encryption_prompt_hint: None,
        ignored_legacy_pretty_entries: 0,
        master_seed_hex: None,
        secret_payload_plaintext: Some(vec![7, 8, 9]),
    };
    wallet_lock_now(&mut w);
    assert!(w.signing_key.is_none());
    assert!(w.secret_payload_plaintext.is_none());
    assert!(w.unlock_expires_at.is_none());
}

/// Load encrypted wallet without CLI passphrase, unlock, then auto-lock clears keys.
#[test]
fn load_enc_unlock_autolock() {
    use pwm_core::hd::account_id_from_parts;
    let raw_key = [11u8; 32];
    let sk = ed25519_dalek::SigningKey::from_bytes(&raw_key);
    let pk = sk.verifying_key().to_bytes();
    let di = 0u32;
    let account_id = account_id_from_parts(&pk, di);
    let human = account_id_to_human(&account_id);
    let domain = u16::from_be_bytes([account_id[0], account_id[1]]);
    let payload = serde_json::json!({"signing_key_hex": hex::encode(sk.to_bytes())});
    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    let passphrase = b"unit-test-pass";
    let iters = pwm_core::WALLET_KDF_ITERS;
    let (salt_b64, nonce_b64, enc_b64) =
        seal_test_enc_wallet(std::str::from_utf8(passphrase).unwrap(), &payload_bytes);
    let yaml = format!(
        "mode: encrypted\nderivation_index: {di}\nderivation_path: m/0/{di}\ndomain_u16: {domain}\naccount_id_human: {human}\nkdf: pbkdf2_sha256\nkdf_iters: {iters}\nkdf_salt_b64: {salt_b64}\naead_nonce_b64: {nonce_b64}\nencrypted_payload_b64: {enc_b64}\naddress_book: []\n"
    );
    let path = std::env::temp_dir().join(format!("pwm-tui-enc-wallet-{}.yml", std::process::id()));
    std::fs::write(&path, yaml).unwrap();
    let mut idw = load_wallet_identity(&path, None, 300, false).expect("load locked");
    assert!(idw.wallet_is_encrypted);
    assert!(idw.signing_key.is_none());
    wallet_try_unlock_with_passphrase(&mut idw, std::str::from_utf8(passphrase).unwrap(), 60)
        .expect("unlock");
    assert!(idw.signing_key.is_some());
    assert!(idw.secret_payload_plaintext.is_some());
    idw.unlock_expires_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(1));
    let mut src = IdentitySource::Wallet(idw);
    wallet_apply_auto_lock(&mut src);
    match src {
        IdentitySource::Wallet(w) => assert!(w.signing_key.is_none()),
        _ => panic!("expected wallet"),
    }
    let _ = std::fs::remove_file(&path);
}

/// Encrypted wallet boot without passphrase still mentions F3 unlock in the note.
#[test]
fn choose_enc_f3_hint() {
    use pwm_core::hd::account_id_from_parts;
    let raw_key = [13u8; 32];
    let sk = ed25519_dalek::SigningKey::from_bytes(&raw_key);
    let pk = sk.verifying_key().to_bytes();
    let di = 0u32;
    let account_id = account_id_from_parts(&pk, di);
    let human = account_id_to_human(&account_id);
    let domain = u16::from_be_bytes([account_id[0], account_id[1]]);
    let payload = serde_json::json!({"signing_key_hex": hex::encode(sk.to_bytes())});
    let payload_bytes = serde_json::to_vec(&payload).unwrap();
    let passphrase = b"pw";
    let iters = pwm_core::WALLET_KDF_ITERS;
    let (salt_b64, nonce_b64, enc_b64) =
        seal_test_enc_wallet(std::str::from_utf8(passphrase).unwrap(), &payload_bytes);
    let yaml = format!(
        "mode: encrypted\nderivation_index: {di}\nderivation_path: m/0/{di}\ndomain_u16: {domain}\naccount_id_human: {human}\nkdf: pbkdf2_sha256\nkdf_iters: {iters}\nkdf_salt_b64: {salt_b64}\naead_nonce_b64: {nonce_b64}\nencrypted_payload_b64: {enc_b64}\naddress_book: []\n"
    );
    let path = std::env::temp_dir().join(format!("pwm-tui-enc-id-{}.yml", std::process::id()));
    std::fs::write(&path, yaml).unwrap();
    let args = Args::parse_from(["pwm-tui", "--wallet", path.to_str().unwrap()]);
    let (id, note) = choose_identity(&args, 300).unwrap();
    assert!(note.contains("F3"), "note={note}");
    match id {
        IdentitySource::Wallet(w) => assert!(w.signing_key.is_none()),
        _ => panic!("expected wallet"),
    }
    let _ = std::fs::remove_file(&path);
}
