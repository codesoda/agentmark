use std::process;

use agentmark::cli::Cli;
use agentmark::commands;
use agentmark::config::{self, Config};
use agentmark::logging;
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    // Initialise daily-rolling file logging under ~/.agentmark/logs/
    if let Ok(home) = config::home_dir() {
        let config_level = Config::load(&home).ok().and_then(|c| c.log_level);
        logging::init(&home, config_level.as_deref());
    }

    if let Err(e) = commands::dispatch(cli.command) {
        eprintln!("error: {e}");
        process::exit(1);
    }
}
