//! Devnet node binary: REST `/v1/*`, PoA seal loop.

use clap::{Parser, ValueEnum};
use pwmd::handshake::{ClusterRole, DeploymentProfile, SealRole};
use pwmd::{
    default_runtime_identity_neutral, init_logging, logger, neutral_listen_dir_tag,
    parse_cluster_domain_hi, resolve_runtime_identity, storage_namespace, ClusterCfg,
    ConsoleColorMode, DebugDumpCfg, DevLane, GenesisSource, LeaseBackendMode, LogFileMode,
    LoggingConfig, PersistSnapKind, PwmdConfig, RuntimeIdentityInput, RuntimeIdentityMode,
    SealControlMode, TransportConfig,
};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Parser)]
#[command(name = "pwmd")]
struct Cli {
    /// Two local dev lanes require explicit identity flags:
    /// `--domain-hi`/`--cluster-domain-hi` + `--cluster-id` + `--node-id`.
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
    /// Stable peer `node_instance_id` (RFC16 `cluster-members` must match this and peers). Default is `{node_id}-{pid}-{time_ms}`.
    #[arg(long, env = "PWM_NODE_INSTANCE_ID")]
    node_instance_id: Option<String>,
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
    /// Static RPC source IP allowlist entries (`IP` or `CIDR`, comma-separated).
    #[arg(
        long = "rpc-allowed-ip",
        env = "PWM_RPC_ALLOWED_IPS",
        value_delimiter = ','
    )]
    rpc_allowed_ips: Vec<String>,
    /// Seconds after startup during which RPC source IPs auto-enroll into the allowlist.
    #[arg(
        long = "rpc-allowed-auto",
        env = "PWM_RPC_ALLOWED_AUTO",
        default_value_t = 0
    )]
    rpc_allowed_auto: u16,
    /// Enable real socket transport loop (seed connect + NodeHello handshake).
    #[arg(long, default_value_t = false)]
    transport_real: bool,
    /// Dedicated peer listener address for stateful peer sessions (must not match RPC listen).
    #[arg(long, env = "PWM_PEER_LISTEN")]
    transport_peer_listen: Option<std::net::SocketAddr>,
    /// Comma-separated seed peers for real transport (e.g. 127.0.0.1:4040,127.0.0.1:4041).
    #[arg(long, value_delimiter = ',', value_name = "ADDR[,ADDR...]")]
    transport_peer_seed: Vec<std::net::SocketAddr>,
    /// Optional YAML file with bootstrap peers.
    /// Legacy v1: `peers: ["127.0.0.1:13030"]`.
    /// Multi-shard v2: `shards: {"0x2C": [{id, peer, validator}]}` (keys: `0xNN` or `0..255`).
    /// If omitted, pwmd checks `<state_root>/peers.yaml` and uses it only when the file exists.
    #[arg(long, value_name = "PATH")]
    peers_list: Option<PathBuf>,
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
    #[arg(long = "transport-soak-health-interval-ticks", default_value_t = 0)]
    transport_soak_health_ticks: u64,
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
    keep_alive_snapshot_error: bool,
    /// [Debug] Put a fake genesis digest in transport `NodeHello` so honest peers reject this node (pair with `--transport-real`).
    #[arg(long, default_value_t = false, action = clap::ArgAction::SetTrue)]
    broke_trust_test: bool,
    /// [Test-only] Graceful stop once canonical height reaches/exceeds this value.
    #[arg(long, value_name = "HEIGHT")]
    debug_stop_height: Option<u64>,
    /// [Test/dev-only] Deterministic seal timestamp source (`base + height`) for hash-parity runs.
    #[arg(
        long = "debug-deterministic-seal-time",
        default_value_t = false,
        action = clap::ArgAction::SetTrue
    )]
    debug_deterministic_seal_time: bool,
    /// [Test/dev-only] Align local seal attempts around second midpoint (`~500ms`) to reduce wall-clock drift.
    #[arg(
        long = "debug-align-seal-mid-second",
        default_value_t = false,
        action = clap::ArgAction::SetTrue
    )]
    debug_align_seal_mid: bool,
    /// [Test/dev-only] Disable periodic local seal-loop for non-cluster follower/replay harnesses.
    /// In cluster mode, `--cluster-role attester` already implies no local competing seal loop (RFC16 §8.2).
    #[arg(
        long = "debug-disable-seal-loop",
        default_value_t = false,
        action = clap::ArgAction::SetTrue
    )]
    debug_disable_seal_loop: bool,
    /// Enable lab seal RPC surface on the local proposer.
    #[arg(long = "lab-seal-api", env = "PWM_LAB_SEAL_API", default_value_t = false, action = clap::ArgAction::SetTrue)]
    lab_seal_api: bool,
    /// Lab seal control mode.
    #[arg(
        long = "seal-control",
        env = "PWM_SEAL_CONTROL",
        default_value = "auto",
        value_parser = parse_seal_control_mode
    )]
    seal_control_mode: SealControlMode,
    /// Runtime deployment profile for same-validator guard policy.
    #[arg(
        long = "deployment-profile",
        env = "PWM_DEPLOYMENT_PROFILE",
        value_enum,
        default_value_t = CliDeploymentProfile::SingleSealer
    )]
    deployment_profile: CliDeploymentProfile,
    /// Optional explicit local seal role override (`active`/`standby`).
    #[arg(long = "seal-role", env = "PWM_SEAL_ROLE", value_enum)]
    seal_role: Option<CliSealRole>,
    /// Single-sealer lease TTL in milliseconds (active renew window).
    #[arg(
        long = "seal-lease-ttl-ms",
        env = "PWM_SEAL_LEASE_TTL_MS",
        default_value_t = 10_000
    )]
    seal_lease_ttl_ms: u64,
    /// Standby takeover timeout after observed lease expiry (milliseconds).
    #[arg(
        long = "seal-takeover-timeout-ms",
        env = "PWM_SEAL_TAKEOVER_TIMEOUT_MS",
        default_value_t = 8_000
    )]
    seal_takeover_timeout_ms: u64,
    /// Max local tip lag tolerated before standby can take over.
    #[arg(
        long = "seal-takeover-max-tip-lag",
        env = "PWM_SEAL_TAKEOVER_MAX_TIP_LAG",
        default_value_t = 1
    )]
    seal_takeover_tip_lag: u64,
    /// Lease backend mode for single-sealer gate (`file` fail-closed default, or explicit `process-local` fallback).
    #[arg(
        long = "seal-lease-backend",
        env = "PWM_SEAL_LEASE_BACKEND",
        value_enum,
        default_value_t = CliLeaseBackend::File
    )]
    seal_lease_backend: CliLeaseBackend,
    /// Directory that stores per-validator lease files for `file` backend.
    #[arg(
        long = "seal-lease-dir",
        env = "PWM_SEAL_LEASE_DIR",
        value_name = "DIR"
    )]
    seal_lease_dir: Option<PathBuf>,
    /// [Debug] Dump local block JSON after persistent sync-tip divergence trigger.
    #[arg(
        long = "debug-dump-on-divergence",
        default_value_t = false,
        action = clap::ArgAction::SetTrue
    )]
    debug_dump_on_divergence: bool,
    /// [Debug] Output directory for block dumps (default: `<data_file_parent>/blocks`).
    #[arg(
        long = "debug-dump-dir",
        env = "PWM_DEBUG_DUMP_DIR",
        value_name = "DIR"
    )]
    debug_dump_dir: Option<PathBuf>,
    /// [Debug] Upper bound for number of dump files written by this process.
    #[arg(
        long = "debug-dump-cap",
        env = "PWM_DEBUG_DUMP_CAP",
        default_value_t = 16
    )]
    debug_dump_cap: u64,
    /// [Debug] Consecutive divergence observations needed before writing a dump.
    #[arg(
        long = "debug-dump-trigger-streak",
        env = "PWM_DEBUG_DUMP_TRIGGER_STREAK",
        default_value_t = 2
    )]
    debug_dump_trigger_streak: u8,
    /// Enable RFC16 cluster attestation logic (default off).
    #[arg(long = "cluster-enabled", default_value_t = false, action = clap::ArgAction::SetTrue)]
    cluster_enabled: bool,
    /// RFC16 role for cluster mode; `attester` implies standby/no local seal loop by role derivation (RFC16 §8.2).
    #[arg(long = "cluster-role", env = "PWM_CLUSTER_ROLE", value_enum, default_value_t = CliClusterRole::None)]
    cluster_role: CliClusterRole,
    /// Comma-separated static cluster members (`node_instance_id` values; use `--node-instance-id` on each node so these match wire).
    #[arg(
        long = "cluster-members",
        value_delimiter = ',',
        value_name = "ID[,ID...]"
    )]
    cluster_members: Vec<String>,
    /// RFC16 §7: attester ACK count (`k`) excludes proposer (2-of-2 intent => k=1, n=2).
    #[arg(long = "cluster-quorum-k", default_value_t = 1)]
    cluster_quorum_k: u8,
    /// Quorum N in k-of-n when cluster mode is enabled (limited to <=3 in this slice).
    #[arg(long = "cluster-quorum-n", default_value_t = 2)]
    cluster_quorum_n: u8,
    /// Proposer: eager cluster propose this many ms before seal grid deadline (0 = off).
    #[arg(
        long = "cluster-seal-ahead-ms",
        env = "PWM_CLUSTER_SEAL_AHEAD_MS",
        default_value_t = 100
    )]
    cluster_seal_ahead_ms: u64,
    /// Proposer: include full block bodies in ClusterPropose.tail_blocks (default sends lean proposals).
    #[arg(long = "cluster-propose-full-blocks", default_value_t = false, action = clap::ArgAction::SetTrue)]
    cluster_prop_full_blocks: bool,
    /// Max local-vs-attester tip lag tolerated for sync-ready preflight.
    #[arg(
        long = "cluster-attest-max-tip-lag",
        env = "PWM_CLUSTER_ATTEST_MAX_TIP_LAG",
        default_value_t = 1
    )]
    cluster_att_tip_lag: u64,
    /// Enable per-block cluster timing JSONL capture.
    #[arg(long = "block-timing-enabled", default_value_t = false)]
    block_timing_enabled: bool,
    /// Shared JSONL path for per-block cluster timing (must be identical for proposer+attester).
    #[arg(long = "block-timing-path", env = "PWM_BLOCK_TIMING_PATH")]
    block_timing_path: Option<PathBuf>,
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

#[derive(Clone, Copy, Debug, ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum CliDeploymentProfile {
    SingleSealer,
    MultiSealerExperimental,
}

impl From<CliDeploymentProfile> for DeploymentProfile {
    fn from(value: CliDeploymentProfile) -> Self {
        match value {
            CliDeploymentProfile::SingleSealer => Self::SingleSealer,
            CliDeploymentProfile::MultiSealerExperimental => Self::MultiSealerExperimental,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum CliSealRole {
    Active,
    Standby,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum CliLeaseBackend {
    File,
    ProcessLocal,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum CliClusterRole {
    None,
    Proposer,
    Attester,
}

impl From<CliLeaseBackend> for LeaseBackendMode {
    fn from(value: CliLeaseBackend) -> Self {
        match value {
            CliLeaseBackend::File => Self::File,
            CliLeaseBackend::ProcessLocal => Self::ProcessLocal,
        }
    }
}

impl From<CliSealRole> for SealRole {
    fn from(value: CliSealRole) -> Self {
        match value {
            CliSealRole::Active => Self::Active,
            CliSealRole::Standby => Self::Standby,
        }
    }
}

impl From<CliClusterRole> for ClusterRole {
    fn from(value: CliClusterRole) -> Self {
        match value {
            CliClusterRole::None => Self::None,
            CliClusterRole::Proposer => Self::Proposer,
            CliClusterRole::Attester => Self::Attester,
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let dev_lane = DevLane::Lane0;
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
    let listen = cli
        .listen
        .unwrap_or(std::net::SocketAddr::from(([127, 0, 0, 1], 3030)));
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
    let identity = if !has_explicit_identity_input {
        default_runtime_identity_neutral()
    } else {
        match resolve_runtime_identity(dev_lane, identity_input) {
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
    log_build_control(log);
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
    let seal_lease_dir = cli
        .seal_lease_dir
        .unwrap_or_else(|| cli.state_root.join("leases"));
    let persist_snap = match cli.snapshot_backend {
        CliSnapBackend::JsonFile => PersistSnapKind::JsonFile,
        #[cfg(feature = "clickhouse-snapshot")]
        CliSnapBackend::Clickhouse => PersistSnapKind::ClickHouse,
    };
    let snapshot_verify_chain = cli.snapshot_verify_chain
        || pwm_env_truthy(std::env::var("PWM_SNAPSHOT_VERIFY_CHAIN").ok().as_deref());
    let exit_on_fatal_snapshot = !(cli.keep_alive_snapshot_error
        || pwm_env_truthy(
            std::env::var("PWM_KEEP_ALIVE_ON_SNAPSHOT_ERROR")
                .ok()
                .as_deref(),
        ));
    let debug_det_seal_time = cli.debug_deterministic_seal_time
        || pwm_env_truthy(
            std::env::var("PWM_DEBUG_DETERMINISTIC_SEAL_TIME")
                .ok()
                .as_deref(),
        );
    let debug_align_mid = cli.debug_align_seal_mid
        || pwm_env_truthy(
            std::env::var("PWM_DEBUG_ALIGN_SEAL_MID_SECOND")
                .ok()
                .as_deref(),
        );
    let debug_disable_seal_loop = cli.debug_disable_seal_loop
        || pwm_env_truthy(std::env::var("PWM_DEBUG_DISABLE_SEAL_LOOP").ok().as_deref());
    let lab_seal_api =
        cli.lab_seal_api || pwm_env_truthy(std::env::var("PWM_LAB_SEAL_API").ok().as_deref());
    let block_timing_enabled = cli.block_timing_enabled
        || pwm_env_truthy(std::env::var("PWM_BLOCK_TIMING_ENABLED").ok().as_deref());
    let debug_dump_on_div = cli.debug_dump_on_divergence
        || pwm_env_truthy(
            std::env::var("PWM_DEBUG_DUMP_ON_DIVERGENCE")
                .ok()
                .as_deref(),
        );
    let peer_listen = resolve_peer_listen(cli.transport_peer_listen, listen).unwrap_or_else(|e| {
        eprintln!("pwmd startup failed: {e}");
        std::process::exit(2);
    });
    let peer_list_explicit = cli.peers_list.is_some();
    let peer_list_path = pwmd::pick_peer_file(cli.peers_list.as_deref(), &cli.state_root);
    let peer_file_loaded = peer_list_path.as_ref().map(|path| {
        pwmd::load_peer_file(
            path.as_path(),
            identity.cluster_domain_hi,
            peer_list_explicit,
        )
        .unwrap_or_else(|e| {
            eprintln!("pwmd startup failed: {e}");
            std::process::exit(2);
        })
    });
    let (peer_file_seeds, peer_file_state) = if let Some(loaded) = peer_file_loaded {
        (loaded.seeds, Some(loaded.state))
    } else {
        (Vec::new(), None)
    };
    let mut effective_peer_seeds =
        pwmd::merge_peer_seeds(&peer_file_seeds, &cli.transport_peer_seed);
    pwmd::drop_self_seed(&mut effective_peer_seeds, peer_listen);
    let cfg = PwmdConfig {
        listen,
        genesis,
        data_file,
        rpc_allowed_ips: cli.rpc_allowed_ips,
        rpc_allowed_auto: cli.rpc_allowed_auto,
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
        debug_stop_height: cli.debug_stop_height,
        debug_det_seal_time,
        debug_align_mid,
        debug_disable_seal_loop,
        lab_seal_api,
        seal_control_mode: cli.seal_control_mode,
        deployment_profile: cli.deployment_profile.into(),
        seal_role_override: cli.seal_role.map(Into::into),
        seal_lease_ttl_ms: cli.seal_lease_ttl_ms.max(1_000),
        seal_takeover_timeout_ms: cli.seal_takeover_timeout_ms.max(1_000),
        seal_takeover_tip_lag: cli.seal_takeover_tip_lag,
        seal_lease_backend: cli.seal_lease_backend.into(),
        seal_lease_dir,
        debug_dump: DebugDumpCfg {
            on_divergence: debug_dump_on_div,
            dir: cli.debug_dump_dir,
            max_files: cli.debug_dump_cap.max(1),
            trigger_streak: cli.debug_dump_trigger_streak.max(2),
        },
        cluster: ClusterCfg {
            enabled: cli.cluster_enabled,
            role: cli.cluster_role.into(),
            members: cli
                .cluster_members
                .iter()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .collect(),
            quorum_k: cli.cluster_quorum_k,
            quorum_n: cli.cluster_quorum_n,
            attest_timeout_ms: ClusterCfg::default().attest_timeout_ms,
            seal_ahead_ms: cli.cluster_seal_ahead_ms,
            full_blocks: cli.cluster_prop_full_blocks,
            block_timing_path: if block_timing_enabled {
                Some(
                    cli.block_timing_path
                        .clone()
                        .unwrap_or_else(|| PathBuf::from("tmp/cy-lab-block-timing.jsonl")),
                )
            } else {
                None
            },
            att_max_tip_lag: cli.cluster_att_tip_lag,
        },
        node_instance_id_override: cli
            .node_instance_id
            .as_ref()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        shard: dev_lane,
        identity,
        transport: TransportConfig {
            enabled: cli.transport_real,
            peer_listen,
            peer_seeds: effective_peer_seeds.clone(),
            relay_http_seeds: cli.transport_relay_http_seed,
            connect_timeout_ms: cli.transport_connect_timeout_ms,
            handshake_timeout_ms: cli.transport_handshake_timeout_ms,
            heartbeat_interval_ms: cli.transport_heartbeat_interval_ms.max(200),
            heartbeat_timeout_ms: cli.transport_heartbeat_timeout_ms.max(500),
            retry_base_ms: cli.transport_retry_base_ms,
            retry_max_ms: cli.transport_retry_max_ms,
            soak_counter_cap: cli.transport_soak_counter_cap,
            soak_health_interval_ticks: cli.transport_soak_health_ticks,
            reconnect_runaway_streak_limit: cli.transport_runaway_streak_limit,
            reconnect_runaway_cooldown_ms: cli.transport_runaway_cooldown_ms,
        },
        logging,
    };
    if let Err(e) = pwmd::run_with(cfg).await {
        log.error(&format!("pwmd runtime failed: {e}"));
        std::process::exit(1);
    }
    if let (Some(path), Some(state)) = (peer_list_path.as_ref(), peer_file_state.as_ref()) {
        if let Err(e) = pwmd::save_peer_file(path.as_path(), state, &effective_peer_seeds) {
            log.error(&format!("pwmd peer list save failed: {e}"));
            std::process::exit(1);
        }
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

fn parse_seal_control_mode(raw: &str) -> Result<SealControlMode, String> {
    SealControlMode::parse(raw)
}

const BUILD_TS_ENV: Option<&str> = option_env!("PWM_BUILD_TIMESTAMP_UTC");
const BUILD_GIT_ENV: Option<&str> = option_env!("PWM_GIT_SHA");

fn build_marker() -> String {
    let mut marker = format!("pwmd/{}", env!("CARGO_PKG_VERSION"));
    if let Some(ts) = BUILD_TS_ENV {
        if !ts.trim().is_empty() {
            marker.push_str("+ts:");
            marker.push_str(ts.trim());
        }
    }
    if let Some(sha) = BUILD_GIT_ENV {
        if !sha.trim().is_empty() {
            marker.push_str("+git:");
            marker.push_str(sha.trim());
        }
    }
    marker
}

fn binary_meta_fields(path: &Path) -> (String, String) {
    let path_field = path.display().to_string();
    match std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|mtime| mtime.duration_since(UNIX_EPOCH).ok())
    {
        Some(dur) => (path_field, format!("{}ms", dur.as_millis())),
        None => (path_field, "unavailable".to_string()),
    }
}

fn log_build_control(log: pwmd::NodeLogger) {
    match std::env::current_exe() {
        Ok(path) => {
            let (path_field, mtime_field) = binary_meta_fields(path.as_path());
            log.info(&format!(
                "build control marker={} binary_path={} binary_mtime_utc_unix={} pid={}",
                build_marker(),
                path_field,
                mtime_field,
                std::process::id()
            ));
        }
        Err(err) => {
            log.info(&format!(
                "build control marker={} binary_path=unavailable binary_mtime_utc_unix=unavailable pid={} reason={}",
                build_marker(),
                std::process::id(),
                err
            ));
        }
    }
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
    use super::{binary_meta_fields, build_marker, resolve_peer_listen};
    use std::fs;
    use std::net::SocketAddr;
    use std::path::PathBuf;

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

    #[test]
    fn build_ctl_marker_has_ver() {
        let marker = build_marker();
        assert!(marker.starts_with("pwmd/"));
    }

    #[test]
    fn binary_meta_marks_missing() {
        let uniq = format!(
            "pwmd-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        );
        let path = PathBuf::from(std::env::temp_dir()).join(uniq);
        let (_, mtime_field) = binary_meta_fields(path.as_path());
        assert_eq!(mtime_field, "unavailable");
    }

    #[test]
    fn binary_meta_reads_mtime() {
        let uniq = format!(
            "pwmd-meta-{}-{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        );
        let path = PathBuf::from(std::env::temp_dir()).join(uniq);
        fs::write(&path, b"pwmd").expect("create temp binary marker");
        let (_, mtime_field) = binary_meta_fields(path.as_path());
        assert!(mtime_field.ends_with("ms"));
        let _ = fs::remove_file(path);
    }
}
