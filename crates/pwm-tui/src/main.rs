//! pwm-tui binary entry: parses Args then drives ratatui event loop.

use clap::Parser;
use pwm_tui::{default_wallet_if_present, init_tx_history_dir, resolve_wallet_file, Args};

fn main() {
    let mut args = Args::parse();
    if args.wallet.is_none() {
        args.wallet = default_wallet_if_present();
    }
    if let Some(wallet) = args.wallet.as_deref().and_then(resolve_wallet_file) {
        args.wallet = Some(wallet);
    }
    init_tx_history_dir(args.wallet.as_deref());
    if let Err(e) = pwm_tui::tui_loop::run(args) {
        eprintln!("{}", e);
    }
}
