//! Subprocess smoke tests for the `pwm` binary (wave13).

use std::path::PathBuf;
use std::process::Command;

fn pwm_exe() -> PathBuf {
    let p = option_env!("CARGO_BIN_EXE_pwm").expect(
        "`CARGO_BIN_EXE_pwm` missing; run pwm-cli integration tests with `cargo test -p pwm-cli`",
    );
    PathBuf::from(p)
}

#[test]
fn help_stdout() {
    let out = Command::new(pwm_exe())
        .args(["--help"])
        .output()
        .expect("spawn pwm --help");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("Usage:") && s.contains("pwm"),
        "unexpected help: {s}"
    );
}

#[test]
fn key_gen_help_stdout() {
    let out = Command::new(pwm_exe())
        .args(["key-gen", "--help"])
        .output()
        .expect("spawn pwm key-gen --help");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("key-gen") && s.contains("seed"),
        "unexpected key-gen help: {s}"
    );
}

#[test]
fn key_gen_hex_line() {
    let out = Command::new(pwm_exe())
        .args(["key-gen"])
        .output()
        .expect("spawn pwm key-gen");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s.trim_end_matches(['\r', '\n']);
    assert_eq!(line.len(), 64, "expected 64 hex chars, got {line:?}");
    assert!(
        line.chars().all(|c| c.is_ascii_hexdigit()),
        "not hex: {line:?}"
    );
}
