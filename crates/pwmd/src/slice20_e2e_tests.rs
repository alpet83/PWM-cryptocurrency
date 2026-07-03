//! Slice-20 end-to-end tests invoking built `pwmd`/`pwm-cli` binaries.
//!
//! The test spawns `pwm` from the workspace `target` tree (see `bin_path`). After changing
//! `pwm-cli`, run `cargo build -p pwm-cli` (or set `CARGO_TARGET_DIR` consistently) so the
//! e2e does not use a stale `pwm` binary.

use pwm_core::address_book::validate_recipient_address_policy;
use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
use slip10_ed25519::derive_ed25519_private_key;
use std::fs::File;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    // crates/pwmd -> crates -> repo
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Root Cargo `target` dir: honors `PWM_WORKSPACE_TARGET_ROOT`, then picks an existing
/// `debug/<probe>` layout (default `./target` vs `[build] target-dir` in workspace `.cargo/config.toml`).
fn cargo_workspace_target_root() -> PathBuf {
    if let Some(p) = std::env::var_os("PWM_WORKSPACE_TARGET_ROOT") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(p);
    }
    let configured = repo_root().join("../rust-target-shared");
    let default_t = repo_root().join("target");
    let exe = if cfg!(windows) { ".exe" } else { "" };
    let probe = format!("pwmd{exe}");
    if configured.join("debug").join(&probe).is_file() {
        return configured;
    }
    if default_t.join("debug").join(&probe).is_file() {
        return default_t;
    }
    configured
}

fn bin_path(bin: &str) -> PathBuf {
    let exe = if cfg!(windows) { ".exe" } else { "" };
    cargo_workspace_target_root()
        .join("debug")
        .join(format!("{bin}{exe}"))
}

fn ensure_cli_bins_ready() {
    let pwmd_bin = bin_path("pwmd");
    let pwm_bin = bin_path("pwm");
    if pwmd_bin.exists() && pwm_bin.exists() {
        return;
    }
    let mut cmd = Command::new("cargo");
    cmd.args(["build", "-p", "pwmd", "-p", "pwm-cli"])
        .current_dir(repo_root());
    let status = cmd.status().expect("cargo build status");
    assert!(status.success(), "cargo build -p pwmd -p pwm-cli failed");
}

fn unique_tmp_dir(name: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "pwm_{name}_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    std::fs::create_dir_all(&base).expect("tmp dir");
    base
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("bind free port")
        .local_addr()
        .expect("local addr")
        .port() as u16
}

fn decode_seed32(hex_seed: &str) -> [u8; 32] {
    let v = hex::decode(hex_seed).expect("seed hex");
    assert_eq!(v.len(), 32, "seed len");
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    a
}

fn derive_account_id(master_seed_hex: &str, derivation_index: u32) -> [u8; 32] {
    let seed = decode_seed32(master_seed_hex);
    let sk_bytes = derive_ed25519_private_key(&seed, &[0, derivation_index]);
    let sk = ed25519_dalek::SigningKey::from_bytes(&sk_bytes);
    let pk = sk.verifying_key().to_bytes();
    account_id_from_parts(&pk, derivation_index)
}

/// Pick first derivation index yielding policy-valid recipient domain hi (formerly `derive_valid_recipient_account_with_di`).
fn rcv_acct_seek_ok(
    master_seed_hex: &str,
    want_hi: u8,
    start: u32,
    max_try: u32,
) -> ([u8; 32], u32) {
    for di in start..start.saturating_add(max_try) {
        let acct = derive_account_id(master_seed_hex, di);
        let hi = domain_of_account_id(&acct).to_be_bytes()[0];
        if hi != want_hi {
            continue;
        }
        if validate_recipient_address_policy(&acct).is_ok() {
            return (acct, di);
        }
    }
    panic!("failed to derive recipient in hi=0x{want_hi:02X}");
}

fn wait_ready(client: &reqwest::blocking::Client, base_url: &str) {
    let start = Instant::now();
    loop {
        if start.elapsed() > Duration::from_secs(45) {
            panic!("timeout waiting for ready: {base_url}");
        }
        let r = client
            .get(format!("{base_url}/v1/status"))
            .send()
            .and_then(|x| x.error_for_status());
        match r {
            Ok(v) => {
                let json: serde_json::Value = v.json().expect("status json");
                if json["ready"].as_bool() == Some(true) {
                    return;
                }
            }
            Err(_) => {
                // Retry (snapshot loader / seal loop warmup).
            }
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn kill_child(mut ch: Child) {
    let _ = ch.kill();
    let _ = ch.wait();
}

struct ProcGuard {
    child: Option<Child>,
}

impl ProcGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }
}

impl Drop for ProcGuard {
    fn drop(&mut self) {
        if let Some(mut ch) = self.child.take() {
            let _ = ch.kill();
            let _ = ch.wait();
        }
    }
}

fn run_checked_capture(cmd: &mut Command) -> String {
    let out = cmd.output().expect("command output");
    if !out.status.success() {
        panic!(
            "command failed: {:?}\nstatus={}\nstdout={}\nstderr={}",
            cmd,
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn parse_export_id(stdout: &str) -> String {
    // ex: "... roaming intent <id> created (export_id=<export_id>, status=exported ..."
    let idx = stdout.find("export_id=").expect("export_id= token present");
    let tail = &stdout[idx + "export_id=".len()..];
    let id = tail
        .split(|c: char| c == ',' || c.is_whitespace() || c == ')')
        .next()
        .unwrap();
    id.to_string()
}

fn parse_intent_id(stdout: &str) -> String {
    // ex: "cross-domain send: roaming intent <intent_id> created ..."
    let token = "roaming intent ";
    let idx = stdout.find(token).expect("roaming intent token present");
    let tail = &stdout[idx + token.len()..];
    tail.split_whitespace().next().unwrap().to_string()
}

/// Genesis fixture guard: fund validator account (derivation index 1) above stake min so seal loop can run.
fn fund_genesis_validator(genesis_json: &Path) {
    let raw = std::fs::read_to_string(genesis_json).expect("read genesis json");
    let mut root: serde_json::Value = serde_json::from_str(&raw).expect("parse genesis json");
    let rows = root["gen_cfg"]["funding"]["accounts"]
        .as_array_mut()
        .expect("funding accounts array");
    let row = rows
        .iter_mut()
        .find(|x| x["der_idx"].as_u64() == Some(1))
        .expect("validator funding row");
    row["bal"] = serde_json::json!("1000000");
    // Keep e2e fixture permissive: validator liveness in this test should not depend on stake floors.
    root["gen_cfg"]["min_validator_stake"] = serde_json::json!("0");
    root["gen_cfg"]["marks_stake_min"] = serde_json::json!("0");
    std::fs::write(
        genesis_json,
        serde_json::to_string_pretty(&root).expect("encode genesis json"),
    )
    .expect("write genesis json");
}

/// Dual-shard pwm-cli roaming contract exercised end-to-end (formerly `slice20_two_shard_e2e_flows_contract`).
#[test]
fn slice20_dual_flow_ok() {
    // Master seed shared by wallet-cy.yaml and wallet-do.yaml fixtures (Sprint 14).
    // Keep it hardcoded so test is self-contained and deterministic.
    let master_seed_hex = "edf0537be8ab846b22990ef31c8c7c61dbdad1d1a53c930fb102d048d6ffb520";
    let sender_hi = 0x2C_u8;
    let do_hi = 0x32_u8;

    let cy_port = free_port();
    let do_port = free_port();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client build");

    let pwmd_bin = bin_path("pwmd");
    let pwm_bin = bin_path("pwm");
    ensure_cli_bins_ready();
    assert!(pwmd_bin.exists(), "pwmd binary missing");
    assert!(pwm_bin.exists(), "pwm binary missing");

    let tmp = unique_tmp_dir("slice20_e2e");
    let wallets_dir = tmp.join("wallets");
    let logs_dir = tmp.join("logs");
    let states_dir = tmp.join("states");
    std::fs::create_dir_all(&wallets_dir).unwrap();
    std::fs::create_dir_all(&logs_dir).unwrap();
    std::fs::create_dir_all(&states_dir).unwrap();

    let wallet_cy = wallets_dir.join("wallet-cy.yaml");
    let wallet_gen = wallets_dir.join("wallet-genesis.yaml");
    let wallet_do = wallets_dir.join("wallet-do.yaml");
    let wallet_recv_cy = wallets_dir.join("wallet-recv-cy.yaml");
    let wallet_do_dest = wallets_dir.join("wallet-do-dest.yaml");
    let genesis_json = tmp.join("genesis.json");

    // Sender in CY: pick deterministic derivation index known to be CY and policy-valid.
    let sender_di = 167708_u32;
    let receiver_di_start = 201143_u32;
    let (receiver, receiver_di) =
        rcv_acct_seek_ok(master_seed_hex, sender_hi, receiver_di_start, 512);
    let receiver_hex = hex::encode(receiver);

    let sender_acct = derive_account_id(master_seed_hex, sender_di);
    assert_eq!(
        domain_of_account_id(&sender_acct).to_be_bytes()[0],
        sender_hi
    );
    let sender_hex = hex::encode(sender_acct);

    // DO signer (Import signer). Keep this account in a dedicated wallet for tx-import.
    let do_signer_di = 61572_u32;
    let do_signer = derive_account_id(master_seed_hex, do_signer_di);
    assert_eq!(domain_of_account_id(&do_signer).to_be_bytes()[0], do_hi);

    // DO destination receiver for IMPORT credit.
    let do_dest_di_start = do_signer_di + 1;
    let (do_dest, do_dest_di) = rcv_acct_seek_ok(master_seed_hex, do_hi, do_dest_di_start, 512);
    let do_dest_hex = hex::encode(do_dest);

    // Generate wallets.
    // Wallet CY contains ONLY the sender account so local transfer creates uninitialized receiver stub.
    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "--genesis-passphrase",
                "12345",
                "wallet",
                "import-seed",
                "--master",
                master_seed_hex,
                "--derivation-index",
                &sender_di.to_string(),
                "--plaintext-dev",
                "--wallet-out",
                wallet_cy.to_str().unwrap(),
            ])
            .current_dir(repo_root()),
    );
    std::fs::copy(&wallet_cy, &wallet_gen).expect("clone wallet for genesis");

    // DO wallet contains ONLY the signer account used for tx-import.
    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "--genesis-passphrase",
                "12345",
                "wallet",
                "import-seed",
                "--master",
                master_seed_hex,
                "--derivation-index",
                &do_signer_di.to_string(),
                "--plaintext-dev",
                "--wallet-out",
                wallet_do.to_str().unwrap(),
            ])
            .current_dir(repo_root()),
    );

    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "--genesis-passphrase",
                "12345",
                "wallet",
                "import-seed",
                "--master",
                master_seed_hex,
                "--derivation-index",
                &receiver_di.to_string(),
                "--plaintext-dev",
                "--wallet-out",
                wallet_recv_cy.to_str().unwrap(),
            ])
            .current_dir(repo_root()),
    );

    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "--genesis-passphrase",
                "12345",
                "wallet",
                "import-seed",
                "--master",
                master_seed_hex,
                "--derivation-index",
                &do_dest_di.to_string(),
                "--plaintext-dev",
                "--wallet-out",
                wallet_do_dest.to_str().unwrap(),
            ])
            .current_dir(repo_root()),
    );

    // Add DO signer into genesis-only wallet so IMPORT has fee floor liquidity
    // (`MIN_IMPORT_FEE_UNITS`) after recent roaming economics updates.
    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "wallet",
                "account",
                "add",
                "--wallet",
                wallet_gen.to_str().unwrap(),
                "--derivation-index",
                &do_signer_di.to_string(),
            ])
            .current_dir(repo_root()),
    );

    // Build genesis from dedicated wallet (sender + import signer); still does not pre-fund recipients.
    run_checked_capture(Command::new(&pwm_bin).args([
        "--genesis-passphrase",
        "12345",
        "genesis-build",
        "--wallet",
        wallet_gen.to_str().unwrap(),
        "--out",
        genesis_json.to_str().unwrap(),
        "--premine-bal",
        "1000000",
    ]));
    fund_genesis_validator(&genesis_json);

    // Start pwmd nodes.
    let cy_state_dir = states_dir.join("cy");
    let do_state_dir = states_dir.join("do");
    std::fs::create_dir_all(&cy_state_dir).unwrap();
    std::fs::create_dir_all(&do_state_dir).unwrap();
    let cy_data_file = cy_state_dir.join("pwm-data.json");
    let do_data_file = do_state_dir.join("pwm-data.json");

    let cy_log = logs_dir.join("pwmd-cy.log");
    let do_log = logs_dir.join("pwmd-do.log");

    let cy_base = format!("http://127.0.0.1:{cy_port}");
    let do_base = format!("http://127.0.0.1:{do_port}");
    let do_peer_port = do_port.checked_add(100).expect("peer port");
    let cy_peer_port = cy_port.checked_add(100).expect("peer port");

    let cy_child = Command::new(&pwmd_bin)
        .args([
            "--listen",
            &format!("127.0.0.1:{cy_port}"),
            "--state-root",
            cy_state_dir.to_str().unwrap(),
            "--data-file",
            cy_data_file.to_str().unwrap(),
            "--genesis-file",
            genesis_json.to_str().unwrap(),
            "--genesis-passphrase",
            "12345",
            "--network-id",
            "testnet-s14",
            "--domain-hi",
            "0x2C",
            "--cluster-id",
            "cluster-CY",
            "--node-id",
            "node-CY",
            "--transport-real",
            "--transport-peer-seed",
            &format!("127.0.0.1:{do_peer_port}"),
        ])
        .stdout(Stdio::from(File::create(&cy_log).unwrap()))
        .stderr(Stdio::from(File::create(&cy_log).unwrap()))
        .spawn()
        .expect("spawn cy pwmd");

    let do_child = Command::new(&pwmd_bin)
        .args([
            "--listen",
            &format!("127.0.0.1:{do_port}"),
            "--state-root",
            do_state_dir.to_str().unwrap(),
            "--data-file",
            do_data_file.to_str().unwrap(),
            "--genesis-file",
            genesis_json.to_str().unwrap(),
            "--genesis-passphrase",
            "12345",
            "--network-id",
            "testnet-s14",
            "--domain-hi",
            "0x32",
            "--cluster-id",
            "cluster-DO",
            "--node-id",
            "node-DO",
            "--transport-real",
            "--transport-peer-seed",
            &format!("127.0.0.1:{cy_peer_port}"),
        ])
        .stdout(Stdio::from(File::create(&do_log).unwrap()))
        .stderr(Stdio::from(File::create(&do_log).unwrap()))
        .spawn()
        .expect("spawn do pwmd");

    let mut cy_proc = ProcGuard::new(cy_child);
    let _do_proc = ProcGuard::new(do_child);

    wait_ready(&client, &cy_base);
    wait_ready(&client, &do_base);

    let sender_di_arg = sender_di.to_string();
    let receiver_di_arg = receiver_di.to_string();
    let do_dest_di_arg = do_dest_di.to_string();
    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "--rpc",
                &cy_base,
                "tx-init",
                "--wallet",
                wallet_recv_cy.to_str().unwrap(),
                "--index",
                &receiver_di_arg,
                "--flags",
                "0",
            ])
            .current_dir(repo_root()),
    );
    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "--rpc",
                &do_base,
                "tx-init",
                "--wallet",
                wallet_do_dest.to_str().unwrap(),
                "--index",
                &do_dest_di_arg,
                "--flags",
                "0",
            ])
            .current_dir(repo_root()),
    );

    // Step A: local same-hi transfer to initialized recipient (CLI preflight requires account present).
    let sender_before = client
        .get(format!("{cy_base}/v1/account/{sender_hex}"))
        .send()
        .unwrap()
        .json::<serde_json::Value>()
        .unwrap();
    let sender_nonce_before = sender_before["nonce"].as_u64().unwrap();
    let sender_balance_before = sender_before["balance_pwm"]
        .as_str()
        .unwrap()
        .parse::<u128>()
        .unwrap();

    let recv_before: serde_json::Value = {
        let rb_deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let r = client
                .get(format!("{cy_base}/v1/account/{receiver_hex}"))
                .send()
                .unwrap();
            if r.status() == reqwest::StatusCode::OK {
                break r.json().unwrap();
            }
            if Instant::now() > rb_deadline {
                panic!("timeout waiting for receiver account after tx-init");
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    };
    assert_eq!(recv_before["initialized"].as_bool(), Some(true));

    let stdout_transfer = run_checked_capture(Command::new(&pwm_bin).args([
        "--rpc",
        &cy_base,
        "tx-send",
        "--wallet",
        wallet_cy.to_str().unwrap(),
        "--index",
        &sender_di_arg,
        "--to",
        &receiver_hex,
        "--amount",
        "10",
        "--fee",
        "1",
    ]));
    assert!(stdout_transfer.contains("No Content") || stdout_transfer.contains("204"));

    // Wait for transfer to seal (poll by sender nonce).
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if Instant::now() > deadline {
            panic!("timeout waiting for transfer to apply");
        }
        let sender_now: serde_json::Value = client
            .get(format!("{cy_base}/v1/account/{sender_hex}"))
            .send()
            .unwrap()
            .json()
            .unwrap();
        let receiver_now = client
            .get(format!("{cy_base}/v1/account/{receiver_hex}"))
            .send()
            .unwrap();
        if receiver_now.status() != reqwest::StatusCode::OK {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }
        let receiver_now: serde_json::Value = receiver_now.json().unwrap();

        let sender_nonce_after = sender_now["nonce"].as_u64().unwrap();
        if sender_nonce_after != sender_nonce_before + 1 {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }

        let sender_balance_after = sender_now["balance_pwm"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap();
        let receiver_balance_after = receiver_now["balance_pwm"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap();
        let receiver_initialized = receiver_now["initialized"].as_bool().unwrap();
        let receiver_nonce = receiver_now["nonce"].as_u64().unwrap();

        assert_eq!(sender_balance_after, sender_balance_before - 11);
        assert_eq!(receiver_balance_after, 10);
        assert_eq!(receiver_initialized, true);
        assert_eq!(receiver_nonce, 1);
        break;
    }

    // Logs: guard labels remain at INFO; per-transfer commit deltas are DEBUG-only.
    let cy_log_txt = std::fs::read_to_string(&cy_log).expect("cy log read");
    assert!(
        cy_log_txt.contains("tx routing guard: shard=CY"),
        "missing CY routing guard"
    );
    assert!(
        !cy_log_txt.contains("shard=A"),
        "legacy shard=A must not appear"
    );

    // Step B: cross-shard CY export -> finalize -> DO import
    let stdout_export = run_checked_capture(Command::new(&pwm_bin).args([
        "--rpc",
        &cy_base,
        "tx-send",
        "--wallet",
        wallet_cy.to_str().unwrap(),
        "--index",
        &sender_di_arg,
        "--to",
        &do_dest_hex,
        "--amount",
        "100",
        "--fee",
        "1",
    ]));
    let export_id = parse_export_id(&stdout_export);
    let intent_id = parse_intent_id(&stdout_export);
    assert!(!export_id.is_empty());
    assert!(!intent_id.is_empty());

    // Finalize operator handoff on CY.
    let finalize_url = format!("{cy_base}/v1/roaming-intents/{intent_id}/finalize");
    let resp = client.post(finalize_url).send().unwrap();
    assert!(resp.status().is_success(), "finalize must succeed");
    let finalize_json: serde_json::Value = resp.json().expect("finalize json");
    let fin_status = finalize_json["status"].as_str().expect("finalize status");
    assert!(
        fin_status == "relayed" || fin_status == "exported",
        "unexpected finalize status={fin_status} (peer relay may still be pending)"
    );
    assert_eq!(finalize_json["handoff"]["export_id"], export_id);

    // Register finalized source provenance on DO through the supported operator handoff path.
    let handoff_json = tmp.join("export-handoff.json");
    std::fs::write(
        &handoff_json,
        serde_json::to_string_pretty(&finalize_json).expect("handoff json serialize"),
    )
    .expect("handoff json write");
    let stdout_handoff = run_checked_capture(Command::new(&pwm_bin).args([
        "--rpc",
        &do_base,
        "tx-handoff-register",
        "--handoff-json",
        handoff_json.to_str().unwrap(),
    ]));
    assert!(stdout_handoff.contains("200 OK"));

    // Import on DO. Target-side IMPORT is not allowed to self-register unknown provenance;
    // it succeeds only after the explicit handoff registration above.
    let stdout_import = run_checked_capture(Command::new(&pwm_bin).args([
        "--rpc",
        &do_base,
        "tx-import",
        "--wallet",
        wallet_do.to_str().unwrap(),
        "--to",
        &do_dest_hex,
        "--amount",
        "100",
        "--export-id",
        &export_id,
    ]));
    assert!(stdout_import.contains("No Content") || stdout_import.contains("204"));

    // Wait for destination credit on DO.
    let dest_deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if Instant::now() > dest_deadline {
            panic!("timeout waiting for DO import to apply");
        }
        let dest_resp = client
            .get(format!("{do_base}/v1/account/{do_dest_hex}"))
            .send()
            .unwrap();
        if dest_resp.status() != reqwest::StatusCode::OK {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        let dest: serde_json::Value = dest_resp.json().unwrap();
        let bal = dest["balance_pwm"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap();
        if bal == 100 {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }

    // DO status should report imported_set.
    let st: serde_json::Value = client
        .get(format!("{do_base}/v1/status"))
        .send()
        .unwrap()
        .json()
        .unwrap();
    let imported_sz = st["bridge_imported_set_size"].as_u64().unwrap_or(0);
    assert!(imported_sz >= 1, "imported_set_size must be >= 1");

    // Logs: import/export commit delta observability + DO routing guard label.
    let do_log_txt = std::fs::read_to_string(&do_log).expect("do log read");
    let cy_log_txt_after_export =
        std::fs::read_to_string(&cy_log).expect("cy log read after export");
    assert!(
        do_log_txt.contains("tx routing guard: shard=DO"),
        "missing DO routing guard"
    );
    assert!(
        !do_log_txt.contains("shard=A"),
        "legacy shard=A must not appear"
    );
    assert!(
        cy_log_txt_after_export.contains("tx commit delta: kind=export"),
        "missing export tx commit delta"
    );
    assert!(
        do_log_txt.contains("tx commit delta: kind=import"),
        "missing import tx commit delta"
    );

    // Step C: restart CY from same data-file; snapshot replay must stay consistent.
    kill_child(cy_proc.child.take().unwrap());
    let cy_log2 = logs_dir.join("pwmd-cy-restart.log");
    let cy_child2 = Command::new(&pwmd_bin)
        .args([
            "--listen",
            &format!("127.0.0.1:{cy_port}"),
            "--state-root",
            cy_state_dir.to_str().unwrap(),
            "--data-file",
            cy_data_file.to_str().unwrap(),
            "--genesis-file",
            genesis_json.to_str().unwrap(),
            "--genesis-passphrase",
            "12345",
            "--network-id",
            "testnet-s14",
            "--domain-hi",
            "0x2C",
            "--cluster-id",
            "cluster-CY",
            "--node-id",
            "node-CY",
            "--transport-real",
            "--transport-peer-seed",
            &format!("127.0.0.1:{do_peer_port}"),
        ])
        .stdout(Stdio::from(File::create(&cy_log2).unwrap()))
        .stderr(Stdio::from(File::create(&cy_log2).unwrap()))
        .spawn()
        .expect("spawn cy pwmd restart");
    let _cy_restart_guard = ProcGuard::new(cy_child2);
    wait_ready(&client, &cy_base);

    let cy_restart_txt = std::fs::read_to_string(&cy_log2).expect("cy restart log read");
    assert!(
        !cy_restart_txt.contains("snapshot chain mismatch"),
        "snapshot replay mismatch must not appear after restart"
    );
}

/// Two **processes** + real transport: cross-shard peers must not enter false `bridge_federation_trust_refused`
/// (level-2 digest compared only for same `domain_hi`).
#[test]
fn cross_shard_bridge_ok() {
    let master_seed_hex = "edf0537be8ab846b22990ef31c8c7c61dbdad1d1a53c930fb102d048d6ffb520";
    let sender_di = 167708_u32;

    let cy_port = free_port();
    let do_port = free_port();
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("client build");

    let pwmd_bin = bin_path("pwmd");
    let pwm_bin = bin_path("pwm");
    ensure_cli_bins_ready();
    assert!(pwmd_bin.exists(), "pwmd binary missing");
    assert!(pwm_bin.exists(), "pwm binary missing");

    let tmp = unique_tmp_dir("bridge_two_node");
    let wallets_dir = tmp.join("wallets");
    let logs_dir = tmp.join("logs");
    let states_dir = tmp.join("states");
    std::fs::create_dir_all(&wallets_dir).unwrap();
    std::fs::create_dir_all(&logs_dir).unwrap();
    std::fs::create_dir_all(&states_dir).unwrap();

    let wallet_cy = wallets_dir.join("wallet-cy.yaml");
    let genesis_json = tmp.join("genesis.json");

    run_checked_capture(
        Command::new(&pwm_bin)
            .args([
                "--genesis-passphrase",
                "12345",
                "wallet",
                "import-seed",
                "--master",
                master_seed_hex,
                "--derivation-index",
                &sender_di.to_string(),
                "--plaintext-dev",
                "--wallet-out",
                wallet_cy.to_str().unwrap(),
            ])
            .current_dir(repo_root()),
    );

    run_checked_capture(Command::new(&pwm_bin).args([
        "--genesis-passphrase",
        "12345",
        "genesis-build",
        "--wallet",
        wallet_cy.to_str().unwrap(),
        "--out",
        genesis_json.to_str().unwrap(),
        "--premine-bal",
        "1000000",
    ]));
    fund_genesis_validator(&genesis_json);

    let cy_state_dir = states_dir.join("cy");
    let do_state_dir = states_dir.join("do");
    std::fs::create_dir_all(&cy_state_dir).unwrap();
    std::fs::create_dir_all(&do_state_dir).unwrap();
    let cy_data_file = cy_state_dir.join("pwm-data.json");
    let do_data_file = do_state_dir.join("pwm-data.json");

    let cy_log = logs_dir.join("pwmd-cy-bridge.log");
    let do_log = logs_dir.join("pwmd-do-bridge.log");

    let cy_base = format!("http://127.0.0.1:{cy_port}");
    let do_base = format!("http://127.0.0.1:{do_port}");
    let do_peer_port = do_port.checked_add(100).expect("peer port");
    let cy_peer_port = cy_port.checked_add(100).expect("peer port");

    let cy_child = Command::new(&pwmd_bin)
        .args([
            "--listen",
            &format!("127.0.0.1:{cy_port}"),
            "--state-root",
            cy_state_dir.to_str().unwrap(),
            "--data-file",
            cy_data_file.to_str().unwrap(),
            "--genesis-file",
            genesis_json.to_str().unwrap(),
            "--genesis-passphrase",
            "12345",
            "--network-id",
            "testnet-s14",
            "--domain-hi",
            "0x2C",
            "--cluster-id",
            "cluster-CY",
            "--node-id",
            "node-CY",
            "--transport-real",
            "--transport-peer-seed",
            &format!("127.0.0.1:{do_peer_port}"),
        ])
        .stdout(Stdio::from(File::create(&cy_log).unwrap()))
        .stderr(Stdio::from(File::create(&cy_log).unwrap()))
        .spawn()
        .expect("spawn cy pwmd");

    let do_child = Command::new(&pwmd_bin)
        .args([
            "--listen",
            &format!("127.0.0.1:{do_port}"),
            "--state-root",
            do_state_dir.to_str().unwrap(),
            "--data-file",
            do_data_file.to_str().unwrap(),
            "--genesis-file",
            genesis_json.to_str().unwrap(),
            "--genesis-passphrase",
            "12345",
            "--network-id",
            "testnet-s14",
            "--domain-hi",
            "0x32",
            "--cluster-id",
            "cluster-DO",
            "--node-id",
            "node-DO",
            "--transport-real",
            "--transport-peer-seed",
            &format!("127.0.0.1:{cy_peer_port}"),
        ])
        .stdout(Stdio::from(File::create(&do_log).unwrap()))
        .stderr(Stdio::from(File::create(&do_log).unwrap()))
        .spawn()
        .expect("spawn do pwmd");

    let _cy_proc = ProcGuard::new(cy_child);
    let _do_proc = ProcGuard::new(do_child);

    wait_ready(&client, &cy_base);
    wait_ready(&client, &do_base);

    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if Instant::now() > deadline {
            panic!("timeout waiting for bridge_federation_trust=ok on both nodes");
        }
        let mut both_ok = true;
        for base in [&cy_base, &do_base] {
            let v: serde_json::Value = client
                .get(format!("{base}/v1/status"))
                .send()
                .and_then(|r| r.error_for_status())
                .expect("status")
                .json()
                .expect("json");
            if v["bridge_federation_trust"].as_str() != Some("ok") {
                both_ok = false;
                break;
            }
        }
        if both_ok {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
