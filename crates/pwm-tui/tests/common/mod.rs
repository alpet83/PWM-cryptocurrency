//! Re-exports for `tests/*.rs` integration crates; each binary uses a different subset.
#![allow(unused_imports, dead_code)]

pub use ::pwm_tui::test_support as pwm_tui;
pub use ::pwm_tui::test_support::{
    choose_identity, default_wallet_candidate, f6_build_send_form, identity_lock_status_suffix,
    load_wallet_identity, merge_rpc_health, mk_acct_row, move_selection_down, move_selection_up,
    nonce_404_account_hint, nonce_from_account_body, owner_and_receivers, parse_nonce_json,
    preflight_recipient_rpc, preflight_sel_init_auto, preflight_xfer_dst, receiver_table_len,
    selected_to_receiver, signing_material_for_sender, submit_roaming_intent, text_input_set_text,
    validate_encrypt_passphrase_inputs, validate_send_form, wallet_apply_auto_lock,
    wallet_lock_now, wallet_unlock, wallet_unlock_secs_clamped, AcctRow, Args, BookPromptModal,
    BookRecipient, IdentitySource, JsonFetchFailure, OwnedWalletAccount, RpcHealth, SendField,
    SendForm, WalletIdentity, FALLBACK_MODE_WARNING, FALLBACK_WARN_CHUNK_ROWS,
};
pub use clap::Parser;
pub use pwm_core::{
    hd::{account_id_from_parts, domain_of_account_id},
    types::account_id_to_human,
    AccountId, WalletReadHeader,
};
pub use std::io::{Read, Write};
pub use std::net::TcpListener;
pub use std::path::PathBuf;
pub use std::sync::Mutex;
pub use std::thread;
pub use std::time::Instant;

pub static TEST_ENV_LOCK: Mutex<()> = Mutex::new(());

/// Derive SLIP10 account `SigningKey` and PWM account id for tests.
pub fn derived_account(seed: &[u8; 32], index: u32) -> (ed25519_dalek::SigningKey, AccountId) {
    let key = slip10_ed25519::derive_ed25519_private_key(seed, &[0, index]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&key);
    let id = account_id_from_parts(&sk.verifying_key().to_bytes(), index);
    (sk, id)
}

/// Brute-force low indices until `AccountId` high byte matches `hi` (test helper).
pub fn find_domain_hi(seed: &[u8; 32], hi: u8) -> (u32, ed25519_dalek::SigningKey, AccountId) {
    for index in 0..100_000 {
        let (sk, id) = derived_account(seed, index);
        if id[0] == hi {
            return (index, sk, id);
        }
    }
    panic!("test seed did not find domain_hi=0x{hi:02X}");
}

/// Minimal HTTP mock: sequential `(request prefix, status, body)` responses on localhost.
pub fn spawn_mock_http_server(script: Vec<(&'static str, u16, &'static str)>) -> String {
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
                400 => "Bad Request",
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

/// Seal wallet JSON payload for encrypted-fixture YAML (matches prod KDF/AEAD).
pub fn seal_test_enc_wallet(passphrase: &str, json: &[u8]) -> (String, String, String) {
    let s = pwm_core::seal_wallet_secret_plaintext(json, passphrase).unwrap();
    (s.kdf_salt_b64, s.aead_nonce_b64, s.encrypted_payload_b64)
}
