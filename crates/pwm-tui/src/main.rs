//! pwm-tui binary entry: parses Args then drives ratatui event loop.

use clap::Parser;
use pwm_tui::{default_wallet_if_present, Args};

fn main() {
    let mut args = Args::parse();
    if args.wallet.is_none() {
        args.wallet = default_wallet_if_present();
    }
    if let Err(e) = pwm_tui::tui_loop::run(args) {
        eprintln!("{}", e);
    }
}
