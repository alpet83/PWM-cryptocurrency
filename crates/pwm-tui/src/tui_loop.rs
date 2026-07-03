//! Ratatui event loop: keyboard routing, modals, send flow, redraw cadence.

use crate::models::PendingConservationRow;
use crate::{
    account_id_to_human, append_addr_book, append_tx, base_url, burn_replay_guard_status,
    centered_rect, choose_identity, debug_json, f5_build_burn_form, f5_burn_hint_text,
    f6_build_send_form, f7_build_stake_form, format_acct_cell, format_amount_compact,
    format_balance_cell, format_hms_utc, format_init_cell, format_marks_compact,
    format_policy_bits, handle_submit_done_history, http_client, identity_f3_action_label,
    identity_lock_status_suffix, inter_shard_status_short, is_cross_domain_route,
    make_journal_filename, mark_pct_hint, masked_with_caret, merge_rpc_health, now_unix_secs,
    owner_and_receivers, pad_input_field, parse_hex_account_id, poll_snapshot,
    preflight_sel_init_auto, preflight_xfer_dst, push_op_history, pwm_pct_hint, read_journal,
    receiver_table_len, selected_row_for_panel, send_replay_guard_status, start_rpc_worker,
    status_footer_line, submit_stake, submit_unstake, validate_burn_form,
    validate_encrypt_passphrase_inputs, validate_send_form, validate_stake_form, value_with_caret,
    wallet_apply_auto_lock, wallet_dir, wallet_lock_now, wallet_rekey, wallet_unlock,
    wallet_unlock_secs_clamped, AcctRow, Args, BookPromptModal, BurnField, BurnForm, DebugCache,
    EncryptField, EncryptModal, IdentitySource, JournalEntry, OpStatus, OperationHistoryEntry,
    Panel, RpcEvent, RpcTask, SendField, SendForm, StakeField, StakeForm, StakeMode, Ui,
    UnlockModal, DEBUG_FETCH_INTERVAL, DETAIL_CHUNK_ROWS, FALLBACK_MODE_WARNING,
    FALLBACK_WARN_CHUNK_ROWS, MODAL_AMOUNT_INPUT_WIDTH, UNKNOWN_INIT_NONCE_SENTINEL,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pwm_core::display::PWM_RAW_SCALE;
use pwm_core::genesis::DEF_BLOCKS_PER_HOUR;
use pwm_core::types::conservation_flag;
use pwm_core::MARKS_CAP;
use ratatui::{
    prelude::{
        Color, Constraint, CrosstermBackend, Direction, Layout, Line, Modifier, Rect, Span, Style,
        Terminal, Text,
    },
    style::Stylize,
    widgets::{
        Block, Borders, Cell, Clear, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState, Wrap,
    },
    Frame,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::stdout;
use std::rc::Rc;
use std::sync::mpsc::TryRecvError;
use std::time::{Duration, Instant};

const F5_BURN_V5_STATUS: &str =
    "V5 marks: stake PWM with S, wait for blocks, then burn materialized marks with F5.";

struct TerminalGuard;

type AccountKey = [u8; 32];

struct PendingJournal {
    file_name: String,
    entry: JournalEntry,
    account_id: AccountKey,
    track_nonce: bool,
}

struct HistoryRow {
    ts: u64,
    time: String,
    kind: String,
    to: String,
    amount: String,
    status: String,
    info: String,
}

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
    let journal_dir = wallet_dir(&args);
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;
    // Drop runs both on normal return and panic unwind, restoring terminal state.
    let _term_guard = TerminalGuard;
    let inherit_host_colors = inherit_host_colors_enabled();

    let mut ui = Ui::default();
    let mut owner_state = TableState::default();
    owner_state.select(Some(0));
    let mut recv_state = TableState::default();
    recv_state.select(Some(0));
    let mut owner_scroll = ScrollbarState::default();
    let mut recv_scroll = ScrollbarState::default();
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
    let mut journal_cache: Vec<JournalEntry> = Vec::new();
    let mut pending_journal: HashMap<u64, PendingJournal> = HashMap::new();
    let mut pending_nonces: HashMap<AccountKey, BTreeSet<u64>> = HashMap::new();
    let mut pending_files: HashMap<AccountKey, String> = HashMap::new();
    let mut last = Instant::now() - Duration::from_secs(10);
    let dbg = debug_json();
    ui.identity_note = identity_note.clone();
    let mut debug_cache = DebugCache::new();
    let mut send_req_id: u64 = 0;
    let mut inflight_send_req_id: Option<u64> = None;
    let mut inflight_burn_req_id: Option<u64> = None;
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
                    confirm_journal_nonces(
                        journal_dir.as_deref(),
                        &ui.rows,
                        &mut pending_nonces,
                        &mut pending_files,
                    );
                    ui.err = snapshot.err;
                    ui.rpc_health = snapshot.rpc_health;
                    ui.rpc_shard_label = snapshot.rpc_shard_label;
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
                    if let Some(mut pending) = pending_journal.remove(&req_id) {
                        if result.is_ok() {
                            if !pending.track_nonce {
                                pending.entry.status = "ok".to_string();
                            }
                            if let Some(dir) = journal_dir.as_deref() {
                                write_journal(dir, &pending.file_name, &pending.entry);
                            }
                            if pending.track_nonce
                                && pending.entry.nonce != UNKNOWN_INIT_NONCE_SENTINEL
                            {
                                pending_nonces
                                    .entry(pending.account_id)
                                    .or_default()
                                    .insert(pending.entry.nonce);
                                pending_files.insert(pending.account_id, pending.file_name);
                            }
                        }
                    }
                    if inflight_burn_req_id == Some(req_id) {
                        inflight_burn_req_id = None;
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
                    if result.is_ok() {
                        let _ = rpc_tx.send(RpcTask::Poll);
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
        let owner_len = owner_rows.len();
        let recv_len = receiver_table_len(&receiver_rows);
        clamp_state(&mut owner_state, owner_len);
        clamp_state(&mut recv_state, recv_len);
        let owner_sel = state_sel(&owner_state);
        let recv_sel = state_sel(&recv_state);

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
                                            if journal_dir.is_some() {
                                                let file_name =
                                                    make_journal_filename(&hex::encode(from));
                                                let nonce = owner_rows
                                                    .iter()
                                                    .find(|row| row.id == from)
                                                    .map(|row| row.nonce)
                                                    .unwrap_or(0);
                                                let to = beneficiary
                                                    .as_ref()
                                                    .map(account_id_to_human)
                                                    .unwrap_or_default();
                                                let entry = JournalEntry::pending(
                                                    "burn",
                                                    to,
                                                    format_amount_compact(u128::from(mark_amount)),
                                                    format_amount_compact(0),
                                                    nonce,
                                                );
                                                pending_journal.insert(
                                                    send_req_id,
                                                    PendingJournal {
                                                        file_name,
                                                        entry,
                                                        account_id: from,
                                                        track_nonce: false,
                                                    },
                                                );
                                            }
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
                                            let nonce = owner_rows
                                                .iter()
                                                .find(|row| row.id == from)
                                                .map(|row| row.nonce)
                                                .unwrap_or(UNKNOWN_INIT_NONCE_SENTINEL);
                                            push_op_history(
                                                &mut op_history,
                                                OperationHistoryEntry {
                                                    req_id: send_req_id,
                                                    created_unix_secs: now_unix_secs(),
                                                    from_human: account_id_to_human(&from),
                                                    to_human: account_id_to_human(&to),
                                                    amount_units: amount,
                                                    fee_units: fee,
                                                    nonce,
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
                                            if journal_dir.is_some() {
                                                let file_name =
                                                    make_journal_filename(&hex::encode(from));
                                                let entry = JournalEntry::pending(
                                                    "send",
                                                    account_id_to_human(&to),
                                                    format_amount_compact(amount),
                                                    format_amount_compact(fee),
                                                    nonce,
                                                );
                                                pending_journal.insert(
                                                    send_req_id,
                                                    PendingJournal {
                                                        file_name,
                                                        entry,
                                                        account_id: from,
                                                        track_nonce: !conservation_flag(&from),
                                                    },
                                                );
                                            }
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
                                        if let Some(msg) = f5_burn_hint_text(
                                            owner.staked,
                                            owner.marks,
                                            owner.effective_marks,
                                        ) {
                                            info_modal = Some(msg.into());
                                            continue;
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
                                                    form.marks_available = fresh_owner
                                                        .effective_marks
                                                        .unwrap_or(fresh_owner.marks);
                                                }
                                                form.status = f5_burn_status(auto_init_msg);
                                                form.status_is_error = false;
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
                            journal_cache = load_journal_cache(
                                journal_dir.as_deref(),
                                selected_row_for_panel(
                                    active,
                                    &owner_rows,
                                    owner_sel,
                                    &receiver_rows,
                                    recv_sel,
                                ),
                            );
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
                            Panel::Owner => state_move_down(&mut owner_state, owner_len),
                            Panel::Receivers => state_move_down(&mut recv_state, recv_len),
                        },
                        KeyCode::Up => match active {
                            Panel::Owner => state_move_up(&mut owner_state),
                            Panel::Receivers => state_move_up(&mut recv_state),
                        },
                        _ => {}
                    }
                }
            }
        }

        clamp_state(&mut owner_state, owner_len);
        clamp_state(&mut recv_state, recv_len);
        let owner_sel = state_sel(&owner_state);
        let recv_sel = state_sel(&recv_state);

        let selected_row =
            selected_row_for_panel(active, &owner_rows, owner_sel, &receiver_rows, recv_sel);
        if let Some(r) = selected_row {
            let marks_txt = detail_marks_txt(r);
            let rescue_txt = r
                .rescue_address
                .as_ref()
                .map(account_id_to_human)
                .unwrap_or_else(|| "-".to_string());
            let pending_inline = conservation_pending_inline(r);
            let mut detail_line = format!(
                "{}
PWM: {}
Staked: {}
Marks: {}
Policy active: {}
Policy dormant: {}
Finalized: {}
Rescue: {}
Owner: {}/{}/{}",
                detail_head_line(r, &pending_inline),
                format_balance_cell(r),
                r.staked,
                marks_txt,
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
            if let Some(left) = marks_hour_left(r, ui.head_height) {
                detail_line.push_str(&format!("\nAccrual: ~{left} blocks until next mark hour"));
            }
            if let Some(pending_txt) = conservation_pending_txt(r) {
                detail_line.push_str(&format!("\n{pending_txt}"));
            }
            ui.detail_line = detail_line;
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
            let rows = PanelRows {
                owner: &owner_rows,
                receiver: &receiver_rows,
            };
            let mut states = PanelStates {
                owner_table: &mut owner_state,
                owner_scroll: &mut owner_scroll,
                recv_table: &mut recv_state,
                recv_scroll: &mut recv_scroll,
            };
            let ctx = MainCtx {
                active,
                identity: &identity,
            };
            render_main_panels(f, chunks[0], rows, &mut states, ctx);

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
                let pending = selected_row
                    .map(|row| row.pending_conservation.as_slice())
                    .unwrap_or(&[]);
                render_history_overlay(
                    f,
                    f.size(),
                    &op_history,
                    &journal_cache,
                    pending,
                    ui.head_height,
                );
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

struct PanelRows<'a> {
    owner: &'a [AcctRow],
    receiver: &'a [AcctRow],
}

struct PanelStates<'a> {
    owner_table: &'a mut TableState,
    owner_scroll: &'a mut ScrollbarState,
    recv_table: &'a mut TableState,
    recv_scroll: &'a mut ScrollbarState,
}

struct MainCtx<'a> {
    active: Panel,
    identity: &'a IdentitySource,
}

struct PanelCtx<'a> {
    active: Panel,
    identity: &'a IdentitySource,
    header: &'a Row<'static>,
}

struct OwnerState<'a> {
    table: &'a mut TableState,
    scroll: &'a mut ScrollbarState,
}

struct RecvState<'a> {
    table: &'a mut TableState,
    scroll: &'a mut ScrollbarState,
}

fn state_sel(state: &TableState) -> usize {
    state.selected().unwrap_or(0)
}

fn clamp_state(state: &mut TableState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let idx = state_sel(state).min(len - 1);
    state.select(Some(idx));
}

fn state_move_down(state: &mut TableState, len: usize) {
    if len == 0 {
        state.select(None);
        return;
    }
    let idx = state_sel(state).saturating_add(1).min(len - 1);
    state.select(Some(idx));
}

fn state_move_up(state: &mut TableState) {
    state.select(Some(state_sel(state).saturating_sub(1)));
}

fn body_rows(area: Rect) -> usize {
    area.height.saturating_sub(1) as usize
}

fn render_scrollbar(
    f: &mut Frame,
    area: Rect,
    rows_len: usize,
    state: &TableState,
    scroll: &mut ScrollbarState,
) {
    if rows_len <= body_rows(area) {
        return;
    }
    *scroll = ScrollbarState::new(rows_len).position(state_sel(state));
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight),
        area,
        scroll,
    );
}

/// Renders owner/receiver top panels with shared header row.
fn render_main_panels(
    f: &mut Frame,
    area: Rect,
    rows: PanelRows,
    states: &mut PanelStates,
    ctx: MainCtx,
) {
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    let header = panel_head_row();
    render_owner_panel(
        f,
        panels[0],
        rows.owner,
        OwnerState {
            table: states.owner_table,
            scroll: states.owner_scroll,
        },
        PanelCtx {
            active: ctx.active,
            identity: ctx.identity,
            header: &header,
        },
    );
    render_recv_panel(
        f,
        panels[1],
        rows.receiver,
        RecvState {
            table: states.recv_table,
            scroll: states.recv_scroll,
        },
        PanelCtx {
            active: ctx.active,
            identity: ctx.identity,
            header: &header,
        },
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

fn marks_cell(row: &AcctRow) -> String {
    let eff = row.effective_marks.unwrap_or(row.marks);
    format_marks_compact(eff)
}

fn detail_marks_txt(row: &AcctRow) -> String {
    let eff = row.effective_marks.unwrap_or(row.marks);
    format!(
        "{} (stored: {}, last_block: {})",
        eff, row.marks, row.marks_last_block
    )
}

fn detail_head_line(row: &AcctRow, pending_inline: &str) -> String {
    let nonce = if row.nonce == UNKNOWN_INIT_NONCE_SENTINEL {
        "?".to_string()
    } else {
        row.nonce.to_string()
    };
    let acct = format_acct_cell(row);
    if pending_inline.is_empty() {
        format!(
            "sel: {acct} | init={} | nonce={nonce}",
            format_init_cell(row)
        )
    } else {
        let pending = pending_inline.trim_start_matches(" | ");
        format!(
            "sel: {acct} | init={} | nonce={nonce} | {pending}",
            format_init_cell(row)
        )
    }
}

fn marks_hour_left(row: &AcctRow, head_h: Option<u64>) -> Option<u64> {
    let eff = row.effective_marks.unwrap_or(row.marks);
    if row.staked < PWM_RAW_SCALE || eff != row.marks {
        return None;
    }
    let head = head_h?;
    let bph = u64::from(DEF_BLOCKS_PER_HOUR);
    if bph == 0 {
        return None;
    }
    let since = head.saturating_sub(row.marks_last_block);
    let rem = bph - (since % bph);
    Some(rem)
}

fn confirm_journal_nonces(
    wallet_dir: Option<&std::path::Path>,
    rows: &[AcctRow],
    pending_nonces: &mut HashMap<AccountKey, BTreeSet<u64>>,
    pending_files: &mut HashMap<AccountKey, String>,
) {
    let Some(dir) = wallet_dir else {
        return;
    };
    let account_keys = pending_nonces.keys().copied().collect::<Vec<_>>();
    for account_id in account_keys {
        let Some(row) = rows.iter().find(|row| row.id == account_id) else {
            continue;
        };
        if let Some(nonces) = pending_nonces.get_mut(&account_id) {
            nonces.remove(&UNKNOWN_INIT_NONCE_SENTINEL);
        }
        let confirmed = pending_nonces
            .get(&account_id)
            .map(|nonces| {
                nonces
                    .iter()
                    .copied()
                    .filter(|nonce| *nonce != UNKNOWN_INIT_NONCE_SENTINEL && *nonce < row.nonce)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if confirmed.is_empty() {
            continue;
        }
        let Some(file_name) = pending_files.get(&account_id).cloned() else {
            continue;
        };
        for nonce in confirmed {
            let entry = JournalEntry::status_update(nonce);
            if write_journal(dir, &file_name, &entry) {
                if let Some(nonces) = pending_nonces.get_mut(&account_id) {
                    nonces.remove(&nonce);
                }
            }
        }
        let empty = pending_nonces
            .get(&account_id)
            .map(BTreeSet::is_empty)
            .unwrap_or(false);
        if empty {
            pending_nonces.remove(&account_id);
            pending_files.remove(&account_id);
        }
    }
}

fn write_journal(dir: &std::path::Path, file_name: &str, entry: &JournalEntry) -> bool {
    match append_tx(dir, file_name, entry) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[journal] write error: {e}");
            false
        }
    }
}

fn load_journal_cache(
    wallet_dir: Option<&std::path::Path>,
    selected: Option<&AcctRow>,
) -> Vec<JournalEntry> {
    let (Some(dir), Some(row)) = (wallet_dir, selected) else {
        return Vec::new();
    };
    read_journal(dir, &make_journal_filename(&row.id_hex))
}

fn conservation_pending_txt(row: &AcctRow) -> Option<String> {
    let next_h = row
        .pending_conservation
        .iter()
        .map(|pending| pending.execute_at_height)
        .min()?;
    Some(format!(
        "conservation pending: {} transfer(s), next execute at height {next_h}",
        row.pending_conservation.len()
    ))
}

fn conservation_pending_inline(row: &AcctRow) -> String {
    let pending_sum: u128 = row
        .pending_conservation
        .iter()
        .map(|pending| pending.amount_pwm)
        .sum();
    if pending_sum == 0 {
        String::new()
    } else {
        format!(" | pending {}", format_amount_compact(pending_sum))
    }
}

fn short_recipient(recipient: &str) -> String {
    const PREFIX_LEN: usize = 28;
    if recipient.len() <= PREFIX_LEN {
        recipient.to_string()
    } else {
        format!("{}...", &recipient[..PREFIX_LEN])
    }
}

fn marks_cell_style(row: &AcctRow) -> Style {
    if row.effective_marks.unwrap_or(row.marks) == MARKS_CAP {
        Style::default().fg(Color::Red)
    } else {
        Style::default()
    }
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
    state: OwnerState,
    ctx: PanelCtx,
) {
    let owner_focus_style = panel_focus_style(ctx.active, Panel::Owner);
    let owner_rows: Vec<Row> = owner_rows
        .iter()
        .map(|r| {
            Row::new(vec![
                Cell::from(format_acct_cell(r)),
                Cell::from(format_balance_cell(r)),
                Cell::from(marks_cell(r)).style(marks_cell_style(r)),
            ])
        })
        .collect();
    let owner_block = Block::default()
        .borders(Borders::ALL)
        .title("Owner")
        .border_style(owner_focus_style)
        .title_style(owner_focus_style);
    let inner = owner_block.inner(area);
    f.render_widget(owner_block, area);
    let rows_len = owner_rows.len();
    let owner_table = Table::new(
        owner_rows,
        [
            Constraint::Min(36),
            Constraint::Length(18),
            Constraint::Length(24),
        ],
    )
    .header(ctx.header.clone())
    .highlight_style(Style::default().reversed());
    f.render_stateful_widget(owner_table, inner, state.table);
    render_scrollbar(f, inner, rows_len, state.table, state.scroll);
}

/// Renders receivers panel with "New Recipient" top row.
fn render_recv_panel(
    f: &mut Frame,
    area: Rect,
    receiver_rows: &[AcctRow],
    state: RecvState,
    ctx: PanelCtx,
) {
    let recv_focus_style = panel_focus_style(ctx.active, Panel::Receivers);
    let recv_rows: Vec<Row> = receiver_rows
        .iter()
        .map(|r| {
            Row::new(vec![
                Cell::from(format_acct_cell(r)),
                Cell::from(format_balance_cell(r)),
                Cell::from(marks_cell(r)).style(marks_cell_style(r)),
            ])
        })
        .collect();
    let mut recv_rows_all = Vec::with_capacity(recv_rows.len() + 1);
    recv_rows_all.push(Row::new(vec![
        Cell::from("New Recipient"),
        Cell::from("-"),
        Cell::from("-"),
    ]));
    recv_rows_all.extend(recv_rows);
    let rows_len = recv_rows_all.len();
    let recv_title = match ctx.identity {
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
            Constraint::Length(18),
            Constraint::Length(24),
        ],
    )
    .header(ctx.header.clone())
    .highlight_style(Style::default().reversed());
    f.render_stateful_widget(recv_table, inner, state.table);
    render_scrollbar(f, inner, rows_len, state.table, state.scroll);
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

fn f5_burn_status(auto_init_msg: Option<String>) -> String {
    if let Some(init_msg) = auto_init_msg {
        format!("{init_msg}; {F5_BURN_V5_STATUS}")
    } else {
        F5_BURN_V5_STATUS.to_string()
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
fn render_history_overlay(
    f: &mut Frame,
    screen: Rect,
    op_history: &[OperationHistoryEntry],
    journal_cache: &[JournalEntry],
    pending: &[PendingConservationRow],
    head_height: Option<u64>,
) {
    let area = centered_rect(86, 62, screen);
    f.render_widget(Clear, area);
    let hist_rows = history_rows(op_history, journal_cache, pending, head_height);
    if hist_rows.is_empty() {
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
            Cell::from("Kind"),
            Cell::from("To"),
            Cell::from("Amount"),
            Cell::from("Status"),
            Cell::from("Info"),
        ])
        .style(Style::default().add_modifier(Modifier::BOLD));
        let rows = hist_rows.into_iter().map(|row| {
            Row::new(vec![
                Cell::from(row.time),
                Cell::from(row.kind),
                Cell::from(row.to),
                Cell::from(row.amount),
                Cell::from(row.status.clone()).style(history_status_style(&row.status)),
                Cell::from(row.info),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Length(10),
                Constraint::Length(13),
                Constraint::Min(22),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Min(16),
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

fn history_rows(
    op_history: &[OperationHistoryEntry],
    journal_cache: &[JournalEntry],
    pending: &[PendingConservationRow],
    head_height: Option<u64>,
) -> Vec<HistoryRow> {
    let journal_nonces = journal_cache
        .iter()
        .filter_map(journal_nonce)
        .collect::<HashSet<_>>();
    let mut rows = pending
        .iter()
        .map(|item| pending_hist_row(item, head_height))
        .chain(journal_cache.iter().map(journal_hist_row))
        .chain(
            op_history
                .iter()
                .filter(|item| !journal_nonces.contains(&item.nonce))
                .map(op_hist_row),
        )
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| std::cmp::Reverse(row.ts));
    rows
}

fn journal_nonce(entry: &JournalEntry) -> Option<u64> {
    if entry.nonce == UNKNOWN_INIT_NONCE_SENTINEL {
        None
    } else {
        Some(entry.nonce)
    }
}

const BLOCKS_PER_HOUR: u64 = 3600;

fn pending_recipient(item: &PendingConservationRow) -> String {
    parse_hex_account_id(&item.recipient)
        .map(|id| account_id_to_human(&id))
        .unwrap_or_else(|| item.recipient.clone())
}

fn pending_hist_row(item: &PendingConservationRow, head_height: Option<u64>) -> HistoryRow {
    let blocks_left = item
        .execute_at_height
        .saturating_sub(head_height.unwrap_or(0));
    let hours = blocks_left / BLOCKS_PER_HOUR;
    HistoryRow {
        ts: u64::MAX,
        time: "-".to_string(),
        kind: "conservation".to_string(),
        to: short_recipient(&pending_recipient(item)),
        amount: format_amount_compact(item.amount_pwm),
        status: "pending".to_string(),
        info: format!("~{}h (blk {})", hours, item.execute_at_height),
    }
}

fn journal_hist_row(item: &JournalEntry) -> HistoryRow {
    HistoryRow {
        ts: item.ts,
        time: format_hms_utc(item.ts),
        kind: item.kind.clone(),
        to: short_recipient(&item.to),
        amount: item.amount_pwm.clone(),
        status: item.status.clone(),
        info: String::new(),
    }
}

fn op_hist_row(item: &OperationHistoryEntry) -> HistoryRow {
    HistoryRow {
        ts: item.created_unix_secs,
        time: format_hms_utc(item.created_unix_secs),
        kind: "session_send".to_string(),
        to: item.to_human.clone(),
        amount: format!("{} (+fee {})", item.amount_units, item.fee_units),
        status: item.status.as_str().to_string(),
        info: String::new(),
    }
}

fn history_status_style(status: &str) -> Style {
    match status {
        "pending" => Style::default().fg(Color::Yellow),
        "ok" => Style::default().fg(Color::Green),
        "error" => Style::default().fg(Color::Red),
        _ => Style::default(),
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
        let shown = if name == "amount" {
            pad_input_field(&shown, MODAL_AMOUNT_INPUT_WIDTH)
        } else {
            shown
        };
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
        if name == "amount" {
            if let Some(hint) = pwm_pct_hint(value, form.balance_units) {
                text.lines.push(Line::from(vec![
                    Span::raw("           "),
                    Span::styled(
                        hint.label,
                        if hint.over_limit {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    ),
                ]));
            }
        }
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
        Line::from("V5 marks: stake PWM with S, wait for blocks, then burn materialized marks with F5."),
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
        let shown = if name == "marks" {
            pad_input_field(&shown, MODAL_AMOUNT_INPUT_WIDTH)
        } else {
            shown
        };
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
        if name == "marks" {
            if let Some(hint) = mark_pct_hint(value, form.marks_available) {
                text.lines.push(Line::from(vec![
                    Span::raw("           "),
                    Span::styled(
                        hint.label,
                        if hint.over_limit {
                            Style::default().fg(Color::Red)
                        } else {
                            Style::default().fg(Color::Gray)
                        },
                    ),
                ]));
            }
        }
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
        conservation_pending_inline, conservation_pending_txt, detail_head_line, detail_marks_txt,
        f5_burn_status, marks_cell, marks_cell_style, marks_hour_left, panel_focus_style, Panel,
        F5_BURN_V5_STATUS,
    };
    use crate::models::PendingConservationRow;
    use crate::AcctRow;
    use pwm_core::MARKS_CAP;
    use ratatui::style::{Color, Modifier};

    fn mk_row(id: [u8; 32], marks: u32) -> AcctRow {
        AcctRow {
            id,
            id_hex: hex::encode(id),
            balance_pwm: 0,
            initialized: true,
            nonce: 0,
            marks,
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

    #[test]
    fn marks_cell_zero_stake() {
        let row = mk_row([1u8; 32], 0);
        assert_eq!(marks_cell(&row), "0");
    }

    #[test]
    fn marks_cell_sat_red() {
        let mut row = mk_row([2u8; 32], 10);
        row.effective_marks = Some(MARKS_CAP);

        assert_eq!(marks_cell(&row), "4.29B");
        assert_eq!(marks_cell_style(&row).fg, Some(Color::Red));
    }

    #[test]
    fn marks_cell_plain_style() {
        let row = mk_row([3u8; 32], 999);

        assert_eq!(marks_cell(&row), "999");
        assert_eq!(marks_cell_style(&row).fg, None);
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

    #[test]
    fn f5_retired_claim_no_submit() {
        let status = f5_burn_status(None);

        assert_eq!(status, F5_BURN_V5_STATUS);
        assert!(!status.contains("Claim submitted"));
        assert!(!status.contains("waiting for confirmation"));
    }

    #[test]
    fn detail_marks_uses_effective() {
        let mut row = mk_row([4u8; 32], 15);
        row.effective_marks = Some(22);

        let txt = detail_marks_txt(&row);
        assert!(txt.contains("22"));
        assert!(txt.contains("stored: 15"));
    }

    #[test]
    fn marks_hour_hint_gate() {
        let mut row = mk_row([5u8; 32], 10);
        row.staked = pwm_core::display::PWM_RAW_SCALE;
        row.marks_last_block = 100;
        row.effective_marks = Some(10);

        assert_eq!(marks_hour_left(&row, Some(100)), Some(3600));
        assert_eq!(marks_hour_left(&row, Some(150)), Some(3550));

        row.staked = pwm_core::display::PWM_RAW_SCALE - 1;
        assert_eq!(marks_hour_left(&row, Some(150)), None);
    }

    #[test]
    fn conservation_pending_line() {
        let mut row = mk_row([6u8; 32], 10);
        row.pending_conservation.push(PendingConservationRow {
            recipient: hex::encode([7u8; 32]),
            amount_pwm: 100,
            nonce: 1,
            enqueue_height: 2,
            execute_at_height: 9,
        });
        row.pending_conservation.push(PendingConservationRow {
            recipient: hex::encode([8u8; 32]),
            amount_pwm: 200,
            nonce: 2,
            enqueue_height: 3,
            execute_at_height: 7,
        });

        let txt = conservation_pending_txt(&row).expect("pending line");
        assert!(txt.contains("2 transfer(s)"));
        assert!(txt.contains("height 7"));
        let inline = conservation_pending_inline(&row);
        assert_eq!(inline, " | pending 300");
        let head = detail_head_line(&row, &inline);
        assert!(head.starts_with("sel:"));
        assert!(head.ends_with(" | pending 300"));
    }

    #[test]
    fn detail_head_plain_sel() {
        let row = mk_row([8u8; 32], 10);
        let head = detail_head_line(&row, "");

        assert!(head.starts_with("sel:"));
        assert!(head.contains(" | init="));
        assert!(!head.starts_with("pending"));
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
