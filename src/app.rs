// Author Dustin Pilgrim
// License: MIT

use crate::{cli::Cli, config::Config, log::Log};
use clap::Parser;
use std::process::ExitCode;

pub fn run() -> ExitCode {
    let cli = Cli::parse();

    let log = Log {
        quiet: cli.quiet,
        verbose: cli.verbose,
    };

    let cfg = match Config::load() {
        Ok(c) => c, // Option<Config>
        Err(e) => {
            log.error(format!("vx: {e}"));
            return ExitCode::from(2);
        }
    };

    crate::ops::dispatch(&log, cli, cfg)
}

