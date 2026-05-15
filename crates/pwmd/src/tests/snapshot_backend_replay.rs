#![cfg(feature = "clickhouse-snapshot")]

//! JsonFile vs ClickHouse snapshot load must yield the same validated v2 wire.
//!
//! **Stable criterion:** byte equality of `serde_json::to_vec(data_to_v2(snapshot))` after each path
//! runs `validate_snapshot` (inside `load_snapshot` / `SnapChCfg::ch_load` → `decode_snapshot_txt`).
//! HTTP is mocked in-process so CI does not need ClickHouse.

use super::helpers::*;
use crate::snapshot::snap_wire_json_bytes;
use crate::snapshot::{SnapshotBackend, SnapshotLoadOpts};

#[test]
fn snap_ch_wire_jsonfile_mock() {
    let (cfg, json_txt) = crate::snap_bench_hlp::mk_dev_cfg_json();
    let p = temp_path("snap_replay_jsonfile");
    std::fs::write(&p, json_txt.as_bytes()).expect("write snap");

    let json_backend = SnapshotBackend::JsonFile { path: p.clone() };
    let a = json_backend
        .load(&cfg, SnapshotLoadOpts::verify_full())
        .expect("json load")
        .0
        .expect("some");
    let wire_a = snap_wire_json_bytes(&a).expect("wire a");

    let base = crate::snap_bench_hlp::spawn_ch_json_mock(&cfg, json_txt.as_str());
    let ch_cfg = crate::snap_bench_hlp::mk_bench_ch_cfg(&base);
    let b = SnapshotBackend::ClickHouse(ch_cfg)
        .load(&cfg, SnapshotLoadOpts::verify_full())
        .expect("ch load")
        .0
        .expect("some");
    let wire_b = snap_wire_json_bytes(&b).expect("wire b");

    assert_eq!(wire_a, wire_b);
    let _ = std::fs::remove_file(&p);
}
