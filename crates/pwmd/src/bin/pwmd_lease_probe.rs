//! Single-step lease probe sharing a file lease dir (multi-process harness / CI).

use clap::Parser;
use pwmd::{step_lease, FileLeaseBackend, LeaseCfg, LeaseRuntime, LeaseState};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "pwmd_lease_probe")]
struct Cli {
    #[arg(long)]
    lease_dir: PathBuf,
    #[arg(long)]
    vh: String,
    #[arg(long)]
    owner: String,
    #[arg(long)]
    now_ms: u64,
    #[arg(long, default_value_t = 0)]
    tip: u64,
    #[arg(long, default_value_t = 10_000)]
    ttl_ms: u64,
    #[arg(long, default_value_t = 8_000)]
    takeover_ms: u64,
    #[arg(long, default_value_t = 1)]
    max_tip_lag: u64,
}

#[derive(Serialize)]
struct ProbeOut {
    allow_seal: bool,
    lease_state: LeaseState,
    last_reason: String,
}

fn main() {
    let cli = Cli::parse();
    let be = match FileLeaseBackend::open(cli.lease_dir.clone()) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pwmd_lease_probe: open backend: {e}");
            std::process::exit(2);
        }
    };
    let cfg = LeaseCfg {
        ttl_ms: cli.ttl_ms,
        takeover_ms: cli.takeover_ms,
        max_tip_lag: cli.max_tip_lag,
    };
    let mut rt = LeaseRuntime::new(cli.owner.clone());
    let step = step_lease(&cli.vh, &cli.owner, cli.now_ms, cli.tip, cfg, &mut rt, &be);
    let out = ProbeOut {
        allow_seal: step.allow_seal,
        lease_state: rt.state,
        last_reason: rt.last_reason.clone(),
    };
    println!("{}", serde_json::to_string(&out).expect("json"));
    std::process::exit(if step.allow_seal { 0 } else { 5 });
}
