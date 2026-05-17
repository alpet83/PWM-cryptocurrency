//! Network account table (public-friendly). Optional debug JSON via PWM_TUI_DEBUG=1.

use pwm_core::{account_id_to_human, append_addr_book, AccountId};
use ratatui::prelude::*;

mod config;
#[doc(hidden)]
pub mod test_support;
pub mod tui_loop;

pub use config::Args;
#[allow(unused_imports)]
pub(crate) use config::{
    base_url, http_client, inter_shard_status_short, parse_status_shard_label, rpc_context_label,
    shard_cli_hint, shard_hint_rpc, wallet_unlock_secs_clamped, DEBUG_FETCH_INTERVAL,
    OP_HISTORY_MAX_ITEMS, SEND_FLOW_STEP_TIMEOUT,
};

mod status;

#[allow(unused_imports)]
pub(crate) use status::{
    debug_json, ellipsis_middle_ascii, fetch_json, format_footer_head_line, merge_rpc_health,
    rpc_health_from_failure, status_footer_line, JsonFetchFailure, RpcHealth,
};

mod models;

#[allow(unused_imports)]
pub(crate) use models::{
    format_balance_cell, format_init_cell, format_policy_bits, parse_hex_account_id, parse_u128,
    parse_u16, parse_u32, AcctRow, BookRecipient, OwnedWalletAccount, WalletIdentity, WalletV3Meta,
    UNKNOWN_BALANCE_SENTINEL, UNKNOWN_INIT_NONCE_SENTINEL,
};

mod modals;
#[allow(unused_imports)]
pub(crate) use modals::{
    masked_with_caret, BookPromptModal, EncryptField, EncryptModal, TextInput, UnlockModal,
};

mod wallet;
pub use wallet::default_wallet_if_present;
#[allow(unused_imports)]
pub(crate) use wallet::{
    build_plaintext_secret_json, choose_identity, decrypt_wallet_secret, default_wallet_candidate,
    identity_f3_action_label, identity_lock_status_suffix, load_owned_accounts,
    load_wallet_identity, merge_normalized_wallet_header, parse_signing_key_hex,
    replace_wallet_file, validate_encrypt_passphrase_inputs, wallet_apply_auto_lock,
    wallet_lock_now, wallet_rekey, wallet_unlock, wallet_upgrade_encryption_hook,
    write_wallet_yaml_atomic, yaml_map_get_string, yaml_root_map, IdentitySource,
    DETAIL_CHUNK_ROWS, FALLBACK_MODE_WARNING, FALLBACK_WARN_CHUNK_ROWS,
};

mod rpc_account;
#[allow(unused_imports)]
pub(crate) use rpc_account::{
    fetch_nonce, nonce_404_account_hint, nonce_from_account_body, parse_nonce_json,
    preflight_recipient_rpc, truncate_rpc_err_hint,
};

mod signing;
#[allow(unused_imports)]
pub(crate) use signing::{
    derive_sender_for_from, derive_wallet_key, signing_material_for_sender, verify_wallet_key,
    wallet_seed, wallet_seed_opt,
};

mod tx_submit;
#[allow(unused_imports)]
pub(crate) use tx_submit::{
    format_submit_transfer_error, is_cross_domain_route, submit_burn_mark, submit_claim,
    submit_init, submit_stake, submit_transfer, submit_unstake,
};

mod burn_form;
#[allow(unused_imports)]
pub(crate) use burn_form::{burn_replay_guard_status, validate_burn_form, BurnField, BurnForm};

mod stake_form;
#[allow(unused_imports)]
pub(crate) use stake_form::{validate_stake_form, StakeField, StakeForm, StakeMode};

mod roaming;
#[allow(unused_imports)]
pub(crate) use roaming::{format_roaming_error, submit_roaming_intent};

mod send_form;
#[allow(unused_imports)]
pub(crate) use send_form::{
    send_replay_guard_status, validate_send_form, value_with_caret, SendField, SendForm,
    SendStepFlow,
};

mod history;
#[allow(unused_imports)]
pub(crate) use history::{
    format_hms_utc, handle_submit_done_history, now_unix_secs, push_op_history,
    set_op_history_status, OpStatus, OperationHistoryEntry,
};

mod account_view;
#[allow(unused_imports)]
pub(crate) use account_view::{
    acct_row_for_id, fetch_debug_account, format_acct_cell, owner_and_receivers, poll_snapshot,
    start_rpc_worker, DebugCache, Panel, PollSnapshot, RpcEvent, RpcTask, Ui,
};

mod selection;
pub(crate) use selection::{
    clamp_sel, move_selection_down, move_selection_up, receiver_table_len, selected_row_for_panel,
    selected_to_receiver,
};

/// Preflight selected row for actions that require init: blocks unknown home-init nonce; may run auto `submit_init`.
pub(crate) fn preflight_sel_init_auto(
    selected_row: Option<&AcctRow>,
    action_label: &str,
    identity: &IdentitySource,
) -> Result<Option<String>, String> {
    match selected_row {
        Some(row) if row.nonce == UNKNOWN_INIT_NONCE_SENTINEL => Err(format!(
            "{action_label} blocked: account init status is unknown (home shard peer lookup unavailable)."
        )),
        Some(row) if !row.initialized => match submit_init(&row.id, row.nonce, identity) {
            Ok(msg) => Ok(Some(msg)),
            Err(reason) => Err(format!(
                "{action_label} blocked: selected account is not initialized and auto-init is unavailable ({reason}). Run `pwm --rpc {} tx-init ...` first.",
                base_url()
            )),
        },
        _ => Ok(None),
    }
}

/// Build F6 send modal state from current owner/receiver panel selection.
pub(crate) fn f6_build_send_form(
    identity: &IdentitySource,
    owner_rows: &[AcctRow],
    owner_sel: usize,
    receiver_rows: &[AcctRow],
    recv_sel: usize,
) -> Result<SendForm, String> {
    let locked = matches!(
        identity,
        IdentitySource::Wallet(w) if w.wallet_is_encrypted && w.signing_key.is_none()
    );
    if locked {
        return Err("Wallet is locked: press F3 to unlock before sending.".into());
    }
    let owner = owner_rows
        .get(owner_sel)
        .ok_or_else(|| "F6 send blocked: no selected Owner row".to_string())?;
    let _ = signing_material_for_sender(&owner.id, identity)
        .map_err(|e| format!("F6 send blocked: {e}"))?;
    let from = account_id_to_human(&owner.id);
    let selected = selected_to_receiver(receiver_rows, recv_sel);
    let to = selected
        .map(|r| account_id_to_human(&r.id))
        .unwrap_or_default();
    Ok(SendForm::new(from, to, selected.is_none()))
}

/// Build F5 burn modal from selected owner row.
pub(crate) fn f5_build_burn_form(
    identity: &IdentitySource,
    owner_rows: &[AcctRow],
    owner_sel: usize,
    receiver_rows: &[AcctRow],
    recv_sel: usize,
) -> Result<BurnForm, String> {
    let locked = matches!(
        identity,
        IdentitySource::Wallet(w) if w.wallet_is_encrypted && w.signing_key.is_none()
    );
    if locked {
        return Err("Wallet is locked: press F3 to unlock before burning marks.".into());
    }
    let owner = owner_rows
        .get(owner_sel)
        .ok_or_else(|| "F5 burn blocked: no selected Owner row".to_string())?;
    let _ = signing_material_for_sender(&owner.id, identity)
        .map_err(|e| format!("F5 burn blocked: {e}"))?;
    let from = account_id_to_human(&owner.id);
    let (beneficiary, beneficiary_editable) = match selected_to_receiver(receiver_rows, recv_sel) {
        Some(r) => (account_id_to_human(&r.id), false),
        None => (String::new(), true),
    };
    Ok(BurnForm::new(
        from,
        owner.marks,
        beneficiary,
        beneficiary_editable,
    ))
}

/// True when F5 burn shows the stake-first hint (nothing staked and no marks).
#[inline]
pub(crate) fn f5_burn_hint_needed(staked: u128, marks: u32) -> bool {
    staked == 0 && marks == 0
}

/// Build F7 stake/unstake modal from selected owner row.
pub(crate) fn f7_build_stake_form(
    identity: &IdentitySource,
    owner_rows: &[AcctRow],
    owner_sel: usize,
    mode: StakeMode,
) -> Result<StakeForm, String> {
    let locked = matches!(
        identity,
        IdentitySource::Wallet(w) if w.wallet_is_encrypted && w.signing_key.is_none()
    );
    if locked {
        return Err("Wallet is locked: press F3 to unlock before stake/unstake.".into());
    }
    let owner = owner_rows
        .get(owner_sel)
        .ok_or_else(|| "F7 stake blocked: no selected Owner row".to_string())?;
    let _ = signing_material_for_sender(&owner.id, identity)
        .map_err(|e| format!("F7 stake blocked: {e}"))?;
    let from = account_id_to_human(&owner.id);
    Ok(StakeForm::new(mode, from, owner.balance_pwm, owner.staked))
}

/// Preflight transfer recipient: must be known in rows or allowed cross-domain route.
pub(crate) fn preflight_xfer_dst(
    from: &AccountId,
    to: &AccountId,
    owner_rows: &[AcctRow],
    receiver_rows: &[AcctRow],
) -> Result<(), String> {
    let known = receiver_rows
        .iter()
        .chain(owner_rows.iter())
        .find(|r| &r.id == to);
    if let Some(row) = known {
        if row.nonce == UNKNOWN_INIT_NONCE_SENTINEL {
            return Err(
                "F6 send blocked: recipient init status is unknown; home shard peer lookup unavailable"
                    .into(),
            );
        }
        if !row.initialized {
            return Err(
                "F6 send blocked: recipient is not initialized; recipient must initialize on target shard first"
                    .into(),
            );
        }
        return Ok(());
    }
    if !is_cross_domain_route(from, to) {
        return Err(
            "F6 send blocked: recipient is missing in current shard view; recipient must initialize on target shard first"
                .into(),
        );
    }
    Ok(())
}

pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
