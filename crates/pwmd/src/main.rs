//! Devnet node binary: REST `/v1/*`, PoA seal loop.

use clap::Parser;
use pwmd::{GenesisSource, PwmdConfig};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "pwmd")]
struct Cli {
    /// HTTP listen address (e.g. `127.0.0.1:3030`).
    #[arg(long, default_value = "127.0.0.1:3030")]
    listen: std::net::SocketAddr,
    /// Genesis JSON (`gen_cfg` + `validator_seeds_hex`); default is built-in `dev_net()`.
    #[arg(long, value_name = "PATH")]
    genesis_file: Option<PathBuf>,
    /// JSON snapshot file with persisted chain (`blocks` + `state`).
    #[arg(long, value_name = "PATH", default_value = "pwm-data.json")]
    data_file: PathBuf,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let genesis = match &cli.genesis_file {
        Some(p) => GenesisSource::JsonFile(p.clone()),
        None => GenesisSource::DevNet,
    };
    let cfg = PwmdConfig {
        listen: cli.listen,
        genesis,
        data_file: cli.data_file,
    };
    if let Err(e) = pwmd::run_with(cfg).await {
        eprintln!("pwmd: {e}");
        std::process::exit(1);
    }
}
