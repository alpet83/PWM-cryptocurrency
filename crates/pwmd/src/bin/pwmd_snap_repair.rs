//! Offline repair tool for JsonFile epoch snapshots (`pwm-data.json` + `epochs/`).

use clap::{ArgGroup, Parser};
use pwmd::{load_genesis_bundle, repair_json_epochs, SnapRepairOpts};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pwmd-snap-repair")]
#[command(group(
    ArgGroup::new("target_mode")
        .required(true)
        .args(["to_height", "auto_last_good"])
))]
struct Cli {
    #[arg(long)]
    data_file: PathBuf,
    #[arg(long)]
    genesis_file: Option<PathBuf>,
    #[arg(long, env = "PWM_GENESIS_PASSPHRASE")]
    genesis_passphrase: Option<String>,
    #[arg(long = "to-height")]
    to_height: Option<u64>,
    #[arg(long = "auto-last-good", default_value_t = false)]
    auto_last_good: bool,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    backup: bool,
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

fn main() {
    let cli = Cli::parse();
    let cfg = if let Some(path) = cli.genesis_file.as_ref() {
        let pass = cli.genesis_passphrase.unwrap_or_default();
        if pass.trim().is_empty() {
            eprintln!("pwmd-snap-repair: set --genesis-passphrase or PWM_GENESIS_PASSPHRASE");
            std::process::exit(2);
        }
        load_genesis_bundle(path, Some(pass.trim()))
            .map(|(cfg, _)| cfg)
            .unwrap_or_else(|e| {
                eprintln!("pwmd-snap-repair: genesis load failed: {e}");
                std::process::exit(2);
            })
    } else {
        pwm_core::dev_net().0
    };

    let target_h = if cli.auto_last_good {
        None
    } else {
        cli.to_height
    };
    let report = repair_json_epochs(
        &cli.data_file,
        &cfg,
        SnapRepairOpts {
            target_h,
            backup: cli.backup,
            dry_run: cli.dry_run,
        },
    )
    .unwrap_or_else(|e| {
        eprintln!("pwmd-snap-repair: repair failed: {e}");
        std::process::exit(2);
    });

    println!(
        "pwmd-snap-repair: OK last_good_h={} target_h={} tip_hash={} wrote_files={} kept_aux_summary={}",
        report.last_good_h,
        report.target_h,
        report.tip_hash,
        report.wrote_files,
        report.kept_aux_summary
    );
    if let Some(dir) = report.backup_dir {
        println!("pwmd-snap-repair: backup_dir={}", dir.display());
    }
}
