#![allow(unused_imports)]

//! Re-export façade wiring pwm-tui smoke and integration helpers.

use pwm_core::AccountId;
use ratatui::prelude::Rect;

pub use crate::account_view::{
    acct_row_for_id, fetch_debug_account, format_acct_cell, owner_and_receivers, poll_snapshot,
    start_rpc_worker, DebugCache, Panel, PollSnapshot, RpcEvent, RpcTask, Ui,
};
pub use crate::config::{
    base_url, http_client, inter_shard_status_short, parse_status_shard_label, rpc_context_label,
    shard_cli_hint, shard_hint_rpc, wallet_unlock_secs_clamped, Args, DEBUG_FETCH_INTERVAL,
    OP_HISTORY_MAX_ITEMS, SEND_FLOW_STEP_TIMEOUT,
};
pub use crate::history::{
    format_hms_utc, handle_submit_done_history, now_unix_secs, push_op_history,
    set_op_history_status, OpStatus, OperationHistoryEntry,
};
pub use crate::modals::{
    masked_with_caret, BookPromptModal, EncryptField, EncryptModal, TextInput, UnlockModal,
};
pub use crate::models::{
    format_balance_cell, format_init_cell, parse_hex_account_id, parse_u128, AcctRow,
    BookRecipient, OwnedWalletAccount, WalletIdentity, WalletV3Meta, UNKNOWN_BALANCE_SENTINEL,
    UNKNOWN_INIT_NONCE_SENTINEL,
};
pub use crate::roaming::{format_roaming_error, submit_roaming_intent};
pub use crate::rpc_account::{
    fetch_nonce, nonce_404_account_hint, nonce_from_account_body, parse_nonce_json,
    preflight_recipient_rpc, truncate_rpc_err_hint,
};
pub use crate::selection::{
    clamp_sel, move_selection_down, move_selection_up, receiver_table_len, selected_row_for_panel,
    selected_to_receiver,
};
pub use crate::send_form::{
    send_replay_guard_status, validate_send_form, value_with_caret, SendField, SendForm,
    SendStepFlow,
};
pub use crate::signing::{
    derive_sender_for_from, derive_wallet_key, signing_material_for_sender, verify_wallet_key,
    wallet_seed, wallet_seed_opt,
};
pub use crate::status::{
    debug_json, ellipsis_middle_ascii, fetch_json, format_footer_head_line, merge_rpc_health,
    rpc_health_from_failure, status_footer_line, JsonFetchFailure, RpcHealth,
};
pub use crate::tx_submit::{
    format_submit_transfer_error, is_cross_domain_route, submit_init, submit_transfer,
};
pub use crate::wallet::{
    build_plaintext_secret_json, choose_identity, decrypt_wallet_secret, default_wallet_candidate,
    default_wallet_if_present, identity_f3_action_label, identity_lock_status_suffix,
    load_owned_accounts, load_wallet_identity, merge_normalized_wallet_header,
    parse_signing_key_hex, replace_wallet_file, validate_encrypt_passphrase_inputs,
    wallet_apply_auto_lock, wallet_lock_now, wallet_rekey, wallet_unlock,
    wallet_upgrade_encryption_hook, write_wallet_yaml_atomic, yaml_map_get_string, yaml_root_map,
    IdentitySource, DETAIL_CHUNK_ROWS, FALLBACK_MODE_WARNING, FALLBACK_WARN_CHUNK_ROWS,
};

pub fn preflight_sel_init_auto(
    selected_row: Option<&AcctRow>,
    action_label: &str,
    identity: &IdentitySource,
) -> Result<Option<String>, String> {
    crate::preflight_sel_init_auto(selected_row, action_label, identity)
}

pub fn f6_build_send_form(
    identity: &IdentitySource,
    owner_rows: &[AcctRow],
    owner_sel: usize,
    receiver_rows: &[AcctRow],
    recv_sel: usize,
) -> Result<SendForm, String> {
    crate::f6_build_send_form(identity, owner_rows, owner_sel, receiver_rows, recv_sel)
}

pub fn preflight_xfer_dst(
    from: &AccountId,
    to: &AccountId,
    owner_rows: &[AcctRow],
    receiver_rows: &[AcctRow],
) -> Result<(), String> {
    crate::preflight_xfer_dst(from, to, owner_rows, receiver_rows)
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    crate::centered_rect(percent_x, percent_y, r)
}

pub fn text_input_set_text(input: &mut TextInput, s: impl Into<String>) {
    input.set_text(s.into());
}

/// Canonical AcctRow fixture defaults for integration tests.
pub fn mk_acct_row(id: AccountId) -> AcctRow {
    AcctRow {
        id,
        id_hex: hex::encode(id),
        balance_pwm: 0,
        initialized: true,
        nonce: 0,
        marks: 0,
        marks_last_block: 0,
        effective_marks: None,
        marks_sat_pct: None,
        pending_conservation: Vec::new(),
        staked: 0,
        rescue_address: None,
        active_policies: 0,
        dormant_policies: 0,
        finalized: false,
        owner_kind: String::new(),
        owner_name: String::new(),
        owner_country: String::new(),
        label: None,
    }
}
