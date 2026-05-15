//! Two-process lease contention via `pwmd_lease_probe` (shared OS dir).

use serde_json::Value;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn probe_exe() -> &'static str {
    env!("CARGO_BIN_EXE_pwmd_lease_probe")
}

fn mk_lease_dir() -> std::path::PathBuf {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("pwmd-two-proc-leases-{ns}"));
    std::fs::create_dir_all(&dir).expect("mkdir leases");
    dir
}

fn run_probe(
    lease_dir: &std::path::Path,
    vh: &str,
    owner: &str,
    now_ms: u64,
    tip: u64,
    ttl_ms: u64,
    takeover_ms: u64,
) -> std::process::Output {
    Command::new(probe_exe())
        .arg("--lease-dir")
        .arg(lease_dir)
        .arg("--vh")
        .arg(vh)
        .arg("--owner")
        .arg(owner)
        .arg("--now-ms")
        .arg(now_ms.to_string())
        .arg("--tip")
        .arg(tip.to_string())
        .arg("--ttl-ms")
        .arg(ttl_ms.to_string())
        .arg("--takeover-ms")
        .arg(takeover_ms.to_string())
        .arg("--max-tip-lag")
        .arg("0")
        .output()
        .expect("spawn pwmd_lease_probe")
}

#[test]
fn two_proc_file_lease_takeover() {
    let lease_dir = mk_lease_dir();
    let vh = "vh-two-proc";
    let ttl_ms = 800u64;
    let takeover_ms = 400u64;
    let base = 100_000u64;
    let tip = 42u64;

    let a = run_probe(&lease_dir, vh, "node-a", base, tip, ttl_ms, takeover_ms);
    assert!(
        a.status.success(),
        "proc-a stderr={}",
        String::from_utf8_lossy(&a.stderr)
    );
    let ja: Value = serde_json::from_slice(&a.stdout).expect("json a");
    assert_eq!(ja["allow_seal"], true);

    let b_early = run_probe(
        &lease_dir,
        vh,
        "node-b",
        base + 300,
        tip,
        ttl_ms,
        takeover_ms,
    );
    assert!(
        !b_early.status.success(),
        "standby must not seal early status={:?}",
        b_early.status.code()
    );
    let jb: Value = serde_json::from_slice(&b_early.stdout).expect("json b early");
    assert_eq!(jb["allow_seal"], false);

    let takeover_at = base + ttl_ms + takeover_ms;
    let b_late = run_probe(
        &lease_dir,
        vh,
        "node-b",
        takeover_at + 50,
        tip,
        ttl_ms,
        takeover_ms,
    );
    assert!(
        b_late.status.success(),
        "proc-b stderr={}",
        String::from_utf8_lossy(&b_late.stderr)
    );
    let jl: Value = serde_json::from_slice(&b_late.stdout).expect("json b late");
    assert_eq!(jl["allow_seal"], true);

    let _ = std::fs::remove_dir_all(&lease_dir);
}
