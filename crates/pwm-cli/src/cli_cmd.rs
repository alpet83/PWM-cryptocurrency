//! Clap root/subcommands for pwm-cli (`pwm` binary entry surface).

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pwm")]
pub struct Cli {
    /// pwmd base URL (trailing slash optional). Same as env `PWM_RPC`.
    /// HTTP timeouts for tx/nonce calls: env `PWM_CLI_RPC_TIMEOUT_MS` (default 10000, max 120000).
    #[arg(
        long,
        global = true,
        env = "PWM_RPC",
        default_value = "http://127.0.0.1:3030"
    )]
    pub(crate) rpc: String,
    /// Wallet encryption passphrase for reading encrypted wallet files. Same as env `PWM_WALLET_PASSPHRASE`.
    #[arg(long, global = true, env = "PWM_WALLET_PASSPHRASE")]
    pub(crate) wallet_passphrase: Option<String>,
    /// Passphrase for encrypting genesis validator keys. Same as env `PWM_GENESIS_PASSPHRASE`.
    #[arg(long, global = true, env = "PWM_GENESIS_PASSPHRASE")]
    pub(crate) genesis_passphrase: Option<String>,
    /// Explicitly upgrade wallet schema v2 -> v3 when loading wallet files.
    #[arg(long, global = true)]
    pub(crate) upgrade_wallet: bool,
    #[command(subcommand)]
    pub(crate) cmd: Cmd,
}

#[derive(Subcommand)]
pub(crate) enum Cmd {
    /// Print random 32-byte master seed (hex).
    KeyGen,
    /// Build genesis JSON from wallet accounts.
    #[command(name = "genesis-build")]
    GenesisBuild {
        #[arg(long, help = "Wallet YAML path")]
        wallet: PathBuf,
        #[arg(long, help = "Output genesis JSON path")]
        out: PathBuf,
        #[arg(
            long,
            help = "Validator account id (default: deterministic wallet account)"
        )]
        val_id: Option<String>,
        #[arg(
            long,
            default_value_t = 1_000_000,
            help = "Premine balance in raw units (1 PWM = 1_000_000 raw)."
        )]
        premine_bal: u128,
        #[arg(long, default_value_t = 100)]
        block_reward: u128,
        #[arg(long, default_value_t = 10_000)]
        marks_coeff: u128,
    },
    /// Brute cluster address for `--domain` (hex u16).
    #[command(name = "addr-derive", visible_alias = "addr-der")]
    AddrDer {
        #[arg(
            long,
            env = "PWM_MASTER_SEED",
            num_args = 0..=1,
            default_missing_value = "",
            help = "32-byte master seed hex. With explicit existing --wallet-out, wallet seed is authoritative; external seed (--master/PWM_MASTER_SEED/MASTER_SEED) must match (or use --overwrite-wallet in addr-bruteforce)."
        )]
        master: Option<String>,
        #[arg(long)]
        domain: String,
        #[arg(long, default_value_t = 500_000)]
        max_try: u32,
        #[arg(long)]
        wallet_out: Option<PathBuf>,
    },
    /// Single-thread linear bruteforce by domain + flags mask with wallet save.
    #[command(name = "addr-bruteforce")]
    AddrBruteforce {
        #[arg(
            long,
            env = "PWM_MASTER_SEED",
            num_args = 0..=1,
            default_missing_value = "",
            help = "32-byte master seed hex. With explicit existing --wallet-out, wallet seed is authoritative; external seed (--master/PWM_MASTER_SEED/MASTER_SEED) must match unless --overwrite-wallet is used."
        )]
        master: Option<String>,
        #[arg(
            long,
            help = "Domain label from pwm_core::domain_index. Phase1 user profile accepts country labels only (e.g. CY, US)"
        )]
        domain: String,
        #[arg(
            long,
            default_value_t = 1023,
            help = "Mask for expected flags (Phase1 user profile: low 10 bits only, default: 1023)"
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
        #[arg(
            long,
            help = "Wallet output path. Default: ~/.pwm-crypto/default-wallet.yaml"
        )]
        wallet_out: Option<PathBuf>,
        #[arg(
            long,
            help = "Overwrite existing wallet file instead of default safe append behavior."
        )]
        overwrite_wallet: bool,
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
        #[arg(long, help = "V4 owner kind (e.g. company, person).")]
        owner_kind: Option<String>,
        #[arg(long, help = "V4 short owner display name.")]
        owner_name: Option<String>,
        #[arg(long, help = "V4 owner country hint.")]
        owner_country: Option<String>,
        #[arg(long, help = "V4 company metadata commitment as 32-byte hex.")]
        metadata_commitment: Option<String>,
        #[arg(long, help = "V4 external verification reference.")]
        verification_ref: Option<String>,
        #[arg(long, help = "V4 requested low byte for destination domain.")]
        requested_domain_lo: Option<u8>,
        #[arg(long, help = "V4 rescue address (pretty/canonical/legacy formats).")]
        rescue_address: Option<String>,
        #[arg(
            long = "initial-policy",
            help = "V4 initial policy entry, format <kind>[:dormant|immediately]. Repeatable."
        )]
        initial_policy: Vec<String>,
    },
    /// Fetch and print account marks/stake view at current chain head.
    #[command(name = "account-info")]
    AccountInfo {
        #[arg(
            long,
            required_unless_present = "wallet",
            help = "Account id (pretty/canonical bech32DX/legacy hex/PWMv0-hex)."
        )]
        account: Option<String>,
        #[arg(
            long,
            required_unless_present = "account",
            help = "Wallet path used to resolve active account id when --account is omitted."
        )]
        wallet: Option<PathBuf>,
    },
    /// POST signed POLICY set action.
    #[command(name = "tx-policy-set")]
    TxPolicySet {
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
        #[arg(
            long,
            help = "Policy kind: sender_filter, routing.emergency_redirect, routing.same_domain_only, default_behavior, cosign_required."
        )]
        policy: String,
        #[arg(long, help = "Activation mode: dormant, immediately, or deferred.")]
        activation: String,
        #[arg(
            long,
            help = "Absolute chain height for --activation deferred (must be > 0)."
        )]
        activate_at_height: Option<u64>,
        #[arg(
            long,
            default_value_t = 1,
            help = "Fee in raw units (1 PWM = 1_000_000 raw)."
        )]
        fee: u128,
    },
    /// POST signed POLICY activation action.
    #[command(name = "tx-policy-activate")]
    TxPolicyActivate {
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
        #[arg(long, help = "Policy kind alternative to --policy-id.")]
        policy: Option<String>,
        #[arg(long, help = "Policy id alternative to --policy.")]
        policy_id: Option<u8>,
        #[arg(
            long,
            default_value_t = 1,
            help = "Fee in raw units (1 PWM = 1_000_000 raw)."
        )]
        fee: u128,
        #[arg(long, help = "Rescue signer derivation index in wallet v3.")]
        rescue_account_index: Option<u32>,
        #[arg(long, help = "Optional dedicated rescue wallet path.")]
        rescue_wallet: Option<PathBuf>,
        #[arg(
            long,
            help = "Rescue signer dev override master seed (requires --rescue-domain).",
            requires = "rescue_domain"
        )]
        rescue_master: Option<String>,
        #[arg(
            long,
            help = "Rescue signer domain for --rescue-master.",
            requires = "rescue_master"
        )]
        rescue_domain: Option<String>,
        #[arg(long, help = "Optional passphrase for rescue wallet unlock.")]
        rescue_passphrase: Option<String>,
    },
    /// POST signed POLICY deactivation action.
    #[command(name = "tx-policy-deactivate")]
    TxPolicyDeactivate {
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
        #[arg(long, help = "Policy kind alternative to --policy-id.")]
        policy: Option<String>,
        #[arg(long, help = "Policy id alternative to --policy.")]
        policy_id: Option<u8>,
        #[arg(
            long,
            default_value_t = 1,
            help = "Fee in raw units (1 PWM = 1_000_000 raw)."
        )]
        fee: u128,
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
        #[arg(long, help = "Amount in raw units (1 PWM = 1_000_000 raw).")]
        amount: Option<u128>,
        #[arg(
            long,
            default_value_t = 1,
            help = "Fee in raw units (1 PWM = 1_000_000 raw)."
        )]
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
        #[arg(long, help = "Stake amount in raw units (1 PWM = 1_000_000 raw).")]
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
        #[arg(long, help = "Unstake amount in raw units (1 PWM = 1_000_000 raw).")]
        amount: u128,
    },
    /// POST signed BURN_MARK to current RPC target (`--rpc` / `PWM_RPC`).
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
        #[arg(
            long,
            help = "Burn amount in marks units. Tx is sent to current RPC target (`--rpc` / `PWM_RPC`)."
        )]
        mark_amount: u32,
        #[arg(
            long,
            help = "Optional beneficiary address. Accepted: pretty (pwm1-LABEL-f<flags8hex>-t<tail52hex>), canonical bech32DX (pwm1...), legacy hex, legacy PWMv0-hex. Keep `--rpc` / `PWM_RPC` pointed to the source-shard node for this signer."
        )]
        beneficiary: Option<String>,
        #[arg(
            long,
            help = "Dedication text for the burn (v2, RFC 0011): trimmed UTF-8, 1..80 bytes, no C0/C1 controls. If omitted, a built-in default is used (stderr note). Supports placeholders: {utc_time} (DD-MM-YY HH:MM:SSZ), {utc_timestamp} (Unix seconds)."
        )]
        purpose: Option<String>,
    },
    /// POST signed ClaimTx (materialize matured marks, use --all to let node compute amount).
    #[command(name = "tx-claim")]
    TxClaim {
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
        #[arg(
            long,
            help = "Claim billing mode: `free` (fee must be 0) or `paid` (fee must be > 0)."
        )]
        claim_mode: String,
        /// Claim units to materialise. Pass 0 or omit to use --all.
        #[arg(long, default_value = "0")]
        claim_units: u32,
        /// Claim all currently matured marks (sends CLAIM_ALL sentinel to node).
        #[arg(long, default_value_t = false)]
        all: bool,
        #[arg(
            long,
            help = "Anchor reference height (per chain rules / wallet integration)."
        )]
        anchor_ref: u64,
        #[arg(
            long,
            help = "Fee in raw units (1 PWM = 1_000_000 raw). Must be 0 for mode=free."
        )]
        fee: u128,
    },
    /// POST signed EXPORT for inter-shard source flow.
    TxExport {
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
        #[arg(
            long,
            help = "Recipient account in target shard. Accepted: pretty (pwm1-LABEL-f<flags8hex>-t<tail52hex>), canonical bech32DX (pwm1...), legacy hex, legacy PWMv0-hex"
        )]
        to: String,
        #[arg(
            long,
            help = "Target domain for import side (label or numeric, same parser as --domain)."
        )]
        target_domain: String,
        #[arg(long, help = "Export amount in raw units (1 PWM = 1_000_000 raw).")]
        amount: u128,
        #[arg(
            long,
            default_value_t = 1,
            help = "Export fee in raw units (1 PWM = 1_000_000 raw)."
        )]
        fee: u128,
    },
    /// POST signed IMPORT for inter-shard target flow.
    TxImport {
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
        #[arg(
            long,
            help = "Recipient account for import credit. Must already be initialized with tx-init on the target shard. Accepted: pretty (pwm1-LABEL-f<flags8hex>-t<tail52hex>), canonical bech32DX (pwm1...), legacy hex, legacy PWMv0-hex"
        )]
        to: String,
        #[arg(long, help = "Import amount in raw units (1 PWM = 1_000_000 raw).")]
        amount: u128,
        #[arg(
            long,
            help = "Export id as 32-byte hex (64 hex chars). Register source handoff first with tx-handoff-register on a target that trusts the source peer."
        )]
        export_id: String,
    },
    /// Persist snapshot (if configured) and shut down pwmd HTTP server (`POST /v1/shutdown`).
    #[command(name = "node-shutdown")]
    NodeShutdown,
    /// Register finalized source export provenance on a trusted target node before tx-import.
    #[command(name = "tx-handoff-register")]
    TxHandoffRegister {
        #[arg(
            long,
            help = "Path to source finalize handoff JSON. Target registration requires trusted source peer context from configured seed connectivity."
        )]
        handoff_json: PathBuf,
    },
    /// Wallet file operations.
    Wallet {
        #[command(subcommand)]
        cmd: WalletCmd,
    },
}

#[derive(Subcommand)]
pub(crate) enum WalletCmd {
    /// Initialize user-mode wallet (user-profile).
    ///
    /// Two modes: (1) **Country-directed brute-force** — pass `--country` and omit explicit derivation selectors.
    /// (2) **Explicit derivation** — pass `--derivation-index` and/or `--derivation-path` (`m/0/N` only); `--country` is then optional and is **not** used to filter the derived domain (validation follows recipient domain policy: reserve/witness/unknown are rejected).
    Init {
        #[arg(
            long,
            required_unless_present_any = ["derivation_index", "derivation_path"],
            help = "Country/regulatory label (required for brute-force mode). With --derivation-index/--derivation-path, optional and not used as a domain filter; metadata uses the label from the derived account domain."
        )]
        country: Option<String>,
        #[arg(
            long,
            help = "Optional 32-byte master seed hex. If omitted, a random seed is generated."
        )]
        master: Option<String>,
        #[arg(long, default_value_t = 500_000)]
        max_try: u32,
        #[arg(
            long,
            help = "Wallet output path. Default: ~/.pwm-crypto/default-wallet.yaml"
        )]
        wallet_out: Option<PathBuf>,
        #[arg(
            long,
            help = "Explicit derivation index for wallet address (skips brute-force search)."
        )]
        derivation_index: Option<u32>,
        #[arg(
            long,
            help = "Explicit derivation path. Only canonical form m/0/<index> is accepted."
        )]
        derivation_path: Option<String>,
        #[arg(
            long,
            help = "Store wallet secrets in plaintext for local dev only (explicit opt-in)."
        )]
        plaintext_dev: bool,
    },
    /// Import existing 32-byte seed and initialize user-mode wallet (same modes as `wallet init`).
    ImportSeed {
        #[arg(
            long,
            required_unless_present_any = ["derivation_index", "derivation_path"],
            help = "Country/regulatory label (required for brute-force mode). With --derivation-index/--derivation-path, optional and not used as a domain filter; metadata uses the label from the derived account domain."
        )]
        country: Option<String>,
        #[arg(long, help = "32-byte master seed hex.")]
        master: String,
        #[arg(long, default_value_t = 500_000)]
        max_try: u32,
        #[arg(
            long,
            help = "Wallet output path. Default: ~/.pwm-crypto/default-wallet.yaml"
        )]
        wallet_out: Option<PathBuf>,
        #[arg(
            long,
            help = "Explicit derivation index for wallet address (skips brute-force search)."
        )]
        derivation_index: Option<u32>,
        #[arg(
            long,
            help = "Explicit derivation path. Only canonical form m/0/<index> is accepted."
        )]
        derivation_path: Option<String>,
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
    /// Account operations for schema v3 wallets.
    Account {
        #[command(subcommand)]
        cmd: WalletAccountCmd,
    },
}

#[derive(Subcommand)]
pub(crate) enum WalletAccountCmd {
    /// List all accounts from schema v3 wallet.
    List {
        #[arg(long)]
        wallet: PathBuf,
    },
    /// Add account by derivation index from same master seed.
    Add {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        derivation_index: u32,
    },
    /// Deprecated: validate account id; signing does not persist an active account.
    Use {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        id_hex: String,
    },
    /// Remove account by account id hex. Refuses to remove the last account.
    Remove {
        #[arg(long)]
        wallet: PathBuf,
        #[arg(long)]
        id_hex: String,
    },
}
