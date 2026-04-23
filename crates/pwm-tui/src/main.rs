//! Network account table (public-friendly). Optional debug JSON via PWM_TUI_DEBUG=1.

use base64::Engine;
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use pwm_core::hd::{brute_cluster_address, domain_of_account_id};
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::{
    account_id_to_human, append_wallet_yaml_address_book, load_wallet_read_header,
    normalize_wallet_header, open_wallet_secret_ciphertext, parse_account_id,
    parse_account_id_for_user_input, seal_wallet_secret_plaintext, AccountId, WalletReadHeader,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};
use serde::Serialize;
use serde_json::Value;
use serde_yaml::{Mapping, Value as YamlValue};
use std::io::{stdout, Write};
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Default auto-lock after unlock for encrypted wallets (seconds).
const DEFAULT_WALLET_UNLOCK_SECS: u64 = 300;
/// Upper bound for `PWM_TUI_WALLET_UNLOCK_SECS` / `--wallet-unlock-secs` (1 week).
const WALLET_UNLOCK_SECS_MAX: u64 = 604_800;

#[derive(Parser, Debug)]
#[command(name = "pwm-tui", about = "PWM terminal UI")]
struct Args {
    #[arg(long, env = "PWM_TUI_WALLET")]
    wallet: Option<PathBuf>,
    #[arg(long, env = "PWM_TUI_WALLET_PASSPHRASE")]
    wallet_passphrase: Option<String>,
    /// Auto-lock encrypted wallet after this many seconds (min 1, max 604800). Env: `PWM_TUI_WALLET_UNLOCK_SECS`.
    #[arg(long = "wallet-unlock-secs", env = "PWM_TUI_WALLET_UNLOCK_SECS", default_value_t = DEFAULT_WALLET_UNLOCK_SECS)]
    wallet_unlock_secs: u64,
}

fn base_url() -> String {
    std::env::var("PWM_RPC").unwrap_or_else(|_| "http://127.0.0.1:3030".into())
}

/// Upper bound so a typo like `PWM_TUI_RPC_TIMEOUT_MS=999999999` does not block tests for hours.
const RPC_TIMEOUT_MS_MAX: u64 = 120_000;
/// Send form decimal scale: 1 PWM = 1_000_000 base units.
const SEND_DECIMAL_SCALE: u128 = 1_000_000;
/// UI-side throttle for debug account JSON pulls.
const DEBUG_FETCH_INTERVAL: Duration = Duration::from_millis(800);
/// Keep a short local timeline in-memory (most recent first).
const OP_HISTORY_MAX_ITEMS: usize = 20;

fn wallet_unlock_secs_clamped(args: &Args) -> u64 {
    args.wallet_unlock_secs.clamp(1, WALLET_UNLOCK_SECS_MAX)
}

fn rpc_timeout() -> Duration {
    const DEFAULT_MS: u64 = 3000;
    std::env::var("PWM_TUI_RPC_TIMEOUT_MS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|&ms| ms > 0 && ms <= RPC_TIMEOUT_MS_MAX)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(DEFAULT_MS))
}

fn http_client() -> reqwest::blocking::Client {
    let t = rpc_timeout();
    reqwest::blocking::Client::builder()
        .connect_timeout(t)
        .timeout(t)
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsonFetchFailure {
    Timeout,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RpcHealth {
    Online,
    Timeout,
    Offline,
}

impl RpcHealth {
    fn severity(self) -> u8 {
        match self {
            RpcHealth::Online => 0,
            RpcHealth::Timeout => 1,
            RpcHealth::Offline => 2,
        }
    }
}

fn merge_rpc_health(lhs: RpcHealth, rhs: RpcHealth) -> RpcHealth {
    if rhs.severity() > lhs.severity() {
        rhs
    } else {
        lhs
    }
}

fn rpc_health_from_failure(f: JsonFetchFailure) -> RpcHealth {
    match f {
        JsonFetchFailure::Timeout => RpcHealth::Timeout,
        JsonFetchFailure::Other => RpcHealth::Offline,
    }
}

/// Max `tip=` payload length before middle-ellipsis (hex hashes are long).
const FOOTER_TIP_FULL_MAX: usize = 24;
const FOOTER_TIP_PREFIX_KEEP: usize = 8;
const FOOTER_TIP_SUFFIX_KEEP: usize = 8;

/// ASCII-only middle ellipsis for footer one-liners (chain tip hex, etc.).
fn ellipsis_middle_ascii(value: &str, keep_prefix: usize, keep_suffix: usize) -> String {
    let max_plain = keep_prefix + keep_suffix;
    if value.len() <= max_plain {
        return value.to_string();
    }
    if keep_prefix + 3 + keep_suffix >= value.len() {
        return value.to_string();
    }
    let mut out = String::new();
    out.push_str(&value[..keep_prefix]);
    out.push_str("...");
    out.push_str(&value[value.len() - keep_suffix..]);
    out
}

/// Shortens `height=… tip=<long>` style head strings for the status footer.
fn format_footer_head_line(head: &str) -> String {
    const SEP: &str = " tip=";
    let Some(pos) = head.find(SEP) else {
        return head.to_string();
    };
    let tip_start = pos + SEP.len();
    let tip_val = &head[tip_start..];
    if tip_val.len() <= FOOTER_TIP_FULL_MAX {
        return head.to_string();
    }
    let short = ellipsis_middle_ascii(tip_val, FOOTER_TIP_PREFIX_KEEP, FOOTER_TIP_SUFFIX_KEEP);
    format!("{}{SEP}{short}", &head[..pos])
}

fn rpc_bad_label(rpc_health: RpcHealth) -> Option<&'static str> {
    match rpc_health {
        RpcHealth::Online => None,
        RpcHealth::Timeout => Some("RPC timeout"),
        RpcHealth::Offline => Some("RPC offline"),
    }
}

/// Builds the bottom status `Line`: RPC health and poll errors first so they stay visible on narrow terminals.
fn status_footer_line(
    head: &str,
    err: &str,
    identity_note: &str,
    f3_action: &str,
    rpc_health: RpcHealth,
    dbg: bool,
    rpc_url: &str,
) -> Line<'static> {
    let head_shown = format_footer_head_line(head);
    let mut tail = format!(
        "{} | Tab switch panel | Arrows move active panel | H history | F3 {} | F4 encrypt | F5 TODO | F6 send | F10 quit | PWM_RPC={}",
        head_shown, f3_action, rpc_url
    );
    if dbg {
        tail.push_str(" | PWM_TUI_DEBUG=1");
    }
    if !identity_note.is_empty() {
        tail.push_str(" | ");
        tail.push_str(identity_note);
    }

    let bad = rpc_bad_label(rpc_health);
    let mut spans: Vec<Span<'static>> = Vec::new();
    if let Some(label) = bad {
        spans.push(Span::styled(label, Style::default().fg(Color::Red)));
    }
    if !err.is_empty() {
        if !spans.is_empty() {
            spans.push(Span::raw(" | "));
        }
        // Own the poll error text so the returned `Line` does not borrow caller locals.
        spans.push(Span::raw(err.to_string()));
    }
    if !spans.is_empty() {
        spans.push(Span::raw(" | "));
    }
    spans.push(Span::raw(tail));
    Line::from(spans)
}

fn debug_json() -> bool {
    matches!(
        std::env::var("PWM_TUI_DEBUG").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

fn fetch_json(client: &reqwest::blocking::Client, url: &str) -> Result<Value, JsonFetchFailure> {
    let r = client.get(url).send().map_err(|e| {
        if e.is_timeout() {
            JsonFetchFailure::Timeout
        } else {
            JsonFetchFailure::Other
        }
    })?;
    if !r.status().is_success() {
        return Err(JsonFetchFailure::Other);
    }
    r.json().map_err(|_| JsonFetchFailure::Other)
}

/// One row from `GET /v1/accounts`.
#[derive(Clone)]
struct AcctRow {
    id: AccountId,
    id_hex: String,
    balance_pwm: u128,
    initialized: bool,
    nonce: u64,
    /// From wallet `address_book` entry (optional).
    label: Option<String>,
}

fn parse_u128(v: &Value) -> u128 {
    match v {
        Value::String(s) => s.parse().unwrap_or(0),
        Value::Number(n) => n.as_u64().map(|x| x as u128).unwrap_or(0),
        _ => 0,
    }
}

fn parse_hex_account_id(hex: &str) -> Option<AccountId> {
    let bytes = hex::decode(hex).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Some(id)
}

#[derive(Clone)]
struct BookRecipient {
    id: AccountId,
    label: Option<String>,
}

#[derive(Clone)]
struct WalletIdentity {
    account_id: AccountId,
    account_id_human: String,
    domain: u16,
    derivation_index: u32,
    /// Present when signing is allowed (`plaintext_dev` always; `encrypted` after unlock).
    signing_key: Option<ed25519_dalek::SigningKey>,
    /// For `encrypted` wallets with an active unlock session.
    unlock_expires_at: Option<Instant>,
    /// True when YAML mode is `encrypted` (unlock/timer UX applies).
    wallet_is_encrypted: bool,
    wallet_path: PathBuf,
    /// When non-empty, right panel lists these (enriched from RPC).
    address_book: Vec<BookRecipient>,
    /// Placeholder for future "encrypt upgraded plaintext wallet" UX hook.
    encryption_prompt_hint: Option<String>,
    /// Legacy pretty entries skipped by wallet loader.
    ignored_legacy_pretty_entries: usize,
    /// Decrypted wallet secret JSON (same bytes as inside the AEAD blob). Cleared on auto-lock.
    /// Never logged. Used for F4 re-key without re-entering the old passphrase.
    secret_payload_plaintext: Option<Vec<u8>>,
}

impl WalletIdentity {
    fn has_recipient(&self, id: &AccountId) -> bool {
        self.address_book.iter().any(|b| b.id == *id)
    }
}

/// After a successful send: offer to append `to` to wallet YAML (same mechanism as `pwm wallet book-add`).
struct BookPromptModal {
    to_display: String,
    label_line: String,
    label_cursor: usize,
    status: String,
}

impl BookPromptModal {
    fn new(to_display: String) -> Self {
        Self {
            to_display,
            label_line: String::new(),
            label_cursor: 0,
            status: "Optional label for address book (Enter=save, Esc=skip)".into(),
        }
    }

    fn clamp_cursor(&mut self) {
        self.label_cursor = self.label_cursor.min(self.label_line.len());
    }

    fn move_left(&mut self) {
        self.label_cursor = prev_char_boundary(&self.label_line, self.label_cursor);
    }

    fn move_right(&mut self) {
        self.label_cursor = next_char_boundary(&self.label_line, self.label_cursor);
    }

    fn move_home(&mut self) {
        self.label_cursor = 0;
    }

    fn move_end(&mut self) {
        self.label_cursor = self.label_line.len();
    }

    fn insert_char(&mut self, c: char) {
        let i = self.label_cursor.min(self.label_line.len());
        self.label_line.insert(i, c);
        self.label_cursor = i + c.len_utf8();
    }

    fn backspace(&mut self) {
        if self.label_cursor == 0 {
            return;
        }
        let from = prev_char_boundary(&self.label_line, self.label_cursor);
        self.label_line.drain(from..self.label_cursor);
        self.label_cursor = from;
    }

    fn delete(&mut self) {
        if self.label_cursor >= self.label_line.len() {
            return;
        }
        let to = next_char_boundary(&self.label_line, self.label_cursor);
        self.label_line.drain(self.label_cursor..to);
    }
}

/// F3 unlock dialog for encrypted wallets (passphrase never logged).
struct UnlockModal {
    passphrase: String,
    pass_cursor: usize,
    status: String,
    status_is_error: bool,
}

impl UnlockModal {
    fn new() -> Self {
        Self {
            passphrase: String::new(),
            pass_cursor: 0,
            status: "Enter passphrase (Enter=unlock, Esc=cancel)".into(),
            status_is_error: false,
        }
    }

    fn clamp_cursor(&mut self) {
        self.pass_cursor = self.pass_cursor.min(self.passphrase.len());
    }

    fn move_left(&mut self) {
        self.pass_cursor = prev_char_boundary(&self.passphrase, self.pass_cursor);
    }

    fn move_right(&mut self) {
        self.pass_cursor = next_char_boundary(&self.passphrase, self.pass_cursor);
    }

    fn move_home(&mut self) {
        self.pass_cursor = 0;
    }

    fn move_end(&mut self) {
        self.pass_cursor = self.passphrase.len();
    }

    fn insert_char(&mut self, c: char) {
        let i = self.pass_cursor.min(self.passphrase.len());
        self.passphrase.insert(i, c);
        self.pass_cursor = i + c.len_utf8();
    }

    fn backspace(&mut self) {
        if self.pass_cursor == 0 {
            return;
        }
        let from = prev_char_boundary(&self.passphrase, self.pass_cursor);
        self.passphrase.drain(from..self.pass_cursor);
        self.pass_cursor = from;
    }

    fn delete(&mut self) {
        if self.pass_cursor >= self.passphrase.len() {
            return;
        }
        let to = next_char_boundary(&self.passphrase, self.pass_cursor);
        self.passphrase.drain(self.pass_cursor..to);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EncryptField {
    Passphrase,
    Confirm,
}

/// F4 encrypt / re-key: new passphrase + confirm (never logged).
struct EncryptModal {
    active: EncryptField,
    passphrase: String,
    confirm: String,
    pass_cursor: usize,
    confirm_cursor: usize,
    status: String,
    status_is_error: bool,
    /// When true, title explains re-key for an already-encrypted wallet.
    is_rekey: bool,
}

impl EncryptModal {
    fn new(is_rekey: bool) -> Self {
        Self {
            active: EncryptField::Passphrase,
            passphrase: String::new(),
            confirm: String::new(),
            pass_cursor: 0,
            confirm_cursor: 0,
            status: if is_rekey {
                "New passphrase + confirm (Enter=apply, Esc=cancel)".into()
            } else {
                "Set passphrase to encrypt wallet (Enter=apply, Esc=cancel)".into()
            },
            status_is_error: false,
            is_rekey,
        }
    }

    fn clamp_cursors(&mut self) {
        self.pass_cursor = self.pass_cursor.min(self.passphrase.len());
        self.confirm_cursor = self.confirm_cursor.min(self.confirm.len());
    }

    fn next_field(&mut self) {
        self.active = match self.active {
            EncryptField::Passphrase => EncryptField::Confirm,
            EncryptField::Confirm => EncryptField::Passphrase,
        };
    }

    fn prev_field(&mut self) {
        self.next_field();
    }

    fn move_left(&mut self) {
        match self.active {
            EncryptField::Passphrase => {
                self.pass_cursor = prev_char_boundary(&self.passphrase, self.pass_cursor);
            }
            EncryptField::Confirm => {
                self.confirm_cursor = prev_char_boundary(&self.confirm, self.confirm_cursor);
            }
        }
    }

    fn move_right(&mut self) {
        match self.active {
            EncryptField::Passphrase => {
                self.pass_cursor = next_char_boundary(&self.passphrase, self.pass_cursor);
            }
            EncryptField::Confirm => {
                self.confirm_cursor = next_char_boundary(&self.confirm, self.confirm_cursor);
            }
        }
    }

    fn move_home(&mut self) {
        match self.active {
            EncryptField::Passphrase => self.pass_cursor = 0,
            EncryptField::Confirm => self.confirm_cursor = 0,
        }
    }

    fn move_end(&mut self) {
        match self.active {
            EncryptField::Passphrase => self.pass_cursor = self.passphrase.len(),
            EncryptField::Confirm => self.confirm_cursor = self.confirm.len(),
        }
    }

    fn insert_char(&mut self, c: char) {
        match self.active {
            EncryptField::Passphrase => {
                let i = self.pass_cursor.min(self.passphrase.len());
                self.passphrase.insert(i, c);
                self.pass_cursor = i + c.len_utf8();
            }
            EncryptField::Confirm => {
                let i = self.confirm_cursor.min(self.confirm.len());
                self.confirm.insert(i, c);
                self.confirm_cursor = i + c.len_utf8();
            }
        }
    }

    fn backspace(&mut self) {
        match self.active {
            EncryptField::Passphrase => {
                if self.pass_cursor == 0 {
                    return;
                }
                let from = prev_char_boundary(&self.passphrase, self.pass_cursor);
                self.passphrase.drain(from..self.pass_cursor);
                self.pass_cursor = from;
            }
            EncryptField::Confirm => {
                if self.confirm_cursor == 0 {
                    return;
                }
                let from = prev_char_boundary(&self.confirm, self.confirm_cursor);
                self.confirm.drain(from..self.confirm_cursor);
                self.confirm_cursor = from;
            }
        }
    }

    fn delete(&mut self) {
        match self.active {
            EncryptField::Passphrase => {
                if self.pass_cursor >= self.passphrase.len() {
                    return;
                }
                let to = next_char_boundary(&self.passphrase, self.pass_cursor);
                self.passphrase.drain(self.pass_cursor..to);
            }
            EncryptField::Confirm => {
                if self.confirm_cursor >= self.confirm.len() {
                    return;
                }
                let to = next_char_boundary(&self.confirm, self.confirm_cursor);
                self.confirm.drain(self.confirm_cursor..to);
            }
        }
    }
}

/// Masked passphrase line (byte cursor, same as other inline editors).
fn masked_with_caret(pass: &str, cursor: usize) -> String {
    let i = cursor.min(pass.len());
    let mut out = String::with_capacity(pass.len() + 1);
    out.push_str(&"*".repeat(i));
    out.push('|');
    out.push_str(&"*".repeat(pass.len().saturating_sub(i)));
    out
}

#[derive(Clone)]
enum IdentitySource {
    Wallet(WalletIdentity),
    SeedFallback,
}

/// Shown on a dedicated layout row when no `--wallet` / `PWM_TUI_WALLET`.
const FALLBACK_MODE_WARNING: &str =
    "FALLBACK MODE: wallet not provided, owner derived from seed/default path";

/// Fixed vertical slot for the bordered WARNING strip (fallback). Use **`Length`**, not `Min`, so
/// ratatui does not assign most free height to this chunk (which made the banner look half-screen).
const FALLBACK_WARN_CHUNK_ROWS: u16 = 5;
/// Bordered detail line (`selected: …`): top + one inner row + bottom.
const DETAIL_CHUNK_ROWS: u16 = 3;

fn parse_signing_key_hex(hex_key: &str) -> Result<ed25519_dalek::SigningKey, String> {
    let bytes = hex::decode(hex_key.trim()).map_err(|e| format!("bad signing key hex: {e}"))?;
    if bytes.len() != 32 {
        return Err("wallet signing key must be 32-byte hex".into());
    }
    let mut k = [0u8; 32];
    k.copy_from_slice(&bytes);
    Ok(ed25519_dalek::SigningKey::from_bytes(&k))
}

/// Decrypt encrypted wallet blob; returns signing key + raw JSON bytes for F4 re-key cache.
fn try_decrypt_wallet_secret_payload(
    wallet: &WalletReadHeader,
    passphrase: Option<&str>,
) -> Result<(ed25519_dalek::SigningKey, Vec<u8>), String> {
    let passphrase = passphrase.ok_or_else(|| {
        "encrypted wallet requires passphrase: use --wallet-passphrase or PWM_TUI_WALLET_PASSPHRASE".to_string()
    })?;
    let kdf = wallet
        .kdf
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing kdf".to_string())?;
    let iters = wallet
        .kdf_iters
        .ok_or_else(|| "encrypted wallet is missing kdf_iters".to_string())?;
    let enc = wallet
        .encrypted_payload_b64
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing encrypted_payload_b64".to_string())?;
    let salt_b64 = wallet
        .kdf_salt_b64
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing kdf_salt_b64".to_string())?;
    let nonce_b64 = wallet
        .aead_nonce_b64
        .as_deref()
        .ok_or_else(|| "encrypted wallet is missing aead_nonce_b64".to_string())?;
    let plaintext =
        open_wallet_secret_ciphertext(enc, salt_b64, nonce_b64, kdf, iters, passphrase)?;
    let payload: serde_json::Value =
        serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
    let signing_key_hex = payload
        .get("signing_key_hex")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "wallet payload is missing signing_key_hex".to_string())?;
    let sk = parse_signing_key_hex(signing_key_hex)?;
    Ok((sk, plaintext))
}

fn load_wallet_identity(
    path: &PathBuf,
    passphrase: Option<&str>,
    unlock_secs: u64,
) -> Result<WalletIdentity, String> {
    let loaded = load_wallet_read_header(path, true)
        .map_err(|e| format!("failed to load wallet '{}': {e}", path.display()))?;
    let wallet: WalletReadHeader = loaded.header;
    let account_id = parse_account_id(&wallet.account_id_human)
        .map_err(|e| format!("wallet account_id_human is invalid: {e}"))?;
    let pass = passphrase.filter(|s| !s.is_empty());
    let (signing_key, unlock_expires_at, wallet_is_encrypted, secret_payload_plaintext) =
        match wallet.mode.as_str() {
            "plaintext_dev" => {
                let sk =
                    parse_signing_key_hex(wallet.signing_key_hex.as_deref().ok_or_else(|| {
                        "wallet plaintext payload is missing signing_key_hex".to_string()
                    })?)?;
                (Some(sk), None, false, None)
            }
            "encrypted" => {
                if let Some(pw) = pass {
                    let (sk, pt) = try_decrypt_wallet_secret_payload(&wallet, Some(pw))?;
                    let until = Instant::now() + Duration::from_secs(unlock_secs);
                    (Some(sk), Some(until), true, Some(pt))
                } else {
                    (None, None, true, None)
                }
            }
            other => return Err(format!("unsupported wallet mode '{other}'")),
        };
    let mut address_book = Vec::new();
    for (i, entry) in wallet.address_book.iter().enumerate() {
        let bid = entry.account_id().map_err(|e| {
            format!(
                "wallet address_book[{i}] invalid address '{}': {e}",
                entry.address_str()
            )
        })?;
        let label = entry.label().map(|s| s.to_string());
        address_book.push(BookRecipient { id: bid, label });
    }
    let encryption_prompt_hint = wallet_upgrade_encryption_hook(&wallet, loaded.upgraded_on_load);
    Ok(WalletIdentity {
        account_id,
        account_id_human: wallet.account_id_human,
        domain: wallet.domain_u16,
        derivation_index: wallet.derivation_index,
        signing_key,
        unlock_expires_at,
        wallet_is_encrypted,
        wallet_path: path.clone(),
        address_book,
        encryption_prompt_hint,
        ignored_legacy_pretty_entries: loaded.ignored_legacy_pretty_entries,
        secret_payload_plaintext,
    })
}

fn wallet_try_unlock_with_passphrase(
    w: &mut WalletIdentity,
    passphrase: &str,
    unlock_secs: u64,
) -> Result<(), String> {
    let loaded = load_wallet_read_header(&w.wallet_path, true)
        .map_err(|e| format!("failed to load wallet: {e}"))?;
    let wallet = loaded.header;
    if wallet.mode != "encrypted" {
        return Err("wallet is not encrypted".into());
    }
    let (sk, pt) = try_decrypt_wallet_secret_payload(&wallet, Some(passphrase))?;
    w.signing_key = Some(sk);
    w.secret_payload_plaintext = Some(pt);
    w.unlock_expires_at = Some(Instant::now() + Duration::from_secs(unlock_secs));
    Ok(())
}

fn wallet_apply_auto_lock(identity: &mut IdentitySource) {
    let IdentitySource::Wallet(w) = identity else {
        return;
    };
    if !w.wallet_is_encrypted {
        return;
    }
    let Some(exp) = w.unlock_expires_at else {
        return;
    };
    if w.signing_key.is_none() {
        return;
    }
    if Instant::now() >= exp {
        wallet_lock_now(w);
    }
}

fn wallet_lock_now(w: &mut WalletIdentity) {
    if !w.wallet_is_encrypted {
        return;
    }
    w.signing_key = None;
    w.secret_payload_plaintext = None;
    w.unlock_expires_at = None;
}

fn validate_encrypt_passphrase_inputs(passphrase: &str, confirm: &str) -> Result<(), &'static str> {
    if passphrase.is_empty() {
        Err("passphrase must not be empty")
    } else if passphrase != confirm {
        Err("passphrases do not match")
    } else {
        Ok(())
    }
}

fn identity_f3_action_label(identity: &IdentitySource) -> &'static str {
    match identity {
        IdentitySource::Wallet(w) if w.wallet_is_encrypted && w.signing_key.is_some() => "lock",
        _ => "unlock",
    }
}

fn identity_lock_status_suffix(identity: &IdentitySource) -> String {
    let IdentitySource::Wallet(w) = identity else {
        return String::new();
    };
    if !w.wallet_is_encrypted {
        return String::new();
    }
    match (&w.signing_key, w.unlock_expires_at) {
        (None, _) => " | wallet: LOCKED".into(),
        (Some(_), Some(exp)) => {
            let left = exp.saturating_duration_since(Instant::now()).as_secs();
            format!(" | wallet: unlocked (~{left}s)")
        }
        (Some(_), None) => " | wallet: unlocked".into(),
    }
}

fn wallet_upgrade_encryption_hook(
    wallet: &WalletReadHeader,
    upgraded_on_load: bool,
) -> Option<String> {
    if upgraded_on_load && wallet.mode == "plaintext_dev" {
        return Some(
            "wallet format auto-upgraded (plaintext): press F4 to encrypt this wallet file".into(),
        );
    }
    None
}

fn yaml_root_map(root: &mut YamlValue) -> Result<&mut Mapping, String> {
    root.as_mapping_mut()
        .ok_or_else(|| "wallet YAML root must be a mapping".to_string())
}

fn yaml_map_get_string(map: &Mapping, key: &str) -> Option<String> {
    map.get(&YamlValue::String(key.to_string()))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

fn merge_normalized_wallet_header(
    root: &mut YamlValue,
    header: &WalletReadHeader,
) -> Result<(), String> {
    let map = yaml_root_map(root)?;
    map.insert(
        YamlValue::String("account_id_human".into()),
        YamlValue::String(header.account_id_human.clone()),
    );
    let hex_line = if let Some(h) = header
        .account_id_hex
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        h.to_string()
    } else {
        let id = parse_account_id(header.account_id_human.trim())
            .map_err(|e| format!("wallet account_id_human invalid: {e}"))?;
        hex::encode(id)
    };
    map.insert(
        YamlValue::String("account_id_hex".into()),
        YamlValue::String(hex_line),
    );
    let book = serde_yaml::to_value(&header.address_book).map_err(|e| e.to_string())?;
    map.insert(YamlValue::String("address_book".into()), book);
    Ok(())
}

#[derive(Serialize)]
struct WalletSecretJson {
    master_seed_hex: String,
    master_seed_b64: String,
    signing_key_hex: String,
    signing_key_b64: String,
    verifying_key_hex: String,
    verifying_key_b64: String,
}

fn build_plaintext_secret_json(map: &Mapping) -> Result<Vec<u8>, String> {
    let b64 = base64::engine::general_purpose::STANDARD;
    let master_seed_hex = yaml_map_get_string(map, "master_seed_hex").ok_or_else(|| {
        "wallet plaintext is missing master_seed_hex (cannot encrypt)".to_string()
    })?;
    let signing_key_hex = yaml_map_get_string(map, "signing_key_hex").ok_or_else(|| {
        "wallet plaintext is missing signing_key_hex (cannot encrypt)".to_string()
    })?;
    let sk = parse_signing_key_hex(&signing_key_hex)?;
    let verifying_key_hex = if let Some(v) = yaml_map_get_string(map, "verifying_key_hex") {
        v
    } else {
        hex::encode(sk.verifying_key().to_bytes())
    };
    let seed_vec = hex::decode(master_seed_hex.trim())
        .map_err(|e| format!("wallet master_seed_hex invalid: {e}"))?;
    if seed_vec.len() != 32 {
        return Err("wallet master_seed_hex must be 32 bytes".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_vec);
    let master_seed_b64 =
        yaml_map_get_string(map, "master_seed_b64").unwrap_or_else(|| b64.encode(seed));
    let signing_key_b64 =
        yaml_map_get_string(map, "signing_key_b64").unwrap_or_else(|| b64.encode(sk.to_bytes()));
    let vk_bytes = hex::decode(verifying_key_hex.trim())
        .map_err(|e| format!("wallet verifying_key_hex invalid: {e}"))?;
    if vk_bytes.len() != 32 {
        return Err("wallet verifying_key_hex must be 32 bytes".into());
    }
    let verifying_key_b64 =
        yaml_map_get_string(map, "verifying_key_b64").unwrap_or_else(|| b64.encode(vk_bytes));
    let payload = WalletSecretJson {
        master_seed_hex,
        master_seed_b64,
        signing_key_hex,
        signing_key_b64,
        verifying_key_hex,
        verifying_key_b64,
    };
    serde_json::to_vec(&payload).map_err(|e| e.to_string())
}

fn replace_wallet_file(tmp: &Path, dest: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        std::fs::rename(tmp, dest).map_err(|e| e.to_string())?;
    }
    #[cfg(windows)]
    {
        if dest.exists() {
            std::fs::remove_file(dest).map_err(|e| e.to_string())?;
        }
        std::fs::rename(tmp, dest).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Atomic replace: write temp in the same directory, `fsync`, then rename (Windows: delete dest first).
fn write_wallet_yaml_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let _ = path
        .file_name()
        .ok_or_else(|| "wallet path has no file name".to_string())?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let tmp_path = dir.join(format!(
        "pwm_wallet_tmp_{}_{}.yml",
        std::process::id(),
        nanos
    ));
    {
        let mut f = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        f.write_all(contents.as_bytes())
            .map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    match replace_wallet_file(&tmp_path, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e)
        }
    }
}

/// Encrypt `plaintext_dev` or re-seal an `encrypted` wallet with `rekey_payload` (decrypted JSON bytes).
fn wallet_encrypt_or_rekey_disk(
    path: &Path,
    new_passphrase: &str,
    rekey_payload: Option<&[u8]>,
) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut root: YamlValue = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
    let header: WalletReadHeader = serde_yaml::from_str(&raw).map_err(|e| e.to_string())?;
    let (normalized, _) = normalize_wallet_header(header)?;
    merge_normalized_wallet_header(&mut root, &normalized)?;
    let map = yaml_root_map(&mut root)?;
    let mode = yaml_map_get_string(map, "mode").unwrap_or_default();
    let secret_json: Vec<u8> = match mode.as_str() {
        "plaintext_dev" => {
            if rekey_payload.is_some() {
                return Err("internal: rekey payload set for plaintext wallet".into());
            }
            build_plaintext_secret_json(map)?
        }
        "encrypted" => rekey_payload.map(|p| p.to_vec()).ok_or_else(|| {
            "encrypted wallet: unlock (F3) or start with PWM_TUI_WALLET_PASSPHRASE before re-key"
                .to_string()
        })?,
        other => return Err(format!("unsupported wallet mode '{other}'")),
    };
    let sealed = seal_wallet_secret_plaintext(&secret_json, new_passphrase)?;
    let map = yaml_root_map(&mut root)?;
    if mode == "plaintext_dev" {
        for k in [
            "master_seed_hex",
            "master_seed_b64",
            "signing_key_hex",
            "signing_key_b64",
            "verifying_key_hex",
            "verifying_key_b64",
        ] {
            map.remove(&YamlValue::String(k.into()));
        }
    }
    map.insert(
        YamlValue::String("schema_version".into()),
        serde_yaml::to_value(2u32).map_err(|e| e.to_string())?,
    );
    map.insert(
        YamlValue::String("mode".into()),
        YamlValue::String("encrypted".into()),
    );
    map.insert(
        YamlValue::String("encrypted_payload_b64".into()),
        YamlValue::String(sealed.encrypted_payload_b64),
    );
    map.insert(
        YamlValue::String("kdf_salt_b64".into()),
        YamlValue::String(sealed.kdf_salt_b64),
    );
    map.insert(
        YamlValue::String("aead_nonce_b64".into()),
        YamlValue::String(sealed.aead_nonce_b64),
    );
    map.insert(
        YamlValue::String("kdf".into()),
        YamlValue::String(sealed.kdf),
    );
    map.insert(
        YamlValue::String("kdf_iters".into()),
        serde_yaml::to_value(sealed.kdf_iters).map_err(|e| e.to_string())?,
    );
    let out = serde_yaml::to_string(&root).map_err(|e| e.to_string())?;
    write_wallet_yaml_atomic(path, &out)
}

fn choose_identity(args: &Args, unlock_secs: u64) -> Result<(IdentitySource, String), String> {
    if let Some(wallet) = &args.wallet {
        let identity =
            load_wallet_identity(wallet, args.wallet_passphrase.as_deref(), unlock_secs)?;
        let mut note = format!(
            "wallet owner: {} (m/0/{}, d={})",
            identity.account_id_human, identity.derivation_index, identity.domain
        );
        if identity.wallet_is_encrypted && identity.signing_key.is_none() {
            note.push_str(" | encrypted: signing key locked — press F3 to unlock");
        }
        let note = if let Some(hook) = identity.encryption_prompt_hint.as_deref() {
            format!("{note} | {hook}")
        } else {
            note
        };
        let note = if identity.ignored_legacy_pretty_entries > 0 {
            format!(
                "{note} | ignored legacy pretty address_book entries={}",
                identity.ignored_legacy_pretty_entries
            )
        } else {
            note
        };
        Ok((IdentitySource::Wallet(identity), note))
    } else {
        // Banner text is `FALLBACK_MODE_WARNING` in the main layout (not duplicated in footer).
        Ok((IdentitySource::SeedFallback, String::new()))
    }
}

/// If `./default.yml` exists under the process current directory, use it as `--wallet`.
fn default_wallet_if_present() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let p = cwd.join("default.yml");
    p.is_file().then_some(p)
}

#[cfg(test)]
fn default_wallet_candidate(base: &std::path::Path) -> Option<PathBuf> {
    let p = base.join("default.yml");
    p.is_file().then_some(p)
}

fn fetch_nonce(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    from: AccountId,
) -> Result<u64, String> {
    let from_hex = hex::encode(from);
    let url = format!("{}/v1/account/{}", rpc_base, from_hex);
    let r = c.get(&url).send().map_err(|e| {
        if e.is_timeout() {
            "nonce: rpc timeout".to_string()
        } else {
            "nonce: rpc offline".to_string()
        }
    })?;
    let is_success = r.status().is_success();
    let body = r.text().unwrap_or_default();
    Ok(nonce_from_account_response(is_success, &body))
}

fn parse_nonce_json(body: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("nonce").and_then(parse_u64_value)
}

fn nonce_from_account_response(is_success: bool, body: &str) -> u64 {
    if !is_success {
        return 0;
    }
    parse_nonce_json(body).unwrap_or(0)
}

fn parse_u64_value(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

fn derive_sender_for_from(
    from: &AccountId,
    master_seed_hex: &str,
) -> Result<(ed25519_dalek::SigningKey, u16, u32), String> {
    let seed_raw =
        hex::decode(master_seed_hex.trim()).map_err(|e| format!("bad master seed: {e}"))?;
    if seed_raw.len() != 32 {
        return Err("master seed must be 32-byte hex (64 chars)".into());
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&seed_raw);
    let dom = domain_of_account_id(from);
    let brute_max = std::env::var("PWM_TUI_BRUTE_MAX")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(500_000);
    let hit = brute_cluster_address(&seed, dom, brute_max)
        .ok_or_else(|| format!("cannot derive sender for domain in first {brute_max} tries"))?;
    if &hit.3 != from {
        return Err("from address does not match derived account from PWM_TUI_MASTER_SEED".into());
    }
    Ok((hit.0, dom, hit.2))
}

fn submit_transfer(
    from: &AccountId,
    to: &AccountId,
    amount: u128,
    fee: u128,
    identity: &IdentitySource,
) -> Result<String, String> {
    let (sk, dom, idx) = match identity {
        IdentitySource::Wallet(w) => {
            if &w.account_id != from {
                return Err("from must match wallet account_id_human".into());
            }
            let sk = w.signing_key.as_ref().ok_or_else(|| {
                "wallet is locked: press F3 to unlock (signing key not held in memory)".to_string()
            })?;
            (sk.clone(), w.domain, w.derivation_index)
        }
        IdentitySource::SeedFallback => {
            let seed_hex = std::env::var("PWM_TUI_MASTER_SEED")
                .map_err(|_| "set PWM_TUI_MASTER_SEED (32-byte hex) for signing".to_string())?;
            derive_sender_for_from(from, &seed_hex)?
        }
    };
    let client = http_client();
    let rpc = base_url();
    let nonce = fetch_nonce(&client, &rpc, *from)?;
    let tx = SignedTx::sign_body(
        &sk,
        dom,
        idx,
        nonce,
        TxBody::Transfer {
            to: *to,
            amount,
            fee,
        },
    );
    let response = client
        .post(format!("{}/v1/tx", rpc))
        .json(&tx)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                "rpc error: timeout".to_string()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if status.is_success() {
        Ok(format!("sent: {status}"))
    } else if body.is_empty() {
        Err(format!("submit failed: {status}"))
    } else {
        Err(format!("submit failed: {status} {body}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SendField {
    To,
    Amount,
    Fee,
    Confirm,
}

struct SendForm {
    from: String,
    to: String,
    to_editable: bool,
    amount: String,
    fee: String,
    confirm: String,
    to_cursor: usize,
    amount_cursor: usize,
    fee_cursor: usize,
    confirm_cursor: usize,
    active: SendField,
    status: String,
    status_is_error: bool,
}

impl SendForm {
    fn new(from: String, to: String, to_editable: bool) -> Self {
        let active = if to_editable {
            SendField::To
        } else {
            SendField::Amount
        };
        let to_len = to.len();
        Self {
            from,
            to,
            to_editable,
            amount: String::new(),
            fee: "0".into(),
            confirm: String::new(),
            to_cursor: to_len,
            amount_cursor: 0,
            fee_cursor: 1,
            confirm_cursor: 0,
            active,
            status: "Fill fields and press Enter on confirm".into(),
            status_is_error: false,
        }
    }

    fn next_field(&mut self) {
        self.active = if self.to_editable {
            match self.active {
                SendField::To => SendField::Amount,
                SendField::Amount => SendField::Fee,
                SendField::Fee => SendField::Confirm,
                SendField::Confirm => SendField::To,
            }
        } else {
            match self.active {
                SendField::To => SendField::Amount,
                SendField::Amount => SendField::Fee,
                SendField::Fee => SendField::Confirm,
                SendField::Confirm => SendField::Amount,
            }
        };
    }

    fn prev_field(&mut self) {
        self.active = if self.to_editable {
            match self.active {
                SendField::To => SendField::Confirm,
                SendField::Amount => SendField::To,
                SendField::Fee => SendField::Amount,
                SendField::Confirm => SendField::Fee,
            }
        } else {
            match self.active {
                SendField::To => SendField::Confirm,
                SendField::Amount => SendField::Confirm,
                SendField::Fee => SendField::Amount,
                SendField::Confirm => SendField::Fee,
            }
        };
    }

    fn active_cursor_mut(&mut self) -> Option<&mut usize> {
        match self.active {
            SendField::To if self.to_editable => Some(&mut self.to_cursor),
            SendField::To => None,
            SendField::Amount => Some(&mut self.amount_cursor),
            SendField::Fee => Some(&mut self.fee_cursor),
            SendField::Confirm => Some(&mut self.confirm_cursor),
        }
    }

    fn active_state_mut(&mut self) -> Option<(&mut String, &mut usize)> {
        match self.active {
            SendField::To if self.to_editable => Some((&mut self.to, &mut self.to_cursor)),
            SendField::To => None,
            SendField::Amount => Some((&mut self.amount, &mut self.amount_cursor)),
            SendField::Fee => Some((&mut self.fee, &mut self.fee_cursor)),
            SendField::Confirm => Some((&mut self.confirm, &mut self.confirm_cursor)),
        }
    }

    fn clamp_active_cursor(&mut self) {
        if let Some((active, cursor)) = self.active_state_mut() {
            *cursor = (*cursor).min(active.len());
        }
    }

    fn move_left(&mut self) {
        if let Some((active, cursor)) = self.active_state_mut() {
            *cursor = prev_char_boundary(active, *cursor);
        }
    }

    fn move_right(&mut self) {
        if let Some((active, cursor)) = self.active_state_mut() {
            *cursor = next_char_boundary(active, *cursor);
        }
    }

    fn move_home(&mut self) {
        if let Some(cursor) = self.active_cursor_mut() {
            *cursor = 0;
        }
    }

    fn move_end(&mut self) {
        if let Some((active, cursor)) = self.active_state_mut() {
            *cursor = active.len();
        }
    }

    fn insert_char(&mut self, c: char) {
        if let Some((active, cursor)) = self.active_state_mut() {
            let i = (*cursor).min(active.len());
            active.insert(i, c);
            *cursor = i + c.len_utf8();
        }
    }

    fn backspace(&mut self) {
        if let Some((active, cursor)) = self.active_state_mut() {
            if *cursor == 0 {
                return;
            }
            let from = prev_char_boundary(active, *cursor);
            active.drain(from..*cursor);
            *cursor = from;
        }
    }

    fn delete(&mut self) {
        if let Some((active, cursor)) = self.active_state_mut() {
            if *cursor >= active.len() {
                return;
            }
            let to = next_char_boundary(active, *cursor);
            active.drain(*cursor..to);
        }
    }
}

fn prev_char_boundary(s: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    let mut i = idx.min(s.len()) - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_char_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = (idx + 1).min(s.len());
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn value_with_caret(value: &str, cursor: usize, active: bool) -> String {
    if !active {
        return value.to_string();
    }
    let i = cursor.min(value.len());
    let mut shown = String::with_capacity(value.len() + 1);
    shown.push_str(&value[..i]);
    shown.push('|');
    shown.push_str(&value[i..]);
    shown
}

fn validate_send_form(form: &SendForm) -> Result<(AccountId, AccountId, u128, u128), String> {
    let from = parse_account_id(&form.from).map_err(|e| format!("from: {e}"))?;
    let to = parse_account_id_for_user_input(&form.to).map_err(|e| format!("to: {e}"))?;
    let amount = parse_decimal_pwm_units(form.amount.trim()).map_err(|e| format!("amount: {e}"))?;
    if amount == 0 {
        return Err("amount must be > 0".into());
    }
    let fee = parse_decimal_pwm_units(form.fee.trim()).map_err(|e| format!("fee: {e}"))?;
    if form.confirm.trim().to_lowercase() != "yes" {
        return Err("confirm must be 'yes'".into());
    }
    Ok((from, to, amount, fee))
}

fn parse_decimal_pwm_units(raw: &str) -> Result<u128, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("value is required".into());
    }
    if s.starts_with('-') {
        return Err("negative values are not allowed".into());
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return Err("must be a decimal number like 12.34".into());
    }
    let whole_units = whole
        .parse::<u128>()
        .map_err(|_| "numeric overflow".to_string())?
        .checked_mul(SEND_DECIMAL_SCALE)
        .ok_or_else(|| "numeric overflow".to_string())?;
    let frac_units = if let Some(frac_raw) = frac {
        if frac_raw.is_empty() || !frac_raw.chars().all(|c| c.is_ascii_digit()) {
            return Err("must be a decimal number like 12.34".into());
        }
        if frac_raw.len() > 6 {
            return Err(
                "supports up to 6 decimal places (scale 1 PWM = 1_000_000 base units)".into(),
            );
        }
        let mut frac_padded = frac_raw.to_string();
        while frac_padded.len() < 6 {
            frac_padded.push('0');
        }
        frac_padded
            .parse::<u128>()
            .map_err(|_| "numeric overflow".to_string())?
    } else {
        0
    };
    whole_units
        .checked_add(frac_units)
        .ok_or_else(|| "numeric overflow".to_string())
}

struct Ui {
    head: String,
    rows: Vec<AcctRow>,
    detail_line: String,
    debug_detail: String,
    err: String,
    rpc_health: RpcHealth,
    identity_note: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Owner,
    Receivers,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            head: "…".into(),
            rows: vec![],
            detail_line: String::new(),
            debug_detail: String::new(),
            err: String::new(),
            rpc_health: RpcHealth::Online,
            identity_note: String::new(),
        }
    }
}

#[derive(Clone)]
struct PollSnapshot {
    head: String,
    rows: Vec<AcctRow>,
    err: String,
    rpc_health: RpcHealth,
}

enum RpcTask {
    Poll,
    DebugAccount {
        id_hex: String,
    },
    SubmitTransfer {
        req_id: u64,
        from: AccountId,
        to: AccountId,
        amount: u128,
        fee: u128,
        identity: IdentitySource,
    },
}

enum RpcEvent {
    PollDone(PollSnapshot),
    DebugAccountDone {
        id_hex: String,
        detail: String,
        rpc_health: RpcHealth,
    },
    SubmitDone {
        req_id: u64,
        to_id: AccountId,
        result: Result<String, String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpStatus {
    Pending,
    Ok,
    Error,
}

impl OpStatus {
    fn as_str(self) -> &'static str {
        match self {
            OpStatus::Pending => "pending",
            OpStatus::Ok => "ok",
            OpStatus::Error => "error",
        }
    }
}

#[derive(Clone, Debug)]
struct OperationHistoryEntry {
    req_id: u64,
    created_unix_secs: u64,
    from_human: String,
    to_human: String,
    amount_units: u128,
    fee_units: u128,
    status: OpStatus,
    note: String,
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn format_hms_utc(ts: u64) -> String {
    let sec = ts % 60;
    let min = (ts / 60) % 60;
    let hour = (ts / 3600) % 24;
    format!("{hour:02}:{min:02}:{sec:02}Z")
}

fn push_op_history(hist: &mut Vec<OperationHistoryEntry>, entry: OperationHistoryEntry) {
    hist.insert(0, entry);
    if hist.len() > OP_HISTORY_MAX_ITEMS {
        hist.truncate(OP_HISTORY_MAX_ITEMS);
    }
}

fn set_op_history_status(
    hist: &mut [OperationHistoryEntry],
    req_id: u64,
    status: OpStatus,
    note: String,
) -> bool {
    if let Some(item) = hist.iter_mut().find(|x| x.req_id == req_id) {
        item.status = status;
        item.note = note;
        true
    } else {
        false
    }
}

fn handle_submit_done_history(
    inflight_send_req_id: &mut Option<u64>,
    op_history: &mut [OperationHistoryEntry],
    req_id: u64,
    result: &Result<String, String>,
) -> bool {
    if *inflight_send_req_id != Some(req_id) {
        return false;
    }
    *inflight_send_req_id = None;
    match result {
        Ok(msg) => {
            let _ = set_op_history_status(op_history, req_id, OpStatus::Ok, msg.clone());
        }
        Err(err) => {
            let _ = set_op_history_status(op_history, req_id, OpStatus::Error, err.clone());
        }
    }
    true
}

struct DebugCache {
    selected_id_hex: Option<String>,
    inflight_id_hex: Option<String>,
    cached_detail: String,
    last_fetch_at: Instant,
}

impl DebugCache {
    fn new() -> Self {
        Self {
            selected_id_hex: None,
            inflight_id_hex: None,
            cached_detail: String::new(),
            last_fetch_at: Instant::now() - DEBUG_FETCH_INTERVAL,
        }
    }
}

fn acct_row_for_id(rows: &[AcctRow], id: &AccountId, label: Option<String>) -> AcctRow {
    let mut base = rows
        .iter()
        .find(|r| r.id == *id)
        .cloned()
        .unwrap_or_else(|| AcctRow {
            id: *id,
            id_hex: hex::encode(id),
            balance_pwm: 0,
            initialized: false,
            nonce: 0,
            label: None,
        });
    if label.is_some() {
        base.label = label;
    }
    base
}

fn owner_and_receivers(
    rows: &[AcctRow],
    identity: &IdentitySource,
) -> (Option<AcctRow>, Vec<AcctRow>) {
    match identity {
        IdentitySource::Wallet(w) => {
            let owner = rows
                .iter()
                .find(|r| r.id == w.account_id)
                .cloned()
                .unwrap_or_else(|| AcctRow {
                    id: w.account_id,
                    id_hex: hex::encode(w.account_id),
                    balance_pwm: 0,
                    initialized: false,
                    nonce: 0,
                    label: None,
                });
            let receivers: Vec<AcctRow> = if !w.address_book.is_empty() {
                w.address_book
                    .iter()
                    .filter(|b| b.id != w.account_id)
                    .map(|b| acct_row_for_id(rows, &b.id, b.label.clone()))
                    .collect()
            } else if let Some((i, _)) = rows.iter().enumerate().find(|(_, r)| r.id == w.account_id)
            {
                rows.iter()
                    .enumerate()
                    .filter(|(idx, _)| *idx != i)
                    .map(|(_, r)| r.clone())
                    .collect()
            } else {
                rows.to_vec()
            };
            (Some(owner), receivers)
        }
        IdentitySource::SeedFallback => {
            let owner = rows.first().cloned();
            let receivers = rows.iter().skip(1).cloned().collect();
            (owner, receivers)
        }
    }
}

fn poll_snapshot(client: &reqwest::blocking::Client) -> PollSnapshot {
    let b = base_url();
    let mut head = "…".to_string();
    let mut rows = Vec::new();
    let mut rpc_health = RpcHealth::Online;
    let mut parts: Vec<&'static str> = Vec::new();
    match fetch_json(client, &format!("{}/v1/head", b)) {
        Ok(v) => {
            head = format!(
                "height={} tip={}",
                v["height"].as_u64().unwrap_or(0),
                v["tip"].as_str().unwrap_or("?")
            );
        }
        Err(e) => {
            parts.push(match e {
                JsonFetchFailure::Timeout => "head: timeout",
                JsonFetchFailure::Other => "head: offline",
            });
            rpc_health = merge_rpc_health(rpc_health, rpc_health_from_failure(e));
        }
    }
    match fetch_json(client, &format!("{}/v1/accounts", b)) {
        Ok(v) => {
            rows = v["accounts"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let id_hex = x["id"].as_str()?.to_string();
                            let id = parse_hex_account_id(&id_hex)?;
                            Some(AcctRow {
                                id,
                                id_hex,
                                balance_pwm: parse_u128(&x["balance_pwm"]),
                                initialized: x["initialized"].as_bool().unwrap_or(false),
                                nonce: x["nonce"].as_u64().unwrap_or(0),
                                label: None,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
        Err(e) => {
            parts.push(match e {
                JsonFetchFailure::Timeout => "accounts: timeout",
                JsonFetchFailure::Other => "accounts: offline",
            });
            rpc_health = merge_rpc_health(rpc_health, rpc_health_from_failure(e));
        }
    }
    PollSnapshot {
        head,
        rows,
        err: parts.join("; "),
        rpc_health,
    }
}

fn fetch_debug_account(client: &reqwest::blocking::Client, id_hex: &str) -> (String, RpcHealth) {
    let b = base_url();
    match fetch_json(client, &format!("{}/v1/account/{}", b, id_hex)) {
        Ok(v) => (
            serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into()),
            RpcHealth::Online,
        ),
        Err(JsonFetchFailure::Timeout) => {
            ("debug: account json timeout".into(), RpcHealth::Timeout)
        }
        Err(JsonFetchFailure::Other) => ("debug: account rpc offline".into(), RpcHealth::Offline),
    }
}

fn start_rpc_worker() -> (Sender<RpcTask>, Receiver<RpcEvent>) {
    let (task_tx, task_rx) = mpsc::channel::<RpcTask>();
    let (evt_tx, evt_rx) = mpsc::channel::<RpcEvent>();
    thread::spawn(move || {
        let client = http_client();
        while let Ok(task) = task_rx.recv() {
            match task {
                RpcTask::Poll => {
                    let _ = evt_tx.send(RpcEvent::PollDone(poll_snapshot(&client)));
                }
                RpcTask::DebugAccount { id_hex } => {
                    let (detail, rpc_health) = fetch_debug_account(&client, &id_hex);
                    let _ = evt_tx.send(RpcEvent::DebugAccountDone {
                        id_hex,
                        detail,
                        rpc_health,
                    });
                }
                RpcTask::SubmitTransfer {
                    req_id,
                    from,
                    to,
                    amount,
                    fee,
                    identity,
                } => {
                    let result = submit_transfer(&from, &to, amount, fee, &identity);
                    let _ = evt_tx.send(RpcEvent::SubmitDone {
                        req_id,
                        to_id: to,
                        result,
                    });
                }
            }
        }
    });
    (task_tx, evt_rx)
}

fn format_acct_cell(r: &AcctRow) -> String {
    if let Some(l) = r
        .label
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        format!("{l} | {}", account_id_to_human(&r.id))
    } else {
        account_id_to_human(&r.id)
    }
}

fn clamp_sel(sel: &mut usize, len: usize) {
    if len == 0 {
        *sel = 0;
    } else if *sel >= len {
        *sel = len - 1;
    }
}

fn receiver_table_len(receiver_rows: &[AcctRow]) -> usize {
    receiver_rows.len() + 1
}

fn move_selection_down(sel: &mut usize, len: usize) {
    if len > 0 {
        *sel = (*sel + 1).min(len - 1);
    }
}

fn move_selection_up(sel: &mut usize) {
    *sel = sel.saturating_sub(1);
}

fn selected_to_receiver(receiver_rows: &[AcctRow], recv_sel: usize) -> Option<&AcctRow> {
    if recv_sel == 0 {
        None
    } else {
        receiver_rows.get(recv_sel - 1)
    }
}

fn f6_send_form_for_identity(
    identity: &IdentitySource,
    owner_row: Option<&AcctRow>,
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
    let from = match identity {
        IdentitySource::Wallet(w) => w.account_id_human.clone(),
        IdentitySource::SeedFallback => owner_row
            .as_ref()
            .map(|r| account_id_to_human(&r.id))
            .unwrap_or_default(),
    };
    let selected = selected_to_receiver(receiver_rows, recv_sel);
    let to = selected
        .map(|r| account_id_to_human(&r.id))
        .unwrap_or_default();
    Ok(SendForm::new(from, to, selected.is_none()))
}

fn run(mut args: Args) -> std::io::Result<()> {
    let unlock_secs = wallet_unlock_secs_clamped(&args);
    let (mut identity, identity_note) = choose_identity(&args, unlock_secs)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(out))?;

    let mut ui = Ui::default();
    let mut owner_sel: usize = 0;
    let mut recv_sel: usize = 0;
    let mut active = Panel::Owner;
    let mut info_modal: Option<String> = None;
    let mut unlock_modal: Option<UnlockModal> = None;
    let mut encrypt_modal: Option<EncryptModal> = None;
    let mut send_form: Option<SendForm> = None;
    let mut book_prompt: Option<BookPromptModal> = None;
    let mut history_open = false;
    let mut op_history: Vec<OperationHistoryEntry> = Vec::new();
    let mut last = Instant::now() - Duration::from_secs(10);
    let dbg = debug_json();
    ui.identity_note = identity_note.clone();
    let mut debug_cache = DebugCache::new();
    let mut send_req_id: u64 = 0;
    let mut inflight_send_req_id: Option<u64> = None;
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
                    ui.rows = snapshot.rows;
                    ui.err = snapshot.err;
                    ui.rpc_health = snapshot.rpc_health;
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
                                form.status = msg;
                                form.status_is_error = false;
                                let offer = if let IdentitySource::Wallet(w) = &identity {
                                    if !w.has_recipient(&to_id) {
                                        Some(account_id_to_human(&to_id))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                };
                                send_form = None;
                                if let Some(tdisp) = offer {
                                    book_prompt = Some(BookPromptModal::new(tdisp));
                                }
                                let _ = rpc_tx.send(RpcTask::Poll);
                            }
                            Err(e) => {
                                form.status = e;
                                form.status_is_error = true;
                            }
                        }
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        let (owner_row, receiver_rows) = owner_and_receivers(&ui.rows, &identity);

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                if let Some(um) = unlock_modal.as_mut() {
                    um.clamp_cursor();
                    match k.code {
                        KeyCode::Esc => unlock_modal = None,
                        KeyCode::Enter => {
                            if let IdentitySource::Wallet(w) = &mut identity {
                                match wallet_try_unlock_with_passphrase(
                                    w,
                                    um.passphrase.trim(),
                                    unlock_secs,
                                ) {
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
                        KeyCode::Left => um.move_left(),
                        KeyCode::Right => um.move_right(),
                        KeyCode::Home => um.move_home(),
                        KeyCode::End => um.move_end(),
                        KeyCode::Backspace => um.backspace(),
                        KeyCode::Delete => um.delete(),
                        KeyCode::Char(c) => um.insert_char(c),
                        _ => {}
                    }
                } else if let Some(em) = encrypt_modal.as_mut() {
                    em.clamp_cursors();
                    match k.code {
                        KeyCode::Esc => encrypt_modal = None,
                        KeyCode::Enter => {
                            let p = em.passphrase.trim();
                            let c = em.confirm.trim();
                            if let Err(err_msg) = validate_encrypt_passphrase_inputs(p, c) {
                                em.status = err_msg.into();
                                em.status_is_error = true;
                            } else if let IdentitySource::Wallet(w) = &identity {
                                let rekey = if w.wallet_is_encrypted {
                                    w.secret_payload_plaintext.as_deref()
                                } else {
                                    None
                                };
                                match wallet_encrypt_or_rekey_disk(&w.wallet_path, p, rekey) {
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
                    bp.clamp_cursor();
                    match k.code {
                        KeyCode::Esc => book_prompt = None,
                        KeyCode::Enter => {
                            if let IdentitySource::Wallet(w) = &identity {
                                let path = &w.wallet_path;
                                let lbl = bp.label_line.trim();
                                match append_wallet_yaml_address_book(
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
                        KeyCode::Left => bp.move_left(),
                        KeyCode::Right => bp.move_right(),
                        KeyCode::Home => bp.move_home(),
                        KeyCode::End => bp.move_end(),
                        KeyCode::Backspace => bp.backspace(),
                        KeyCode::Delete => bp.delete(),
                        KeyCode::Char(c) => bp.insert_char(c),
                        _ => {}
                    }
                } else if let Some(form) = send_form.as_mut() {
                    form.clamp_active_cursor();
                    match k.code {
                        KeyCode::Esc => send_form = None,
                        KeyCode::Up => form.prev_field(),
                        KeyCode::Down | KeyCode::Tab => form.next_field(),
                        KeyCode::Left => form.move_left(),
                        KeyCode::Right => form.move_right(),
                        KeyCode::Home => form.move_home(),
                        KeyCode::End => form.move_end(),
                        KeyCode::Backspace => form.backspace(),
                        KeyCode::Delete => form.delete(),
                        KeyCode::Enter => {
                            if form.active == SendField::Confirm {
                                if inflight_send_req_id.is_some() {
                                    form.status = "submit already in progress".into();
                                    form.status_is_error = true;
                                } else {
                                    match validate_send_form(form) {
                                        Ok((from, to, amount, fee)) => {
                                            send_req_id = send_req_id.wrapping_add(1);
                                            inflight_send_req_id = Some(send_req_id);
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
                                                    note: "submitting tx...".into(),
                                                },
                                            );
                                            form.status = "submitting tx...".into();
                                            form.status_is_error = false;
                                            let _ = rpc_tx.send(RpcTask::SubmitTransfer {
                                                req_id: send_req_id,
                                                from,
                                                to,
                                                amount,
                                                fee,
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
                    let owner_len = if owner_row.is_some() { 1 } else { 0 };
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
                        KeyCode::F(5) => info_modal = Some("F5 burn/send: MVP TODO".into()),
                        KeyCode::F(6) => {
                            match f6_send_form_for_identity(
                                &identity,
                                owner_row.as_ref(),
                                &receiver_rows,
                                recv_sel,
                            ) {
                                Ok(form) => send_form = Some(form),
                                Err(msg) => info_modal = Some(msg),
                            }
                        }
                        KeyCode::Char('h') | KeyCode::Char('H') => {
                            history_open = true;
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

        let owner_len = if owner_row.is_some() { 1 } else { 0 };
        let recv_len = receiver_table_len(&receiver_rows);
        clamp_sel(&mut owner_sel, owner_len);
        clamp_sel(&mut recv_sel, recv_len);

        let selected_row = match active {
            Panel::Owner => owner_row.as_ref(),
            Panel::Receivers => {
                selected_to_receiver(&receiver_rows, recv_sel).or_else(|| owner_row.as_ref())
            }
        };
        if let Some(r) = selected_row {
            ui.detail_line = format!(
                "selected: {} | init={} | nonce={}",
                format_acct_cell(r),
                r.initialized,
                r.nonce
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
            let dbg = debug_json();
            let is_fallback = matches!(identity, IdentitySource::SeedFallback);
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
                .split(f.size());
            let panels = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[0]);

            let header = Row::new(vec![Cell::from("Address"), Cell::from("PWM")])
            .style(Style::default().add_modifier(Modifier::BOLD));
            let owner_rows: Vec<Row> = owner_row
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
                        Cell::from(r.balance_pwm.to_string()),
                    ])
                    .style(style)
                })
                .collect();
            let owner_block = Block::default()
                .borders(Borders::ALL)
                .title("Owner")
                .border_style(if active == Panel::Owner {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                });
            let owner_table = Table::new(
                owner_rows,
                [Constraint::Min(40), Constraint::Length(12)],
            )
            .header(header.clone())
            .block(owner_block);
            f.render_widget(owner_table, panels[0]);

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
                        Cell::from(r.balance_pwm.to_string()),
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
                Row::new(vec![Cell::from("New Recipient"), Cell::from("-")]).style(new_recipient_style),
            );
            recv_rows_all.extend(recv_rows);
            let recv_title = match &identity {
                IdentitySource::Wallet(w) if !w.address_book.is_empty() => "Receivers (address book)",
                _ => "Receivers",
            };
            let recv_block = Block::default()
                .borders(Borders::ALL)
                .title(recv_title)
                .border_style(if active == Panel::Receivers {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default()
                });
            let recv_table = Table::new(
                recv_rows_all,
                [Constraint::Min(40), Constraint::Length(12)],
            )
            .header(header)
            .block(recv_block);
            f.render_widget(recv_table, panels[1]);

            if let Some(wi) = warn_chunk {
                f.render_widget(
                    Paragraph::new(FALLBACK_MODE_WARNING)
                        .style(Style::default().fg(Color::Yellow))
                        .wrap(Wrap { trim: true })
                        .block(Block::default().borders(Borders::ALL).title("WARNING")),
                    chunks[wi],
                );
            }

            f.render_widget(
                Paragraph::new(ui.detail_line.clone())
                    .block(Block::default().borders(Borders::ALL)),
                chunks[detail_chunk],
            );

            if let Some(di) = debug_chunk {
                f.render_widget(
                    Paragraph::new(ui.debug_detail.clone())
                        .block(Block::default().borders(Borders::ALL).title("debug JSON")),
                    chunks[di],
                );
            }

            let foot_identity = format!(
                "{}{}",
                ui.identity_note,
                identity_lock_status_suffix(&identity)
            );
            let foot_line = status_footer_line(
                &ui.head,
                &ui.err,
                &foot_identity,
                identity_f3_action_label(&identity),
                ui.rpc_health,
                dbg,
                &base_url(),
            );
            // Single-line status: no `Borders::ALL` here — `Length(1)` cannot fit a full box (broken corners).
            f.render_widget(Paragraph::new(foot_line), chunks[foot_chunk]);

            if let Some(msg) = info_modal.as_ref() {
                let area = centered_rect(50, 20, f.size());
                f.render_widget(Clear, area);
                f.render_widget(
                    Paragraph::new(format!("{msg}\n\nPress Enter/Esc"))
                        .block(Block::default().borders(Borders::ALL).title("Action")),
                    area,
                );
            }

            if history_open {
                let area = centered_rect(86, 62, f.size());
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
                                Cell::from(format!(
                                    "{} (+fee {})",
                                    it.amount_units, it.fee_units
                                )),
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

            if let Some(form) = send_form.as_ref() {
                let area = centered_rect(70, 55, f.size());
                f.render_widget(Clear, area);
                let fields = [
                    ("from", &form.from, false, false),
                    (
                        "to",
                        &form.to,
                        form.active == SendField::To,
                        form.to_editable,
                    ),
                    ("amount", &form.amount, form.active == SendField::Amount, true),
                    ("fee", &form.fee, form.active == SendField::Fee, true),
                    ("confirm", &form.confirm, form.active == SendField::Confirm, true),
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
                        "to" => form.to_cursor,
                        "amount" => form.amount_cursor,
                        "fee" => form.fee_cursor,
                        "confirm" => form.confirm_cursor,
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
                    "amount/fee: decimal PWM allowed (scale 1 PWM = 1_000_000 base units, max 6 decimals)",
                ));
                text.lines.push(Line::from(""));
                text.lines.push(if form.status_is_error {
                    Line::from(vec![
                        Span::raw("status: "),
                        Span::styled(form.status.clone(), Style::default().fg(Color::Red)),
                    ])
                } else {
                    Line::from(format!("status: {}", form.status))
                });
                f.render_widget(
                    Paragraph::new(text)
                        .block(Block::default().borders(Borders::ALL).title("Send")),
                    area,
                );
            }

            if let Some(bp) = book_prompt.as_ref() {
                let area = centered_rect(72, 40, f.size());
                f.render_widget(Clear, area);
                let shown = value_with_caret(&bp.label_line, bp.label_cursor, true);
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

            if let Some(um) = unlock_modal.as_ref() {
                let area = centered_rect(62, 38, f.size());
                f.render_widget(Clear, area);
                let shown = masked_with_caret(&um.passphrase, um.pass_cursor);
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
                    Paragraph::new(body)
                        .block(Block::default().borders(Borders::ALL).title("Unlock")),
                    area,
                );
            }

            if let Some(em) = encrypt_modal.as_ref() {
                let area = centered_rect(68, 46, f.size());
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
                let pass_shown = masked_with_caret(&em.passphrase, em.pass_cursor);
                let conf_shown = masked_with_caret(&em.confirm, em.confirm_cursor);
                let mut body = Text::from(vec![
                    Line::from(intro),
                    Line::from(""),
                    Line::from(vec![
                        Span::raw(if pass_active { "> " } else { "  " }),
                        Span::raw("passphrase: "),
                        Span::styled(
                            pass_shown,
                            Style::default()
                                .bg(Color::DarkGray)
                                .fg(if pass_active {
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
                            Style::default()
                                .bg(Color::DarkGray)
                                .fg(if !pass_active {
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
                    Paragraph::new(body)
                        .block(Block::default().borders(Borders::ALL).title(title)),
                    area,
                );
            }
        })?;
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

fn main() {
    let mut args = Args::parse();
    if args.wallet.is_none() {
        args.wallet = default_wallet_if_present();
    }
    if let Err(e) = run(args) {
        eprintln!("{}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        choose_identity, default_wallet_candidate, f6_send_form_for_identity,
        identity_lock_status_suffix, load_wallet_identity, merge_rpc_health, move_selection_down,
        move_selection_up, nonce_from_account_response, owner_and_receivers,
        parse_decimal_pwm_units, parse_nonce_json, receiver_table_len, selected_to_receiver,
        validate_encrypt_passphrase_inputs, validate_send_form, wallet_apply_auto_lock,
        wallet_lock_now, wallet_try_unlock_with_passphrase, wallet_unlock_secs_clamped, AcctRow,
        Args, BookPromptModal, BookRecipient, IdentitySource, JsonFetchFailure, RpcHealth,
        SendField, SendForm, WalletIdentity, FALLBACK_MODE_WARNING, FALLBACK_WARN_CHUNK_ROWS,
    };
    use clap::Parser;
    use pwm_core::{types::account_id_to_human, WalletReadHeader};
    use std::path::PathBuf;

    #[test]
    fn validate_send_form_accepts_pretty_addresses() {
        let from = account_id_to_human(&[1u8; 32]);
        let to = account_id_to_human(&[2u8; 32]);
        let mut form = SendForm::new(from, to, true);
        form.amount = "10.5".into();
        form.fee = "0.001".into();
        form.confirm = "yes".into();
        form.active = SendField::Confirm;
        let parsed = validate_send_form(&form).unwrap();
        assert_eq!(parsed.2, 10_500_000);
        assert_eq!(parsed.3, 1_000);
    }

    #[test]
    fn validate_send_form_requires_yes_confirm() {
        let from = account_id_to_human(&[3u8; 32]);
        let to = account_id_to_human(&[4u8; 32]);
        let mut form = SendForm::new(from, to, true);
        form.amount = "1".into();
        form.fee = "0".into();
        form.confirm = "ok".into();
        form.active = SendField::Confirm;
        let err = validate_send_form(&form).unwrap_err();
        assert!(err.contains("confirm"));
    }

    #[test]
    fn validate_send_form_rejects_ambiguous_legacy_pretty_to_input() {
        let from = account_id_to_human(&[3u8; 32]);
        let ambiguous_to =
            "pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000".to_string();
        let mut form = SendForm::new(from, ambiguous_to, true);
        form.amount = "1".into();
        form.fee = "0".into();
        form.confirm = "yes".into();
        form.active = SendField::Confirm;
        let err = validate_send_form(&form).unwrap_err();
        assert!(err.contains("to:"));
        assert!(err.contains("missing '/LO'"));
        assert!(err.contains("strict pretty"));
    }

    #[test]
    fn parse_decimal_pwm_units_accepts_integer_and_fraction() {
        assert_eq!(parse_decimal_pwm_units("12").unwrap(), 12_000_000);
        assert_eq!(parse_decimal_pwm_units("12.34").unwrap(), 12_340_000);
        assert_eq!(parse_decimal_pwm_units("0.001").unwrap(), 1_000);
        assert_eq!(parse_decimal_pwm_units("0.000001").unwrap(), 1);
    }

    #[test]
    fn parse_decimal_pwm_units_rejects_invalid_and_overprecise_values() {
        let bad = ["", " ", "-1", "abc", "1.", ".1", "1.1234567", "1,23"];
        for v in bad {
            assert!(
                parse_decimal_pwm_units(v).is_err(),
                "expected invalid value: {v}"
            );
        }
    }

    #[test]
    fn send_form_fixed_to_skips_to_field_editing() {
        let mut form = SendForm::new("from".into(), "to".into(), false);
        assert_eq!(form.active, SendField::Amount);
        assert!(form.active_state_mut().is_some());
        form.prev_field();
        assert_eq!(form.active, SendField::Confirm);
        form.next_field();
        assert_eq!(form.active, SendField::Amount);
    }

    #[test]
    fn send_form_new_recipient_starts_with_editable_to() {
        let mut form = SendForm::new("from".into(), String::new(), true);
        assert_eq!(form.active, SendField::To);
        for c in "pwm1-".chars() {
            form.insert_char(c);
        }
        assert_eq!(form.to, "pwm1-");
    }

    #[test]
    fn send_form_inline_edit_supports_cursor_navigation_and_mid_string_ops() {
        let mut form = SendForm::new("from".into(), "abcd".into(), true);
        assert_eq!(form.active, SendField::To);
        form.move_left();
        form.move_left();
        form.insert_char('X');
        assert_eq!(form.to, "abXcd");
        form.backspace();
        assert_eq!(form.to, "abcd");
        form.move_left();
        form.delete();
        assert_eq!(form.to, "acd");
        form.move_home();
        form.insert_char('0');
        assert_eq!(form.to, "0acd");
        form.move_end();
        form.insert_char('9');
        assert_eq!(form.to, "0acd9");
    }

    #[test]
    fn book_prompt_inline_edit_supports_cursor_navigation_and_mid_string_ops() {
        let mut bp = BookPromptModal::new("pwm1-test".into());
        for c in "abcd".chars() {
            bp.insert_char(c);
        }
        assert_eq!(bp.label_line, "abcd");
        assert_eq!(bp.label_cursor, 4);
        bp.move_left();
        bp.move_left();
        bp.insert_char('X');
        assert_eq!(bp.label_line, "abXcd");
        bp.backspace();
        assert_eq!(bp.label_line, "abcd");
        bp.move_left();
        bp.delete();
        assert_eq!(bp.label_line, "acd");
        bp.move_home();
        bp.insert_char('0');
        assert_eq!(bp.label_line, "0acd");
        bp.move_end();
        bp.insert_char('9');
        assert_eq!(bp.label_line, "0acd9");
        assert_eq!(bp.label_cursor, bp.label_line.len());
    }

    #[test]
    fn nonce_json_policy_falls_back_to_zero_for_invalid_or_missing_nonce() {
        assert_eq!(parse_nonce_json("{\"nonce\": 7}"), Some(7));
        assert_eq!(parse_nonce_json("{\"nonce\":\"12\"}"), Some(12));
        assert_eq!(parse_nonce_json("{\"nonce\":\"bad\"}"), None);
        assert_eq!(parse_nonce_json("{\"height\":1}"), None);
        assert_eq!(parse_nonce_json("not-json"), None);
    }

    #[test]
    fn nonce_http_policy_uses_zero_for_non_success_or_bad_json() {
        assert_eq!(nonce_from_account_response(false, "{\"nonce\": 99}"), 0);
        assert_eq!(nonce_from_account_response(true, "not-json"), 0);
        assert_eq!(nonce_from_account_response(true, "{\"nonce\": 5}"), 5);
    }

    #[test]
    fn receivers_panel_includes_new_recipient_row() {
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

    #[test]
    fn receiver_selection_reaches_last_row_without_overflow() {
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

    #[test]
    fn owner_selection_boundaries_are_symmetric() {
        let mut sel = 0usize;
        move_selection_down(&mut sel, 1);
        assert_eq!(sel, 0);
        move_selection_up(&mut sel);
        assert_eq!(sel, 0);
        move_selection_down(&mut sel, 0);
        assert_eq!(sel, 0);
    }

    #[test]
    fn choose_identity_without_wallet_is_fallback_with_empty_footer_note() {
        let args = Args::parse_from(["pwm-tui"]);
        let (id, note) = choose_identity(&args, 300).unwrap();
        assert!(matches!(id, IdentitySource::SeedFallback));
        assert!(note.is_empty());
    }

    #[test]
    fn default_wallet_candidate_finds_default_yml() {
        let td = std::env::temp_dir().join(format!("pwm-tui-default-yml-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&td);
        std::fs::create_dir_all(&td).unwrap();
        std::fs::write(td.join("default.yml"), b"x").unwrap();
        let got = default_wallet_candidate(&td);
        assert_eq!(got, Some(td.join("default.yml")));
        let _ = std::fs::remove_dir_all(&td);
    }

    #[test]
    fn default_wallet_candidate_none_if_missing() {
        let td = std::env::temp_dir().join(format!("pwm-tui-no-default-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&td);
        std::fs::create_dir_all(&td).unwrap();
        assert!(default_wallet_candidate(&td).is_none());
        let _ = std::fs::remove_dir_all(&td);
    }

    #[test]
    fn fallback_mode_warning_constant_matches_operator_text() {
        assert_eq!(
            FALLBACK_MODE_WARNING,
            "FALLBACK MODE: wallet not provided, owner derived from seed/default path"
        );
    }

    #[test]
    fn fallback_warn_chunk_uses_fixed_length_not_min() {
        assert!(
            FALLBACK_WARN_CHUNK_ROWS >= 3 && FALLBACK_WARN_CHUNK_ROWS <= 8,
            "WARNING chunk: compact Length() slot, still room for borders/title/1–2 wrapped lines"
        );
    }

    #[test]
    fn merge_rpc_health_keeps_worst_state() {
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

    #[test]
    fn rpc_health_mapping_from_failure_is_stable() {
        assert_eq!(
            super::rpc_health_from_failure(JsonFetchFailure::Timeout),
            RpcHealth::Timeout
        );
        assert_eq!(
            super::rpc_health_from_failure(JsonFetchFailure::Other),
            RpcHealth::Offline
        );
    }

    #[test]
    fn wallet_unlock_secs_default_is_300() {
        let args = Args::parse_from(["pwm-tui"]);
        assert_eq!(args.wallet_unlock_secs, 300);
        assert_eq!(wallet_unlock_secs_clamped(&args), 300);
    }

    #[test]
    fn wallet_unlock_secs_cli_override_is_used() {
        let args = Args::parse_from(["pwm-tui", "--wallet-unlock-secs", "42"]);
        assert_eq!(args.wallet_unlock_secs, 42);
        assert_eq!(wallet_unlock_secs_clamped(&args), 42);
    }

    #[test]
    fn wallet_unlock_secs_env_override_is_used() {
        // SAFETY: tests in this module do not spawn threads that depend on this env var.
        unsafe { std::env::set_var("PWM_TUI_WALLET_UNLOCK_SECS", "77") };
        let args = Args::parse_from(["pwm-tui"]);
        assert_eq!(args.wallet_unlock_secs, 77);
        assert_eq!(wallet_unlock_secs_clamped(&args), 77);
        // SAFETY: cleanup for test isolation.
        unsafe { std::env::remove_var("PWM_TUI_WALLET_UNLOCK_SECS") };
    }

    #[test]
    fn wallet_unlock_secs_is_clamped_to_valid_bounds() {
        let low = Args::parse_from(["pwm-tui", "--wallet-unlock-secs", "0"]);
        assert_eq!(wallet_unlock_secs_clamped(&low), 1);
        let high = Args::parse_from(["pwm-tui", "--wallet-unlock-secs", "999999999"]);
        assert_eq!(wallet_unlock_secs_clamped(&high), 604_800);
    }

    #[test]
    fn owner_and_receivers_prefers_wallet_owner() {
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
            address_book: vec![],
            encryption_prompt_hint: None,
            ignored_legacy_pretty_entries: 0,
            secret_payload_plaintext: None,
        });
        let (owner_row, receivers) = owner_and_receivers(&rows, &identity);
        assert_eq!(owner_row.unwrap().id, owner);
        assert_eq!(receivers.len(), 1);
        assert_eq!(receivers[0].id, other);
    }

    #[test]
    fn owner_and_receivers_uses_wallet_address_book() {
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
            secret_payload_plaintext: None,
        });
        let (owner_row, receivers) = owner_and_receivers(&rows, &identity);
        assert_eq!(owner_row.unwrap().id, owner);
        assert_eq!(receivers.len(), 2);
        assert_eq!(receivers[0].id, book_a);
        assert_eq!(receivers[0].balance_pwm, 3);
        assert_eq!(receivers[1].id, book_b);
        assert_eq!(receivers[1].balance_pwm, 0);
    }

    #[test]
    fn owner_and_receivers_keeps_regulatory_lo_zero_in_receivers() {
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
            address_book: vec![],
            encryption_prompt_hint: None,
            ignored_legacy_pretty_entries: 0,
            secret_payload_plaintext: None,
        });
        let (_owner_row, receivers) = owner_and_receivers(&rows, &identity);
        assert_eq!(receivers.len(), 2);
        assert_eq!(receivers[0].id, lo_zero);
        assert_eq!(receivers[1].id, allowed);
    }

    #[test]
    fn wallet_upgrade_encryption_hook_only_for_upgraded_plaintext() {
        let plain = WalletReadHeader {
            mode: "plaintext_dev".into(),
            derivation_index: 1,
            derivation_path: Some("m/0/1".into()),
            domain_u16: 0x2C00,
            account_id_hex: None,
            account_id_human: account_id_to_human(&[1u8; 32]),
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
        assert!(super::wallet_upgrade_encryption_hook(&plain, true).is_some());
        assert!(super::wallet_upgrade_encryption_hook(&plain, false).is_none());
        let mut enc = plain.clone();
        enc.mode = "encrypted".into();
        assert!(super::wallet_upgrade_encryption_hook(&enc, true).is_none());
    }

    fn seal_test_enc_wallet(passphrase: &str, json: &[u8]) -> (String, String, String) {
        let s = pwm_core::seal_wallet_secret_plaintext(json, passphrase).unwrap();
        (s.kdf_salt_b64, s.aead_nonce_b64, s.encrypted_payload_b64)
    }

    #[test]
    fn encrypted_wallet_auto_lock_clears_signing_key() {
        let w = WalletIdentity {
            account_id: [1u8; 32],
            account_id_human: "pwm1-test".into(),
            domain: 1,
            derivation_index: 0,
            signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[2u8; 32])),
            unlock_expires_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(10)),
            wallet_is_encrypted: true,
            wallet_path: PathBuf::from("_unused_"),
            address_book: vec![],
            encryption_prompt_hint: None,
            ignored_legacy_pretty_entries: 0,
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

    #[test]
    fn encrypted_wallet_manual_lock_clears_sensitive_state() {
        let mut w = WalletIdentity {
            account_id: [4u8; 32],
            account_id_human: "pwm1-test".into(),
            domain: 1,
            derivation_index: 0,
            signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[8u8; 32])),
            unlock_expires_at: Some(std::time::Instant::now() + std::time::Duration::from_secs(60)),
            wallet_is_encrypted: true,
            wallet_path: PathBuf::from("_unused_"),
            address_book: vec![],
            encryption_prompt_hint: None,
            ignored_legacy_pretty_entries: 0,
            secret_payload_plaintext: Some(vec![7, 8, 9]),
        };
        wallet_lock_now(&mut w);
        assert!(w.signing_key.is_none());
        assert!(w.secret_payload_plaintext.is_none());
        assert!(w.unlock_expires_at.is_none());
    }

    #[test]
    fn load_encrypted_wallet_without_cli_passphrase_then_unlock_and_auto_lock() {
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
        let path =
            std::env::temp_dir().join(format!("pwm-tui-enc-wallet-{}.yml", std::process::id()));
        std::fs::write(&path, yaml).unwrap();
        let mut idw = load_wallet_identity(&path, None, 300).expect("load locked");
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

    #[test]
    fn choose_identity_encrypted_without_passphrase_mentions_f3() {
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

    #[test]
    fn identity_lock_suffix_empty_for_plaintext_wallet() {
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
            address_book: vec![],
            encryption_prompt_hint: None,
            ignored_legacy_pretty_entries: 0,
            secret_payload_plaintext: None,
        };
        assert!(identity_lock_status_suffix(&IdentitySource::Wallet(w)).is_empty());
    }

    #[test]
    fn ellipsis_middle_ascii_short_round_trip() {
        assert_eq!(super::ellipsis_middle_ascii("abc", 2, 2), "abc");
    }

    #[test]
    fn ellipsis_middle_ascii_long() {
        let s = "012345678901234567890";
        assert_eq!(super::ellipsis_middle_ascii(s, 4, 4), "0123...7890");
    }

    #[test]
    fn format_footer_head_line_keeps_short_tip() {
        let s = "height=2 tip=deadbeef";
        assert_eq!(super::format_footer_head_line(s), s);
    }

    #[test]
    fn format_footer_head_line_truncates_long_tip() {
        let tip = "ab".repeat(40);
        let head = format!("height=9 tip={tip}");
        let got = super::format_footer_head_line(&head);
        assert!(got.contains("..."));
        assert!(got.starts_with("height=9 tip="));
        assert!(got.len() < head.len());
    }

    #[test]
    fn status_footer_line_rpc_offline_leads_then_poll_err() {
        use ratatui::style::Color;
        let line = super::status_footer_line(
            "height=1 tip=x",
            "accounts: offline",
            "wallet: ok",
            "lock",
            super::RpcHealth::Offline,
            true,
            "http://example:3030",
        );
        assert_eq!(line.spans[0].content, "RPC offline");
        assert_eq!(line.spans[0].style.fg, Some(Color::Red));
        let flat: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            flat.starts_with("RPC offline | accounts: offline | height=1 tip=x | Tab switch"),
            "unexpected order/prefix: {flat}"
        );
        assert!(
            flat.contains("F3 lock"),
            "footer should advertise F3: {flat}"
        );
        assert!(flat.contains("F4 encrypt"), "{flat}");
        assert!(flat.contains("PWM_TUI_DEBUG=1"));
        assert!(flat.contains("wallet: ok"));
    }

    #[test]
    fn status_footer_line_online_single_segment_without_red() {
        use ratatui::style::Color;
        let line = super::status_footer_line(
            "…",
            "",
            "",
            "unlock",
            super::RpcHealth::Online,
            false,
            "http://127.0.0.1:3030",
        );
        assert_eq!(line.spans.len(), 1);
        assert_ne!(line.spans[0].style.fg, Some(Color::Red));
        let flat = line.spans[0].content.as_ref();
        assert!(flat.starts_with("… | Tab switch"), "{flat}");
    }

    #[test]
    fn wallet_encrypt_disk_rejects_rekey_without_decrypted_material() {
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
        let err = super::wallet_encrypt_or_rekey_disk(&path, "newpw", None).expect_err("must fail");
        assert!(
            err.contains("unlock") || err.contains("Passphrase"),
            "err={err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn submit_transfer_fails_fast_when_encrypted_wallet_locked() {
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
            address_book: vec![],
            encryption_prompt_hint: None,
            ignored_legacy_pretty_entries: 0,
            secret_payload_plaintext: None,
        });
        let err =
            super::submit_transfer(&from, &to, 1_000_000, 0, &id).expect_err("must be locked");
        assert!(err.contains("wallet is locked"), "err={err}");
    }

    #[test]
    fn f6_path_after_unlock_timeout_shows_locked_modal_message() {
        let mut id = IdentitySource::Wallet(WalletIdentity {
            account_id: [50u8; 32],
            account_id_human: account_id_to_human(&[50u8; 32]),
            domain: 0x2C00,
            derivation_index: 0,
            signing_key: Some(ed25519_dalek::SigningKey::from_bytes(&[51u8; 32])),
            unlock_expires_at: Some(std::time::Instant::now() - std::time::Duration::from_secs(1)),
            wallet_is_encrypted: true,
            wallet_path: PathBuf::from("unused.yml"),
            address_book: vec![],
            encryption_prompt_hint: None,
            ignored_legacy_pretty_entries: 0,
            secret_payload_plaintext: Some(vec![1, 2, 3]),
        });
        wallet_apply_auto_lock(&mut id);
        let err = match f6_send_form_for_identity(&id, None, &[], 0) {
            Ok(_) => panic!("must stay locked"),
            Err(e) => e,
        };
        assert!(
            err.contains("Wallet is locked"),
            "F6 must use locked-wallet path, err={err}"
        );
    }

    #[test]
    fn wallet_encrypt_plaintext_yml_to_encrypted_roundtrip() {
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
        let path =
            std::env::temp_dir().join(format!("pwm-tui-plain-enc-{}.yml", std::process::id()));
        std::fs::write(&path, yaml).unwrap();
        super::wallet_encrypt_or_rekey_disk(&path, "encrypt-me", None).expect("encrypt");
        let id = load_wallet_identity(&path, Some("encrypt-me"), 300).expect("reload encrypted");
        assert!(id.wallet_is_encrypted);
        assert!(id.signing_key.is_some());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wallet_rekey_roundtrip_requires_new_passphrase_after_reload() {
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

        let unlocked = load_wallet_identity(&path, Some("old-pass"), 300).expect("unlock old");
        let decrypted = unlocked
            .secret_payload_plaintext
            .clone()
            .expect("decrypted payload must be cached after unlock");
        super::wallet_encrypt_or_rekey_disk(&path, "new-pass", Some(decrypted.as_slice()))
            .expect("rekey");

        let old_err = match load_wallet_identity(&path, Some("old-pass"), 300) {
            Ok(_) => panic!("old passphrase must fail after re-key"),
            Err(e) => e,
        };
        assert!(old_err.contains("failed to decrypt"), "old_err={old_err}");
        let reloaded = load_wallet_identity(&path, Some("new-pass"), 300).expect("new must load");
        assert!(reloaded.wallet_is_encrypted);
        assert!(reloaded.signing_key.is_some());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wallet_rekey_corrupted_ciphertext_fails_safely() {
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

        let unlocked = load_wallet_identity(&path, Some("old-pass"), 300).expect("unlock old");
        let decrypted = unlocked
            .secret_payload_plaintext
            .clone()
            .expect("decrypted payload must be cached after unlock");
        super::wallet_encrypt_or_rekey_disk(&path, "new-pass", Some(decrypted.as_slice()))
            .expect("rekey");

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

        let err = match load_wallet_identity(&path, Some("new-pass"), 300) {
            Ok(_) => panic!("corrupted encrypted payload must fail safely"),
            Err(e) => e,
        };
        assert!(
            err.contains("encrypted_payload_b64:") || err.contains("failed to decrypt"),
            "err={err}"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn op_status_labels_are_stable() {
        assert_eq!(super::OpStatus::Pending.as_str(), "pending");
        assert_eq!(super::OpStatus::Ok.as_str(), "ok");
        assert_eq!(super::OpStatus::Error.as_str(), "error");
    }

    #[test]
    fn op_history_insert_caps_to_max() {
        let mut hist = Vec::new();
        for i in 0..(super::OP_HISTORY_MAX_ITEMS + 5) {
            super::push_op_history(
                &mut hist,
                super::OperationHistoryEntry {
                    req_id: i as u64,
                    created_unix_secs: i as u64,
                    from_human: "from".into(),
                    to_human: "to".into(),
                    amount_units: 1,
                    fee_units: 0,
                    status: super::OpStatus::Pending,
                    note: "queued".into(),
                },
            );
        }
        assert_eq!(hist.len(), super::OP_HISTORY_MAX_ITEMS);
        assert_eq!(hist[0].req_id, (super::OP_HISTORY_MAX_ITEMS + 4) as u64);
    }

    #[test]
    fn set_op_history_status_updates_matching_req() {
        let mut hist = vec![super::OperationHistoryEntry {
            req_id: 17,
            created_unix_secs: 1,
            from_human: "f".into(),
            to_human: "t".into(),
            amount_units: 7,
            fee_units: 1,
            status: super::OpStatus::Pending,
            note: "submitting".into(),
        }];
        let changed =
            super::set_op_history_status(&mut hist, 17, super::OpStatus::Ok, "sent".into());
        assert!(changed);
        assert_eq!(hist[0].status, super::OpStatus::Ok);
        assert_eq!(hist[0].note, "sent");
    }

    #[test]
    fn submit_done_updates_history_even_when_form_closed() {
        let mut inflight_send_req_id = Some(42_u64);
        let mut hist = vec![super::OperationHistoryEntry {
            req_id: 42,
            created_unix_secs: 1,
            from_human: "f".into(),
            to_human: "t".into(),
            amount_units: 7,
            fee_units: 1,
            status: super::OpStatus::Pending,
            note: "submitting".into(),
        }];
        // Form is closed (None), but SubmitDone must still resolve op_history.
        let changed = super::handle_submit_done_history(
            &mut inflight_send_req_id,
            &mut hist,
            42,
            &Ok("sent".into()),
        );
        assert!(changed);
        assert_eq!(inflight_send_req_id, None);
        assert_eq!(hist[0].status, super::OpStatus::Ok);
        assert_eq!(hist[0].note, "sent");
    }

    #[test]
    fn validate_encrypt_passphrase_inputs_rejects_empty_and_mismatch() {
        let empty = validate_encrypt_passphrase_inputs("", "x").expect_err("must reject empty");
        assert_eq!(empty, "passphrase must not be empty");
        let mismatch = validate_encrypt_passphrase_inputs("abc", "xyz").expect_err("must reject");
        assert_eq!(mismatch, "passphrases do not match");
        assert!(validate_encrypt_passphrase_inputs("ok", "ok").is_ok());
    }
}
