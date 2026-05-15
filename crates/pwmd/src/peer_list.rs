//! Peer bootstrap file helpers for pwmd CLI.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tracing::warn;

const DEFAULT_FILE_NAME: &str = "peers.yaml";

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PeerListDoc {
    #[serde(default)]
    peers: Vec<SocketAddr>,
    #[serde(default)]
    shards: BTreeMap<String, Vec<PeerShardRow>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PeerShardRow {
    id: String,
    peer: SocketAddr,
    validator: bool,
}

#[derive(Debug, Clone)]
enum PeerDocMode {
    Legacy,
    Sharded { shard_key: String },
}

#[derive(Debug, Clone)]
pub struct PeerDocState {
    doc: PeerListDoc,
    mode: PeerDocMode,
}

#[derive(Debug, Clone)]
pub struct PeerFileLoad {
    pub seeds: Vec<SocketAddr>,
    pub state: PeerDocState,
}

#[derive(Debug, Serialize)]
struct LegacyPeerListDoc<'a> {
    peers: &'a [SocketAddr],
}

pub fn default_peer_file(state_root: &Path) -> PathBuf {
    state_root.join(DEFAULT_FILE_NAME)
}

pub fn pick_peer_file(explicit: Option<&Path>, state_root: &Path) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_path_buf());
    }
    let fallback = default_peer_file(state_root);
    if fallback.exists() {
        return Some(fallback);
    }
    None
}

pub fn load_peer_file(
    path: &Path,
    domain_hi: u8,
    explicit_path: bool,
) -> Result<PeerFileLoad, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| format!("failed to read peers list file {}: {err}", path.display()))?;
    let parsed: PeerListDoc = serde_yaml::from_str(&raw).map_err(|err| {
        format!(
            "failed to parse peers list YAML {}: {err}; expected format: peers: [\"127.0.0.1:13030\"] or shards: {{\"0x2C\": [{{id, peer, validator}}]}}",
            path.display()
        )
    })?;
    let mode = if parsed.shards.is_empty() {
        PeerDocMode::Legacy
    } else {
        PeerDocMode::Sharded {
            shard_key: select_shard_key(path, &parsed.shards, domain_hi, explicit_path)?,
        }
    };
    let seeds = load_seeds_for_mode(path, &parsed, &mode, domain_hi, explicit_path)?;
    Ok(PeerFileLoad {
        seeds,
        state: PeerDocState { doc: parsed, mode },
    })
}

pub fn merge_peer_seeds(file_peers: &[SocketAddr], cli_peers: &[SocketAddr]) -> Vec<SocketAddr> {
    let mut merged = Vec::with_capacity(file_peers.len().saturating_add(cli_peers.len()));
    let mut uniq = HashSet::new();
    for addr in file_peers.iter().chain(cli_peers.iter()).copied() {
        if uniq.insert(addr) {
            merged.push(addr);
        }
    }
    merged
}

pub fn drop_self_seed(seeds: &mut Vec<SocketAddr>, self_addr: SocketAddr) {
    seeds.retain(|addr| *addr != self_addr);
}

pub fn save_peer_file(
    path: &Path,
    state: &PeerDocState,
    seeds: &[SocketAddr],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create peers list parent dir {}: {err}",
                parent.display()
            )
        })?;
    }
    let payload = match &state.mode {
        PeerDocMode::Legacy => serde_yaml::to_string(&LegacyPeerListDoc { peers: seeds })
            .map_err(|err| format!("failed to serialize peers list {}: {err}", path.display()))?,
        PeerDocMode::Sharded { shard_key } => {
            let mut next_doc = state.doc.clone();
            let shard_rows = next_doc.shards.remove(shard_key).unwrap_or_default();
            let next_rows = merge_shard_rows(&shard_rows, seeds);
            next_doc.shards.insert(shard_key.clone(), next_rows);
            serde_yaml::to_string(&next_doc).map_err(|err| {
                format!("failed to serialize peers list {}: {err}", path.display())
            })?
        }
    };
    std::fs::write(path, payload)
        .map_err(|err| format!("failed to write peers list file {}: {err}", path.display()))
}

fn load_seeds_for_mode(
    path: &Path,
    doc: &PeerListDoc,
    mode: &PeerDocMode,
    domain_hi: u8,
    explicit_path: bool,
) -> Result<Vec<SocketAddr>, String> {
    match mode {
        PeerDocMode::Legacy => Ok(doc.peers.clone()),
        PeerDocMode::Sharded { shard_key } => {
            let Some(rows) = doc.shards.get(shard_key) else {
                if explicit_path {
                    return Err(format!(
                        "peers list {} has shards but no matching shard for domain_hi={}; add key {}",
                        path.display(),
                        fmt_domain_hi(domain_hi),
                        fmt_domain_hi(domain_hi)
                    ));
                }
                warn!(
                    "pwmd peers list {} has no matching shard for domain_hi={}; using empty file peer seeds",
                    path.display(),
                    fmt_domain_hi(domain_hi)
                );
                return Ok(Vec::new());
            };
            Ok(rows.iter().map(|row| row.peer).collect())
        }
    }
}

fn select_shard_key(
    path: &Path,
    shards: &BTreeMap<String, Vec<PeerShardRow>>,
    domain_hi: u8,
    explicit_path: bool,
) -> Result<String, String> {
    let mut match_key: Option<String> = None;
    for key in shards.keys() {
        let key_hi = parse_shard_key(path, key)?;
        if key_hi != domain_hi {
            continue;
        }
        if let Some(prev) = match_key.as_ref() {
            return Err(format!(
                "peers list {} contains duplicate shard aliases for {}: {} and {}",
                path.display(),
                fmt_domain_hi(domain_hi),
                prev,
                key
            ));
        }
        match_key = Some(key.clone());
    }
    if let Some(key) = match_key {
        return Ok(key);
    }
    if explicit_path {
        return Err(format!(
            "peers list {} has shards but no matching shard key for domain_hi={}; add key {} or decimal {}",
            path.display(),
            fmt_domain_hi(domain_hi),
            fmt_domain_hi(domain_hi),
            domain_hi
        ));
    }
    Ok(fmt_domain_hi(domain_hi))
}

fn parse_shard_key(path: &Path, raw: &str) -> Result<u8, String> {
    let key = raw.trim();
    if let Some(hex) = key.strip_prefix("0x").or_else(|| key.strip_prefix("0X")) {
        if hex.is_empty() {
            return Err(format!(
                "peers list {} has invalid shard key {:?}: expected hex 0xNN or decimal 0..255",
                path.display(),
                raw
            ));
        }
        return u8::from_str_radix(hex, 16).map_err(|err| {
            format!(
                "peers list {} has invalid shard key {:?}: {err}; expected hex 0xNN or decimal 0..255",
                path.display(),
                raw
            )
        });
    }
    key.parse::<u8>().map_err(|err| {
        format!(
            "peers list {} has invalid shard key {:?}: {err}; expected hex 0xNN or decimal 0..255",
            path.display(),
            raw
        )
    })
}

fn fmt_domain_hi(domain_hi: u8) -> String {
    format!("0x{:02X}", domain_hi)
}

fn merge_shard_rows(current_rows: &[PeerShardRow], seeds: &[SocketAddr]) -> Vec<PeerShardRow> {
    let mut by_peer = HashMap::new();
    for row in current_rows {
        by_peer.entry(row.peer).or_insert_with(|| row.clone());
    }
    let mut next_rows = Vec::with_capacity(seeds.len());
    for seed in seeds {
        if let Some(found) = by_peer.remove(seed) {
            next_rows.push(found);
            continue;
        }
        next_rows.push(PeerShardRow {
            id: mk_boot_id(seed),
            peer: *seed,
            validator: false,
        });
    }
    next_rows
}

fn mk_boot_id(peer: &SocketAddr) -> String {
    format!("bootstrap-{}", peer.port())
}

#[cfg(test)]
mod tests {
    use super::{drop_self_seed, load_peer_file, merge_peer_seeds, pick_peer_file, save_peer_file};
    use std::net::SocketAddr;
    use std::path::PathBuf;

    fn mk_tmp_path(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("pwmd-peer-list-{tag}-{nanos}"))
    }

    #[test]
    fn merge_cli_yaml_order() {
        let yaml_peers = vec![
            SocketAddr::from(([127, 0, 0, 1], 13030)),
            SocketAddr::from(([127, 0, 0, 1], 13031)),
        ];
        let cli_peers = vec![
            SocketAddr::from(([127, 0, 0, 1], 13031)),
            SocketAddr::from(([127, 0, 0, 1], 13032)),
        ];
        let got = merge_peer_seeds(&yaml_peers, &cli_peers);
        assert_eq!(
            got,
            vec![
                SocketAddr::from(([127, 0, 0, 1], 13030)),
                SocketAddr::from(([127, 0, 0, 1], 13031)),
                SocketAddr::from(([127, 0, 0, 1], 13032)),
            ]
        );
    }

    #[test]
    fn drop_self_seed_from_list() {
        let self_peer = SocketAddr::from(([127, 0, 0, 1], 13030));
        let mut seeds = vec![
            SocketAddr::from(([127, 0, 0, 1], 13030)),
            SocketAddr::from(([127, 0, 0, 1], 13031)),
            SocketAddr::from(([127, 0, 0, 1], 13030)),
        ];
        drop_self_seed(&mut seeds, self_peer);
        assert_eq!(seeds, vec![SocketAddr::from(([127, 0, 0, 1], 13031))]);
    }

    #[test]
    fn malformed_yaml_reports_hint() {
        let file_path = mk_tmp_path("bad").with_extension("yaml");
        std::fs::write(&file_path, "peers:\n  - not-an-address\n")
            .expect("write malformed peers yaml");
        let err = load_peer_file(file_path.as_path(), 0x2C, false)
            .expect_err("must reject malformed yaml");
        assert!(err.contains("failed to parse peers list YAML"));
        assert!(err.contains(file_path.to_string_lossy().as_ref()));
        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn pick_default_file_if_exists() {
        let state_root = mk_tmp_path("state");
        std::fs::create_dir_all(&state_root).expect("create state root");
        assert!(pick_peer_file(None, &state_root).is_none());
        let default_file = state_root.join("peers.yaml");
        std::fs::write(&default_file, "peers: []\n").expect("create peers yaml");
        let picked = pick_peer_file(None, &state_root).expect("must pick existing default file");
        assert_eq!(picked, default_file);
        let _ = std::fs::remove_file(default_file);
        let _ = std::fs::remove_dir(state_root);
    }

    #[test]
    fn legacy_roundtrip_kept() {
        let file_path = mk_tmp_path("legacy").with_extension("yaml");
        std::fs::write(
            &file_path,
            "peers:\n  - 127.0.0.1:13030\n  - 127.0.0.1:13031\n",
        )
        .expect("write legacy peers");
        let loaded = load_peer_file(file_path.as_path(), 0x2C, false).expect("load legacy peers");
        assert_eq!(
            loaded.seeds,
            vec![
                SocketAddr::from(([127, 0, 0, 1], 13030)),
                SocketAddr::from(([127, 0, 0, 1], 13031))
            ]
        );
        let next = vec![
            SocketAddr::from(([127, 0, 0, 1], 13031)),
            SocketAddr::from(([127, 0, 0, 1], 13032)),
        ];
        save_peer_file(file_path.as_path(), &loaded.state, &next).expect("save legacy peers");
        let saved = std::fs::read_to_string(&file_path).expect("read saved legacy peers");
        assert!(saved.contains("peers:"));
        assert!(saved.contains("- 127.0.0.1:13031"));
        assert!(saved.contains("- 127.0.0.1:13032"));
        assert!(!saved.contains("shards:"));
        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn multishard_load_selects_domain() {
        let file_path = mk_tmp_path("shard-load").with_extension("yaml");
        std::fs::write(
            &file_path,
            "shards:\n  \"0x2C\":\n    - id: cy-proposer\n      peer: 127.0.0.1:13030\n      validator: true\n  \"32\":\n    - id: do-peer\n      peer: 127.0.0.1:14030\n      validator: false\n",
        )
        .expect("write shard peers");
        let loaded = load_peer_file(file_path.as_path(), 0x20, false).expect("load shard peers");
        assert_eq!(
            loaded.seeds,
            vec![SocketAddr::from(([127, 0, 0, 1], 14030))]
        );
        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn save_updates_one_shard() {
        let file_path = mk_tmp_path("shard-save").with_extension("yaml");
        std::fs::write(
            &file_path,
            "shards:\n  \"0x2C\":\n    - id: cy-proposer\n      peer: 127.0.0.1:13030\n      validator: true\n    - id: cy-attester\n      peer: 127.0.0.1:13031\n      validator: true\n  \"0x20\":\n    - id: do-peer\n      peer: 127.0.0.1:14030\n      validator: false\n",
        )
        .expect("write shard peers");
        let loaded = load_peer_file(file_path.as_path(), 0x2C, true).expect("load shard peers");
        let next = vec![
            SocketAddr::from(([127, 0, 0, 1], 13031)),
            SocketAddr::from(([127, 0, 0, 1], 13032)),
        ];
        save_peer_file(file_path.as_path(), &loaded.state, &next).expect("save shard peers");
        let saved = std::fs::read_to_string(file_path.as_path()).expect("read shard peers");
        assert!(saved.contains("id: cy-attester"));
        assert!(saved.contains("peer: 127.0.0.1:13031"));
        assert!(saved.contains("id: bootstrap-13032"));
        assert!(saved.contains("peer: 127.0.0.1:14030"));
        assert!(saved.contains("id: do-peer"));
        assert!(!saved.contains("cy-proposer"));
        let _ = std::fs::remove_file(file_path);
    }

    #[test]
    fn malformed_shard_row_err() {
        let file_path = mk_tmp_path("bad-shard-row").with_extension("yaml");
        std::fs::write(
            &file_path,
            "shards:\n  \"0x2C\":\n    - id: bad\n      peer: not-an-address\n      validator: true\n",
        )
        .expect("write malformed shard peers");
        let err = load_peer_file(file_path.as_path(), 0x2C, false)
            .expect_err("must reject malformed shard");
        assert!(err.contains("failed to parse peers list YAML"));
        let _ = std::fs::remove_file(file_path);
    }

    /// `--peers-list` path: sharded file must include a key matching `domain_hi` or startup fails.
    #[test]
    fn explicit_shard_missing_fails() {
        let file_path = mk_tmp_path("explicit-miss").with_extension("yaml");
        std::fs::write(
            &file_path,
            "shards:\n  \"0x20\":\n    - id: only-other-shard\n      peer: 127.0.0.1:14030\n      validator: false\n",
        )
        .expect("write shard peers");
        let err =
            load_peer_file(file_path.as_path(), 0x2C, true).expect_err("explicit missing shard");
        assert!(
            err.contains("no matching shard key"),
            "unexpected err: {err}"
        );
        let _ = std::fs::remove_file(file_path);
    }

    /// Default `state_root/peers.yaml`: if file is sharded but has no key for this shard, warn + empty seeds.
    #[test]
    fn default_shard_missing_seeds() {
        let file_path = mk_tmp_path("implicit-miss").with_extension("yaml");
        std::fs::write(
            &file_path,
            "shards:\n  \"0x20\":\n    - id: only-other-shard\n      peer: 127.0.0.1:14030\n      validator: false\n",
        )
        .expect("write shard peers");
        let loaded = load_peer_file(file_path.as_path(), 0x2C, false).expect("implicit load ok");
        assert!(
            loaded.seeds.is_empty(),
            "expected no file seeds for other shard only"
        );
        let _ = std::fs::remove_file(file_path);
    }
}
