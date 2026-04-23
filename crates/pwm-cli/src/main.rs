//! Wallet CLI: keys, cluster derive, submit txs to `pwmd`.

mod bruteforce;
mod wallet;

use crate::bruteforce::{
    brute_force_domain_flags_with_progress,
    brute_force_domain_flags_with_progress_and_match_policy, format_eta_human, BruteforceProgress,
    DomainMatchMode,
};
use crate::wallet::{
    assert_tx_recipient_in_wallet_address_book, backup_wallet_file, load_wallet_yaml,
    recover_wallet_file, save_wallet_yaml, to_wallet_yaml, to_wallet_yaml_with_metadata,
    wallet_address_book_add, wallet_address_book_remove, wallet_secrets, WalletProtection,
    WalletSecrets, WalletYaml,
};
use clap::{Parser, Subcommand};
use ed25519_dalek::SigningKey;
use pwm_core::domain_index::{DomainCategory, DomainEntry};
use pwm_core::hd::brute_cluster_address;
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::{
    account_id_to_bech32dx, account_id_to_human, format_domain_for_display, merkle_root,
    parse_account_id, parse_account_id_for_user_input, sign_batch,
    validate_recipient_domain_policy, AccountId,
};
use rand::RngCore;
use std::path::PathBuf;
use std::process;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "pwm")]
struct Cli {
    /// pwmd base URL (trailing slash optional). Same as env `PWM_RPC`.
    #[arg(
        long,
        global = true,
        env = "PWM_RPC",
        default_value = "http://127.0.0.1:3030"
    )]
    rpc: String,
    /// Wallet encryption passphrase for reading encrypted wallet files. Same as env `PWM_WALLET_PASSPHRASE`.
    #[arg(long, global = true, env = "PWM_WALLET_PASSPHRASE")]
    wallet_passphrase: Option<String>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print random 32-byte master seed (hex).
    KeyGen,
    /// Brute cluster address for `--domain` (hex u16).
    #[command(name = "addr-derive", visible_alias = "addr-der")]
    AddrDer {
        #[arg(long)]
        master: String,
        #[arg(long)]
        domain: String,
        #[arg(long, default_value_t = 500_000)]
        max_try: u32,
    },
    /// Single-thread linear bruteforce by domain + flags mask with wallet save.
    #[command(name = "addr-bruteforce")]
    AddrBruteforce {
        #[arg(long)]
        master: String,
        #[arg(
            long,
            help = "Domain label from pwm_core::domain_index. Phase1 user profile accepts country labels only (e.g. CY, US)"
        )]
        domain: String,
        #[arg(
            long,
            help = "Mask for expected flags (Phase1 user profile: low 10 bits only)"
        )]
        flags_mask: u32,
        #[arg(
            long = "expected-flags",
            alias = "expected-result",
            help = "Expected masked flags value (Phase1 user profile: low 10 bits only; alias: --expected-result)"
        )]
        expected_flags: u32,
        #[arg(long, default_value_t = 500_000)]
        max_try: u32,
        #[arg(long)]
        wallet_out: PathBuf,
    },
    /// POST signed INIT to pwmd.
    TxInit {
        #[arg(
            long,
            help = "Wallet YAML path (primary signing source). Used unless --master override is provided.",
            required_unless_present = "master"
        )]
        wallet: Option<PathBuf>,
        #[arg(
            long,
            help = "Dev override for signing source. When set, signer is derived from master+domain instead of wallet.",
            requires = "domain"
        )]
        master: Option<String>,
        #[arg(
            long,
            help = "Sender domain for --master override (label/raw as before).",
            requires = "master"
        )]
        domain: Option<String>,
        #[arg(long, default_value_t = 0)]
        index: u32,
        #[arg(long, default_value_t = 0)]
        flags: u32,
    },
    /// Demo: Merkle root over two leaves + provider Ed25519 sig (stdout JSON).
    OffDemo,
    /// POST signed TRANSFER.
    TxSend {
        #[arg(
            long,
            help = "Wallet YAML path (primary signing source). Used unless --master override is provided.",
            required_unless_present = "master"
        )]
        wallet: Option<PathBuf>,
        #[arg(
            long,
            help = "Dev override for signing source. When set, signer is derived from master+domain instead of wallet. Also skips the wallet address_book allow-list check for --to (non-empty book is enforced only with --wallet and without --master).",
            requires = "domain"
        )]
        master: Option<String>,
        #[arg(
            long,
            help = "Sender domain for --master override (label/raw as before).",
            requires = "master"
        )]
        domain: Option<String>,
        #[arg(
            long,
            help = "Recipient address. Accepted: pretty (pwm1-LABEL-f<flags8hex>-t<tail52hex>), canonical bech32DX (pwm1...), legacy hex, legacy PWMv0-hex"
        )]
        to: String,
        #[arg(long)]
        amount: Option<u128>,
        #[arg(long, default_value_t = 1)]
        fee: u128,
    },
    /// POST signed STAKE.
    TxStake {
        #[arg(
            long,
            help = "Wallet YAML path (primary signing source). Used unless --master override is provided.",
            required_unless_present = "master"
        )]
        wallet: Option<PathBuf>,
        #[arg(
            long,
            help = "Dev override for signing source. When set, signer is derived from master+domain instead of wallet.",
            requires = "domain"
        )]
        master: Option<String>,
        #[arg(
            long,
            help = "Sender domain for --master override (label/raw as before).",
            requires = "master"
        )]
        domain: Option<String>,
        #[arg(long)]
        amount: u128,
    },
    /// POST signed UNSTAKE.
    TxUnstake {
        #[arg(
            long,
            help = "Wallet YAML path (primary signing source). Used unless --master override is provided.",
            required_unless_present = "master"
        )]
        wallet: Option<PathBuf>,
        #[arg(
            long,
            help = "Dev override for signing source. When set, signer is derived from master+domain instead of wallet.",
            requires = "domain"
        )]
        master: Option<String>,
        #[arg(
            long,
            help = "Sender domain for --master override (label/raw as before).",
            requires = "master"
        )]
        domain: Option<String>,
        #[arg(long)]
        amount: u128,
    },
    /// POST signed BURN_MARK.
    TxBurnMark {
        #[arg(
            long,
            help = "Wallet YAML path (primary signing source). Used unless --master override is provided.",
            required_unless_present = "master"
        )]
        wallet: Option<PathBuf>,
        #[arg(
            long,
            help = "Dev override for signing source. When set, signer is derived from master+domain instead of wallet.",
            requires = "domain"
        )]
        master: Option<String>,
        #[arg(
            long,
            help = "Sender domain for --master override (label/raw as before).",
            requires = "master"
        )]
        domain: Option<String>,
        #[arg(long)]
        mark_amount: u128,
        #[arg(
            long,
            help = "Optional beneficiary address. Accepted: pretty (pwm1-LABEL-f<flags8hex>-t<tail52hex>), canonical bech32DX (pwm1...), legacy hex, legacy PWMv0-hex"
        )]
        beneficiary: Option<String>,
    },
    /// Wallet file operations.
    Wallet {
        #[command(subcommand)]
        cmd: WalletCmd,
    },
}

#[derive(Subcommand)]
enum WalletCmd {
    /// Initialize user-mode wallet with country label and one bruteforce hit.
    Init {
        #[arg(
            long,
            help = "Country code label (Phase1 regulatory label from pwm_core::domain_index, e.g. CY, US)"
        )]
        country: String,
        #[arg(
            long,
            help = "Optional 32-byte master seed hex. If omitted, a random seed is generated."
        )]
        master: Option<String>,
        #[arg(long, default_value_t = 500_000)]
        max_try: u32,
        #[arg(long)]
        wallet_out: PathBuf,
        #[arg(
            long,
            help = "Store wallet secrets in plaintext for local dev only (explicit opt-in)."
        )]
        plaintext_dev: bool,
    },
    /// Import existing 32-byte seed and initialize user-mode wallet.
    ImportSeed {
        #[arg(
            long,
            help = "Country code label (Phase1 regulatory label from pwm_core::domain_index, e.g. CY, US)"
        )]
        country: String,
        #[arg(long, help = "32-byte master seed hex.")]
        master: String,
        #[arg(long, default_value_t = 500_000)]
        max_try: u32,
        #[arg(long)]
        wallet_out: PathBuf,
        #[arg(
            long,
            help = "Store wallet secrets in plaintext for local dev only (explicit opt-in)."
        )]
        plaintext_dev: bool,
    },
    /// Show wallet metadata and verify decrypt path.
    Show {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(
            long,
            help = "Unsafe debug mode: reveal wallet secrets (master/signing keys) in stdout."
        )]
        unsafe_show_secrets: bool,
    },
    /// Add a recipient to `address_book` (allow-list for `tx-send --wallet`).
    #[command(name = "book-add")]
    BookAdd {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(
            long,
            help = "Recipient in the same formats as `tx-send --to` (pretty / canonical / legacy hex)"
        )]
        address: String,
        #[arg(
            long,
            help = "Optional label stored with the entry (shown in pwm-tui)."
        )]
        label: Option<String>,
    },
    /// List `address_book` entries (normalized pretty).
    #[command(name = "book-list")]
    BookList {
        #[arg(long)]
        wallet: PathBuf,
    },
    /// Remove a recipient from `address_book` (same address formats as `book-add`).
    #[command(name = "book-remove")]
    BookRemove {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        address: String,
    },
    /// Create a validated wallet backup copy.
    Backup {
        #[arg(long, help = "Source wallet path")]
        wallet: PathBuf,
        #[arg(long, help = "Destination backup path")]
        out: PathBuf,
    },
    /// Restore wallet from backup with payload validation.
    Recover {
        #[arg(long, help = "Backup wallet path")]
        backup: PathBuf,
        #[arg(long, help = "Destination restored wallet path")]
        out: PathBuf,
    },
}

fn hex32(s: &str) -> Result<[u8; 32], String> {
    let v = hex::decode(s.trim()).map_err(|e| e.to_string())?;
    if v.len() != 32 {
        return Err("need 32 bytes hex".into());
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    Ok(a)
}

fn parse_domain(s: &str) -> Result<u16, String> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u16::from_str_radix(hex, 16).map_err(|e| e.to_string());
    }
    if t.chars()
        .any(|c| c.is_ascii_hexdigit() && c.is_ascii_alphabetic())
    {
        return u16::from_str_radix(t, 16).map_err(|e| e.to_string());
    }
    t.parse::<u16>()
        .or_else(|_| u16::from_str_radix(t, 16))
        .map_err(|e| e.to_string())
}

fn parse_domain_label_only(s: &str) -> Result<&'static DomainEntry, String> {
    let input = s.trim();
    if input.is_empty() {
        return Err("domain label is required".into());
    }
    if input.starts_with("0x")
        || input.starts_with("0X")
        || input.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "numeric domain input is not allowed for addr-bruteforce: '{input}'. Use a domain label from pwm_core::domain_index (e.g. CY, MSFT)"
        ));
    }
    pwm_core::domain_index::lookup_by_label(input).ok_or_else(|| {
            format!(
                "unknown domain label '{input}'. Use a label from pwm_core::domain_index (e.g. CY, MSFT)"
            )
        })
}

fn validate_user_profile_flags(flags_mask: u32, expected_flags: u32) -> Result<(), String> {
    const USER_FLAGS_MASK: u32 = 0x03FF;
    if (flags_mask & !USER_FLAGS_MASK) != 0 {
        return Err(format!(
            "Phase1 user profile allows only low 10 bits in --flags-mask (max 0x03FF), got 0x{flags_mask:08X}"
        ));
    }
    if (expected_flags & !USER_FLAGS_MASK) != 0 {
        return Err(format!(
            "Phase1 user profile allows only low 10 bits in --expected-flags (max 0x03FF), got 0x{expected_flags:08X}"
        ));
    }
    if (expected_flags & !flags_mask) != 0 {
        return Err("expected_flags must not set bits outside flags_mask".to_string());
    }
    Ok(())
}

fn master_seed(s: &str) -> Result<[u8; 32], String> {
    hex32(s)
}

const ADDRESS_FORMAT_HINT: &str =
    "pretty pwm1-<label_or_$hex!>-f<flags8hex>-t<tail52hex>, canonical pwm1..., legacy PWMv0-... / hex";

fn parse_address_arg(field: &str, value: &str) -> Result<AccountId, String> {
    parse_account_id_for_user_input(value).map_err(|e| {
        format!(
            "Invalid value for {field}: '{value}'. Accepted formats: {ADDRESS_FORMAT_HINT}. Parse details: {e}"
        )
    })
}

fn parse_amount_value(field: &str, value: &str) -> Result<u128, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} value must not be empty"));
    }
    trimmed
        .parse::<u128>()
        .map_err(|_| format!("{field} value must be an unsigned integer, got '{value}'"))
}

/// Accept plain address input and URI form `pwm:<address>?amount=<u128>`.
fn parse_address_input(field: &str, value: &str) -> Result<(AccountId, Option<u128>), String> {
    let trimmed = value.trim();
    if !trimmed.starts_with("pwm:") {
        return parse_address_arg(field, trimmed).map(|id| (id, None));
    }
    let rest = &trimmed["pwm:".len()..];
    if rest.is_empty() {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: missing address after 'pwm:'"
        ));
    }
    if rest.starts_with("//") {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: authority form is not supported; expected 'pwm:<address>'"
        ));
    }
    if rest.contains('#') {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: fragments are not supported"
        ));
    }
    let (address_part, query_part) = match rest.split_once('?') {
        Some(parts) => parts,
        None => (rest, ""),
    };
    if address_part.trim().is_empty() {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: missing address before query"
        ));
    }
    let address = parse_address_arg(field, address_part.trim())?;
    if query_part.is_empty() {
        return Ok((address, None));
    }
    let mut amount: Option<u128> = None;
    for pair in query_part.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        if raw_key != "amount" {
            return Err(format!(
                "Invalid value for {field}: '{value}'. unsupported pwm URI query parameter '{raw_key}'"
            ));
        }
        if amount.is_some() {
            return Err(format!(
                "Invalid value for {field}: '{value}'. duplicate 'amount' query parameter"
            ));
        }
        amount = Some(parse_amount_value("URI amount", raw_value)?);
    }
    Ok((address, amount))
}

fn resolve_tx_send_amount(
    cli_amount: Option<u128>,
    uri_amount: Option<u128>,
) -> Result<u128, String> {
    match (cli_amount, uri_amount) {
        (Some(cli), Some(uri)) if cli != uri => Err(format!(
            "amount conflict: --amount={cli} differs from URI amount={uri}. Use exactly one source or the same value in both"
        )),
        (Some(cli), Some(_)) => Ok(cli),
        (Some(cli), None) => Ok(cli),
        (None, Some(uri)) => Ok(uri),
        (None, None) => Err("missing amount: provide --amount or use URI query '?amount='".to_string()),
    }
}

fn exit_user_error(msg: &str) -> ! {
    eprintln!("{msg}");
    process::exit(2);
}

fn resolve_wallet_protection(
    wallet_passphrase: Option<&str>,
    plaintext_dev: bool,
) -> Result<WalletProtection, String> {
    if plaintext_dev {
        return Ok(WalletProtection::PlaintextDev);
    }
    let passphrase = wallet_passphrase.ok_or_else(|| {
        "encrypted wallet mode is default. Provide --wallet-passphrase (or PWM_WALLET_PASSPHRASE), or use --plaintext-dev only for explicit local dev mode.".to_string()
    })?;
    if passphrase.trim().is_empty() {
        return Err("wallet passphrase must not be empty".to_string());
    }
    Ok(WalletProtection::Encrypted {
        passphrase: passphrase.to_string(),
    })
}

fn derive_sender(master: &str, domain: &str) -> Result<(SigningKey, u16, u32, AccountId), String> {
    let seed = master_seed(master).map_err(|e| format!("invalid --master: {e}"))?;
    let dom = parse_domain(domain).map_err(|e| format!("invalid --domain: {e}"))?;
    let (sk, _pk, i, from) = brute_cluster_address(&seed, dom, 500_000)
        .ok_or_else(|| "no sender match found in derivation window".to_string())?;
    Ok((sk, dom, i, from))
}

struct TxSignerSource {
    sk: SigningKey,
    dom: u16,
    idx: u32,
    from: AccountId,
}

fn load_sender_from_wallet(
    path: &PathBuf,
    wallet_passphrase: Option<&str>,
) -> Result<TxSignerSource, String> {
    let wallet = load_wallet_yaml(path)
        .map_err(|e| format!("failed to read wallet '{}': {e}", path.display()))?;
    let secrets = wallet_secrets(&wallet, wallet_passphrase)
        .map_err(|e| format!("failed to unlock wallet '{}': {e}", path.display()))?;
    let sk_bytes = hex32(&secrets.signing_key_hex).map_err(|e| {
        format!(
            "invalid signing_key_hex in wallet '{}': {e}",
            path.display()
        )
    })?;
    let from = parse_account_id(&wallet.account_id_hex).map_err(|e| {
        format!(
            "invalid account_id_hex in wallet '{}': {}",
            path.display(),
            e
        )
    })?;
    Ok(TxSignerSource {
        sk: SigningKey::from_bytes(&sk_bytes),
        dom: wallet.domain_u16,
        idx: wallet.derivation_index,
        from,
    })
}

fn load_tx_signer_source(
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
) -> Result<TxSignerSource, String> {
    if let Some(master_hex) = master {
        let domain_str = domain
            .ok_or_else(|| "--domain is required when --master override is set".to_string())?;
        let (sk, dom, idx, from) = derive_sender(&master_hex, &domain_str)?;
        return Ok(TxSignerSource { sk, dom, idx, from });
    }
    let wallet_path =
        wallet.ok_or_else(|| "either --wallet or --master must be provided".to_string())?;
    load_sender_from_wallet(&wallet_path, wallet_passphrase)
}

fn wallet_show_lines(
    doc: &WalletYaml,
    wallet_path: &PathBuf,
    secrets: Option<&WalletSecrets>,
) -> Vec<String> {
    let mut lines = vec![
        format!("wallet_path {}", wallet_path.display()),
        format!("schema_version {}", doc.schema_version),
        format!("wallet_mode {}", doc.mode),
        format!("created_at_unix_sec {}", doc.created_at_unix_sec),
        format!(
            "country_label {}",
            doc.country_code_label.as_deref().unwrap_or("-")
        ),
        format!("derivation_index {}", doc.derivation_index),
        format!(
            "derivation_path {}",
            doc.derivation_path.as_deref().unwrap_or("-")
        ),
        format!("domain_u16 {}", doc.domain_u16),
        format!("flags_mask_u32 {}", doc.flags_mask_u32),
        format!("expected_flags_u32 {}", doc.expected_flags_u32),
        format!("flags_derived_u32 {}", doc.flags_derived_u32),
        format!("account_id_hex {}", doc.account_id_hex),
        format!("account_id_human {}", doc.account_id_human),
    ];
    if doc.address_book.is_empty() {
        lines.push("address_book (empty — tx-send allows any policy-valid recipient)".into());
    } else {
        lines.push(format!("address_book_count {}", doc.address_book.len()));
        for (i, e) in doc.address_book.iter().enumerate() {
            let id = parse_account_id(e.address_str())
                .unwrap_or_else(|_| panic!("normalized address_book contains invalid address"));
            let addr = account_id_to_human(&id);
            if let Some(l) = e.label() {
                lines.push(format!("address_book[{i}] {addr}  label={l}"));
            } else {
                lines.push(format!("address_book[{i}] {addr}"));
            }
        }
    }
    if doc.ignored_legacy_pretty_entries > 0 {
        lines.push(format!(
            "address_book_ignored_legacy_pretty {}",
            doc.ignored_legacy_pretty_entries
        ));
    }
    if let Some(secrets) = secrets {
        lines.push(format!("master_seed_hex {}", secrets.master_seed_hex));
        lines.push(format!("signing_key_hex {}", secrets.signing_key_hex));
        lines.push(format!("verifying_key_hex {}", secrets.verifying_key_hex));
    }
    lines
}

fn fetch_nonce(c: &reqwest::blocking::Client, rpc_base: &str, from: AccountId) -> u64 {
    let from_hex = hex::encode(from);
    let url = format!("{}/v1/account/{}", rpc_base, from_hex);
    match c.get(&url).send() {
        Ok(r) if r.status().is_success() => r
            .json::<serde_json::Value>()
            .ok()
            .and_then(|v| v.get("nonce")?.as_u64())
            .unwrap_or(0),
        _ => 0,
    }
}

fn post_tx(c: &reqwest::blocking::Client, rpc_base: &str, tx: &SignedTx) {
    let r = c
        .post(format!("{}/v1/tx", rpc_base))
        .json(tx)
        .send()
        .expect("http");
    println!("{}", r.status());
}

fn main() {
    let cli = Cli::parse();
    let rpc_base = cli.rpc.trim_end_matches('/').to_string();
    let wallet_passphrase = cli.wallet_passphrase.clone();
    match cli.cmd {
        Cmd::OffDemo => {
            let a = [1u8; 32];
            let b = [2u8; 32];
            let root = merkle_root(&[a, b]);
            let skb = slip10_ed25519::derive_ed25519_private_key(&[5u8; 32], &[]);
            let sk = SigningKey::from_bytes(&skb);
            let sig = sign_batch(&sk, 1u64, root);
            let out = serde_json::json!({
                "batch_id": 1u64,
                "merkle_root_hex": hex::encode(root),
                "sig_hex": hex::encode(sig),
            });
            println!("{}", serde_json::to_string_pretty(&out).unwrap());
        }
        Cmd::KeyGen => {
            let mut s = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut s);
            println!("{}", hex::encode(s));
        }
        Cmd::AddrDer {
            master,
            domain,
            max_try,
        } => {
            let seed = master_seed(&master).expect("master");
            let dom = parse_domain(&domain).expect("domain");
            let r = brute_cluster_address(&seed, dom, max_try).expect("no match");
            let (domain_display, domain_ok) = format_domain_for_display(dom as u32);
            let account_id_pretty = account_id_to_human(&r.3);
            let account_id_bech32dx = account_id_to_bech32dx(&r.3);
            println!("account_id_hex {}", hex::encode(r.3));
            println!("account_id_human {}", account_id_pretty);
            println!("account_id_bech32dx {}", account_id_bech32dx);
            println!("domain_display {}", domain_display);
            println!("domain_known {}", domain_ok);
            println!("derivation_index {}", r.2);
            println!("pubkey_hex {}", hex::encode(r.1));
        }
        Cmd::AddrBruteforce {
            master,
            domain,
            flags_mask,
            expected_flags,
            max_try,
            wallet_out,
        } => {
            let seed = master_seed(&master).expect("master");
            let domain_entry = parse_domain_label_only(&domain).expect("domain");
            if domain_entry.category != DomainCategory::Regulatory {
                panic!(
                    "Phase1 addr-bruteforce supports country/regulatory labels only. Label '{}' is corporate/other and is rejected in this phase.",
                    domain_entry.label
                );
            }
            validate_user_profile_flags(flags_mask, expected_flags).expect("flags policy");
            let dom = domain_entry.raw as u16;
            let domain_mode = DomainMatchMode::HighByteOnly;
            let started = Instant::now();
            let hit = brute_force_domain_flags_with_progress(
                &seed,
                dom,
                domain_mode,
                flags_mask,
                expected_flags,
                max_try,
                5,
                |p: BruteforceProgress| {
                    println!(
                        "progress checked_derivations={:.4}M rate_per_sec={:.0} eta={}",
                        p.checked as f64 / 1_000_000.0,
                        p.attempts_per_sec,
                        format_eta_human(p.eta_sec)
                    );
                },
            )
            .expect("no match");
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let attempts = (hit.derivation_index as u64) + 1;
            let attempts_per_sec = if elapsed_ms > 0.0 {
                (attempts as f64) / (elapsed_ms / 1000.0)
            } else {
                0.0
            };

            let account_id_hex = hex::encode(hit.account_id);
            let account_id_human = account_id_to_human(&hit.account_id);
            let account_id_bech32dx = account_id_to_bech32dx(&hit.account_id);
            let wallet = to_wallet_yaml(
                seed,
                hit.signing_key,
                hit.verifying_key,
                hit.derivation_index,
                hit.domain,
                flags_mask,
                expected_flags,
                hit.derived_flags,
                account_id_hex.clone(),
                account_id_human.clone(),
            )
            .unwrap_or_else(|e| exit_user_error(&format!("failed to build wallet file: {e}")));
            save_wallet_yaml(&wallet_out, &wallet).expect("save wallet");

            println!("mode single_thread_linear");
            println!("profile phase1_user_country_hi8");
            println!("domain_match_mode high_byte_only");
            println!("account_id_hex {}", account_id_hex);
            println!("account_id_human {}", account_id_human);
            println!("account_id_bech32dx {}", account_id_bech32dx);
            println!("derivation_index {}", hit.derivation_index);
            println!("domain_u16 {}", hit.domain);
            println!("domain_label {}", domain_entry.label);
            println!("flags_mask_u32 {}", flags_mask);
            println!("expected_flags_u32 {}", expected_flags);
            println!("flags_derived_u32 {}", hit.derived_flags);
            println!("wallet_path {}", wallet_out.display());
            println!("benchmark_attempts {}", attempts);
            println!("benchmark_elapsed_ms {:.3}", elapsed_ms);
            println!("benchmark_attempts_per_sec {:.3}", attempts_per_sec);
        }
        Cmd::Wallet { cmd } => match cmd {
            WalletCmd::Init {
                country,
                master,
                max_try,
                wallet_out,
                plaintext_dev,
            } => {
                let domain_entry = parse_domain_label_only(&country).expect("country");
                if domain_entry.category != DomainCategory::Regulatory {
                    panic!(
                        "wallet init supports country/regulatory labels only. Label '{}' is corporate/other and is rejected in this phase.",
                        domain_entry.label
                    );
                }
                let seed = master.map_or_else(
                    || {
                        let mut s = [0u8; 32];
                        rand::thread_rng().fill_bytes(&mut s);
                        s
                    },
                    |m| master_seed(&m).expect("master"),
                );
                let flags_mask = 0x03FF;
                let expected_flags = 0x0000;
                validate_user_profile_flags(flags_mask, expected_flags).expect("flags policy");
                let protection =
                    resolve_wallet_protection(wallet_passphrase.as_deref(), plaintext_dev)
                        .unwrap_or_else(|e| exit_user_error(&e));
                let dom = domain_entry.raw as u16;
                let started = Instant::now();
                let hit = brute_force_domain_flags_with_progress_and_match_policy(
                    &seed,
                    dom,
                    DomainMatchMode::HighByteOnly,
                    flags_mask,
                    expected_flags,
                    max_try,
                    5,
                    |p: BruteforceProgress| {
                        println!(
                            "progress checked_derivations={:.4}M rate_per_sec={:.0} eta={}",
                            p.checked as f64 / 1_000_000.0,
                            p.attempts_per_sec,
                            format_eta_human(p.eta_sec)
                        );
                    },
                    |_| true,
                )
                .expect("no match");
                let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                let attempts = (hit.derivation_index as u64) + 1;
                let attempts_per_sec = if elapsed_ms > 0.0 {
                    (attempts as f64) / (elapsed_ms / 1000.0)
                } else {
                    0.0
                };
                let account_id_hex = hex::encode(hit.account_id);
                let account_id_human = account_id_to_human(&hit.account_id);
                let account_id_bech32dx = account_id_to_bech32dx(&hit.account_id);
                let wallet = to_wallet_yaml_with_metadata(
                    seed,
                    hit.signing_key,
                    hit.verifying_key,
                    hit.derivation_index,
                    hit.domain,
                    flags_mask,
                    expected_flags,
                    hit.derived_flags,
                    account_id_hex.clone(),
                    account_id_human.clone(),
                    Some(domain_entry.label.to_string()),
                    protection,
                )
                .unwrap_or_else(|e| exit_user_error(&format!("failed to build wallet file: {e}")));
                save_wallet_yaml(&wallet_out, &wallet).expect("save wallet");
                println!("mode wallet_init_user_profile");
                println!("wallet_mode {}", wallet.mode);
                println!("country_label {}", domain_entry.label);
                println!("domain_match_mode high_byte_only");
                println!("flags_mask_u32 {}", flags_mask);
                println!("expected_flags_u32 {}", expected_flags);
                println!("account_id_hex {}", account_id_hex);
                println!("account_id_human {}", account_id_human);
                println!("address_bech32dx {}", account_id_bech32dx);
                println!("derivation_index {}", hit.derivation_index);
                println!("derivation_path m/0/{}", hit.derivation_index);
                println!("domain_u16 {}", hit.domain);
                println!("wallet_path {}", wallet_out.display());
                println!("benchmark_attempts {}", attempts);
                println!("benchmark_elapsed_ms {:.3}", elapsed_ms);
                println!("benchmark_attempts_per_sec {:.3}", attempts_per_sec);
            }
            WalletCmd::ImportSeed {
                country,
                master,
                max_try,
                wallet_out,
                plaintext_dev,
            } => {
                let domain_entry = parse_domain_label_only(&country).expect("country");
                if domain_entry.category != DomainCategory::Regulatory {
                    panic!(
                        "wallet import-seed supports country/regulatory labels only. Label '{}' is corporate/other and is rejected in this phase.",
                        domain_entry.label
                    );
                }
                let seed = master_seed(&master)
                    .unwrap_or_else(|e| exit_user_error(&format!("invalid --master seed: {e}")));
                let flags_mask = 0x03FF;
                let expected_flags = 0x0000;
                validate_user_profile_flags(flags_mask, expected_flags).expect("flags policy");
                let protection =
                    resolve_wallet_protection(wallet_passphrase.as_deref(), plaintext_dev)
                        .unwrap_or_else(|e| exit_user_error(&e));
                let dom = domain_entry.raw as u16;
                let started = Instant::now();
                let hit = brute_force_domain_flags_with_progress_and_match_policy(
                    &seed,
                    dom,
                    DomainMatchMode::HighByteOnly,
                    flags_mask,
                    expected_flags,
                    max_try,
                    5,
                    |p: BruteforceProgress| {
                        println!(
                            "progress checked_derivations={:.4}M rate_per_sec={:.0} eta={}",
                            p.checked as f64 / 1_000_000.0,
                            p.attempts_per_sec,
                            format_eta_human(p.eta_sec)
                        );
                    },
                    |_| true,
                )
                .expect("no match");
                let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
                let attempts = (hit.derivation_index as u64) + 1;
                let attempts_per_sec = if elapsed_ms > 0.0 {
                    (attempts as f64) / (elapsed_ms / 1000.0)
                } else {
                    0.0
                };
                let account_id_hex = hex::encode(hit.account_id);
                let account_id_human = account_id_to_human(&hit.account_id);
                let account_id_bech32dx = account_id_to_bech32dx(&hit.account_id);
                let wallet = to_wallet_yaml_with_metadata(
                    seed,
                    hit.signing_key,
                    hit.verifying_key,
                    hit.derivation_index,
                    hit.domain,
                    flags_mask,
                    expected_flags,
                    hit.derived_flags,
                    account_id_hex.clone(),
                    account_id_human.clone(),
                    Some(domain_entry.label.to_string()),
                    protection,
                )
                .unwrap_or_else(|e| exit_user_error(&format!("failed to build wallet file: {e}")));
                save_wallet_yaml(&wallet_out, &wallet).expect("save wallet");
                println!("mode wallet_import_seed_user_profile");
                println!("wallet_mode {}", wallet.mode);
                println!("country_label {}", domain_entry.label);
                println!("domain_match_mode high_byte_only");
                println!("flags_mask_u32 {}", flags_mask);
                println!("expected_flags_u32 {}", expected_flags);
                println!("account_id_hex {}", account_id_hex);
                println!("account_id_human {}", account_id_human);
                println!("address_bech32dx {}", account_id_bech32dx);
                println!("derivation_index {}", hit.derivation_index);
                println!("derivation_path m/0/{}", hit.derivation_index);
                println!("domain_u16 {}", hit.domain);
                println!("wallet_path {}", wallet_out.display());
                println!("benchmark_attempts {}", attempts);
                println!("benchmark_elapsed_ms {:.3}", elapsed_ms);
                println!("benchmark_attempts_per_sec {:.3}", attempts_per_sec);
            }
            WalletCmd::Show {
                wallet,
                unsafe_show_secrets,
            } => {
                let doc = load_wallet_yaml(&wallet).unwrap_or_else(|e| {
                    exit_user_error(&format!("failed to read wallet file: {e}"))
                });
                let secrets = if unsafe_show_secrets {
                    Some(
                        wallet_secrets(&doc, wallet_passphrase.as_deref()).unwrap_or_else(|e| {
                            exit_user_error(&format!("failed to decode wallet secrets: {e}"))
                        }),
                    )
                } else {
                    None
                };
                for line in wallet_show_lines(&doc, &wallet, secrets.as_ref()) {
                    println!("{line}");
                }
            }
            WalletCmd::BookAdd {
                wallet,
                address,
                label,
            } => {
                wallet_address_book_add(&wallet, &address, label.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&e));
                println!("ok");
            }
            WalletCmd::BookList { wallet } => {
                let doc = load_wallet_yaml(&wallet).unwrap_or_else(|e| {
                    exit_user_error(&format!("failed to read wallet file: {e}"))
                });
                if doc.address_book.is_empty() {
                    println!("(address_book empty — tx-send allows any policy-valid recipient)");
                } else {
                    for e in &doc.address_book {
                        let id = parse_account_id(e.address_str()).unwrap_or_else(|err| {
                            exit_user_error(&format!(
                                "wallet address_book contains invalid canonical address: {err}"
                            ))
                        });
                        let mut s = account_id_to_human(&id);
                        if let Some(l) = e.label() {
                            s.push_str("  label=");
                            s.push_str(l);
                        }
                        println!("{s}");
                    }
                }
                if doc.ignored_legacy_pretty_entries > 0 {
                    println!(
                        "warning: ignored {} legacy pretty address_book entries from wallet file",
                        doc.ignored_legacy_pretty_entries
                    );
                }
            }
            WalletCmd::BookRemove { wallet, address } => {
                wallet_address_book_remove(&wallet, &address)
                    .unwrap_or_else(|e| exit_user_error(&e));
                println!("ok");
            }
            WalletCmd::Backup { wallet, out } => {
                backup_wallet_file(&wallet, &out, wallet_passphrase.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&format!("wallet backup failed: {e}")));
                println!("mode wallet_backup");
                println!("wallet_source {}", wallet.display());
                println!("backup_path {}", out.display());
                println!("status ok");
            }
            WalletCmd::Recover { backup, out } => {
                recover_wallet_file(&backup, &out, wallet_passphrase.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&format!("wallet recovery failed: {e}")));
                println!("mode wallet_recover");
                println!("backup_path {}", backup.display());
                println!("wallet_restored {}", out.display());
                println!("status ok");
            }
        },
        Cmd::TxInit {
            wallet,
            master,
            domain,
            index,
            flags,
        } => {
            let source =
                load_tx_signer_source(wallet, master, domain, wallet_passphrase.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&e));
            let tx = SignedTx::sign_body(
                &source.sk,
                source.dom,
                source.idx,
                0,
                TxBody::Init { index, flags },
            );
            let c = reqwest::blocking::Client::new();
            let r = c
                .post(format!("{}/v1/tx", rpc_base))
                .json(&tx)
                .send()
                .expect("http");
            println!("{}", r.status());
        }
        Cmd::TxSend {
            wallet,
            master,
            domain,
            to,
            amount,
            fee,
        } => {
            let source = load_tx_signer_source(
                wallet.clone(),
                master.clone(),
                domain,
                wallet_passphrase.as_deref(),
            )
            .unwrap_or_else(|e| exit_user_error(&e));
            let (to_id, uri_amount) =
                parse_address_input("--to", &to).unwrap_or_else(|e| exit_user_error(&e));
            let amount =
                resolve_tx_send_amount(amount, uri_amount).unwrap_or_else(|e| exit_user_error(&e));
            validate_recipient_domain_policy(&to_id, Some("--to"))
                .unwrap_or_else(|e| exit_user_error(&e));
            if master.is_none() {
                if let Some(ref wp) = wallet {
                    let doc = load_wallet_yaml(wp).unwrap_or_else(|e| {
                        exit_user_error(&format!("failed to read wallet for address_book: {e}"))
                    });
                    assert_tx_recipient_in_wallet_address_book(&doc, &to_id)
                        .unwrap_or_else(|e| exit_user_error(&e));
                }
            }
            let c = reqwest::blocking::Client::new();
            let nonce = fetch_nonce(&c, &rpc_base, source.from);
            let tx = SignedTx::sign_body(
                &source.sk,
                source.dom,
                source.idx,
                nonce,
                TxBody::Transfer {
                    to: to_id,
                    amount,
                    fee,
                },
            );
            post_tx(&c, &rpc_base, &tx);
        }
        Cmd::TxStake {
            wallet,
            master,
            domain,
            amount,
        } => {
            let source =
                load_tx_signer_source(wallet, master, domain, wallet_passphrase.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&e));
            let c = reqwest::blocking::Client::new();
            let nonce = fetch_nonce(&c, &rpc_base, source.from);
            let tx = SignedTx::sign_body(
                &source.sk,
                source.dom,
                source.idx,
                nonce,
                TxBody::Stake { amount },
            );
            post_tx(&c, &rpc_base, &tx);
        }
        Cmd::TxUnstake {
            wallet,
            master,
            domain,
            amount,
        } => {
            let source =
                load_tx_signer_source(wallet, master, domain, wallet_passphrase.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&e));
            let c = reqwest::blocking::Client::new();
            let nonce = fetch_nonce(&c, &rpc_base, source.from);
            let tx = SignedTx::sign_body(
                &source.sk,
                source.dom,
                source.idx,
                nonce,
                TxBody::Unstake { amount },
            );
            post_tx(&c, &rpc_base, &tx);
        }
        Cmd::TxBurnMark {
            wallet,
            master,
            domain,
            mark_amount,
            beneficiary,
        } => {
            let source =
                load_tx_signer_source(wallet, master, domain, wallet_passphrase.as_deref())
                    .unwrap_or_else(|e| exit_user_error(&e));
            let c = reqwest::blocking::Client::new();
            let nonce = fetch_nonce(&c, &rpc_base, source.from);
            let beneficiary = beneficiary
                .as_deref()
                .map(|v| {
                    let (parsed, uri_amount) = parse_address_input("--beneficiary", v)?;
                    if uri_amount.is_some() {
                        return Err(
                            "URI amount is not allowed for --beneficiary in tx-burn-mark"
                                .to_string(),
                        );
                    }
                    Ok(parsed)
                })
                .transpose()
                .unwrap_or_else(|e| exit_user_error(&e));
            beneficiary
                .as_ref()
                .map(|b| validate_recipient_domain_policy(b, Some("--beneficiary")))
                .transpose()
                .unwrap_or_else(|e| exit_user_error(&e));
            let tx = SignedTx::sign_body(
                &source.sk,
                source.dom,
                source.idx,
                nonce,
                TxBody::BurnMark {
                    mark_amount,
                    beneficiary,
                },
            );
            post_tx(&c, &rpc_base, &tx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_address_arg, parse_address_input, parse_domain_label_only, resolve_tx_send_amount,
        resolve_wallet_protection, validate_user_profile_flags, wallet_show_lines, Cli, Cmd,
        WalletCmd,
    };
    use clap::Parser;
    use pwm_core::domain_index::{lookup_by_raw, DomainCategory};
    use pwm_core::{account_id_to_human, parse_account_id, validate_recipient_domain_policy};
    use std::path::PathBuf;

    #[test]
    fn label_only_accepts_short_label() {
        let entry = parse_domain_label_only("MSFT").expect("must parse");
        assert_eq!(entry.raw, 0xC01E);
        assert_eq!(entry.category, DomainCategory::Tnc);
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
    fn user_profile_rejects_flags_outside_low_10_bits() {
        let err = validate_user_profile_flags(0x0400, 0).expect_err("must reject high bits");
        assert!(err.contains("low 10 bits"));
    }

    #[test]
    fn user_profile_rejects_expected_flags_outside_mask() {
        let err = validate_user_profile_flags(0x0003, 0x0004).expect_err("must reject mismatch");
        assert!(err.contains("outside flags_mask"));
    }

    #[test]
    fn corporate_label_is_not_regulatory() {
        let entry = parse_domain_label_only("MSFT").expect("must parse");
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
                    plaintext_dev,
                    ..
                } => {
                    assert_eq!(country, "CY");
                    assert!(master.is_none());
                    assert_eq!(max_try, 500_000);
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
                    plaintext_dev,
                    ..
                } => {
                    assert_eq!(country, "CY");
                    assert_eq!(master, "11".repeat(32));
                    assert_eq!(max_try, 500_000);
                    assert!(!plaintext_dev);
                }
                _ => panic!("unexpected wallet cmd"),
            },
            _ => panic!("unexpected cmd"),
        }
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
    fn wallet_show_cli_parsing_with_unsafe_flag() {
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
    fn wallet_show_lines_redact_secrets_by_default() {
        let doc = super::WalletYaml {
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
        assert!(!joined.contains("master_seed_hex"));
        assert!(!joined.contains("signing_key_hex"));
        assert!(!joined.contains("verifying_key_hex"));
    }

    #[test]
    fn wallet_show_lines_reveal_secrets_in_unsafe_mode() {
        let doc = super::WalletYaml {
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
        let secrets = super::WalletSecrets {
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
    fn wallet_protection_requires_passphrase_without_plaintext_opt_in() {
        let err = resolve_wallet_protection(None, false).expect_err("must require passphrase");
        assert!(err.contains("encrypted wallet mode is default"));
    }

    #[test]
    fn wallet_protection_allows_plaintext_only_with_explicit_opt_in() {
        let mode = resolve_wallet_protection(Some("ignored"), true).expect("must allow plaintext");
        match mode {
            super::WalletProtection::PlaintextDev => {}
            _ => panic!("unexpected mode"),
        }
    }

    #[test]
    fn tx_send_cli_accepts_pretty_recipient_form() {
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
                to,
                amount,
                fee,
            } => {
                assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
                assert!(master.is_none());
                assert!(domain.is_none());
                assert_eq!(parse_account_id(&to).unwrap(), recipient_id);
                assert_eq!(amount, Some(7));
                assert_eq!(fee, 1);
            }
            _ => panic!("unexpected cmd"),
        }
    }

    #[test]
    fn tx_burn_mark_cli_accepts_pretty_beneficiary_form() {
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
                ..
            } => {
                assert_eq!(mark_amount, 12);
                assert_eq!(
                    parse_account_id(got.as_deref().unwrap()).unwrap(),
                    beneficiary_id
                );
            }
            _ => panic!("unexpected cmd"),
        }
    }

    #[test]
    fn tx_send_cli_accepts_canonical_recipient_form() {
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
    fn tx_burn_mark_cli_accepts_canonical_beneficiary_form() {
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
                beneficiary: got, ..
            } => {
                assert_eq!(
                    parse_account_id(got.as_deref().unwrap()).unwrap(),
                    [6u8; 32]
                );
            }
            _ => panic!("unexpected cmd"),
        }
    }

    #[test]
    fn tx_send_cli_accepts_master_override_over_wallet() {
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
                ..
            } => {
                assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
                assert_eq!(master.unwrap(), "11".repeat(32));
                assert_eq!(domain.as_deref(), Some("CY"));
            }
            _ => panic!("unexpected cmd"),
        }
    }

    #[test]
    fn parse_address_input_accepts_pwm_uri_with_amount() {
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
    fn parse_address_input_rejects_unknown_query_param() {
        let id = pwm_core::account_id_to_bech32dx(&[4u8; 32]);
        let err = parse_address_input("--to", &format!("pwm:{id}?memo=abc")).expect_err("reject");
        assert!(err.contains("unsupported pwm URI query parameter"));
    }

    #[test]
    fn parse_address_input_rejects_malformed_uri_without_address() {
        let err = parse_address_input("--to", "pwm:?amount=1").expect_err("reject");
        assert!(err.contains("missing address"));
    }

    #[test]
    fn tx_send_amount_resolution_detects_conflict() {
        let err = resolve_tx_send_amount(Some(10), Some(11)).expect_err("must conflict");
        assert!(err.contains("amount conflict"));
    }

    #[test]
    fn tx_send_amount_resolution_accepts_uri_without_cli_amount() {
        let amount = resolve_tx_send_amount(None, Some(15)).expect("uri amount");
        assert_eq!(amount, 15);
    }

    #[test]
    fn tx_send_cli_allows_uri_recipient_without_explicit_amount_flag() {
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
    fn parse_address_input_rejects_uri_with_invalid_address() {
        let err = parse_address_input("--to", "pwm:not-an-address?amount=1").expect_err("reject");
        assert!(err.contains("Invalid value for --to"));
    }

    #[test]
    fn tx_send_cli_rejects_master_without_domain() {
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
    fn tx_init_cli_rejects_when_neither_wallet_nor_master_provided() {
        let err = match Cli::try_parse_from(["pwm", "tx-init", "--index", "0", "--flags", "0"]) {
            Ok(_) => panic!("must reject missing signing source"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("--wallet"));
    }

    #[test]
    fn tx_stake_cli_accepts_wallet_only_source() {
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
                amount,
            } => {
                assert_eq!(wallet.unwrap(), PathBuf::from("wallet.yaml"));
                assert!(master.is_none());
                assert!(domain.is_none());
                assert_eq!(amount, 15);
            }
            _ => panic!("unexpected cmd"),
        }
    }

    #[test]
    fn parse_address_arg_reports_formats_for_malformed_pretty_input() {
        let err = parse_address_arg("--to", "pwm1-CY-f00000003-tABCDEF")
            .expect_err("must reject malformed pretty input");
        assert!(err.contains("Invalid value for --to"));
        assert!(err
            .contains("Accepted formats: pretty pwm1-<label_or_$hex!>-f<flags8hex>-t<tail52hex>"));
        assert!(err.contains("canonical pwm1..."));
        assert!(err.contains("legacy PWMv0-... / hex"));
    }

    #[test]
    fn parse_address_arg_rejects_ambiguous_legacy_pretty_without_lo() {
        let legacy = "pwm1-CY-f00000000-t0000000000000000000000000000000000000000000000000000";
        let err = parse_address_arg("--to", legacy).expect_err("must reject ambiguous pretty");
        assert!(err.contains("Invalid value for --to"));
        assert!(err.contains("missing '/LO'"));
        assert!(err.contains("strict pretty"));
        assert!(err.contains("canonical bech32dx"));
    }

    #[test]
    fn parse_address_arg_accepts_canonical_regulatory_lo_zero() {
        let mut id = [0u8; 32];
        id[0] = 0x2C;
        id[1] = 0x00;
        let canonical = pwm_core::account_id_to_bech32dx(&id);
        let parsed = parse_address_arg("--to", &canonical).expect("must accept canonical /00");
        assert_eq!(parsed, id);
    }

    #[test]
    fn parse_address_arg_rejects_canonical_with_bad_checksum() {
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
    fn tx_recipient_rejects_unknown_regulatory_domain() {
        let mut id = [0u8; 32];
        id[0] = 0xBF;
        id[1] = 0x00;
        let err =
            validate_recipient_domain_policy(&id, Some("--to")).expect_err("must reject unknown");
        assert!(err.contains("not recognized by domain index"));
    }

    #[test]
    fn tx_recipient_rejects_reserve_domain() {
        let mut id = [0u8; 32];
        id[0] = 0xE0;
        id[1] = 0x03;
        let err =
            validate_recipient_domain_policy(&id, Some("--to")).expect_err("must reject reserve");
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
    fn tx_path_recipient_policy_rejects_unknown_reserve_witness() {
        let cases = [
            (
                "--to",
                "pwm1-$BF00!-f00000000-t0000000000000000000000000000000000000000000000000000",
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
            let err =
                validate_recipient_domain_policy(&parsed, Some(field)).expect_err("must reject");
            assert!(err.contains(expected), "expected '{expected}' in '{err}'");
        }
    }

    #[test]
    fn wallet_profile_allows_country_label_without_lo_byte_filter() {
        let entry = parse_domain_label_only("CY").expect("must parse label");
        assert_eq!(entry.category, DomainCategory::Regulatory);
    }
}
