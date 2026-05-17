//! Ratatui event loop: keyboard routing, modals, send flow, redraw cadence.

use crate::{
    account_id_to_human, append_addr_book, base_url, burn_replay_guard_status, centered_rect,
    choose_identity, clamp_sel, debug_json, f5_build_burn_form, f5_burn_hint_needed,
    f6_build_send_form, f7_build_stake_form, format_acct_cell, format_balance_cell, format_hms_utc,
    format_init_cell, format_policy_bits, handle_submit_done_history, http_client,
    identity_f3_action_label, identity_lock_status_suffix, inter_shard_status_short,
    is_cross_domain_route, masked_with_caret, merge_rpc_health, move_selection_down,
    move_selection_up, now_unix_secs, owner_and_receivers, poll_snapshot, preflight_sel_init_auto,
    preflight_xfer_dst, push_op_history, receiver_table_len, selected_row_for_panel,
    send_replay_guard_status, start_rpc_worker, status_footer_line, submit_claim, submit_stake,
    submit_unstake, validate_burn_form, validate_encrypt_passphrase_inputs, validate_send_form,
    validate_stake_form, value_with_caret, wallet_apply_auto_lock, wallet_lock_now, wallet_rekey,
    wallet_unlock, wallet_unlock_secs_clamped, AcctRow, Args, BookPromptModal, BurnField, BurnForm,
    DebugCache, EncryptField, EncryptModal, IdentitySource, OpStatus, OperationHistoryEntry, Panel,
    RpcEvent, RpcTask, SendField, SendForm, StakeField, StakeForm, StakeMode, Ui, UnlockModal,
    DEBUG_FETCH_INTERVAL, DETAIL_CHUNK_ROWS, FALLBACK_MODE_WARNING, FALLBACK_WARN_CHUNK_ROWS,
    UNKNOWN_INIT_NONCE_SENTINEL,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::{
        Color, Constraint, CrosstermBackend, Direction, Layout, Line, Modifier, Rect, Span, Style,
        Terminal, Text,
    },
    style::Stylize,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
    Frame,
};
use std::io::stdout;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

const CLAIM_NOTE_WAIT_SECS: u64 = 6;
const FREE_CLAIM_LIMIT_CODE: &str = "E_FREE_CLAIM_DAILY_LIMIT";
const FREE_CLAIM_LIMIT_NOTE: &str =
    "Free claim limit is exhausted. Use coin top-up/spend to force progress.";

#[derive(Clone, Copy)]
struct PendingClaimNote {
    owner_id: [u8; 32],
    base_marks: u32,
    wait_until: Instant,
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}

pub fn run(mut args: Args) -> std::io::Result<()> {
    let unlock_secs = wallet_unlock_secs_clamped(&args);
    let (mut identity, identity_note) = choose_identity(&args, unlock_secs)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;
    // Drop runs both on normal return and panic unwind, restoring terminal state.
    let _term_guard = TerminalGuard;
    let inherit_host_colors = inherit_host_colors_enabled();

    let mut ui = Ui::default();
    let mut owner_sel: usize = 0;
    let mut recv_sel: usize = 0;
    let mut active = Panel::Owner;
    let mut info_modal: Option<String> = None;
    let mut action_note: Option<String> = None;
    let mut action_note_warn = false;
    let mut unlock_modal: Option<UnlockModal> = None;
    let mut encrypt_modal: Option<EncryptModal> = None;
    let mut send_form: Option<SendForm> = None;
    let mut burn_form: Option<BurnForm> = None;
    let mut stake_form: Option<StakeForm> = None;
    let mut book_prompt: Option<BookPromptModal> = None;
    let mut history_open = false;
    let mut op_history: Vec<OperationHistoryEntry> = Vec::new();
    let mut last = Instant::now() - Duration::from_secs(10);
    let dbg = debug_json();
    ui.identity_note = identity_note.clone();
    let mut debug_cache = DebugCache::new();
    let mut send_req_id: u64 = 0;
    let mut inflight_send_req_id: Option<u64> = None;
    let mut inflight_burn_req_id: Option<u64> = None;
    let mut pending_claim_note: Option<PendingClaimNote> = None;
    let (rpc_tx, rpc_rx) = start_rpc_worker();

    loop {
        wallet_apply_auto_lock(&mut identity);
        if last.elapsed() >= Duration::from_secs(1) {
            let _ = rpc_tx.send(RpcTask::Poll);
            last = Instant::now();
        }
        loop {
            match rpc_rx.try_recv() {
                Ok(RpcEvent::PollDone(snapshot)) => {
                    ui.head = snapshot.head;
                    if let Some(head_height) = snapshot.head_height {
                        ui.head_height = Some(head_height);
                    }
                    ui.rows = snapshot.rows;
                    ui.err = snapshot.err;
                    ui.rpc_health = snapshot.rpc_health;
                    ui.rpc_shard_label = snapshot.rpc_shard_label;
                    if ui.err.is_empty() {
                        sync_claim_note(
                            &mut action_note,
                            &mut action_note_warn,
                            &mut pending_claim_note,
                            &ui.rows,
                            Instant::now(),
                        );
                    }
                }
                Ok(RpcEvent::DebugAccountDone {
                    id_hex,
                    detail,
                    rpc_health,
                }) => {
                    if debug_cache.inflight_id_hex.as_deref() == Some(id_hex.as_str()) {
                        debug_cache.inflight_id_hex = None;
                        debug_cache.last_fetch_at = Instant::now();
                    }
                    if debug_cache.selected_id_hex.as_deref() == Some(id_hex.as_str()) {
                        debug_cache.cached_detail = detail;
                    }
                    ui.rpc_health = merge_rpc_health(ui.rpc_health, rpc_health);
                }
                Ok(RpcEvent::SubmitDone {
                    req_id,
                    to_id,
                    result,
                }) => {
                    if inflight_burn_req_id == Some(req_id) {
                        inflight_burn_req_id = None;
                        pending_claim_note = None;
                        action_note = None;
                        action_note_warn = false;
                        if let Some(form) = burn_form.as_mut() {
                            form.apply_submit_result(&result);
                        }
                        let _ = rpc_tx.send(RpcTask::Poll);
                        continue;
                    }
                    if !handle_submit_done_history(
                        &mut inflight_send_req_id,
                        &mut op_history,
                        req_id,
                        &result,
                    ) {
                        continue;
                    }
                    if let Some(form) = send_form.as_mut() {
                        match result {
                            Ok(msg) => {
                                let offer = if let IdentitySource::Wallet(w) = &identity {
                                    if !w.has_recipient(&to_id) {
                                        Some(account_id_to_human(&to_id))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                                form.apply_submit_result(&Ok(msg), offer);
                                let _ = rpc_tx.send(RpcTask::Poll);
                            }
                            Err(e) => {
                                form.apply_submit_result(&Err(e), None);
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        if let Some(form) = burn_form.as_mut() {
            let _ = form.auto_advance_flow(Instant::now());
        }
        if let Some(form) = send_form.as_mut() {
            let _ = form.auto_advance_flow(Instant::now());
        }
        let (owner_rows, _active_owner_idx, receiver_rows) =
            owner_and_receivers(&ui.rows, &identity);

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                if let Some(um) = unlock_modal.as_mut() {
                    um.passphrase.clamp_cursor();
                    match k.code {
                        KeyCode::Esc => unlock_modal = None,
                        KeyCode::Enter => {
                            if let IdentitySource::Wallet(w) = &mut identity {
                                match wallet_unlock(w, um.passphrase.as_str().trim(), unlock_secs) {
                                    Ok(()) => unlock_modal = None,
                                    Err(_) => {
                                        um.status =
                                            "Unlock failed: wrong passphrase or corrupted wallet."
                                                .into();
                                        um.status_is_error = true;
                                    }
                                }
                            } else {
                                unlock_modal = None;
                            }
                        }
                        KeyCode::Left => um.passphrase.move_left(),
                        KeyCode::Right => um.passphrase.move_right(),
                        KeyCode::Home => um.passphrase.move_home(),
                        KeyCode::End => um.passphrase.move_end(),
                        KeyCode::Backspace => um.passphrase.backspace(),
                        KeyCode::Delete => um.passphrase.delete(),
                        KeyCode::Char(c) => um.passphrase.insert_char(c),
                        _ => {}
                    }
                } else if let Some(em) = encrypt_modal.as_mut() {
                    em.clamp_cursors();
                    match k.code {
                        KeyCode::Esc => encrypt_modal = None,
                        KeyCode::Enter => {
                            let p = em.passphrase.as_str().trim();
                            let c = em.confirm.as_str().trim();
                            if let Err(err_msg) = validate_encrypt_passphrase_inputs(p, c) {
                                em.status = err_msg.into();
                                em.status_is_error = true;
                            } else if let IdentitySource::Wallet(w) = &identity {
                                let rekey = if w.wallet_is_encrypted {
                                    w.secret_payload_plaintext.as_deref()
                                } else {
                                    None
                                };
                                match wallet_rekey(&w.wallet_path, p, rekey) {
                                    Ok(()) => {
                                        args.wallet_passphrase = Some(p.to_string());
                                        match choose_identity(&args, unlock_secs) {
                                            Ok((id, note)) => {
                                                identity = id;
                                                ui.identity_note = note;
                                                encrypt_modal = None;
                                            }
                                            Err(e) => {
                                                em.status = format!(
                                                    "wallet updated but reload failed: {e}"
                                                );
                                                em.status_is_error = true;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        em.status = e;
                                        em.status_is_error = true;
                                    }
                                }
                            } else {
                                encrypt_modal = None;
                            }
                        }
                        KeyCode::Tab | KeyCode::Down => em.next_field(),
                        KeyCode::Up => em.prev_field(),
                        KeyCode::Left => em.move_left(),
                        KeyCode::Right => em.move_right(),
                        KeyCode::Home => em.move_home(),
                        KeyCode::End => em.move_end(),
                        KeyCode::Backspace => em.backspace(),
                        KeyCode::Delete => em.delete(),
                        KeyCode::Char(ch) => em.insert_char(ch),
                        _ => {}
                    }
                } else if let Some(bp) = book_prompt.as_mut() {
                    bp.label.clamp_cursor();
                    match k.code {
                        KeyCode::Esc => book_prompt = None,
                        KeyCode::Enter => {
                            if let IdentitySource::Wallet(w) = &identity {
                                let path = &w.wallet_path;
                                let lbl = bp.label.as_str().trim();
                                match append_addr_book(
                                    path,
                                    &bp.to_display,
                                    if lbl.is_empty() { None } else { Some(lbl) },
                                ) {
                                    Ok(()) => match choose_identity(&args, unlock_secs) {
                                        Ok((id, note)) => {
                                            identity = id;
                                            ui.identity_note = note;
                                            book_prompt = None;
                                        }
                                        Err(e) => bp.status = format!("reload wallet: {e}"),
                                    },
                                    Err(e) => bp.status = e,
                                }
                            } else {
                                bp.status = "internal: no wallet path".into();
                            }
                        }
                        KeyCode::Left => bp.label.move_left(),
                        KeyCode::Right => bp.label.move_right(),
                        KeyCode::Home => bp.label.move_home(),
                        KeyCode::End => bp.label.move_end(),
                        KeyCode::Backspace => bp.label.backspace(),
                        KeyCode::Delete => bp.label.delete(),
                        KeyCode::Char(c) => bp.label.insert_char(c),
                        _ => {}
                    }
                } else if let Some(form) = stake_form.as_mut() {
                    form.clamp_active_cursor();
                    match k.code {
                        KeyCode::Esc => stake_form = None,
                        KeyCode::Up => form.prev_field(),
                        KeyCode::Down | KeyCode::Tab => form.next_field(),
                        KeyCode::Left => form.move_left(),
                        KeyCode::Right => form.move_right(),
                        KeyCode::Home => form.move_home(),
                        KeyCode::End => form.move_end(),
                        KeyCode::Backspace => form.backspace(),
                        KeyCode::Delete => form.delete(),
                        KeyCode::Enter => {
                            if form.active == StakeField::Confirm {
                                match validate_stake_form(form) {
                                    Ok((from, amount)) => {
                                        let tx_result = match form.mode {
                                            StakeMode::Stake => submit_stake(
                                                &from,
                                                amount,
                                                ui.head_height.unwrap_or(0),
                                                &identity,
                                            ),
                                            StakeMode::Unstake => submit_unstake(
                                                &from,
                                                amount,
                                                ui.head_height.unwrap_or(0),
                                                &identity,
                                            ),
                                        };
                                        match tx_result {
                                            Ok(()) => {
                                                form.status =
                                                    "submitted; refreshing account state...".into();
                                                form.status_is_error = false;
                                                let snapshot = poll_snapshot(&http_client());
                                                ui.head = snapshot.head;
                                                ui.head_height = snapshot.head_height;
                                                ui.rows = snapshot.rows;
                                                ui.err = snapshot.err;
                                                ui.rpc_health = snapshot.rpc_health;
                                                ui.rpc_shard_label = snapshot.rpc_shard_label;
                                            }
                                            Err(e) => {
                                                form.status = e;
                                                form.status_is_error = true;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        form.status = e;
                                        form.status_is_error = true;
                                    }
                                }
                            } else {
                                form.next_field();
                            }
                        }
                        KeyCode::Char(c) => form.insert_char(c),
                        _ => {}
                    }
                } else if let Some(form) = burn_form.as_mut() {
                    form.clamp_active_cursor();
                    match k.code {
                        KeyCode::Esc => burn_form = None,
                        KeyCode::Up => form.prev_field(),
                        KeyCode::Down | KeyCode::Tab => form.next_field(),
                        KeyCode::Left => form.move_left(),
                        KeyCode::Right => form.move_right(),
                        KeyCode::Home => form.move_home(),
                        KeyCode::End => form.move_end(),
                        KeyCode::Backspace => form.backspace(),
                        KeyCode::Delete => form.delete(),
                        KeyCode::Enter => {
                            if form.try_advance_flow(Instant::now()) {
                                continue;
                            }
                            if form.active == BurnField::Confirm {
                                if let Some(guard_msg) =
                                    burn_replay_guard_status(form, inflight_burn_req_id)
                                {
                                    form.status = guard_msg.into();
                                    form.status_is_error = true;
                                } else {
                                    match validate_burn_form(form) {
                                        Ok((from, mark_amount, beneficiary, purpose)) => {
                                            send_req_id = send_req_id.wrapping_add(1);
                                            inflight_burn_req_id = Some(send_req_id);
                                            form.status = "submitting burn...".into();
                                            form.status_is_error = false;
                                            form.flow = None;
                                            let _ = rpc_tx.send(RpcTask::SubmitBurnMark {
                                                req_id: send_req_id,
                                                from,
                                                mark_amount,
                                                beneficiary,
                                                purpose,
                                                identity: identity.clone(),
                                            });
                                        }
                                        Err(e) => {
                                            form.status = e;
                                            form.status_is_error = true;
                                        }
                                    }
                                }
                            } else {
                                form.next_field();
                            }
                        }
                        KeyCode::Char(c) => form.insert_char(c),
                        _ => {}
                    }
                } else if let Some(form) = send_form.as_mut() {
                    form.clamp_active_cursor();
                    match k.code {
                        KeyCode::Esc => {
                            let deferred_prompt = form.take_book_prompt();
                            send_form = None;
                            if let Some(to_display) = deferred_prompt {
                                book_prompt = Some(BookPromptModal::new(to_display));
                            }
                        }
                        KeyCode::Up => form.prev_field(),
                        KeyCode::Down | KeyCode::Tab => form.next_field(),
                        KeyCode::Left => form.move_left(),
                        KeyCode::Right => form.move_right(),
                        KeyCode::Home => form.move_home(),
                        KeyCode::End => form.move_end(),
                        KeyCode::Backspace => form.backspace(),
                        KeyCode::Delete => form.delete(),
                        KeyCode::Enter => {
                            if form.try_advance_flow(Instant::now()) {
                                continue;
                            }
                            if form.active == SendField::Confirm {
                                if let Some(guard_msg) =
                                    send_replay_guard_status(form, inflight_send_req_id)
                                {
                                    form.status = guard_msg.into();
                                    form.status_is_error = true;
                                } else {
                                    match validate_send_form(form) {
                                        Ok((from, to, amount, fee)) => {
                                            if let Err(e) = preflight_xfer_dst(
                                                &from,
                                                &to,
                                                &owner_rows,
                                                &receiver_rows,
                                            ) {
                                                form.status = e;
                                                form.status_is_error = true;
                                                continue;
                                            }
                                            send_req_id = send_req_id.wrapping_add(1);
                                            inflight_send_req_id = Some(send_req_id);
                                            let cross_domain = is_cross_domain_route(&from, &to);
                                            push_op_history(
                                                &mut op_history,
                                                OperationHistoryEntry {
                                                    req_id: send_req_id,
                                                    created_unix_secs: now_unix_secs(),
                                                    from_human: account_id_to_human(&from),
                                                    to_human: account_id_to_human(&to),
                                                    amount_units: amount,
                                                    fee_units: fee,
                                                    status: OpStatus::Pending,
                                                    note: if cross_domain {
                                                        "starting roaming intent...".into()
                                                    } else {
                                                        "submitting tx...".into()
                                                    },
                                                },
                                            );
                                            form.status = if cross_domain {
                                                inter_shard_status_short().into()
                                            } else {
                                                "submitting tx...".into()
                                            };
                                            form.status_is_error = false;
                                            form.flow = None;
                                            if cross_domain {
                                                let _ = rpc_tx.send(RpcTask::SubmitRoamingIntent {
                                                    req_id: send_req_id,
                                                    from,
                                                    to,
                                                    amount,
                                                    fee,
                                                    identity: identity.clone(),
                                                });
                                            } else {
                                                let _ = rpc_tx.send(RpcTask::SubmitTransfer {
                                                    req_id: send_req_id,
                                                    from,
                                                    to,
                                                    amount,
                                                    fee,
                                                    identity: identity.clone(),
                                                });
                                            }
                                        }
                                        Err(e) => {
                                            form.status = e;
                                            form.status_is_error = true;
                                        }
                                    }
                                }
                            } else {
                                form.next_field();
                            }
                        }
                        KeyCode::Char(c) => form.insert_char(c),
                        _ => {}
                    }
                } else if info_modal.is_some() {
                    match k.code {
                        KeyCode::Enter | KeyCode::Esc => info_modal = None,
                        KeyCode::Char('q') | KeyCode::F(10) => break,
                        _ => {}
                    }
                } else if history_open {
                    match k.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('h') | KeyCode::Char('H') => {
                            history_open = false;
                        }
                        KeyCode::Char('q') | KeyCode::F(10) => break,
                        _ => {}
                    }
                } else {
                    let owner_len = owner_rows.len();
                    let recv_len = receiver_table_len(&receiver_rows);
                    match k.code {
                        KeyCode::Char('q') | KeyCode::F(10) => break,
                        KeyCode::Tab => {
                            active = if active == Panel::Owner {
                                Panel::Receivers
                            } else {
                                Panel::Owner
                            };
                        }
                        KeyCode::F(3) => match &mut identity {
                            IdentitySource::SeedFallback => {
                                info_modal = Some(
                                    "F3 unlock/lock applies to `--wallet` / PWM_TUI_WALLET only."
                                        .into(),
                                );
                            }
                            IdentitySource::Wallet(w) if !w.wallet_is_encrypted => {
                                info_modal = Some(
                                    "This wallet is plaintext (dev mode): F3 unlock/lock is not needed."
                                        .into(),
                                );
                            }
                            IdentitySource::Wallet(w)
                                if w.wallet_is_encrypted && w.signing_key.is_some() =>
                            {
                                wallet_lock_now(w);
                                info_modal = Some(
                                    "Wallet locked: signing key, decrypted re-key cache, and unlock timer were cleared."
                                        .into(),
                                );
                            }
                            IdentitySource::Wallet(_) => {
                                unlock_modal = Some(UnlockModal::new());
                            }
                        },
                        KeyCode::F(4) => match &identity {
                            IdentitySource::SeedFallback => {
                                info_modal = Some(
                                    "F4 encrypt applies to a wallet file only (--wallet / PWM_TUI_WALLET)."
                                        .into(),
                                );
                            }
                            IdentitySource::Wallet(w) => {
                                if w.wallet_is_encrypted && w.secret_payload_plaintext.is_none() {
                                    info_modal = Some(
                                        "F4 re-key: unlock with F3 first, or start with PWM_TUI_WALLET_PASSPHRASE (cached decrypted material is cleared on auto-lock)."
                                            .into(),
                                    );
                                } else {
                                    encrypt_modal = Some(EncryptModal::new(w.wallet_is_encrypted));
                                }
                            }
                        },
                        KeyCode::F(5) => {
                            let selected_owner = owner_rows.get(owner_sel);
                            match preflight_sel_init_auto(selected_owner, "F5 burn", &identity) {
                                Ok(auto_init_msg) => {
                                    if let Some(owner) = owner_rows.get(owner_sel) {
                                        if f5_burn_hint_needed(owner.staked, owner.marks) {
                                            info_modal = Some(
                                                "No materialized marks yet. Use Claim or Stake/Unstake; Burn uses materialized marks."
                                                    .into(),
                                            );
                                            continue;
                                        }
                                        use pwm_core::tx::CLAIM_ALL;
                                        let claim_res = submit_claim(
                                            &owner.id,
                                            CLAIM_ALL,
                                            ui.head_height.unwrap_or(0),
                                            &identity,
                                        );
                                        let (mut claim_form_msg, mut claim_form_error) =
                                            match claim_res {
                                                Ok(()) => {
                                                    action_note_warn = false;
                                                    action_note = Some(
                                                    "Claim submitted; waiting for confirmation..."
                                                        .to_string(),
                                                );
                                                    pending_claim_note = Some(PendingClaimNote {
                                                        owner_id: owner.id,
                                                        base_marks: owner.marks,
                                                        wait_until: Instant::now()
                                                            + Duration::from_secs(
                                                                CLAIM_NOTE_WAIT_SECS,
                                                            ),
                                                    });
                                                    (
                                                        action_note
                                                            .clone()
                                                            .unwrap_or_else(String::new),
                                                        false,
                                                    )
                                                }
                                                Err(e) if e.contains("E_CLAIM_OVER_MATURED") => {
                                                    pending_claim_note = None;
                                                    action_note_warn = false;
                                                    action_note = Some(
                                                        "Claim accepted; no new marks yet".into(),
                                                    );
                                                    (
                                                        action_note
                                                            .clone()
                                                            .unwrap_or_else(String::new),
                                                        false,
                                                    )
                                                }
                                                Err(e) => {
                                                    pending_claim_note = None;
                                                    let (claim_note, is_error, is_warn) =
                                                        map_claim_err_note(&e);
                                                    action_note_warn = is_warn;
                                                    action_note = Some(claim_note.clone());
                                                    (claim_note, is_error)
                                                }
                                            };
                                        let snapshot = poll_snapshot(&http_client());
                                        ui.head = snapshot.head;
                                        ui.head_height = snapshot.head_height;
                                        ui.rows = snapshot.rows;
                                        ui.err = snapshot.err;
                                        ui.rpc_health = snapshot.rpc_health;
                                        ui.rpc_shard_label = snapshot.rpc_shard_label;
                                        if ui.err.is_empty() {
                                            sync_claim_note(
                                                &mut action_note,
                                                &mut action_note_warn,
                                                &mut pending_claim_note,
                                                &ui.rows,
                                                Instant::now(),
                                            );
                                            claim_form_msg = action_note
                                                .clone()
                                                .unwrap_or_else(|| claim_form_msg.clone());
                                            claim_form_error = claim_form_error
                                                || claim_form_msg
                                                    .starts_with("Claim attempt failed:");
                                        }

                                        let (fresh_owner_rows, _, _) =
                                            owner_and_receivers(&ui.rows, &identity);
                                        match f5_build_burn_form(
                                            &identity,
                                            &fresh_owner_rows,
                                            owner_sel,
                                            &receiver_rows,
                                            recv_sel,
                                        ) {
                                            Ok(mut form) => {
                                                if let Some(fresh_owner) =
                                                    fresh_owner_rows.get(owner_sel)
                                                {
                                                    form.marks_available = fresh_owner.marks;
                                                }
                                                if let Some(init_msg) = auto_init_msg {
                                                    form.status = format!(
                                                        "{init_msg}; {claim_form_msg}; fill burn fields"
                                                    );
                                                } else {
                                                    form.status = format!(
                                                        "{claim_form_msg}; fill burn fields"
                                                    );
                                                }
                                                form.status_is_error = claim_form_error;
                                                burn_form = Some(form);
                                            }
                                            Err(e) => info_modal = Some(e),
                                        }
                                    } else {
                                        info_modal =
                                            Some("F5 burn blocked: no selected Owner row".into());
                                    }
                                }
                                Err(msg) => info_modal = Some(msg),
                            }
                        }
                        KeyCode::F(6) => {
                            let selected_owner = owner_rows.get(owner_sel);
                            match preflight_sel_init_auto(selected_owner, "F6 send", &identity) {
                                Ok(auto_init_msg) => match f6_build_send_form(
                                    &identity,
                                    &owner_rows,
                                    owner_sel,
                                    &receiver_rows,
                                    recv_sel,
                                ) {
                                    Ok(mut form) => {
                                        if let Some(init_msg) = auto_init_msg {
                                            form.status = format!(
                                                "{init_msg}; sender initialized, continue with transfer fields"
                                            );
                                            form.status_is_error = false;
                                        }
                                        send_form = Some(form);
                                    }
                                    Err(msg) => info_modal = Some(msg),
                                },
                                Err(msg) => info_modal = Some(msg),
                            }
                        }
                        KeyCode::Char('h') | KeyCode::Char('H') => {
                            history_open = true;
                        }
                        KeyCode::Char('u') | KeyCode::Char('U') => {
                            match f7_build_stake_form(
                                &identity,
                                &owner_rows,
                                owner_sel,
                                StakeMode::Unstake,
                            ) {
                                Ok(form) => stake_form = Some(form),
                                Err(msg) => info_modal = Some(msg),
                            }
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            match f7_build_stake_form(
                                &identity,
                                &owner_rows,
                                owner_sel,
                                StakeMode::Stake,
                            ) {
                                Ok(form) => stake_form = Some(form),
                                Err(msg) => info_modal = Some(msg),
                            }
                        }
                        KeyCode::Down => match active {
                            Panel::Owner => {
                                move_selection_down(&mut owner_sel, owner_len);
                            }
                            Panel::Receivers => {
                                move_selection_down(&mut recv_sel, recv_len);
                            }
                        },
                        KeyCode::Up => match active {
                            Panel::Owner => move_selection_up(&mut owner_sel),
                            Panel::Receivers => move_selection_up(&mut recv_sel),
                        },
                        _ => {}
                    }
                }
            }
        }

        let owner_len = owner_rows.len();
        let recv_len = receiver_table_len(&receiver_rows);
        clamp_sel(&mut owner_sel, owner_len);
        clamp_sel(&mut recv_sel, recv_len);

        let selected_row =
            selected_row_for_panel(active, &owner_rows, owner_sel, &receiver_rows, recv_sel);
        if let Some(r) = selected_row {
            let rescue_txt = r
                .rescue_address
                .as_ref()
                .map(account_id_to_human)
                .unwrap_or_else(|| "-".to_string());
            ui.detail_line = format!(
                "selected: {} | init={} | nonce={}\nPWM: {}\nStaked: {}\nMarks: {}\nPolicy active: {}\nPolicy dormant: {}\nFinalized: {}\nRescue: {}\nOwner: {}/{}/{}",
                format_acct_cell(r),
                format_init_cell(r),
                if r.nonce == UNKNOWN_INIT_NONCE_SENTINEL {
                    "?".to_string()
                } else {
                    r.nonce.to_string()
                },
                format_balance_cell(r),
                r.staked,
                r.marks,
                format_policy_bits(r.active_policies),
                format_policy_bits(r.dormant_policies),
                if r.finalized { "yes" } else { "no" },
                rescue_txt,
                if r.owner_kind.is_empty() {
                    "-"
                } else {
                    r.owner_kind.as_str()
                },
                if r.owner_name.is_empty() {
                    "-"
                } else {
                    r.owner_name.as_str()
                },
                if r.owner_country.is_empty() {
                    "-"
                } else {
                    r.owner_country.as_str()
                },
            );
            if dbg {
                let selected_hex = r.id_hex.clone();
                if debug_cache.selected_id_hex.as_deref() != Some(selected_hex.as_str()) {
                    debug_cache.selected_id_hex = Some(selected_hex.clone());
                    debug_cache.cached_detail.clear();
                    debug_cache.inflight_id_hex = None;
                    debug_cache.last_fetch_at = Instant::now() - DEBUG_FETCH_INTERVAL;
                }
                let should_fetch = debug_cache.inflight_id_hex.is_none()
                    && debug_cache.last_fetch_at.elapsed() >= DEBUG_FETCH_INTERVAL;
                if should_fetch {
                    debug_cache.inflight_id_hex = Some(selected_hex.clone());
                    let _ = rpc_tx.send(RpcTask::DebugAccount {
                        id_hex: selected_hex,
                    });
                }
                ui.debug_detail = debug_cache.cached_detail.clone();
            } else {
                debug_cache.selected_id_hex = None;
                debug_cache.inflight_id_hex = None;
                debug_cache.cached_detail.clear();
                ui.debug_detail.clear();
            }
        } else {
            debug_cache.selected_id_hex = None;
            debug_cache.inflight_id_hex = None;
            debug_cache.cached_detail.clear();
            ui.detail_line.clear();
            ui.debug_detail.clear();
        }

        term.draw(|f| {
            if !inherit_host_colors {
                f.render_widget(
                    Block::default().style(Style::default().bg(Color::Black)),
                    f.size(),
                );
            }
            let dbg = debug_json();
            let is_fallback = matches!(identity, IdentitySource::SeedFallback);
            let (chunks, warn_chunk, detail_chunk, debug_chunk, foot_chunk) =
                split_main_layout(f.size(), dbg, is_fallback);
            render_main_panels(
                f,
                chunks[0],
                &owner_rows,
                owner_sel,
                &receiver_rows,
                recv_sel,
                active,
                &identity,
            );

            render_warn_strip(f, &chunks, warn_chunk);

            render_detail_debug_strip(f, &chunks, detail_chunk, debug_chunk, &ui);

            render_footer_area(
                f,
                chunks[foot_chunk],
                &ui,
                &identity,
                dbg,
                action_note.as_deref(),
                action_note_warn,
            );

            if let Some(msg) = info_modal.as_ref() {
                render_info_modal(f, f.size(), msg);
            }

            if history_open {
                render_history_overlay(f, f.size(), &op_history);
            }

            if let Some(form) = send_form.as_ref() {
                render_send_modal(f, f.size(), form);
            }

            if let Some(form) = burn_form.as_ref() {
                render_burn_modal(f, f.size(), form);
            }
            if let Some(form) = stake_form.as_ref() {
                render_stake_modal(f, f.size(), form);
            }

            if let Some(bp) = book_prompt.as_ref() {
                render_book_modal(f, f.size(), bp);
            }

            if let Some(um) = unlock_modal.as_ref() {
                render_unlock_modal(f, f.size(), um);
            }

            if let Some(em) = encrypt_modal.as_ref() {
                render_encrypt_modal(f, f.size(), em);
            }
        })?;
    }

    Ok(())
}

fn inherit_host_colors_enabled() -> bool {
    std::env::var("PWM_TUI_INHERIT_HOST_COLORS")
        .ok()
        .map(|raw| {
            let v = raw.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes")
        })
        .unwrap_or(false)
}

/// Renders owner/receiver top panels with shared header row.
fn render_main_panels(
    f: &mut Frame,
    area: Rect,
    owner_rows: &[AcctRow],
    owner_sel: usize,
    receiver_rows: &[AcctRow],
    recv_sel: usize,
    active: Panel,
    identity: &IdentitySource,
) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let header = panel_head_row();
    render_owner_panel(f, panels[0], owner_rows, owner_sel, active, &header);
    render_recv_panel(
        f,
        panels[1],
        receiver_rows,
        recv_sel,
        active,
        identity,
        &header,
    );
}

/// Builds the standard owner/receiver table header.
fn panel_head_row() -> Row<'static> {
    Row::new(vec![
        Cell::from("Address"),
        Cell::from("Balance"),
        Cell::from("Marks"),
    ])
    .style(Style::default().add_modifier(Modifier::BOLD))
}

/// Returns bright focus style for active panel border and title.
fn panel_focus_style(active: Panel, panel: Panel) -> Style {
    if active == panel {
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

/// Renders fallback seed mode warning strip.
fn render_warn_strip(f: &mut Frame, chunks: &[Rect], warn_chunk: Option<usize>) {
    if let Some(wi) = warn_chunk {
        f.render_widget(
            Paragraph::new(FALLBACK_MODE_WARNING)
                .style(Style::default().fg(Color::Yellow))
                .wrap(Wrap { trim: true })
                .block(Block::default().borders(Borders::ALL).title("WARNING")),
            chunks[wi],
        );
    }
}

/// Renders owner table panel with active row and focus border.
fn render_owner_panel(
    f: &mut Frame,
    area: Rect,
    owner_rows: &[AcctRow],
    owner_sel: usize,
    active: Panel,
    header: &Row<'static>,
) {
    let owner_focus_style = panel_focus_style(active, Panel::Owner);
    let owner_rows: Vec<Row> = owner_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let style = if i == owner_sel {
                Style::default().reversed()
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(format_acct_cell(r)),
                Cell::from(format_balance_cell(r)),
                Cell::from(r.marks.to_string()),
            ])
            .style(style)
        })
        .collect();
    let owner_block = Block::default()
        .borders(Borders::ALL)
        .title("Owner")
        .border_style(owner_focus_style)
        .title_style(owner_focus_style);
    let inner = owner_block.inner(area);
    f.render_widget(owner_block, area);
    let owner_table = Table::new(
        owner_rows,
        [
            Constraint::Min(36),
            Constraint::Length(18),
            Constraint::Length(10),
        ],
    )
    .header(header.clone());
    f.render_widget(owner_table, inner);
}

/// Renders receivers panel with "New Recipient" top row.
fn render_recv_panel(
    f: &mut Frame,
    area: Rect,
    receiver_rows: &[AcctRow],
    recv_sel: usize,
    active: Panel,
    identity: &IdentitySource,
    header: &Row<'static>,
) {
    let recv_focus_style = panel_focus_style(active, Panel::Receivers);
    let recv_rows: Vec<Row> = receiver_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let display_idx = i + 1;
            let style = if display_idx == recv_sel {
                Style::default().reversed()
            } else {
                Style::default()
            };
            Row::new(vec![
                Cell::from(format_acct_cell(r)),
                Cell::from(format_balance_cell(r)),
                Cell::from(r.marks.to_string()),
            ])
            .style(style)
        })
        .collect();
    let new_recipient_style = if recv_sel == 0 {
        Style::default().reversed()
    } else {
        Style::default()
    };
    let mut recv_rows_all = Vec::with_capacity(recv_rows.len() + 1);
    recv_rows_all.push(
        Row::new(vec![
            Cell::from("New Recipient"),
            Cell::from("-"),
            Cell::from("-"),
        ])
        .style(new_recipient_style),
    );
    recv_rows_all.extend(recv_rows);
    let recv_title = match identity {
        IdentitySource::Wallet(w) if !w.address_book.is_empty() => "Receivers (address book)",
        _ => "Receivers",
    };
    let recv_block = Block::default()
        .borders(Borders::ALL)
        .title(recv_title)
        .border_style(recv_focus_style)
        .title_style(recv_focus_style);
    let inner = recv_block.inner(area);
    f.render_widget(recv_block, area);
    let recv_table = Table::new(
        recv_rows_all,
        [
            Constraint::Min(36),
            Constraint::Length(12),
            Constraint::Length(10),
        ],
    )
    .header(header.clone());
    f.render_widget(recv_table, inner);
}

/// Renders detail line and optional debug json area.
fn render_detail_debug_strip(
    f: &mut Frame,
    chunks: &[Rect],
    detail_chunk: usize,
    debug_chunk: Option<usize>,
    ui: &Ui,
) {
    f.render_widget(
        Paragraph::new(ui.detail_line.clone()).block(Block::default().borders(Borders::ALL)),
        chunks[detail_chunk],
    );
    if let Some(di) = debug_chunk {
        f.render_widget(
            Paragraph::new(ui.debug_detail.clone())
                .block(Block::default().borders(Borders::ALL).title("debug JSON")),
            chunks[di],
        );
    }
}

/// Split root screen into stable chunk indexes for layout and footer.
fn split_main_layout(
    area: Rect,
    dbg: bool,
    is_fallback: bool,
) -> (Rc<[Rect]>, Option<usize>, usize, Option<usize>, usize) {
    let main_constraints = match (dbg, is_fallback) {
        (false, false) => vec![
            Constraint::Min(4),
            Constraint::Length(DETAIL_CHUNK_ROWS),
            Constraint::Length(1),
        ],
        (false, true) => vec![
            Constraint::Min(4),
            Constraint::Length(FALLBACK_WARN_CHUNK_ROWS),
            Constraint::Length(DETAIL_CHUNK_ROWS),
            Constraint::Length(1),
        ],
        (true, false) => vec![
            Constraint::Min(4),
            Constraint::Length(DETAIL_CHUNK_ROWS),
            Constraint::Percentage(35),
            Constraint::Length(1),
        ],
        (true, true) => vec![
            Constraint::Min(4),
            Constraint::Length(FALLBACK_WARN_CHUNK_ROWS),
            Constraint::Length(DETAIL_CHUNK_ROWS),
            Constraint::Percentage(35),
            Constraint::Length(1),
        ],
    };
    let (warn_chunk, detail_chunk, debug_chunk, foot_chunk) = match (dbg, is_fallback) {
        (false, false) => (None, 1, None, 2),
        (false, true) => (Some(1), 2, None, 3),
        (true, false) => (None, 1, Some(2), 3),
        (true, true) => (Some(1), 2, Some(3), 4),
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(main_constraints)
        .split(area);
    (chunks, warn_chunk, detail_chunk, debug_chunk, foot_chunk)
}

/// Render one-line footer with current identity/rpc status.
fn render_footer_area(
    f: &mut Frame,
    area: Rect,
    ui: &Ui,
    identity: &IdentitySource,
    dbg: bool,
    action_note: Option<&str>,
    action_note_warn: bool,
) {
    let foot_identity = format!(
        "{}{}",
        ui.identity_note,
        identity_lock_status_suffix(identity)
    );
    let foot_line = status_footer_line(
        &ui.head,
        &ui.err,
        &foot_identity,
        identity_f3_action_label(identity),
        ui.rpc_health,
        dbg,
        &base_url(),
        ui.rpc_shard_label.as_deref(),
        action_note,
        action_note_warn,
    );
    // Single-line status: no `Borders::ALL` here — `Length(1)` cannot fit a full box (broken corners).
    f.render_widget(Paragraph::new(foot_line), area);
}

fn sync_claim_note(
    action_note: &mut Option<String>,
    action_note_warn: &mut bool,
    pending_claim_note: &mut Option<PendingClaimNote>,
    rows: &[AcctRow],
    now: Instant,
) {
    let Some(pending) = pending_claim_note else {
        return;
    };
    if let Some(row) = rows.iter().find(|row| row.id == pending.owner_id) {
        let gain = row.marks.saturating_sub(pending.base_marks);
        if gain > 0 {
            *action_note = Some(format!("Claim confirmed (+{gain} marks)"));
            *action_note_warn = false;
            *pending_claim_note = None;
            return;
        }
    }
    if now >= pending.wait_until {
        *action_note = Some("Claim accepted; no new marks yet".into());
        *action_note_warn = false;
        *pending_claim_note = None;
    }
}

fn map_claim_err_note(err: &str) -> (String, bool, bool) {
    if err.contains(FREE_CLAIM_LIMIT_CODE) {
        (FREE_CLAIM_LIMIT_NOTE.to_string(), false, true)
    } else {
        (format!("Claim attempt failed: {err}"), true, false)
    }
}

/// Renders the generic info popup shown for action feedback.
fn render_info_modal(f: &mut Frame, screen: Rect, msg: &str) {
    let area = centered_rect(50, 20, screen);
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(format!("{msg}\n\nPress Enter/Esc"))
            .block(Block::default().borders(Borders::ALL).title("Action")),
        area,
    );
}

/// Renders stake/unstake modal.
fn render_stake_modal(f: &mut Frame, screen: Rect, form: &StakeForm) {
    let area = centered_rect(66, 44, screen);
    f.render_widget(Clear, area);
    let amount_val = value_with_caret(
        form.amount.as_str(),
        form.amount.cursor(),
        form.active == StakeField::Amount,
    );
    let conf_val = value_with_caret(
        form.confirm.as_str(),
        form.confirm.cursor(),
        form.active == StakeField::Confirm,
    );
    let mut text = Text::from(vec![
        Line::from(form.title()),
        Line::from(""),
        Line::from(format!("From: {}", form.from)),
        Line::from(format!(
            "{}: {} PWM",
            form.limit_label(),
            pwm_core::format_pwm(form.limit_units)
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw(if form.active == StakeField::Amount {
                "> Amount: "
            } else {
                "  Amount: "
            }),
            Span::styled(
                amount_val,
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(if form.active == StakeField::Amount {
                        Color::Yellow
                    } else {
                        Color::White
                    }),
            ),
        ]),
        Line::from(""),
        Line::from("Type 'yes' to confirm, Enter to submit"),
        Line::from(vec![
            Span::raw(if form.active == StakeField::Confirm {
                "> Confirm: "
            } else {
                "  Confirm: "
            }),
            Span::styled(
                conf_val,
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(if form.active == StakeField::Confirm {
                        Color::Yellow
                    } else {
                        Color::White
                    }),
            ),
        ]),
    ]);
    text.lines.push(Line::from(""));
    let status_style = if form.status_is_error {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    text.lines
        .push(Line::from(Span::styled(form.status.clone(), status_style)));
    text.lines.push(Line::from(""));
    text.lines
        .push(Line::from("Esc to cancel | Tab/Up/Down move fields"));
    f.render_widget(
        Paragraph::new(text).block(Block::default().borders(Borders::ALL).title("Stake")),
        area,
    );
}

/// Renders operation history overlay in empty/table modes.
fn render_history_overlay(f: &mut Frame, screen: Rect, op_history: &[OperationHistoryEntry]) {
    let area = centered_rect(86, 62, screen);
    f.render_widget(Clear, area);
    if op_history.is_empty() {
        let body = Text::from(vec![
            Line::from("No operations yet."),
            Line::from(""),
            Line::from("Use F6 to submit a transfer; status timeline appears here."),
            Line::from(""),
            Line::from("Close: H / Enter / Esc"),
        ]);
        f.render_widget(
            Paragraph::new(body).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Operations History"),
            ),
            area,
        );
    } else {
        let header = Row::new(vec![
            Cell::from("Time"),
            Cell::from("Status"),
            Cell::from("To"),
            Cell::from("Amount"),
            Cell::from("Note"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD));
        let rows: Vec<Row> = op_history
            .iter()
            .map(|it| {
                let status_style = match it.status {
                    OpStatus::Pending => Style::default().fg(Color::Yellow),
                    OpStatus::Ok => Style::default().fg(Color::Green),
                    OpStatus::Error => Style::default().fg(Color::Red),
                };
                Row::new(vec![
                    Cell::from(format_hms_utc(it.created_unix_secs)),
                    Cell::from(it.status.as_str()).style(status_style),
                    Cell::from(it.to_human.clone()),
                    Cell::from(format!("{} (+fee {})", it.amount_units, it.fee_units)),
                    Cell::from(format!("{} | from {}", it.note, it.from_human)),
                ])
            })
            .collect();
        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(9),
                Constraint::Percentage(33),
                Constraint::Length(24),
                Constraint::Min(20),
            ],
        )
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Operations History (latest first, H/Esc close)"),
        );
        f.render_widget(table, area);
    }
}

/// Renders the F6 send form modal.
fn render_send_modal(f: &mut Frame, screen: Rect, form: &SendForm) {
    let area = centered_rect(70, 55, screen);
    f.render_widget(Clear, area);
    let fields = [
        ("from", form.from.as_str(), false, false),
        (
            "to",
            form.to.as_str(),
            form.active == SendField::To,
            form.to_editable,
        ),
        (
            "amount",
            form.amount.as_str(),
            form.active == SendField::Amount,
            true,
        ),
        (
            "fee",
            form.fee.as_str(),
            form.active == SendField::Fee,
            true,
        ),
        (
            "confirm",
            form.confirm.as_str(),
            form.active == SendField::Confirm,
            true,
        ),
    ];
    let mut text = Text::from(vec![Line::from("F6 Send form"), Line::from("")]);
    for (name, value, active_field, editable) in fields {
        let lock_hint = match name {
            "from" => " [fixed]",
            "to" if !form.to_editable => " [fixed from receiver]",
            _ => "",
        };
        let prefix = if active_field { "> " } else { "  " };
        let cursor = match name {
            "to" => form.to.cursor(),
            "amount" => form.amount.cursor(),
            "fee" => form.fee.cursor(),
            "confirm" => form.confirm.cursor(),
            _ => 0,
        };
        let shown = value_with_caret(value, cursor, active_field && editable);
        let label = format!("{prefix}{name:<7}: ");
        let bg = if editable {
            Color::DarkGray
        } else {
            Color::Black
        };
        let style = if active_field && editable {
            Style::default().bg(bg).fg(Color::Yellow)
        } else if editable {
            Style::default().bg(bg)
        } else {
            Style::default()
        };
        text.lines.push(Line::from(vec![
            Span::raw(label),
            Span::styled(shown, style),
            Span::raw(lock_hint),
        ]));
    }
    text.lines.push(Line::from(""));
    text.lines.push(Line::from(
        "Enter=next/submit(confirm), Tab/Up/Down=move, Left/Right/Home/End, Backspace/Delete, Esc=close",
    ));
    text.lines.push(Line::from(
        "amount/fee/balance: decimal PWM (1 PWM = 1_000_000 raw, max 6 decimals)",
    ));
    text.lines.push(Line::from(""));
    text.lines.push(Line::from("status:"));
    let status_style = if form.status_is_error {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    for line in form.status.lines() {
        text.lines
            .push(Line::from(Span::styled(line.to_string(), status_style)));
    }
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Send")),
        area,
    );
}

/// Renders the F5 burn-mark form modal (v2 purpose field).
fn render_burn_modal(f: &mut Frame, screen: Rect, form: &BurnForm) {
    let area = centered_rect(72, 58, screen);
    f.render_widget(Clear, area);
    let fields = [
        ("from", form.from.as_str(), false, false),
        (
            "marks",
            form.mark_amount.as_str(),
            form.active == BurnField::MarkAmount,
            true,
        ),
        (
            "benefic",
            form.beneficiary.as_str(),
            form.active == BurnField::Beneficiary,
            form.beneficiary_editable,
        ),
        (
            "purpose",
            form.purpose.as_str(),
            form.active == BurnField::Purpose,
            true,
        ),
        (
            "confirm",
            form.confirm.as_str(),
            form.active == BurnField::Confirm,
            true,
        ),
    ];
    let mut text = Text::from(vec![
        Line::from("F5 Burn marks (v2)"),
        Line::from("marks: whole mark units; beneficiary: empty for none; purpose: RFC 0011 (default ok for dev)."),
        Line::from("Marks materialize via Claim or Stake/Unstake. Burn uses materialized marks."),
        Line::from(format!("Marks available: {}", form.marks_available)),
        Line::from(""),
    ]);
    for (name, value, active_field, editable) in fields {
        let lock_hint = match name {
            "from" => " [fixed]",
            "benefic" if !form.beneficiary_editable => " [fixed from receiver]",
            _ => "",
        };
        let prefix = if active_field { "> " } else { "  " };
        let cursor = match name {
            "from" => 0,
            "marks" => form.mark_amount.cursor(),
            "benefic" => form.beneficiary.cursor(),
            "purpose" => form.purpose.cursor(),
            "confirm" => form.confirm.cursor(),
            _ => 0,
        };
        let shown = value_with_caret(value, cursor, active_field && editable);
        let label = format!("{prefix}{name:<7}: ");
        let bg = if editable {
            Color::DarkGray
        } else {
            Color::Black
        };
        let style = if active_field && editable {
            Style::default().bg(bg).fg(Color::Yellow)
        } else if editable {
            Style::default().bg(bg)
        } else {
            Style::default()
        };
        text.lines.push(Line::from(vec![
            Span::raw(label),
            Span::styled(shown, style),
            Span::raw(lock_hint),
        ]));
    }
    text.lines.push(Line::from(""));
    text.lines.push(Line::from(
        "Enter=next/submit on confirm (type yes), Tab/Up/Down=field, Esc=close",
    ));
    text.lines.push(Line::from(""));
    text.lines.push(Line::from("status:"));
    let status_style = if form.status_is_error {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    };
    for line in form.status.lines() {
        text.lines
            .push(Line::from(Span::styled(line.to_string(), status_style)));
    }
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Burn marks")),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        map_claim_err_note, panel_focus_style, sync_claim_note, Panel, PendingClaimNote,
        FREE_CLAIM_LIMIT_NOTE,
    };
    use crate::AcctRow;
    use ratatui::style::{Color, Modifier};
    use std::time::{Duration, Instant};

    fn mk_row(id: [u8; 32], marks: u32) -> AcctRow {
        AcctRow {
            id,
            id_hex: hex::encode(id),
            balance_pwm: 0,
            initialized: true,
            nonce: 0,
            marks,
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

    #[test]
    fn claim_note_confirms_delta() {
        let id = [7u8; 32];
        let mut action_note = Some("Claim submitted; waiting for confirmation...".to_string());
        let mut action_note_warn = true;
        let mut pending = Some(PendingClaimNote {
            owner_id: id,
            base_marks: 10,
            wait_until: Instant::now() + Duration::from_secs(5),
        });
        let rows = vec![mk_row(id, 14)];
        sync_claim_note(
            &mut action_note,
            &mut action_note_warn,
            &mut pending,
            &rows,
            Instant::now(),
        );
        assert_eq!(action_note.as_deref(), Some("Claim confirmed (+4 marks)"));
        assert!(!action_note_warn);
        assert!(pending.is_none());
    }

    #[test]
    fn claim_note_wait_timeout() {
        let id = [9u8; 32];
        let mut action_note = Some("Claim submitted; waiting for confirmation...".to_string());
        let mut action_note_warn = true;
        let mut pending = Some(PendingClaimNote {
            owner_id: id,
            base_marks: 10,
            wait_until: Instant::now() - Duration::from_secs(1),
        });
        let rows = vec![mk_row(id, 10)];
        sync_claim_note(
            &mut action_note,
            &mut action_note_warn,
            &mut pending,
            &rows,
            Instant::now(),
        );
        assert_eq!(
            action_note.as_deref(),
            Some("Claim accepted; no new marks yet")
        );
        assert!(!action_note_warn);
        assert!(pending.is_none());
    }

    #[test]
    fn claim_daily_limit_warn_note() {
        let err =
            "claim failed: 400 Bad Request reject: code=E_FREE_CLAIM_DAILY_LIMIT trace_id=abc";
        let (note, is_error, is_warn) = map_claim_err_note(err);
        assert_eq!(note, FREE_CLAIM_LIMIT_NOTE);
        assert!(!is_error);
        assert!(is_warn);
        assert!(!note.contains("trace_id="));
    }

    #[test]
    fn claim_other_error_path() {
        let err = "claim failed: 503 Service Unavailable";
        let (note, is_error, is_warn) = map_claim_err_note(err);
        assert_eq!(note, format!("Claim attempt failed: {err}"));
        assert!(is_error);
        assert!(!is_warn);
    }

    #[test]
    fn panel_focus_active_bright() {
        let style = panel_focus_style(Panel::Owner, Panel::Owner);
        assert_eq!(style.fg, Some(Color::LightYellow));
        assert!(style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn panel_focus_inactive_neutral() {
        let style = panel_focus_style(Panel::Owner, Panel::Receivers);
        assert_eq!(style.fg, None);
        assert_eq!(style.add_modifier, Modifier::empty());
    }
}

/// Renders save-to-address-book prompt modal.
fn render_book_modal(f: &mut Frame, screen: Rect, bp: &BookPromptModal) {
    let area = centered_rect(72, 40, screen);
    f.render_widget(Clear, area);
    let shown = value_with_caret(bp.label.as_str(), bp.label.cursor(), true);
    let body = Text::from(vec![
        Line::from("Recipient is not in the wallet address book yet."),
        Line::from(""),
        Line::from("Address:"),
        Line::from(bp.to_display.clone()),
        Line::from(""),
        Line::from(vec![
            Span::raw("Label (optional, prefix in table): "),
            Span::styled(
                shown,
                Style::default().bg(Color::DarkGray).fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(bp.status.clone()),
        Line::from(""),
        Line::from(
            "Enter = append to wallet file   Esc = skip   Left/Right/Home/End/Backspace/Delete",
        ),
    ]);
    f.render_widget(
        Paragraph::new(body).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Save to address book?"),
        ),
        area,
    );
}

/// Renders F3 wallet unlock modal.
fn render_unlock_modal(f: &mut Frame, screen: Rect, um: &UnlockModal) {
    let area = centered_rect(62, 38, screen);
    f.render_widget(Clear, area);
    let shown = masked_with_caret(um.passphrase.as_str(), um.passphrase.cursor());
    let mut body = Text::from(vec![
        Line::from("F3 Unlock wallet (passphrase is not logged)"),
        Line::from(""),
        Line::from(vec![
            Span::raw("passphrase: "),
            Span::styled(
                shown,
                Style::default().bg(Color::DarkGray).fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
    ]);
    body.lines.push(if um.status_is_error {
        Line::from(vec![
            Span::raw("status: "),
            Span::styled(um.status.clone(), Style::default().fg(Color::Red)),
        ])
    } else {
        Line::from(um.status.clone())
    });
    body.lines.push(Line::from(""));
    body.lines.push(Line::from(
        "Enter = unlock   Esc = cancel   Left/Right/Home/End/Backspace/Delete",
    ));
    f.render_widget(
        Paragraph::new(body).block(Block::default().borders(Borders::ALL).title("Unlock")),
        area,
    );
}

/// Renders F4 encrypt/re-key modal.
fn render_encrypt_modal(f: &mut Frame, screen: Rect, em: &EncryptModal) {
    let area = centered_rect(68, 46, screen);
    f.render_widget(Clear, area);
    let title = if em.is_rekey {
        "Encrypt (re-key)"
    } else {
        "Encrypt wallet"
    };
    let intro = if em.is_rekey {
        "F4 Re-key: new passphrase replaces the old one on disk (passphrase never logged)."
    } else {
        "F4 Encrypt plaintext wallet (KDF+AEAD matches pwm-cli; passphrase never logged)."
    };
    let pass_active = em.active == EncryptField::Passphrase;
    let pass_shown = masked_with_caret(em.passphrase.as_str(), em.passphrase.cursor());
    let conf_shown = masked_with_caret(em.confirm.as_str(), em.confirm.cursor());
    let mut body = Text::from(vec![
        Line::from(intro),
        Line::from(""),
        Line::from(vec![
            Span::raw(if pass_active { "> " } else { "  " }),
            Span::raw("passphrase: "),
            Span::styled(
                pass_shown,
                Style::default().bg(Color::DarkGray).fg(if pass_active {
                    Color::Yellow
                } else {
                    Color::White
                }),
            ),
        ]),
        Line::from(vec![
            Span::raw(if !pass_active { "> " } else { "  " }),
            Span::raw("confirm:    "),
            Span::styled(
                conf_shown,
                Style::default().bg(Color::DarkGray).fg(if !pass_active {
                    Color::Yellow
                } else {
                    Color::White
                }),
            ),
        ]),
        Line::from(""),
    ]);
    body.lines.push(if em.status_is_error {
        Line::from(vec![
            Span::raw("status: "),
            Span::styled(em.status.clone(), Style::default().fg(Color::Red)),
        ])
    } else {
        Line::from(em.status.clone())
    });
    body.lines.push(Line::from(""));
    body.lines.push(Line::from(
        "Enter = apply   Esc = cancel   Tab/Up/Down = field   Left/Right/Home/End edit",
    ));
    f.render_widget(
        Paragraph::new(body).block(Block::default().borders(Borders::ALL).title(title)),
        area,
    );
}
