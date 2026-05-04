//! Unstable hooks for `cargo bench -p pwmd` and clickhouse-snapshot replay tests (not semver API).

use pwm_core::genesis::GenCfg;
use std::path::{Path, PathBuf};

/// Default paths aligned with repo-root `./node-1.ps1` / `./node-2.ps1` (`--data-file`, `--genesis-file`).
pub const DEFAULT_BENCH_SNAPSHOT_PATH: &str = "./tmp/state-testnet/pwm-data.json";
pub const DEFAULT_BENCH_GENESIS_PATH: &str = "./tmp/genesis-custom.json";

/// Directory with `pwm-data.json` (and optional `genesis-custom.json`) for full-chain benches.
/// Example: `PWM_SNAPSHOT_BENCH_DIR=P:\opt\docker\PWM-cryptocurrency\tmp\state-testnet`
pub const ENV_SNAPSHOT_BENCH_DIR: &str = "PWM_SNAPSHOT_BENCH_DIR";

fn bench_snap_path_from_env() -> PathBuf {
    if let Ok(dir) = std::env::var(ENV_SNAPSHOT_BENCH_DIR) {
        let d = PathBuf::from(dir.trim());
        if !d.as_os_str().is_empty() {
            return d.join("pwm-data.json");
        }
    }
    std::env::var("PWM_SNAPSHOT_BENCH_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_BENCH_SNAPSHOT_PATH))
}

fn bench_genesis_path_from_env() -> PathBuf {
    if let Ok(dir) = std::env::var(ENV_SNAPSHOT_BENCH_DIR) {
        let d = PathBuf::from(dir.trim());
        let g = d.join("genesis-custom.json");
        if g.is_file() {
            return g;
        }
    }
    std::env::var("PWM_SNAPSHOT_BENCH_GENESIS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_BENCH_GENESIS_PATH))
}

/// Resolved bench inputs: real node tree or synthetic temp file.
pub struct BenchSnapCtx {
    pub cfg: GenCfg,
    pub path: PathBuf,
    /// `true` when `PWM_SNAPSHOT_BENCH_*` or default paths yielded a loadable pair.
    pub from_node_scripts: bool,
}

/// Prefer on-disk QA snapshot+genesis from PS1 scripts; fall back to synthetic dev_net fixture.
pub fn resolve_bench_snapshot() -> BenchSnapCtx {
    let snap_pb = bench_snap_path_from_env();
    let gen_pb = bench_genesis_path_from_env();
    let pass = std::env::var("PWM_SNAPSHOT_BENCH_GENESIS_PASS").unwrap_or_else(|_| "12345".into());
    if snap_pb.is_file() && gen_pb.is_file() {
        if let Ok((cfg, _sks)) = crate::snapshot::load_genesis_bundle(&gen_pb, Some(pass.trim())) {
            if crate::snapshot::load_snapshot(&snap_pb, &cfg)
                .ok()
                .flatten()
                .is_some()
            {
                return BenchSnapCtx {
                    cfg,
                    path: snap_pb,
                    from_node_scripts: true,
                };
            }
        }
    }
    let (cfg, json) = mk_dev_cfg_and_json();
    let path = std::env::temp_dir().join(format!("pwmd_bench_syn_{}.json", std::process::id()));
    std::fs::write(&path, json.as_bytes()).expect("bench syn write");
    BenchSnapCtx {
        cfg,
        path,
        from_node_scripts: false,
    }
}

/// Deterministic dev-net chain snapshot used across JsonFile vs CH benches/tests.
pub fn mk_dev_cfg_and_json() -> (GenCfg, String) {
    let (cfg, sks) = pwm_core::dev_net();
    let mut chain = pwm_core::Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("bench seal");
    chain.seal(vec![]).expect("bench seal");
    let inner = crate::state::Inner {
        chain,
        pool: pwm_core::Mpool::new(16),
        roaming_pool: Default::default(),
        cross_shard: Default::default(),
        federation: Default::default(),
        peer_account_views: Default::default(),
        recent_flow: Default::default(),
    };
    let json = crate::snapshot::encode_inner_snap_json(&inner, None).expect("bench encode");
    (cfg, json)
}

/// JsonFile load path; returns validated snapshot as stable v2 wire bytes.
pub fn load_jsonfile_wire(path: &Path, cfg: &GenCfg) -> Result<Vec<u8>, String> {
    let snap = crate::snapshot::load_snapshot(path, cfg)?
        .ok_or_else(|| "snapshot file missing or empty".to_string())?;
    crate::snapshot::snap_wire_json_bytes(&snap)
}

/// Snapshot decoded from disk **without** replay validation — benches / cost splits only.
pub struct SnapBenchParsed(crate::snapshot::SnapshotData);

impl SnapBenchParsed {
    pub fn decode_raw(path: &Path, cfg: &GenCfg) -> Result<Self, String> {
        let txt =
            std::fs::read_to_string(path).map_err(|e| format!("read snapshot for bench: {e}"))?;
        let snap = crate::snapshot::decode_snap_raw(&txt, cfg)?
            .ok_or_else(|| "snapshot empty".to_string())?;
        Ok(Self(snap))
    }

    /// Full-chain replay verification (production load runs this after JSON decode).
    pub fn replay_validate(&self, cfg: &GenCfg) -> Result<(), String> {
        let mut s = self.0.clone();
        crate::snapshot::replay_validate(&mut s, cfg)
    }

    pub fn wire_bytes(&self) -> Result<Vec<u8>, String> {
        crate::snapshot::snap_wire_json_bytes(&self.0)
    }
}

#[cfg(feature = "clickhouse-snapshot")]
pub fn mk_bench_ch_cfg(http_base: &str) -> crate::snapshot::ch_http::SnapChCfg {
    crate::snapshot::ch_http::SnapChCfg {
        http_base: http_base.trim_end_matches('/').to_string(),
        database: "pwm_snapshots".into(),
        table_blocks: "blocks__0x11".into(),
        table_checkpoints: "checkpoints__0x11".into(),
        table_validators_accept: "validators_accept__0x11".into(),
        legacy_snapshot_table: "node_snapshot".into(),
        row_key: "s15_slice6_bench".into(),
        json_fallback: None,
    }
}

/// Spins a minimal HTTP responder compatible with `SnapChCfg::ch_load` (blocks table first, else legacy).
#[cfg(feature = "clickhouse-snapshot")]
pub fn spawn_ch_json_mock(cfg: &pwm_core::genesis::GenCfg, snapshot_json: &str) -> String {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    let snap = crate::snapshot::decode_snap_raw(snapshot_json, cfg)
        .expect("mock decode_snap_raw")
        .expect("mock snap some");
    let block_lines: Vec<String> = snap
        .blocks
        .iter()
        .map(|b| {
            let payload_json = serde_json::to_string(b).expect("mock block json");
            serde_json::json!({ "payload_json": payload_json }).to_string()
        })
        .collect();
    let legacy_row = serde_json::json!({ "snapshot_json": snapshot_json }).to_string();
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock ch");
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 32768];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let is_blocks_query = req.contains("payload_json")
                && req.contains("ORDER")
                && req.contains("height")
                && req.contains("FORMAT");
            let body = if is_blocks_query {
                block_lines.join("\n") + "\n"
            } else {
                legacy_row.clone()
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    format!("http://{}", addr)
}

#[cfg(feature = "clickhouse-snapshot")]
pub fn load_ch_wire(
    ch: &crate::snapshot::ch_http::SnapChCfg,
    cfg: &GenCfg,
) -> Result<Vec<u8>, String> {
    let (snap, _) = ch.ch_load(cfg)?;
    let snap = snap.ok_or_else(|| "clickhouse snapshot row missing".to_string())?;
    crate::snapshot::snap_wire_json_bytes(&snap)
}
