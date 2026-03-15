use std::process;

use agentmark::cli::Cli;
use agentmark::commands;
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = commands::dispatch(cli.command) {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
