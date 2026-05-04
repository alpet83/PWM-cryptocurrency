//! One-shot migration: validated `--snapshot-file` (JsonFile pwm-data shape) → ClickHouse INSERT.
//! Identity/genesis flags must match the node that produced the snapshot (see `./node-1.ps1`).

use clap::Parser;
use pwm_core::digest;
use pwmd::{
    load_genesis_bundle, norm_ch_http_base, parse_cluster_domain_hi, pwmd_snap_row_key,
    resolve_ch_database, resolve_runtime_identity, snap_ch_sql_id, snap_ch_tbl_pair,
    snap_ch_tbl_validators, RuntimeIdentityInput, ShardId, SnapChCfg,
};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pwmd-ch-snap-import")]
struct Cli {
    #[arg(long)]
    genesis_file: PathBuf,
    #[arg(long, env = "PWM_GENESIS_PASSPHRASE")]
    genesis_passphrase: Option<String>,
    #[arg(long)]
    snapshot_file: PathBuf,
    #[arg(long, env = "PWM_CLICKHOUSE_URL")]
    clickhouse_url: String,
    #[arg(long, default_value = "pwm_snapshots")]
    clickhouse_database: String,
    #[arg(long, default_value = "node_snapshot")]
    clickhouse_table: String,
    #[arg(long)]
    network_id: String,
    #[arg(long)]
    domain_hi: String,
    #[arg(long)]
    cluster_id: String,
    #[arg(long)]
    node_id: String,
    #[arg(long)]
    snapshot_store_key: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let pass = cli.genesis_passphrase.unwrap_or_default();
    if pass.trim().is_empty() {
        eprintln!("pwmd-ch-snap-import: set --genesis-passphrase or PWM_GENESIS_PASSPHRASE");
        std::process::exit(2);
    }
    let (cfg, _) = load_genesis_bundle(&cli.genesis_file, Some(pass.trim())).unwrap_or_else(|e| {
        eprintln!("pwmd-ch-snap-import: genesis load failed: {e}");
        std::process::exit(2);
    });
    let hi = parse_cluster_domain_hi(cli.domain_hi.trim()).unwrap_or_else(|e| {
        eprintln!("pwmd-ch-snap-import: {e}");
        std::process::exit(2);
    });
    let identity_input = RuntimeIdentityInput {
        network_id: Some(cli.network_id.trim().to_string()),
        cluster_domain_hi: Some(hi),
        cluster_id: Some(cli.cluster_id.trim().to_string()),
        node_id: Some(cli.node_id.trim().to_string()),
    };
    let identity = resolve_runtime_identity(ShardId::A, identity_input).unwrap_or_else(|e| {
        eprintln!("pwmd-ch-snap-import: identity: {e}");
        std::process::exit(2);
    });
    let base = norm_ch_http_base(cli.clickhouse_url.trim()).unwrap_or_else(|e| {
        eprintln!("pwmd-ch-snap-import: {e}");
        std::process::exit(2);
    });
    let g0_digest = hex::encode(digest(&cfg.state0()));
    let row_key = pwmd_snap_row_key(cli.snapshot_store_key.as_deref(), &g0_digest, &identity)
        .unwrap_or_else(|e| {
            eprintln!("pwmd-ch-snap-import: row key: {e}");
            std::process::exit(2);
        });
    let db = resolve_ch_database(cli.clickhouse_database.trim(), cli.network_id.trim())
        .unwrap_or_else(|e| {
            eprintln!("pwmd-ch-snap-import: database id: {e}");
            std::process::exit(2);
        });
    let (tbl_b, tbl_c) = snap_ch_tbl_pair(hi, cli.clickhouse_table.trim());
    let tbl_va = snap_ch_tbl_validators(hi, cli.clickhouse_table.trim());
    snap_ch_sql_id(&tbl_b).unwrap_or_else(|e| {
        eprintln!("pwmd-ch-snap-import: {e}");
        std::process::exit(2);
    });
    snap_ch_sql_id(&tbl_c).unwrap_or_else(|e| {
        eprintln!("pwmd-ch-snap-import: {e}");
        std::process::exit(2);
    });
    snap_ch_sql_id(&tbl_va).unwrap_or_else(|e| {
        eprintln!("pwmd-ch-snap-import: {e}");
        std::process::exit(2);
    });
    let ch = SnapChCfg {
        http_base: base,
        database: db,
        table_blocks: tbl_b,
        table_checkpoints: tbl_c,
        table_validators_accept: tbl_va,
        legacy_snapshot_table: "node_snapshot".into(),
        row_key,
        json_fallback: None,
    };
    ch.import_snapshot_file(&cli.snapshot_file, &cfg)
        .unwrap_or_else(|e| {
            eprintln!("pwmd-ch-snap-import: import failed: {e}");
            std::process::exit(2);
        });
    println!(
        "pwmd-ch-snap-import: OK row_key={} db={}.blocks={} chk={} validators_accept={}",
        ch.row_key, ch.database, ch.table_blocks, ch.table_checkpoints, ch.table_validators_accept
    );
}
