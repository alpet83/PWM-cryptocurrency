//! Resolved daemon configuration: listen addr, genesis source, logging, transport.

use crate::default_runtime_identity_neutral;
use crate::snapshot::SnapshotBackend;
use crate::RuntimeIdentity;
use crate::ShardId;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Where to load genesis + validator signing keys from.
#[derive(Clone, Debug)]
pub enum GenesisSource {
    DevNet,
    JsonFile { path: PathBuf, passphrase: String },
}

/// Persisted autosnapshot routing (default file JSON matching [`PwmdConfig::data_file`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistSnapKind {
    JsonFile,
    #[cfg(feature = "clickhouse-snapshot")]
    ClickHouse,
}

impl Default for PersistSnapKind {
    fn default() -> Self {
        Self::JsonFile
    }
}

/// Runtime options for `pwmd` (domain-first identity; internal `ShardId` for Phase1 guards).
#[derive(Clone, Debug)]
pub struct PwmdConfig {
    pub listen: SocketAddr,
    pub genesis: GenesisSource,
    pub data_file: PathBuf,
    #[cfg(feature = "clickhouse-snapshot")]
    pub clickhouse_url: Option<String>,
    #[cfg(feature = "clickhouse-snapshot")]
    pub clickhouse_database: String,
    #[cfg(feature = "clickhouse-snapshot")]
    pub clickhouse_table: String,
    #[cfg(feature = "clickhouse-snapshot")]
    pub snapshot_store_key: Option<String>,
    /// When `persist_snap == ClickHouse`, write seal/tip to `data_file` if CH HTTP fails (default: on).
    #[cfg(feature = "clickhouse-snapshot")]
    pub snapshot_ch_fallback_json: bool,
    pub persist_snap: PersistSnapKind,
    /// When true, epoch snapshots replay genesis→tip on load (audit). Default false: trust checkpoint + tail blocks.
    pub snapshot_verify_chain: bool,
    /// When true, unrecoverable snapshot load / into-runtime failures terminate the process instead of `ready_degraded`.
    pub exit_on_fatal_snapshot: bool,
    /// Debug only: fake genesis digest in peer `NodeHello` (peers reject; use with `--transport-real`).
    pub broke_trust_test: bool,
    pub shard: ShardId,
    pub identity: RuntimeIdentity,
    pub transport: TransportConfig,
    pub logging: LoggingConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogFileMode {
    On,
    Off,
    Required,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConsoleColorMode {
    Auto,
    Always,
    Never,
}

impl ConsoleColorMode {
    pub fn use_ansi(self, is_tty: bool) -> bool {
        match self {
            Self::Auto => is_tty,
            Self::Always => true,
            Self::Never => false,
        }
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "always" => Ok(Self::Always),
            "never" => Ok(Self::Never),
            other => Err(format!(
                "invalid console color mode {other:?}; expected auto|always|never"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoggingConfig {
    pub log_name: String,
    pub log_dir: PathBuf,
    pub file_template: String,
    pub file_mode: LogFileMode,
    pub peer_file_template: String,
    pub peer_file_mode: LogFileMode,
    pub console_color: ConsoleColorMode,
    pub rotate_size_mb: u64,
    pub rotate_max_files: usize,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_name: "pwmd".to_string(),
            log_dir: PathBuf::from("logs"),
            file_template: "{date}/{log_name}-{node_id}-{time}.log".to_string(),
            file_mode: LogFileMode::On,
            peer_file_template: "{date}/pwmd-peer-{node_id}-{time}.log".to_string(),
            peer_file_mode: LogFileMode::On,
            console_color: ConsoleColorMode::Auto,
            rotate_size_mb: 32,
            rotate_max_files: 7,
        }
    }
}

impl LoggingConfig {
    pub const MIN_ROTATE_SIZE_MB: u64 = 1;
    pub const MAX_ROTATE_SIZE_MB: u64 = 1024;
    pub const MIN_ROTATE_MAX_FILES: usize = 1;
    pub const MAX_ROTATE_MAX_FILES: usize = 1024;

    pub fn validate(&self) -> Result<(), String> {
        if self.log_name.trim().is_empty() {
            return Err("log name must not be empty".to_string());
        }
        if self.rotate_size_mb < Self::MIN_ROTATE_SIZE_MB
            || self.rotate_size_mb > Self::MAX_ROTATE_SIZE_MB
        {
            return Err(format!(
                "log rotate size must be in [{}..={}] MB",
                Self::MIN_ROTATE_SIZE_MB,
                Self::MAX_ROTATE_SIZE_MB
            ));
        }
        if self.rotate_max_files < Self::MIN_ROTATE_MAX_FILES
            || self.rotate_max_files > Self::MAX_ROTATE_MAX_FILES
        {
            return Err(format!(
                "log rotate max files must be in [{}..={}]",
                Self::MIN_ROTATE_MAX_FILES,
                Self::MAX_ROTATE_MAX_FILES
            ));
        }
        Ok(())
    }
}

impl LogFileMode {
    pub fn parse(raw: &str) -> Result<Self, String> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "on" => Ok(Self::On),
            "off" => Ok(Self::Off),
            "required" => Ok(Self::Required),
            other => Err(format!(
                "invalid log file mode {other:?}; expected on|off|required"
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub struct TransportConfig {
    pub enabled: bool,
    pub peer_listen: SocketAddr,
    pub peer_seeds: Vec<SocketAddr>,
    /// HTTP bases for one-window relay (`/v1/status`, `/v1/export-provenance`). Empty ⇒ derive from
    /// `peer_seeds` using convention `rpc_port = peer_tcp_port - 100` (inverse of default `--listen`+100 peer).
    pub relay_http_seeds: Vec<SocketAddr>,
    pub connect_timeout_ms: u64,
    pub handshake_timeout_ms: u64,
    pub heartbeat_interval_ms: u64,
    pub heartbeat_timeout_ms: u64,
    pub retry_base_ms: u64,
    pub retry_max_ms: u64,
    pub soak_counter_cap: u64,
    pub soak_health_interval_ticks: u64,
    pub reconnect_runaway_streak_limit: u32,
    pub reconnect_runaway_cooldown_ms: u64,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            peer_listen: SocketAddr::from(([127, 0, 0, 1], 3130)),
            peer_seeds: Vec::new(),
            relay_http_seeds: Vec::new(),
            connect_timeout_ms: 1_000,
            handshake_timeout_ms: 1_000,
            heartbeat_interval_ms: 1_500,
            heartbeat_timeout_ms: 4_500,
            retry_base_ms: 500,
            retry_max_ms: 60_000,
            soak_counter_cap: 1_000_000,
            soak_health_interval_ticks: 0,
            reconnect_runaway_streak_limit: 12,
            reconnect_runaway_cooldown_ms: 60_000,
        }
    }
}

impl Default for PwmdConfig {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 3030)),
            genesis: GenesisSource::DevNet,
            // Neutral default isolates snapshot path per RPC listen (see main.rs).
            data_file: PathBuf::from("state/neutral/127.0.0.1+3030/pwm-data.json"),
            #[cfg(feature = "clickhouse-snapshot")]
            clickhouse_url: None,
            #[cfg(feature = "clickhouse-snapshot")]
            clickhouse_database: "pwm_snapshots".into(),
            #[cfg(feature = "clickhouse-snapshot")]
            clickhouse_table: "node_snapshot".into(),
            #[cfg(feature = "clickhouse-snapshot")]
            snapshot_store_key: None,
            #[cfg(feature = "clickhouse-snapshot")]
            snapshot_ch_fallback_json: true,
            persist_snap: PersistSnapKind::default(),
            snapshot_verify_chain: false,
            exit_on_fatal_snapshot: true,
            broke_trust_test: false,
            shard: ShardId::A,
            identity: default_runtime_identity_neutral(),
            transport: TransportConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl PwmdConfig {
    /// Cross-field checks for snapshot routing (genesis-independent).
    pub fn validate_persist_snap(&self) -> Result<(), String> {
        #[cfg(feature = "clickhouse-snapshot")]
        if self.persist_snap == PersistSnapKind::ClickHouse {
            let u = self
                .clickhouse_url
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    String::from(
                        "clickhouse snapshot requires --clickhouse-url or PWM_CLICKHOUSE_URL",
                    )
                })?;
            crate::snapshot::ch_http::norm_ch_http_base(u)?;
            let db = crate::snapshot::ch_http::resolve_ch_database(
                &self.clickhouse_database,
                &self.identity.network_id,
            )?;
            crate::snapshot::ch_http::snap_ch_sql_id(&db)?;
            let (tb, tc) = crate::snapshot::ch_http::snap_ch_tbl_pair(
                self.identity.cluster_domain_hi,
                &self.clickhouse_table,
            );
            let tv = crate::snapshot::ch_http::snap_ch_tbl_validators(
                self.identity.cluster_domain_hi,
                &self.clickhouse_table,
            );
            crate::snapshot::ch_http::snap_ch_sql_id(&tb)?;
            crate::snapshot::ch_http::snap_ch_sql_id(&tc)?;
            crate::snapshot::ch_http::snap_ch_sql_id(&tv)?;
            crate::snapshot::ch_http::snap_ch_sql_id("node_snapshot")?;
        }
        Ok(())
    }

    /// Startup log hint for whichever persistence routing is configured.
    pub fn persist_diag_hint(&self) -> String {
        match self.persist_snap {
            PersistSnapKind::JsonFile => format!("json_file path={}", self.data_file.display()),
            #[cfg(feature = "clickhouse-snapshot")]
            PersistSnapKind::ClickHouse => {
                let url = self
                    .clickhouse_url
                    .as_deref()
                    .unwrap_or("(unset)")
                    .to_string();
                let (tb, tc) = crate::snapshot::ch_http::snap_ch_tbl_pair(
                    self.identity.cluster_domain_hi,
                    &self.clickhouse_table,
                );
                let tv = crate::snapshot::ch_http::snap_ch_tbl_validators(
                    self.identity.cluster_domain_hi,
                    &self.clickhouse_table,
                );
                let fb = if self.snapshot_ch_fallback_json {
                    format!(" json_fallback={}", self.data_file.display())
                } else {
                    String::new()
                };
                format!("clickhouse url={url} blocks={tb} checkpoints={tc} val_acc={tv}{fb}")
            }
        }
    }

    /// Builds autosnapshot/load backend (needs genesis state0 digest hex for CH row identity).
    #[allow(unused_variables)]
    pub(crate) fn persisted_snap_backend(
        &self,
        genesis_st0_digest_hex: &str,
    ) -> Result<SnapshotBackend, String> {
        match self.persist_snap {
            PersistSnapKind::JsonFile => Ok(SnapshotBackend::JsonFile {
                path: self.data_file.clone(),
            }),
            #[cfg(feature = "clickhouse-snapshot")]
            PersistSnapKind::ClickHouse => {
                let url = self
                    .clickhouse_url
                    .as_deref()
                    .ok_or_else(|| "clickhouse-url missing".to_string())?;
                let base = crate::snapshot::ch_http::norm_ch_http_base(url)?;
                let db = crate::snapshot::ch_http::resolve_ch_database(
                    &self.clickhouse_database,
                    &self.identity.network_id,
                )?;
                let (tbl_b, tbl_c) = crate::snapshot::ch_http::snap_ch_tbl_pair(
                    self.identity.cluster_domain_hi,
                    &self.clickhouse_table,
                );
                let tbl_va = crate::snapshot::ch_http::snap_ch_tbl_validators(
                    self.identity.cluster_domain_hi,
                    &self.clickhouse_table,
                );
                crate::snapshot::ch_http::snap_ch_sql_id(&tbl_b)?;
                crate::snapshot::ch_http::snap_ch_sql_id(&tbl_c)?;
                crate::snapshot::ch_http::snap_ch_sql_id(&tbl_va)?;
                let row_key = crate::snapshot::ch_http::pwmd_snap_row_key(
                    self.snapshot_store_key.as_deref(),
                    genesis_st0_digest_hex,
                    &self.identity,
                )?;
                let json_fallback = if self.snapshot_ch_fallback_json {
                    Some(self.data_file.clone())
                } else {
                    None
                };
                Ok(SnapshotBackend::ClickHouse(
                    crate::snapshot::ch_http::SnapChCfg {
                        http_base: base,
                        database: db,
                        table_blocks: tbl_b,
                        table_checkpoints: tbl_c,
                        table_validators_accept: tbl_va,
                        legacy_snapshot_table: "node_snapshot".into(),
                        row_key,
                        json_fallback,
                    },
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConsoleColorMode, LogFileMode, LoggingConfig};

    #[test]
    fn parse_log_file_mode_values() {
        assert_eq!(LogFileMode::parse("on").expect("on"), LogFileMode::On);
        assert_eq!(LogFileMode::parse("OFF").expect("off"), LogFileMode::Off);
        assert_eq!(
            LogFileMode::parse("required").expect("required"),
            LogFileMode::Required
        );
        assert!(LogFileMode::parse("bad").is_err());
    }

    #[test]
    fn parse_console_color_mode_values() {
        assert_eq!(
            ConsoleColorMode::parse("auto").expect("auto"),
            ConsoleColorMode::Auto
        );
        assert_eq!(
            ConsoleColorMode::parse("always").expect("always"),
            ConsoleColorMode::Always
        );
        assert_eq!(
            ConsoleColorMode::parse("never").expect("never"),
            ConsoleColorMode::Never
        );
        assert!(ConsoleColorMode::parse("bad").is_err());
    }

    /// `ConsoleColorMode::Auto` skips ANSI when stdout is non-TTY (formerly `auto_color_is_non_tty_safe`).
    #[test]
    fn auto_color_tty_gate() {
        assert!(!ConsoleColorMode::Auto.use_ansi(false));
        assert!(ConsoleColorMode::Auto.use_ansi(true));
    }

    /// `LoggingConfig::validate` rejects zero rotate knobs (formerly `logging_bounds_reject_invalid_values`).
    #[test]
    fn log_cfg_bounds_reject_zero() {
        let mut cfg = LoggingConfig::default();
        cfg.rotate_size_mb = 0;
        assert!(cfg.validate().is_err());
        cfg.rotate_size_mb = 1;
        cfg.rotate_max_files = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn logging_defaults_match_slice30_template() {
        let cfg = LoggingConfig::default();
        assert_eq!(cfg.log_dir, std::path::PathBuf::from("logs"));
        assert_eq!(cfg.file_template, "{date}/{log_name}-{node_id}-{time}.log");
        assert_eq!(
            cfg.peer_file_template,
            "{date}/pwmd-peer-{node_id}-{time}.log"
        );
    }
}
