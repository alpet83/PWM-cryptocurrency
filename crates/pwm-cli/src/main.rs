//! Wallet CLI: keys, cluster derive, submit txs to `pwmd`.

// Thin binary that delegates to the library (pwm_cli).
// All shared modules live in the library facade (see lib.rs).

use clap::Parser;
use pwm_cli::Cli;

fn main() {
    pwm_cli::cli_dispatch::run(Cli::parse());
}
