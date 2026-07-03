include!("../../build/windows_resource.rs");

fn main() {
    configure_windows_resource("PWM Node Daemon");
    emit_build_info();
}

fn emit_build_info() {
    // Rerun when git HEAD or index changes (commit / stage).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");

    let git_hash = run_git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());

    // Dirty = uncommitted changes to tracked files.
    let dirty = std::process::Command::new("git")
        .args(["diff", "--quiet", "HEAD"])
        .current_dir(repo_root())
        .status()
        .map(|s| !s.success())
        .unwrap_or(false);

    let git_ref = if dirty {
        format!("{git_hash}+dirty")
    } else {
        git_hash
    };

    // ISO-8601 build timestamp (UTC, seconds precision).
    let ts = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        // Format as YYYY-MM-DDTHH:MM:SSZ without external deps.
        let s = secs;
        let (d, rem) = (s / 86400, s % 86400);
        let (h, rem) = (rem / 3600, rem % 3600);
        let (m, sec) = (rem / 60, rem % 60);
        // Days since Unix epoch → calendar date (Gregorian, good until 2100).
        let (y, mo, day) = days_to_ymd(d);
        format!("{y:04}-{mo:02}-{day:02}T{h:02}:{m:02}:{sec:02}Z")
    };

    println!("cargo:rustc-env=PWM_GIT_REF={git_ref}");
    println!("cargo:rustc-env=PWM_BUILD_TS={ts}");
}

fn repo_root() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    std::path::PathBuf::from(manifest)
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from("../.."))
}

fn run_git(args: &[&str]) -> Option<String> {
    std::process::Command::new("git")
        .args(args)
        .current_dir(repo_root())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Convert days since Unix epoch (1970-01-01) to (year, month, day).
/// Valid for dates between 1970 and 2099.
fn days_to_ymd(mut d: u64) -> (u64, u64, u64) {
    // 400-year cycles
    let (c400, rem) = (d / 146097, d % 146097);
    d = rem;
    let (c100, rem) = (d / 36524, d % 36524);
    let c100 = c100.min(3);
    d = if c100 == 3 { 36524 } else { rem };
    let (c4, rem) = (d / 1461, d % 1461);
    d = rem;
    let (c1, rem) = (d / 365, d % 365);
    let c1 = c1.min(3);
    d = if c1 == 3 { 365 } else { rem };
    let year = c400 * 400 + c100 * 100 + c4 * 4 + c1 + 1970;
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let months = if leap {
        [31u64, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31u64, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &mlen in &months {
        if d < mlen {
            break;
        }
        d -= mlen;
        month += 1;
    }
    (year, month, d + 1)
}
