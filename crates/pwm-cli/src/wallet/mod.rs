//! Wallet YAML/schema helpers for pwm-cli (split across `wallet/` submodules).
#![allow(unused_imports)] // Facade-only `pub use` reexports surface as unused within this module.

pub mod account;
pub mod address_book;
pub mod crypto;
pub mod store;
pub mod types;

pub use account::{
    wallet_account_add, wallet_account_add_seed, wallet_account_list, wallet_account_remove,
    wallet_account_use,
};
pub use address_book::{
    assert_tx_recipient_allowed, wallet_address_book_add, wallet_address_book_contains,
    wallet_address_book_remove,
};
pub use crypto::wallet_secrets;
pub use store::{
    backup_wallet_file, build_wallet_yaml, detect_resume_der_index, load_wallet_yaml,
    load_wallet_yaml_upgrade, parse_wallet_yaml, recover_wallet_file, save_wallet_v3_new,
    save_wallet_yaml, to_wallet_yaml,
};
pub use types::{
    WalletAccountEntry, WalletAccountRemoveResult, WalletProtection, WalletSecrets, WalletYaml,
};

#[cfg(test)]
mod tests {
    use super::store::{detect_schema_version, load_wallet_yaml_v3_raw};
    use super::types::{
        WalletSecretPayload, WalletYamlV3, WalletYamlV3Account, LEGACY_ACTIVE_ACCOUNT_KEY,
    };
    use super::*;
    use crate::bruteforce::domain_matches;
    use base64::Engine;
    use pwm_core::{
        account_id_to_human, append_wallet_yaml_address_book, parse_account_id,
        seal_wallet_secret_plaintext, AddressBookEntry,
    };
    use slip10_ed25519::derive_ed25519_private_key;
    use std::path::Path;

    /// Two distinct accounts on a recognized regulatory domain (`brute_cluster_address` like `wallet init`).
    fn two_policy_valid_wallets() -> ((String, String), (String, String)) {
        use pwm_core::hd::brute_cluster_address;
        const MAX: u32 = 500_000;
        let (_, _, _, owner) = brute_cluster_address(&[101u8; 32], 0x2C00, MAX)
            .expect("brute owner for CY high-byte domain");
        let (_, _, _, peer) = brute_cluster_address(&[102u8; 32], 0x2C00, MAX)
            .expect("brute peer for CY high-byte domain");
        assert_ne!(owner, peer);
        (
            (hex::encode(owner), account_id_to_human(&owner)),
            (hex::encode(peer), account_id_to_human(&peer)),
        )
    }

    fn encrypted_wallet_fixture(seed: [u8; 32], passphrase: &str) -> WalletYaml {
        let (sk, pk, idx, id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
        build_wallet_yaml(
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
        .expect("fixture wallet")
    }

    /// Load wallet from disk and apply the same recipient check as `tx-send --wallet` (no `--master`).
    fn check_tx_send_recipient_book(wallet_path: &Path, to_str: &str) -> Result<(), String> {
        let doc = load_wallet_yaml(wallet_path)?;
        let to = parse_account_id(to_str.trim()).map_err(|e| e.to_string())?;
        assert_tx_recipient_allowed(&doc, &to)
    }

    /// Mirrors `crate::main` `Cmd::TxSend`: `address_book` is enforced only when `master` is `None`
    /// and a `--wallet` path is present.
    fn tx_send_address_book_gate(
        wallet_path: Option<&Path>,
        master: Option<&str>,
        to_str: &str,
    ) -> Result<(), String> {
        let to = parse_account_id(to_str.trim()).map_err(|e| e.to_string())?;
        if master.is_none() {
            if let Some(wp) = wallet_path {
                let doc = load_wallet_yaml(wp)?;
                assert_tx_recipient_allowed(&doc, &to)?;
            }
        }
        Ok(())
    }

    #[test]
    fn wallet_yaml_roundtrip() {
        let y = build_wallet_yaml(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            7,
            0x007E,
            0x00FF_00FF,
            0x0000_00FF,
            0xAAFF_00FF,
            "11".repeat(32),
            "pwm1test".to_string(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        let text = serde_yaml::to_string(&y).unwrap();
        let parsed = parse_wallet_yaml(&text).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.mode, "plaintext_dev");
        assert_eq!(parsed.country_code_label.as_deref(), Some("CY"));
        assert_eq!(parsed.derivation_index, 7);
        assert_eq!(parsed.derivation_path.as_deref(), Some("m/0/7"));
        assert_eq!(parsed.domain_u16, 0x007E);
        assert_eq!(parsed.flags_mask_u32, 0x00FF_00FF);
        assert_eq!(parsed.expected_flags_u32, 0x0000_00FF);
        assert_eq!(parsed.flags_derived_u32, 0xAAFF_00FF);
        assert_eq!(parsed.master_seed_hex, Some(hex::encode([1u8; 32])));
        assert_eq!(
            parsed.master_seed_b64,
            Some(base64::engine::general_purpose::STANDARD.encode([1u8; 32]))
        );
    }

    /// Normalize legacy pretty account lines when loading wallet YAML (formerly `load_wallet_yaml_normalizes_legacy_pretty`).
    #[test]
    fn load_yaml_norm_legacy_pretty() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_loader_norm_{}.yaml",
            rand::random::<u128>()
        ));
        let raw = r#"schema_version: 1
mode: plaintext_dev
created_at_unix_sec: 1
derivation_index: 1
domain_u16: 11264
flags_mask_u32: 0
expected_flags_u32: 0
flags_derived_u32: 0
account_id_hex: "2c00000000000000000000000000000000000000000000000000000000000000"
account_id_human: pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000
"#;
        std::fs::write(&path, raw).unwrap();
        let loaded = load_wallet_yaml(&path).expect("load");
        assert!(loaded.account_id_human.contains("CY/00"));
        let _ = std::fs::remove_file(&path);
    }

    /// Prefer seed-derived truth when cached account ids disagree (formerly `load_wallet_yaml_uses_truth_source_when_cached_ids_mismatch`).
    #[test]
    fn yaml_truth_cache_ids() {
        let seed = [9u8; 32];
        let idx = 5u32;
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let true_id = pwm_core::hd::account_id_from_parts(&pk, idx);
        let mut wallet = build_wallet_yaml(
            seed,
            sk.to_bytes(),
            pk,
            idx,
            u16::from_be_bytes([true_id[0], true_id[1]]),
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            account_id_to_human(&[1u8; 32]),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        wallet.account_id_hex = "ff".repeat(32);
        wallet.account_id_human = account_id_to_human(&[2u8; 32]);
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_truth_source_{}.yaml",
            rand::random::<u128>()
        ));
        save_wallet_yaml(&path, &wallet).unwrap();
        let loaded = load_wallet_yaml(&path).expect("must load from truth source");
        assert_eq!(loaded.account_id_hex, hex::encode(true_id));
        assert_eq!(loaded.account_id_human, account_id_to_human(&true_id));
        let _ = std::fs::remove_file(&path);
    }

    /// Loader ignores ambiguous legacy pretty rows in address book (formerly `load_wallet_yaml_ignores_legacy_pretty_address_book_entry`).
    #[test]
    fn yaml_skip_ab_legacy() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_loader_book_{}.yaml",
            rand::random::<u128>()
        ));
        let raw = r#"schema_version: 1
mode: plaintext_dev
created_at_unix_sec: 1
derivation_index: 1
domain_u16: 11264
flags_mask_u32: 0
expected_flags_u32: 0
flags_derived_u32: 0
account_id_hex: "2c00000000000000000000000000000000000000000000000000000000000000"
account_id_human: pwm1-CY/00-f00000000-t0000000000000000000000000000000000000000000000000000
address_book:
  - pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000
"#;
        std::fs::write(&path, raw).unwrap();
        let loaded = load_wallet_yaml(&path).expect("must load");
        assert_eq!(loaded.address_book.len(), 0);
        assert_eq!(loaded.ignored_legacy_pretty_entries, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn encrypted_wallet_roundtrip_decrypts() {
        let wallet = build_wallet_yaml(
            [7u8; 32],
            [8u8; 32],
            [9u8; 32],
            12,
            0x4359,
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::Encrypted {
                passphrase: "secret".to_string(),
            },
        )
        .unwrap();
        assert_eq!(wallet.mode, "encrypted");
        assert_eq!(wallet.schema_version, 2);
        assert!(wallet.master_seed_hex.is_none());
        let secrets = wallet_secrets(&wallet, Some("secret")).unwrap();
        assert_eq!(secrets.master_seed_hex, hex::encode([7u8; 32]));
    }

    #[test]
    fn encrypted_wallet_rejects_wrong_passphrase() {
        let wallet = build_wallet_yaml(
            [4u8; 32],
            [5u8; 32],
            [6u8; 32],
            1,
            0x4359,
            0x03FF,
            0,
            0,
            "bb".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::Encrypted {
                passphrase: "secret".to_string(),
            },
        )
        .unwrap();
        let err = wallet_secrets(&wallet, Some("wrong")).expect_err("must fail");
        assert!(err.contains("failed to decrypt wallet payload"));
    }

    /// Empty allow-list skips recipient checks (formerly `address_book_allow_list_skips_when_empty`).
    #[test]
    fn ab_allow_skip_when_empty() {
        let mut w = build_wallet_yaml(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            0,
            0x4359,
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        w.address_book.clear();
        let to = parse_account_id(w.account_id_hex.as_str()).unwrap();
        assert_tx_recipient_allowed(&w, &to).unwrap();
    }

    /// Non-empty allow-list enforces entries (formerly `address_book_allow_list_enforces_when_non_empty`).
    #[test]
    fn ab_allow_enforce_nonempty() {
        let mut w = build_wallet_yaml(
            [1u8; 32],
            [2u8; 32],
            [3u8; 32],
            0,
            0x4359,
            0x03FF,
            0,
            0,
            "aa".repeat(32),
            "pwm1demo".to_string(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        let owner = parse_account_id(w.account_id_hex.as_str()).unwrap();
        w.address_book = vec![AddressBookEntry::AddressOnly(account_id_to_human(&owner))];
        let other = [9u8; 32];
        assert!(assert_tx_recipient_allowed(&w, &other).is_err());
    }

    /// Recipient book tempfile: reject unknown, succeed after append (formerly `tx_send_recipient_book_tempfile_rejects_unknown_then_allows_after_append`).
    #[test]
    fn ts_book_reject_then_allow() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_addrbook_{}.yaml",
            rand::random::<u128>()
        ));
        let (_owner_pair, (_peer_hex, peer_human)) = two_policy_valid_wallets();
        let seed = [1u8; 32];
        let (owner_sk, owner_pk, owner_idx, owner_id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("owner hit");
        let owner_hex = hex::encode(owner_id);
        let owner_human = account_id_to_human(&owner_id);
        let w = build_wallet_yaml(
            seed,
            owner_sk.to_bytes(),
            owner_pk,
            owner_idx,
            0x2C00,
            0x03FF,
            0,
            0,
            owner_hex,
            owner_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();

        let owner = parse_account_id(w.account_id_hex.as_str()).unwrap();
        let owner_human = account_id_to_human(&owner);
        append_wallet_yaml_address_book(&path, &owner_human, None).unwrap();

        assert!(check_tx_send_recipient_book(&path, &peer_human).is_err());

        append_wallet_yaml_address_book(&path, &peer_human, None).unwrap();
        check_tx_send_recipient_book(&path, &peer_human).unwrap();

        let _ = std::fs::remove_file(&path);
    }

    /// `--master Some` skips address-book gate like CLI `tx-send` (formerly `tx_send_address_book_skipped_when_master_some_matches_cli_tx_send_gate`).
    #[test]
    fn master_some_skips_ab() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_master_bypass_{}.yaml",
            rand::random::<u128>()
        ));
        let ((owner_hex, owner_human), (_outsider_hex, outsider_human)) =
            two_policy_valid_wallets();
        let w = build_wallet_yaml(
            [3u8; 32],
            [4u8; 32],
            [5u8; 32],
            0,
            0x2C00,
            0x03FF,
            0,
            0,
            owner_hex,
            owner_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();
        let owner = parse_account_id(w.account_id_hex.as_str()).unwrap();
        append_wallet_yaml_address_book(&path, &account_id_to_human(&owner), None).unwrap();

        let err = tx_send_address_book_gate(Some(path.as_path()), None, &outsider_human)
            .expect_err("book");
        assert!(!err.is_empty());

        // `Cmd::TxSend` uses `if master.is_none() { ... }` вЂ” any `Some(_)` skips the allow-list read.
        tx_send_address_book_gate(Some(path.as_path()), Some("deadbeef"), &outsider_human).unwrap();

        let _ = std::fs::remove_file(&path);
    }

    /// `wallet_address_book_add` errors on duplicate file entry (formerly `wallet_address_book_add_duplicate_returns_error_on_file`).
    #[test]
    fn wab_add_dup_err_file() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_dup_{}.yaml",
            rand::random::<u128>()
        ));
        let ((dup_hex, dup_human), _) = two_policy_valid_wallets();
        let w = build_wallet_yaml(
            [6u8; 32],
            [7u8; 32],
            [8u8; 32],
            0,
            0x2C00,
            0x03FF,
            0,
            0,
            dup_hex,
            dup_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();
        let id = parse_account_id(w.account_id_hex.as_str()).unwrap();
        let human = account_id_to_human(&id);
        assert_eq!(human, dup_human);
        wallet_address_book_add(&path, &human, None).unwrap();
        let err = wallet_address_book_add(&path, &human, None).expect_err("duplicate");
        assert!(err.contains("already in address_book"));
        let _ = std::fs::remove_file(&path);
    }

    /// Remove rejects ambiguous legacy-pretty recipient input (formerly `wallet_address_book_remove_rejects_ambiguous_legacy_pretty_input`).
    #[test]
    fn wab_rm_reject_amb_legacy() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_remove_ambiguous_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [9u8; 32];
        let (owner_sk, owner_pk, owner_idx, owner_id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("owner hit");
        let owner_hex = hex::encode(owner_id);
        let owner_human = account_id_to_human(&owner_id);
        let w = build_wallet_yaml(
            seed,
            owner_sk.to_bytes(),
            owner_pk,
            owner_idx,
            0x2C00,
            0x03FF,
            0,
            0,
            owner_hex,
            owner_human.clone(),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .unwrap();
        save_wallet_yaml(&path, &w).unwrap();
        let ambiguous = "pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000";
        let err = wallet_address_book_remove(&path, ambiguous).expect_err("must reject");
        assert!(err.contains("missing '/LO'"));
        let _ = std::fs::remove_file(&path);
    }

    /// Backup refuses wrong passphrase for encrypted wallet (formerly `backup_wallet_file_rejects_wrong_passphrase_for_encrypted`).
    #[test]
    fn backup_bad_pass_crypt() {
        let source = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_source_{}.yaml",
            rand::random::<u128>()
        ));
        let out = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_out_{}.yaml",
            rand::random::<u128>()
        ));
        let wallet = encrypted_wallet_fixture([1u8; 32], "secret-good");
        save_wallet_yaml(&source, &wallet).unwrap();
        let err = backup_wallet_file(&source, &out, Some("secret-bad")).expect_err("must fail");
        assert!(err.contains("wallet encrypted payload validation failed"));
        assert!(err.contains("correct --wallet-passphrase"));
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&out);
    }

    /// Backup refuses corrupted ciphertext (formerly `backup_wallet_file_rejects_corrupted_encrypted_payload`).
    #[test]
    fn backup_corr_crypt_payload() {
        let source = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_corrupt_{}.yaml",
            rand::random::<u128>()
        ));
        let out = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_backup_corrupt_out_{}.yaml",
            rand::random::<u128>()
        ));
        let mut wallet = encrypted_wallet_fixture([4u8; 32], "secret-good");
        wallet.encrypted_payload_b64 = Some("%%%not-base64%%%".to_string());
        save_wallet_yaml(&source, &wallet).unwrap();
        let err = backup_wallet_file(&source, &out, Some("secret-good")).expect_err("must fail");
        assert!(err.contains("wallet encrypted payload validation failed"));
        assert!(err.contains("corrupted"));
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&out);
    }

    /// Recover writes a decrypted verified copy (formerly `recover_wallet_file_creates_verified_copy`).
    #[test]
    fn recover_writes_ver_copy() {
        let backup = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_recover_backup_{}.yaml",
            rand::random::<u128>()
        ));
        let restored = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_recover_out_{}.yaml",
            rand::random::<u128>()
        ));
        let wallet = encrypted_wallet_fixture([7u8; 32], "secret-good");
        save_wallet_yaml(&backup, &wallet).unwrap();
        recover_wallet_file(&backup, &restored, Some("secret-good")).expect("recover");
        let restored_wallet = load_wallet_yaml(&restored).expect("load restored");
        let secrets = wallet_secrets(&restored_wallet, Some("secret-good")).expect("unlock");
        assert_eq!(secrets.master_seed_hex, hex::encode([7u8; 32]));
        let _ = std::fs::remove_file(&backup);
        let _ = std::fs::remove_file(&restored);
    }

    /// Minimal v3 plaintext YAML parses fields (formerly `load_wallet_yaml_parses_schema_v3_plaintext_minimal`).
    #[test]
    fn load_yaml_v3_plain_min() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_plaintext_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [3u8; 32];
        let idx = 17u32;
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let id = pwm_core::hd::account_id_from_parts(&pk, idx);
        let raw = format!(
            r#"schema_version: 3
mode: plaintext_dev
created_at_unix_sec: 1
accounts:
  - derivation_path: "m/0/{idx}"
    derivation_index: {idx}
    domain_u16: {domain}
    flags_mask_u32: 1023
    expected_flags_u32: 1
    flags_derived_u32: 1
    id_hex: "{id_hex}"
    id_pretty: "{id_pretty}"
master_seed_hex: "{seed_hex}"
"#,
            id_hex = hex::encode(id),
            id_pretty = account_id_to_human(&id),
            seed_hex = hex::encode(seed),
            domain = u16::from_be_bytes([id[0], id[1]])
        );
        std::fs::write(&path, raw).unwrap();
        let loaded = load_wallet_yaml(&path).expect("load v3 plaintext");
        assert_eq!(loaded.schema_version, 3);
        assert_eq!(loaded.account_id_hex, hex::encode(id));
        let _ = std::fs::remove_file(&path);
    }

    /// v3 load drops legacy-only active-account key (formerly `load_wallet_yaml_ignores_v3_legacy_active_account`).
    #[test]
    fn skip_act_marker_v3() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_bad_active_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [4u8; 32];
        let idx = 9u32;
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let id = pwm_core::hd::account_id_from_parts(&pk, idx);
        let raw = format!(
            r#"schema_version: 3
mode: plaintext_dev
created_at_unix_sec: 1
active_account_id_hex: "{active}"
accounts:
  - derivation_path: "m/0/{idx}"
    derivation_index: {idx}
    domain_u16: {domain}
    flags_mask_u32: 1023
    expected_flags_u32: 1
    flags_derived_u32: 1
    id_hex: "{id_hex}"
    id_pretty: "{id_pretty}"
master_seed_hex: "{seed_hex}"
"#,
            active = "ff".repeat(32),
            id_hex = hex::encode(id),
            id_pretty = account_id_to_human(&id),
            seed_hex = hex::encode(seed),
            domain = u16::from_be_bytes([id[0], id[1]])
        );
        std::fs::write(&path, raw).unwrap();
        let loaded = load_wallet_yaml(&path).expect("load ignores legacy active marker");
        assert_eq!(loaded.account_id_hex, hex::encode(id));
        let _ = std::fs::remove_file(&path);
    }

    /// v3 load rejects inconsistent id hex vs embedded seed (formerly `load_wallet_yaml_rejects_v3_inconsistent_id_hex_with_master_seed`).
    #[test]
    fn rej_v3_id_seed() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_bad_id_hex_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [5u8; 32];
        let idx = 13u32;
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let id = pwm_core::hd::account_id_from_parts(&pk, idx);
        let mut bad_id = id;
        bad_id[31] ^= 1;
        let raw = format!(
            r#"schema_version: 3
mode: plaintext_dev
created_at_unix_sec: 1
active_account_id_hex: "{active_id_hex}"
accounts:
  - derivation_path: "m/0/{idx}"
    derivation_index: {idx}
    domain_u16: {domain}
    flags_mask_u32: 1023
    expected_flags_u32: 1
    flags_derived_u32: 1
    id_hex: "{bad_id_hex}"
    id_pretty: "{id_pretty}"
master_seed_hex: "{seed_hex}"
"#,
            active_id_hex = hex::encode(bad_id),
            bad_id_hex = hex::encode(bad_id),
            id_pretty = account_id_to_human(&bad_id),
            seed_hex = hex::encode(seed),
            domain = u16::from_be_bytes([bad_id[0], bad_id[1]])
        );
        std::fs::write(&path, raw).unwrap();
        let err = load_wallet_yaml(&path).expect_err("must reject");
        assert!(err.contains("account id_hex mismatch"));
        let _ = std::fs::remove_file(&path);
    }

    /// v3 load rejects derivation path vs index clash (formerly `load_wallet_yaml_rejects_v3_derivation_path_index_mismatch`).
    #[test]
    fn v3_path_idx_bad() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_path_mismatch_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [6u8; 32];
        let idx = 3u32;
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, idx]);
        let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let id = pwm_core::hd::account_id_from_parts(&pk, idx);
        let raw = format!(
            r#"schema_version: 3
mode: plaintext_dev
created_at_unix_sec: 1
active_account_id_hex: "{id_hex}"
accounts:
  - derivation_path: "m/0/99"
    derivation_index: {idx}
    domain_u16: {domain}
    flags_mask_u32: 1023
    expected_flags_u32: 1
    flags_derived_u32: 1
    id_hex: "{id_hex}"
    id_pretty: "{id_pretty}"
master_seed_hex: "{seed_hex}"
"#,
            id_hex = hex::encode(id),
            id_pretty = account_id_to_human(&id),
            seed_hex = hex::encode(seed),
            domain = u16::from_be_bytes([id[0], id[1]])
        );
        std::fs::write(&path, raw).unwrap();
        let err = load_wallet_yaml(&path).expect_err("must reject");
        assert!(err.contains("derivation_path"));
        assert!(err.contains("derivation_index"));
        let _ = std::fs::remove_file(&path);
    }

    /// Build a minimal v3 wallet YAML on disk with two accounts (formerly `write_v3_wallet_with_two_accounts`).
    fn write_v3_wallet_two_accts(path: &std::path::Path) -> ([u8; 32], String, String) {
        let seed = [9u8; 32];
        let idx0 = 3u32;
        let sk0 = derive_ed25519_private_key(&seed, &[0, idx0]);
        let pk0 = ed25519_dalek::SigningKey::from_bytes(&sk0)
            .verifying_key()
            .to_bytes();
        let id0 = pwm_core::hd::account_id_from_parts(&pk0, idx0);
        let idx1 = 5u32;
        let sk1 = derive_ed25519_private_key(&seed, &[0, idx1]);
        let pk1 = ed25519_dalek::SigningKey::from_bytes(&sk1)
            .verifying_key()
            .to_bytes();
        let id1 = pwm_core::hd::account_id_from_parts(&pk1, idx1);
        let raw = format!(
            r#"schema_version: 3
mode: plaintext_dev
created_at_unix_sec: 1
active_account_id_hex: "{id0_hex}"
accounts:
  - derivation_path: "m/0/{idx0}"
    derivation_index: {idx0}
    domain_u16: {domain0}
    flags_mask_u32: 1023
    expected_flags_u32: 0
    flags_derived_u32: {flags0}
    id_hex: "{id0_hex}"
    id_pretty: "{id0_pretty}"
  - derivation_path: "m/0/{idx1}"
    derivation_index: {idx1}
    domain_u16: {domain1}
    flags_mask_u32: 1023
    expected_flags_u32: 0
    flags_derived_u32: {flags1}
    id_hex: "{id1_hex}"
    id_pretty: "{id1_pretty}"
master_seed_hex: "{seed_hex}"
"#,
            idx0 = idx0,
            idx1 = idx1,
            domain0 = u16::from_be_bytes([id0[0], id0[1]]),
            domain1 = u16::from_be_bytes([id1[0], id1[1]]),
            flags0 = u32::from_be_bytes([id0[2], id0[3], id0[4], id0[5]]),
            flags1 = u32::from_be_bytes([id1[2], id1[3], id1[4], id1[5]]),
            id0_hex = hex::encode(id0),
            id1_hex = hex::encode(id1),
            id0_pretty = account_id_to_human(&id0),
            id1_pretty = account_id_to_human(&id1),
            seed_hex = hex::encode(seed),
        );
        std::fs::write(path, raw).unwrap();
        (seed, hex::encode(id0), hex::encode(id1))
    }

    fn write_v3_encrypted_wallet(path: &std::path::Path, passphrase: &str) -> ([u8; 32], String) {
        let seed = [10u8; 32];
        let idx0 = 4u32;
        let sk0 = derive_ed25519_private_key(&seed, &[0, idx0]);
        let signing = ed25519_dalek::SigningKey::from_bytes(&sk0);
        let pk0 = signing.verifying_key().to_bytes();
        let id0 = pwm_core::hd::account_id_from_parts(&pk0, idx0);
        let secret = WalletSecretPayload {
            master_seed_hex: hex::encode(seed),
            master_seed_b64: base64::engine::general_purpose::STANDARD.encode(seed),
            signing_key_hex: hex::encode(signing.to_bytes()),
            signing_key_b64: base64::engine::general_purpose::STANDARD.encode(signing.to_bytes()),
            verifying_key_hex: hex::encode(pk0),
            verifying_key_b64: base64::engine::general_purpose::STANDARD.encode(pk0),
        };
        let sealed = seal_wallet_secret_plaintext(
            &serde_json::to_vec(&secret).expect("serialize secret"),
            passphrase,
        )
        .expect("seal secret");
        let wallet_v3 = WalletYamlV3 {
            schema_version: 3,
            mode: "encrypted".to_string(),
            created_at_unix_sec: 11,
            country_code_label: Some("CY".to_string()),
            active_account_id_hex: Some(hex::encode(id0)),
            accounts: vec![WalletYamlV3Account {
                derivation_index: idx0,
                derivation_path: format!("m/0/{idx0}"),
                domain_u16: u16::from_be_bytes([id0[0], id0[1]]),
                flags_mask_u32: 1023,
                expected_flags_u32: 0,
                flags_derived_u32: u32::from_be_bytes([id0[2], id0[3], id0[4], id0[5]]),
                id_hex: hex::encode(id0),
                id_pretty: account_id_to_human(&id0),
                added_at_unix_sec: Some(11),
            }],
            master_seed_hex: None,
            master_seed_b64: None,
            signing_key_hex: None,
            signing_key_b64: None,
            verifying_key_hex: None,
            verifying_key_b64: None,
            encrypted_payload_b64: Some(sealed.encrypted_payload_b64),
            kdf_salt_b64: Some(sealed.kdf_salt_b64),
            aead_nonce_b64: Some(sealed.aead_nonce_b64),
            kdf: Some(sealed.kdf),
            kdf_iters: Some(sealed.kdf_iters),
            address_book: Vec::new(),
        };
        std::fs::write(path, serde_yaml::to_string(&wallet_v3).unwrap()).unwrap();
        (seed, hex::encode(id0))
    }

    /// List command returns accounts and marks active (formerly `wallet_account_list_returns_all_accounts_with_active_mark`).
    #[test]
    fn wal_ac_list_mark_active() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_account_list_{}.yaml",
            rand::random::<u128>()
        ));
        let (_seed, active_hex, other_hex) = write_v3_wallet_two_accts(&path);
        let accounts = wallet_account_list(&path).expect("list");
        assert_eq!(accounts.len(), 2);
        assert!(accounts
            .iter()
            .any(|a| a.id_hex == active_hex && a.is_active));
        assert!(accounts
            .iter()
            .any(|a| a.id_hex == other_hex && !a.is_active));
        let _ = std::fs::remove_file(&path);
    }

    /// Add derives a new row and persists (formerly `wallet_account_add_derives_and_persists_new_account`).
    #[test]
    fn wal_ac_add_derives_row() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_account_add_{}.yaml",
            rand::random::<u128>()
        ));
        let (_seed, active_hex, _other_hex) = write_v3_wallet_two_accts(&path);
        let added = wallet_account_add(&path, 17, None).expect("add");
        assert_eq!(added.derivation_index, 17);
        let sk = derive_ed25519_private_key(&[9u8; 32], &[0, 17]);
        let pk = ed25519_dalek::SigningKey::from_bytes(&sk)
            .verifying_key()
            .to_bytes();
        let expected_id = hex::encode(pwm_core::hd::account_id_from_parts(&pk, 17));
        assert_eq!(added.id_hex, expected_id);
        let accounts = wallet_account_list(&path).expect("list after add");
        assert_eq!(accounts.len(), 3);
        assert!(accounts.iter().any(|a| a.id_hex == added.id_hex));
        assert!(accounts
            .iter()
            .any(|a| a.id_hex == active_hex && a.is_active));
        let _ = std::fs::remove_file(&path);
    }

    /// Add removes stale legacy active key (formerly `wallet_account_add_drops_legacy_active_account_key`).
    #[test]
    fn strip_legacy_on_add() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_account_add_clean_active_{}.yaml",
            rand::random::<u128>()
        ));
        let (_seed, active_hex, _other_hex) = write_v3_wallet_two_accts(&path);
        let before = std::fs::read_to_string(&path).expect("read before");
        assert!(before.contains(LEGACY_ACTIVE_ACCOUNT_KEY));

        let _ = load_wallet_yaml(&path).expect("old v3 must load");
        let _ = wallet_account_add(&path, 17, None).expect("add");

        let raw = std::fs::read_to_string(&path).expect("read after");
        assert!(!raw.contains(LEGACY_ACTIVE_ACCOUNT_KEY));
        let loaded = load_wallet_yaml(&path).expect("saved v3 must load");
        assert_eq!(loaded.account_id_hex, active_hex);
        let _ = std::fs::remove_file(&path);
    }

    /// Use validates without persisting markers (formerly `wallet_account_use_validates_without_persisting_marker`).
    #[test]
    fn use_cmd_no_marker() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_account_use_{}.yaml",
            rand::random::<u128>()
        ));
        let (_seed, active_hex, other_hex) = write_v3_wallet_two_accts(&path);
        let before = std::fs::read_to_string(&path).expect("read before");
        wallet_account_use(&path, &other_hex).expect("validate account");
        let after = std::fs::read_to_string(&path).expect("read after");
        assert_eq!(before, after);
        let accounts = wallet_account_list(&path).expect("list after use");
        assert!(accounts
            .iter()
            .any(|a| a.id_hex == active_hex && a.is_active));
        assert!(accounts
            .iter()
            .any(|a| a.id_hex == other_hex && !a.is_active));
        let _ = std::fs::remove_file(&path);
    }

    /// Cannot remove sole account row (formerly `wallet_account_remove_rejects_last_account`).
    #[test]
    fn wal_rm_last_blocked() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_account_remove_last_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [19u8; 32];
        let idx0 = 0u32;
        let sk0 = derive_ed25519_private_key(&seed, &[0, idx0]);
        let signing0 = ed25519_dalek::SigningKey::from_bytes(&sk0);
        let pk0 = signing0.verifying_key().to_bytes();
        let id0 = pwm_core::hd::account_id_from_parts(&pk0, idx0);
        let wallet = build_wallet_yaml(
            seed,
            signing0.to_bytes(),
            pk0,
            idx0,
            u16::from_be_bytes([id0[0], id0[1]]),
            0x03FF,
            0,
            0,
            hex::encode(id0),
            account_id_to_human(&id0),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .expect("wallet");
        save_wallet_v3_new(&path, &wallet).expect("save");
        let err = wallet_account_remove(&path, &hex::encode(id0)).expect_err("must reject");
        assert_eq!(
            err,
            "wallet account remove refused: cannot remove last account"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// Removing active picks deterministic fallback row (formerly `wallet_account_remove_active_switches_to_deterministic_fallback`).
    #[test]
    fn wal_ac_rm_act_fallback() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_account_remove_active_{}.yaml",
            rand::random::<u128>()
        ));
        let (_seed, active_hex, other_hex) = write_v3_wallet_two_accts(&path);
        let result = wallet_account_remove(&path, &active_hex).expect("remove active");
        assert!(result.removed_was_active);
        assert_eq!(result.removed_id_hex, active_hex);
        assert_eq!(result.new_active_id_hex, other_hex);
        let accounts = wallet_account_list(&path).expect("list");
        assert_eq!(accounts.len(), 1);
        assert!(accounts
            .iter()
            .any(|a| a.id_hex == other_hex && a.is_active));
        let _ = std::fs::remove_file(&path);
    }

    /// Wallet v2 rejects new account UX paths (formerly `wallet_account_commands_reject_v2_wallet`).
    #[test]
    fn wal_cmds_reject_v2() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v2_account_reject_{}.yaml",
            rand::random::<u128>()
        ));
        let wallet = encrypted_wallet_fixture([8u8; 32], "pw");
        save_wallet_yaml(&path, &wallet).unwrap();
        let err = wallet_account_list(&path).expect_err("must reject v2");
        assert_eq!(err, "wallet account commands require schema v3 wallet file");
        let _ = std::fs::remove_file(&path);
    }

    /// v2 without upgrade flag stays on disk unchanged (formerly `load_wallet_yaml_v2_without_upgrade_flag_does_not_rewrite_file`).
    #[test]
    fn yaml_v2_stays_inplace() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v2_autoup_plain_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [12u8; 32];
        let (sk, pk, idx, id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
        let mut wallet = build_wallet_yaml(
            seed,
            sk.to_bytes(),
            pk,
            idx,
            u16::from_be_bytes([id[0], id[1]]),
            0x03FF,
            0,
            0,
            hex::encode(id),
            account_id_to_human(&id),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .expect("wallet");
        wallet.schema_version = 2;
        save_wallet_yaml(&path, &wallet).expect("save");
        let before_raw = std::fs::read_to_string(&path).expect("read before");
        let loaded = load_wallet_yaml(&path).expect("load no migrate");
        assert_eq!(loaded.schema_version, 3);
        assert_eq!(loaded.mode, "plaintext_dev");
        assert!(loaded.master_seed_hex.is_some());
        let after_raw = std::fs::read_to_string(&path).expect("read after");
        assert_eq!(before_raw, after_raw);
        assert_eq!(detect_schema_version(&after_raw).expect("schema"), 2);
        let _ = std::fs::remove_file(&path);
    }

    /// Resume picks max derivation matching domain (formerly `detect_resume_der_index_uses_max_matching_index`).
    #[test]
    fn resume_der_pick_max_hit() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_resume_domain_idx_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [3u8; 32];
        let idx0 = 0u32;
        let sk0 = derive_ed25519_private_key(&seed, &[0, idx0]);
        let signing0 = ed25519_dalek::SigningKey::from_bytes(&sk0);
        let pk0 = signing0.verifying_key().to_bytes();
        let id0 = pwm_core::hd::account_id_from_parts(&pk0, idx0);
        let wallet = build_wallet_yaml(
            seed,
            signing0.to_bytes(),
            pk0,
            idx0,
            u16::from_be_bytes([id0[0], id0[1]]),
            0x03FF,
            0,
            0,
            hex::encode(id0),
            account_id_to_human(&id0),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .expect("wallet");
        save_wallet_v3_new(&path, &wallet).expect("save");
        let _ = wallet_account_add(&path, 12, None).expect("add account");
        let _ = wallet_account_add(&path, 7, None).expect("add account");
        let target_domain = u16::from_be_bytes([id0[0], id0[1]]);
        let next = detect_resume_der_index(
            &path,
            false,
            target_domain,
            crate::bruteforce::DomainMatchMode::HighByteOnly,
        )
        .expect("resume");
        let wallet_v3 = load_wallet_yaml_v3_raw(&path).expect("wallet");
        let expected = wallet_v3
            .accounts
            .iter()
            .filter(|a| {
                domain_matches(
                    a.domain_u16,
                    target_domain,
                    crate::bruteforce::DomainMatchMode::HighByteOnly,
                )
            })
            .map(|a| a.derivation_index)
            .max()
            .expect("matching domain must exist")
            .saturating_add(1);
        assert_eq!(next, expected);
        let _ = std::fs::remove_file(&path);
    }

    /// Resume prefers cluster-matching hit when branching (formerly `detect_resume_der_index_prefers_matching_cluster`).
    #[test]
    fn resume_der_pick_cluster_pref() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_resume_cluster_{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [0x55u8; 32];
        let derive = |idx: u32| {
            let sk = derive_ed25519_private_key(&seed, &[0, idx]);
            let signing = ed25519_dalek::SigningKey::from_bytes(&sk);
            let pk = signing.verifying_key().to_bytes();
            let id = pwm_core::hd::account_id_from_parts(&pk, idx);
            (u16::from_be_bytes([id[0], id[1]]), id)
        };

        let (target_domain, id0) = derive(0);
        let target_hi = target_domain >> 8;
        let mut same_hi_idx: Option<(u32, [u8; 32], u16)> = None;
        let mut other_hi_idx: Option<(u32, [u8; 32], u16)> = None;
        for idx in 1..4096u32 {
            let (domain, id) = derive(idx);
            if (domain >> 8) == target_hi {
                same_hi_idx = Some((idx, id, domain));
            } else {
                other_hi_idx = Some((idx, id, domain));
            }
            if same_hi_idx.is_some() && other_hi_idx.is_some() {
                break;
            }
        }
        let (same_idx, same_id, same_domain) = same_hi_idx.expect("must find same-hi domain");
        let (other_idx, other_id, other_domain) =
            other_hi_idx.expect("must find different-hi domain");
        let wallet_v3 = WalletYamlV3 {
            schema_version: 3,
            mode: "plaintext_dev".to_string(),
            created_at_unix_sec: WalletYaml::now_unix_sec(),
            country_code_label: None,
            active_account_id_hex: Some(hex::encode(id0)),
            accounts: vec![
                WalletYamlV3Account {
                    derivation_index: 0,
                    derivation_path: "m/0/0".to_string(),
                    domain_u16: target_domain,
                    flags_mask_u32: 0x03FF,
                    expected_flags_u32: 0,
                    flags_derived_u32: 0,
                    id_hex: hex::encode(id0),
                    id_pretty: account_id_to_human(&id0),
                    added_at_unix_sec: Some(WalletYaml::now_unix_sec()),
                },
                WalletYamlV3Account {
                    derivation_index: same_idx,
                    derivation_path: format!("m/0/{same_idx}"),
                    domain_u16: same_domain,
                    flags_mask_u32: 0x03FF,
                    expected_flags_u32: 0,
                    flags_derived_u32: 0,
                    id_hex: hex::encode(same_id),
                    id_pretty: account_id_to_human(&same_id),
                    added_at_unix_sec: Some(WalletYaml::now_unix_sec()),
                },
                WalletYamlV3Account {
                    derivation_index: other_idx,
                    derivation_path: format!("m/0/{other_idx}"),
                    domain_u16: other_domain,
                    flags_mask_u32: 0x03FF,
                    expected_flags_u32: 0,
                    flags_derived_u32: 0,
                    id_hex: hex::encode(other_id),
                    id_pretty: account_id_to_human(&other_id),
                    added_at_unix_sec: Some(WalletYaml::now_unix_sec()),
                },
            ],
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
        };
        std::fs::write(&path, serde_yaml::to_string(&wallet_v3).unwrap()).expect("write wallet");

        let scoped = detect_resume_der_index(
            &path,
            false,
            target_domain,
            crate::bruteforce::DomainMatchMode::HighByteOnly,
        )
        .expect("scoped");
        assert_eq!(scoped, same_idx.saturating_add(1));

        let absent_hi_target =
            (((target_hi.wrapping_add(1)) as u16) << 8) | (target_domain & 0x00FF);
        let fallback = detect_resume_der_index(
            &path,
            false,
            absent_hi_target,
            crate::bruteforce::DomainMatchMode::HighByteOnly,
        )
        .expect("fallback");
        assert_eq!(fallback, 0);
        let _ = std::fs::remove_file(&path);
    }

    /// `save_wallet_v3_new` creates parent dirs (formerly `save_wallet_v3_new_creates_parent_directories`).
    #[test]
    fn save_v3_mk_parent_dirs() {
        let dir = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_parent_dir_{}",
            rand::random::<u128>()
        ));
        let path = dir.join("nested").join("wallet.yaml");
        let seed = [0x31u8; 32];
        let hit = pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("hit");
        let wallet = build_wallet_yaml(
            seed,
            hit.0.to_bytes(),
            hit.1,
            hit.2,
            u16::from_be_bytes([hit.3[0], hit.3[1]]),
            0x03FF,
            0,
            0,
            hex::encode(hit.3),
            account_id_to_human(&hit.3),
            None,
            WalletProtection::PlaintextDev,
        )
        .expect("wallet");
        save_wallet_v3_new(&path, &wallet).expect("save");
        assert!(path.exists());
        let raw = std::fs::read_to_string(&path).expect("read saved wallet");
        assert!(!raw.contains("active_account_id_hex"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Upgrade flag migrates encrypted v2 → v3 (formerly `load_wallet_yaml_upgrade_flag_migrates_encrypted_v2_to_v3`).
    #[test]
    fn migr_crypt_v3_flag() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v2_autoup_enc_{}.yaml",
            rand::random::<u128>()
        ));
        let wallet = encrypted_wallet_fixture([15u8; 32], "secret-good");
        save_wallet_yaml(&path, &wallet).expect("save");
        let loaded = load_wallet_yaml_upgrade(&path, true).expect("load + migrate");
        assert_eq!(loaded.schema_version, 3);
        assert_eq!(loaded.mode, "encrypted");
        assert!(loaded.encrypted_payload_b64.is_some());
        assert!(loaded.master_seed_hex.is_none());
        let unlocked = wallet_secrets(&loaded, Some("secret-good")).expect("unlock");
        assert_eq!(unlocked.master_seed_hex, hex::encode([15u8; 32]));
        let raw = std::fs::read_to_string(&path).expect("read");
        assert_eq!(detect_schema_version(&raw).expect("schema"), 3);
        let _ = std::fs::remove_file(&path);
    }

    /// `save_wallet_v3_new` overwrites file without legacy baggage (formerly `save_wallet_v3_new_overwrites_existing_file_without_legacy_baggage`).
    #[test]
    fn save_v3_ovr_writes_clean() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_create_overwrite_{}.yaml",
            rand::random::<u128>()
        ));
        let stale_raw = r#"schema_version: 1
mode: plaintext_dev
created_at_unix_sec: 1
derivation_index: 77
domain_u16: 11264
flags_mask_u32: 0
expected_flags_u32: 0
flags_derived_u32: 0
account_id_hex: "2c00000000000000000000000000000000000000000000000000000000000000"
account_id_human: pwm1-CY/00-f00000000-t0000000000000000000000000000000000000000000000000000
future_raw_key: keep-me-if-merged
"#;
        std::fs::write(&path, stale_raw).expect("write stale");
        let seed = [18u8; 32];
        let (sk, pk, idx, id) =
            pwm_core::hd::brute_cluster_address(&seed, 0x2C00, 500_000).expect("fixture hit");
        let wallet = build_wallet_yaml(
            seed,
            sk.to_bytes(),
            pk,
            idx,
            u16::from_be_bytes([id[0], id[1]]),
            0x03FF,
            0,
            0,
            hex::encode(id),
            account_id_to_human(&id),
            Some("CY".to_string()),
            WalletProtection::PlaintextDev,
        )
        .expect("wallet");
        save_wallet_v3_new(&path, &wallet).expect("save");
        let raw = std::fs::read_to_string(&path).expect("read");
        let val: serde_yaml::Value = serde_yaml::from_str(&raw).expect("yaml");
        let map = val.as_mapping().expect("mapping");
        assert_eq!(
            map.get(&serde_yaml::Value::String("schema_version".to_string())),
            Some(&serde_yaml::Value::Number(serde_yaml::Number::from(3u64)))
        );
        assert!(!map.contains_key(&serde_yaml::Value::String("derivation_index".to_string())));
        assert!(!map.contains_key(&serde_yaml::Value::String("future_raw_key".to_string())));
        let _ = std::fs::remove_file(&path);
    }

    /// Upgrade strips legacy/top-level unknown fields when persisting (formerly `upgrade_wallet_persistence_drops_legacy_and_unknown_top_level_fields`).
    #[test]
    fn upg_wal_strip_extra_top() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v2_upgrade_cleanup_{}.yaml",
            rand::random::<u128>()
        ));
        let mut wallet = encrypted_wallet_fixture([19u8; 32], "secret-good");
        wallet.derivation_path = Some(format!("m/0/{}", wallet.derivation_index));
        save_wallet_yaml(&path, &wallet).expect("save");
        let mut before_val: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&path).expect("read before"))
                .expect("yaml");
        let map = before_val.as_mapping_mut().expect("mapping");
        map.insert(
            serde_yaml::Value::String("future_raw_key".to_string()),
            serde_yaml::Value::String("keep-me-if-merged".to_string()),
        );
        std::fs::write(&path, serde_yaml::to_string(&before_val).expect("dump")).expect("write");

        let loaded = load_wallet_yaml_upgrade(&path, true).expect("upgrade");
        assert_eq!(loaded.schema_version, 3);
        let raw = std::fs::read_to_string(&path).expect("read after");
        let val: serde_yaml::Value = serde_yaml::from_str(&raw).expect("yaml");
        let after = val.as_mapping().expect("mapping");
        assert!(!after.contains_key(&serde_yaml::Value::String("derivation_index".to_string())));
        assert!(!after.contains_key(&serde_yaml::Value::String("account_id_human".to_string())));
        assert!(!after.contains_key(&serde_yaml::Value::String("future_raw_key".to_string())));
        let _ = std::fs::remove_file(&path);
    }

    /// Encrypted v3 add needs passphrase argument (formerly `wallet_account_add_encrypted_v3_requires_passphrase`).
    #[test]
    fn wal_add_crypt_must_pass() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_enc_account_add_{}.yaml",
            rand::random::<u128>()
        ));
        let (_seed, _active_hex) = write_v3_encrypted_wallet(&path, "good-pass");
        let added = wallet_account_add(&path, 19, Some("good-pass")).expect("add encrypted");
        assert_eq!(added.derivation_index, 19);
        let err_missing = wallet_account_add(&path, 20, None).expect_err("missing passphrase");
        assert!(err_missing.contains("encrypted wallet requires passphrase"));
        let err_wrong =
            wallet_account_add(&path, 20, Some("bad-pass")).expect_err("wrong passphrase");
        assert!(err_wrong.contains("failed to decrypt wallet payload"));
        let _ = std::fs::remove_file(&path);
    }

    /// v3 account rewrite keeps unknown per-account metadata (formerly `wallet_v3_account_rewrite_preserves_unknown_and_created_metadata`).
    #[test]
    fn v3_ac_rewrite_keep_meta() {
        let path = std::env::temp_dir().join(format!(
            "pwm_cli_wallet_v3_preserve_meta_{}.yaml",
            rand::random::<u128>()
        ));
        let (_seed, _active_hex, other_hex) = write_v3_wallet_two_accts(&path);
        let raw_before = std::fs::read_to_string(&path).expect("read before");
        let mut before_val: serde_yaml::Value = serde_yaml::from_str(&raw_before).expect("yaml");
        let map = before_val.as_mapping_mut().expect("mapping");
        map.insert(
            serde_yaml::Value::String("wallet_created_at_unix_sec".to_string()),
            serde_yaml::Value::Number(serde_yaml::Number::from(1_717_171_717u64)),
        );
        map.insert(
            serde_yaml::Value::String("future_raw_key".to_string()),
            serde_yaml::Value::String("keep-me".to_string()),
        );
        std::fs::write(&path, serde_yaml::to_string(&before_val).unwrap()).unwrap();

        let _ = wallet_account_add(&path, 27, None).expect("add");
        wallet_account_use(&path, &other_hex).expect("use");

        let raw_after = std::fs::read_to_string(&path).expect("read after");
        let after_val: serde_yaml::Value = serde_yaml::from_str(&raw_after).expect("yaml");
        let after = after_val.as_mapping().expect("mapping");
        assert_eq!(
            after.get(&serde_yaml::Value::String(
                "wallet_created_at_unix_sec".to_string()
            )),
            Some(&serde_yaml::Value::Number(serde_yaml::Number::from(
                1_717_171_717u64
            )))
        );
        assert_eq!(
            after.get(&serde_yaml::Value::String("future_raw_key".to_string())),
            Some(&serde_yaml::Value::String("keep-me".to_string()))
        );
        let _ = std::fs::remove_file(&path);
    }
}
