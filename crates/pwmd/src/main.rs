//! Devnet node binary: REST `/v1/*`, PoA seal loop.

use clap::{Parser, ValueEnum};
use pwmd::{
    default_runtime_identity_neutral, init_logging, logger, neutral_listen_dir_tag,
    parse_cluster_domain_hi, resolve_runtime_identity, storage_namespace, ConsoleColorMode,
    GenesisSource, LogFileMode, LoggingConfig, PersistSnapKind, PwmdConfig, RuntimeIdentityInput,
    RuntimeIdentityMode, ShardId, TransportConfig,
};
use std::io::IsTerminal;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pwmd")]
struct Cli {
    /// DEPRECATED legacy compat selector (`A` or `B`) for relay baseline fallback.
    /// Domain-first contract: prefer explicit identity flags (`--network-id`,
    /// `--domain-hi`/`--domain-cluster`, `--cluster-id`, `--node-id`).
    /// Sprint 11 soft-break policy: kept as compatibility path, no hard removal.
    #[deprecated(
        since = "0.1.0",
        note = "prefer domain-first identity flags (--network-id, --domain-hi, --cluster-id, --node-id)"
    )]
    #[arg(long, value_name = "A|B")]
    shard: Option<String>,
    /// Domain-first runtime identity: explicit network id (e.g. `devnet`, `testnet-v1`).
    #[arg(long)]
    network_id: Option<String>,
    /// Domain-first runtime identity: explicit cluster domain high byte (u8; decimal or hex `0xNN`).
    /// Primary UX flags: `--domain-hi`, `--domain-cluster`.
    /// Deprecated compatibility alias: `--cluster-domain-hi`.
    #[arg(
        long = "domain-hi",
        visible_alias = "domain-cluster",
        alias = "cluster-domain-hi",
        alias = "domain_cluster"
    )]
    cluster_domain_hi: Option<String>,
    /// Domain-first runtime identity: explicit stable cluster label.
    #[arg(long)]
    cluster_id: Option<String>,
    /// Domain-first runtime identity: explicit stable node identifier in this network.
    #[arg(long)]
    node_id: Option<String>,
    /// HTTP listen address (e.g. `127.0.0.1:3030`).
    #[arg(long)]
    listen: Option<std::net::SocketAddr>,
    /// Genesis JSON (`schema_version=4`, encrypted `validator_keys`); default is built-in `dev_net()`.
    #[arg(long, value_name = "PATH")]
    genesis_file: Option<PathBuf>,
    /// Passphrase for encrypted validator keys in `--genesis-file`. Same as env `PWM_GENESIS_PASSPHRASE`.
    #[arg(long, env = "PWM_GENESIS_PASSPHRASE")]
    genesis_passphrase: Option<String>,
    /// State root dir (default data path uses neutral relay path or explicit namespace).
    #[arg(long, value_name = "DIR", default_value = "state")]
    state_root: PathBuf,
    /// Optional explicit JSON snapshot path override (`blocks` + `state`).
    #[arg(long, value_name = "PATH")]
    data_file: Option<PathBuf>,
    /// Enable real socket transport loop (seed connect + NodeHello handshake).
    #[arg(long, default_value_t = false)]
    transport_real: bool,
    /// Dedicated peer listener address for stateful peer sessions (must not match RPC listen).
    #[arg(long, env = "PWM_PEER_LISTEN")]
    transport_peer_listen: Option<std::net::SocketAddr>,
    /// Comma-separated seed peers for real transport (e.g. 127.0.0.1:4040,127.0.0.1:4041).
    #[arg(long, value_delimiter = ',', value_name = "ADDR[,ADDR...]")]
    transport_peer_seed: Vec<std::net::SocketAddr>,
    /// HTTP peer base(s) for one-window relay (`/v1/status`, provenance). Defaults: each `--transport-peer-seed` host with port−100 (inverse of rpc+100 peer convention).
    #[arg(long, value_delimiter = ',', value_name = "ADDR[,ADDR...]")]
    transport_relay_http_seed: Vec<std::net::SocketAddr>,
    /// TCP connect timeout for real transport (milliseconds).
    #[arg(long, default_value_t = 1000)]
    transport_connect_timeout_ms: u64,
    /// Handshake read/write timeout for real transport (milliseconds).
    #[arg(long, default_value_t = 1000)]
    transport_handshake_timeout_ms: u64,
    /// Heartbeat interval for established peer sessions (milliseconds).
    #[arg(long, default_value_t = 1500)]
    transport_heartbeat_interval_ms: u64,
    /// Heartbeat timeout for established peer sessions (milliseconds).
    #[arg(long, default_value_t = 4500)]
    transport_heartbeat_timeout_ms: u64,
    /// Retry backoff base delay for real transport seed reconnects (milliseconds).
    #[arg(long, default_value_t = 500)]
    transport_retry_base_ms: u64,
    /// Retry backoff max delay for real transport seed reconnects (milliseconds).
    #[arg(long, default_value_t = 60000)]
    transport_retry_max_ms: u64,
    /// Upper bound for long-run soak counters/rollups in transport observability.
    #[arg(long, default_value_t = 1_000_000)]
    transport_soak_counter_cap: u64,
    /// Optional periodic health aggregation interval in transport ticks (0 disables).
    #[arg(long, default_value_t = 0)]
    transport_soak_health_interval_ticks: u64,
    /// Runaway reconnect streak limit before safety stop guard activates.
    #[arg(long, default_value_t = 12)]
    transport_runaway_streak_limit: u32,
    /// Safety stop cooldown for runaway reconnect guard (milliseconds).
    #[arg(long, default_value_t = 60000)]
    transport_runaway_cooldown_ms: u64,
    /// Logical log stream name used in file template placeholders.
    #[arg(long, env = "PWM_LOG_NAME", default_value = "pwmd")]
    log_name: String,
    /// Log root directory for file sink output.
    #[arg(long, env = "PWM_LOG_DIR", default_value = "logs")]
    log_dir: PathBuf,
    /// Relative log file template (supports {date}, {time}, {datetime}, {log_name}, {node_id}, {pid}).
    #[arg(
        long = "log-file-template",
        env = "PWM_LOG_FILE_TEMPLATE",
        default_value = "{date}/{log_name}-{node_id}-{time}.log"
    )]
    log_file_template: String,
    /// File sink mode: on (best effort), off, required (startup fails when file sink is unavailable).
    #[arg(long = "log-file", env = "PWM_LOG_FILE", value_enum, default_value_t = CliLogFileMode::On)]
    log_file: CliLogFileMode,
    /// Dedicated transport/peer file sink mode.
    #[arg(
        long = "peer-log-file",
        env = "PWM_PEER_LOG_FILE",
        value_enum,
        default_value_t = CliLogFileMode::On
    )]
    peer_log_file: CliLogFileMode,
    /// Relative peer log file template (default prefix `pwmd-peer`).
    #[arg(
        long = "peer-log-file-template",
        env = "PWM_PEER_LOG_FILE_TEMPLATE",
        default_value = "{date}/pwmd-peer-{node_id}-{time}.log"
    )]
    peer_log_file_template: String,
    /// Console color mode for stderr sink.
    #[arg(
        long = "log-console-color",
        env = "PWM_LOG_CONSOLE_COLOR",
        value_enum,
        default_value_t = CliConsoleColorMode::Auto
    )]
    log_console_color: CliConsoleColorMode,
    /// Rotate active log file when it exceeds this size in MB.
    #[arg(
        long = "log-rotate-size-mb",
        env = "PWM_LOG_ROTATE_SIZE_MB",
        default_value_t = 32
    )]
    log_rotate_size_mb: u64,
    /// Number of rotated files to retain.
    #[arg(
        long = "log-rotate-max-files",
        env = "PWM_LOG_ROTATE_MAX_FILES",
        default_value_t = 7
    )]
    log_rotate_max_files: usize,
    /// Snapshot persistence: `json-file` (default path: `--data-file`) or `clickhouse` (feature).
    #[arg(long = "snapshot-backend", value_enum, default_value_t = CliSnapBackend::JsonFile)]
    snapshot_backend: CliSnapBackend,
    #[cfg(feature = "clickhouse-snapshot")]
    #[arg(long = "clickhouse-url", env = "PWM_CLICKHOUSE_URL")]
    clickhouse_url: Option<String>,
    #[cfg(feature = "clickhouse-snapshot")]
    #[arg(
        long = "clickhouse-database",
        env = "PWM_CLICKHOUSE_DATABASE",
        default_value = "pwm_snapshots"
    )]
    clickhouse_database: String,
    #[cfg(feature = "clickhouse-snapshot")]
    #[arg(
        long = "clickhouse-table",
        env = "PWM_CLICKHOUSE_TABLE",
        default_value = "node_snapshot"
    )]
    clickhouse_table: String,
    #[cfg(feature = "clickhouse-snapshot")]
    /// Overrides ClickHouse row identity (default: network|domain|cluster|node|genesis digest).
    #[arg(long = "snapshot-store-key", env = "PWM_SNAPSHOT_STORE_KEY")]
    snapshot_store_key: Option<String>,
    #[cfg(feature = "clickhouse-snapshot")]
    /// Disable writing `--data-file` when ClickHouse snapshot insert fails (default: fallback on).
    #[arg(long = "no-snapshot-ch-fallback", default_value_t = false)]
    no_snapshot_ch_fallback: bool,
    /// Full genesis→tip replay when loading epoch snapshots (slow; audit). Env `PWM_SNAPSHOT_VERIFY_CHAIN` (truthy) enables.
    #[arg(long = "snapshot-verify-chain", default_value_t = false, action = clap::ArgAction::SetTrue)]
    snapshot_verify_chain: bool,
    /// After fatal snapshot load errors, keep HTTP up in `ready_degraded` instead of exiting (default: exit).
    #[arg(long = "keep-alive-on-snapshot-error", default_value_t = false, action = clap::ArgAction::SetTrue)]
    keep_alive_on_snapshot_error: bool,
    /// [Debug] Put a fake genesis digest in transport `NodeHello` so honest peers reject this node (pair with `--transport-real`).
    #[arg(long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    broke_trust_test: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliLogFileMode {
    On,
    Off,
    Required,
}

impl From<CliLogFileMode> for LogFileMode {
    fn from(value: CliLogFileMode) -> Self {
        match value {
            CliLogFileMode::On => Self::On,
            CliLogFileMode::Off => Self::Off,
            CliLogFileMode::Required => Self::Required,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliConsoleColorMode {
    Auto,
    Always,
    Never,
}

impl From<CliConsoleColorMode> for ConsoleColorMode {
    fn from(value: CliConsoleColorMode) -> Self {
        match value {
            CliConsoleColorMode::Auto => Self::Auto,
            CliConsoleColorMode::Always => Self::Always,
            CliConsoleColorMode::Never => Self::Never,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum CliSnapBackend {
    JsonFile,
    #[cfg(feature = "clickhouse-snapshot")]
    Clickhouse,
}

#[tokio::main]
async fn main() {
    let shard_arg_used = deprecated_shard_arg_was_used();
    let cli = Cli::parse();
    let compat_shard =
        compat_shard_flag(&cli).map(|raw| match raw.trim().to_ascii_uppercase().as_str() {
            "A" => ShardId::A,
            "B" => ShardId::B,
            other => {
                eprintln!("pwmd: invalid --shard value {other:?}; expected A or B");
                std::process::exit(2);
            }
        });
    let shard = compat_shard.unwrap_or(ShardId::A);
    let genesis_passphrase = resolve_genesis_passphrase(&cli).unwrap_or_else(|e| {
        eprintln!("pwmd startup failed: {e}");
        std::process::exit(2);
    });
    let genesis = match &cli.genesis_file {
        Some(p) => GenesisSource::JsonFile {
            path: p.clone(),
            passphrase: genesis_passphrase.unwrap_or_default(),
        },
        None => GenesisSource::DevNet,
    };
    let listen = cli.listen.unwrap_or_else(|| match compat_shard {
        Some(ShardId::B) => std::net::SocketAddr::from(([127, 0, 0, 1], 3031)),
        _ => std::net::SocketAddr::from(([127, 0, 0, 1], 3030)),
    });
    let has_explicit_identity_input = cli.network_id.is_some()
        || cli.cluster_domain_hi.is_some()
        || cli.cluster_id.is_some()
        || cli.node_id.is_some();
    let identity_input = RuntimeIdentityInput {
        network_id: cli.network_id.map(|v| v.trim().to_string()),
        cluster_domain_hi: match cli.cluster_domain_hi {
            Some(raw) => match parse_cluster_domain_hi(&raw) {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("pwmd startup failed: {e}");
                    std::process::exit(2);
                }
            },
            None => None,
        },
        cluster_id: cli.cluster_id.map(|v| v.trim().to_string()),
        node_id: cli.node_id.map(|v| v.trim().to_string()),
    };
    let identity = if !has_explicit_identity_input && compat_shard.is_none() {
        default_runtime_identity_neutral()
    } else {
        match resolve_runtime_identity(shard, identity_input) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("pwmd startup failed: {e}");
                std::process::exit(2);
            }
        }
    };
    let logging = LoggingConfig {
        log_name: cli.log_name.trim().to_string(),
        log_dir: cli.log_dir.clone(),
        file_template: cli.log_file_template.clone(),
        file_mode: cli.log_file.into(),
        peer_file_template: cli.peer_log_file_template.clone(),
        peer_file_mode: cli.peer_log_file.into(),
        console_color: cli.log_console_color.into(),
        rotate_size_mb: cli.log_rotate_size_mb,
        rotate_max_files: cli.log_rotate_max_files,
    };
    if let Err(e) = init_logging(
        &logging,
        std::io::stderr().is_terminal(),
        Some(&identity.node_id),
    ) {
        match logging.file_mode {
            LogFileMode::Required => {
                eprintln!("pwmd: {e}");
                std::process::exit(2);
            }
            _ => {
                eprintln!("pwmd: logging degraded to console-only: {e}");
                let fallback = LoggingConfig {
                    file_mode: LogFileMode::Off,
                    ..logging.clone()
                };
                if let Err(e2) = init_logging(
                    &fallback,
                    std::io::stderr().is_terminal(),
                    Some(&identity.node_id),
                ) {
                    eprintln!("pwmd: failed to initialize console logging: {e2}");
                    std::process::exit(2);
                }
            }
        }
    }
    let log = logger();
    if shard_arg_used {
        log.info("deprecated --shard compatibility path used; prefer domain-first identity flags");
    }
    let data_file = cli.data_file.unwrap_or_else(|| {
        let ns_dir = if matches!(identity.mode, RuntimeIdentityMode::Neutral) {
            cli.state_root
                .join("neutral")
                .join(neutral_listen_dir_tag(listen))
        } else {
            cli.state_root.join(storage_namespace(&identity))
        };
        ns_dir.join("pwm-data.json")
    });
    let persist_snap = match cli.snapshot_backend {
        CliSnapBackend::JsonFile => PersistSnapKind::JsonFile,
        #[cfg(feature = "clickhouse-snapshot")]
        CliSnapBackend::Clickhouse => PersistSnapKind::ClickHouse,
    };
    let snapshot_verify_chain = cli.snapshot_verify_chain
        || pwm_env_truthy(std::env::var("PWM_SNAPSHOT_VERIFY_CHAIN").ok().as_deref());
    let exit_on_fatal_snapshot = !(cli.keep_alive_on_snapshot_error
        || pwm_env_truthy(
            std::env::var("PWM_KEEP_ALIVE_ON_SNAPSHOT_ERROR")
                .ok()
                .as_deref(),
        ));
    let cfg = PwmdConfig {
        listen,
        genesis,
        data_file,
        #[cfg(feature = "clickhouse-snapshot")]
        clickhouse_url: cli.clickhouse_url,
        #[cfg(feature = "clickhouse-snapshot")]
        clickhouse_database: cli.clickhouse_database,
        #[cfg(feature = "clickhouse-snapshot")]
        clickhouse_table: cli.clickhouse_table,
        #[cfg(feature = "clickhouse-snapshot")]
        snapshot_store_key: cli.snapshot_store_key,
        #[cfg(feature = "clickhouse-snapshot")]
        snapshot_ch_fallback_json: snapshot_ch_fallback_enabled(cli.no_snapshot_ch_fallback),
        persist_snap,
        snapshot_verify_chain,
        exit_on_fatal_snapshot,
        broke_trust_test: cli.broke_trust_test,
        shard,
        identity,
        transport: TransportConfig {
            enabled: cli.transport_real,
            peer_listen: resolve_peer_listen(cli.transport_peer_listen, listen).unwrap_or_else(
                |e| {
                    eprintln!("pwmd startup failed: {e}");
                    std::process::exit(2);
                },
            ),
            peer_seeds: cli.transport_peer_seed,
            relay_http_seeds: cli.transport_relay_http_seed,
            connect_timeout_ms: cli.transport_connect_timeout_ms,
            handshake_timeout_ms: cli.transport_handshake_timeout_ms,
            heartbeat_interval_ms: cli.transport_heartbeat_interval_ms.max(200),
            heartbeat_timeout_ms: cli.transport_heartbeat_timeout_ms.max(500),
            retry_base_ms: cli.transport_retry_base_ms,
            retry_max_ms: cli.transport_retry_max_ms,
            soak_counter_cap: cli.transport_soak_counter_cap,
            soak_health_interval_ticks: cli.transport_soak_health_interval_ticks,
            reconnect_runaway_streak_limit: cli.transport_runaway_streak_limit,
            reconnect_runaway_cooldown_ms: cli.transport_runaway_cooldown_ms,
        },
        logging,
    };
    if let Err(e) = pwmd::run_with(cfg).await {
        log.error(&format!("pwmd runtime failed: {e}"));
        std::process::exit(1);
    }
}

#[cfg(feature = "clickhouse-snapshot")]
fn snapshot_ch_fallback_enabled(cli_disables: bool) -> bool {
    if cli_disables {
        return false;
    }
    match std::env::var("PWM_SNAPSHOT_CH_FALLBACK_JSON") {
        Ok(s) => {
            let t = s.trim().to_ascii_lowercase();
            !(t.is_empty() || t == "0" || t == "false" || t == "no" || t == "off")
        }
        Err(_) => true,
    }
}

fn pwm_env_truthy(raw: Option<&str>) -> bool {
    let Some(s) = raw else {
        return false;
    };
    let t = s.trim().to_ascii_lowercase();
    !(t.is_empty() || t == "0" || t == "false" || t == "no" || t == "off")
}

fn deprecated_shard_arg_was_used() -> bool {
    std::env::args_os().skip(1).any(|arg| {
        let v = arg.to_string_lossy();
        v == "--shard" || v.starts_with("--shard=")
    })
}

/// Reads deprecated `--shard`; isolated so `main` stays free of `deprecated` field warnings.
#[allow(deprecated)]
fn compat_shard_flag(cli: &Cli) -> Option<&str> {
    cli.shard.as_deref()
}

fn resolve_genesis_passphrase(cli: &Cli) -> Result<Option<String>, String> {
    if cli.genesis_file.is_none() {
        return Ok(None);
    }
    if let Some(pass) = cli.genesis_passphrase.as_deref() {
        if pass.trim().is_empty() {
            return Err("genesis passphrase must not be empty".to_string());
        }
        return Ok(Some(pass.to_string()));
    }
    if !std::io::stdin().is_terminal() {
        return Err(
            "missing genesis passphrase in non-tty mode: pass --genesis-passphrase or set PWM_GENESIS_PASSPHRASE"
                .to_string(),
        );
    }
    let pass = rpassword::prompt_password("Enter genesis passphrase: ")
        .map_err(|e| format!("failed to read genesis passphrase: {e}"))?;
    if pass.trim().is_empty() {
        return Err("genesis passphrase must not be empty".to_string());
    }
    Ok(Some(pass))
}

fn resolve_peer_listen(
    explicit: Option<std::net::SocketAddr>,
    rpc_listen: std::net::SocketAddr,
) -> Result<std::net::SocketAddr, String> {
    let peer = if let Some(addr) = explicit {
        addr
    } else {
        let rpc_port = rpc_listen.port() as u32;
        let Some(peer_port) = rpc_port.checked_add(100) else {
            return Err(format!(
                "cannot derive peer listen port from rpc listen {rpc_listen}: rpc_port+100 overflow"
            ));
        };
        if peer_port > u16::MAX as u32 {
            return Err(format!(
                "cannot derive peer listen port from rpc listen {rpc_listen}: rpc_port+100 exceeds u16"
            ));
        }
        std::net::SocketAddr::new(rpc_listen.ip(), peer_port as u16)
    };
    if peer == rpc_listen {
        return Err(format!(
            "peer listen address must differ from rpc listen address: both are {peer}"
        ));
    }
    Ok(peer)
}

#[cfg(test)]
mod tests {
    use super::resolve_peer_listen;
    use std::net::SocketAddr;

    /// Default derived peer listens at rpc_listen+100 (formerly `peer_listen_defaults_to_rpc_plus_100`).
    #[test]
    fn plist_follow_rpc_der() {
        let rpc = SocketAddr::from(([127, 0, 0, 1], 3030));
        let peer = resolve_peer_listen(None, rpc).expect("derived peer listen");
        assert_eq!(peer, SocketAddr::from(([127, 0, 0, 1], 3130)));
    }

    #[test]
    fn peer_listen_prefers_explicit_value() {
        let rpc = SocketAddr::from(([127, 0, 0, 1], 3030));
        let explicit = SocketAddr::from(([127, 0, 0, 1], 4040));
        let peer = resolve_peer_listen(Some(explicit), rpc).expect("explicit peer listen");
        assert_eq!(peer, explicit);
    }

    /// Reject peer listen accidentally reusing rpc socket (formerly `peer_listen_rejects_rpc_socket_reuse`).
    #[test]
    fn peer_listen_reuse_rpc_bad() {
        let rpc = SocketAddr::from(([127, 0, 0, 1], 3030));
        let err = resolve_peer_listen(Some(rpc), rpc).expect_err("must reject same socket");
        assert!(err.contains("must differ"));
    }
}
