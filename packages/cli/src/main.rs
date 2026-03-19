use std::process;

use agentmark::cli::Cli;
use agentmark::commands;
use agentmark::config::{self, Config};
use agentmark::logging;
use clap::Parser;

fn main() {
    // Chrome invokes native messaging hosts with a single arg: the extension
    // origin (e.g. "chrome-extension://abc.../"). Rewrite args so clap sees
    // the `native-host` subcommand instead.
    let args: Vec<String> = std::env::args().collect();
    let use_native_host = args.len() == 2 && args[1].starts_with("chrome-extension://");

    let cli = if use_native_host {
        Cli::parse_from([&args[0], "native-host"])
    } else {
        Cli::parse()
    };

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
