// Author Dustin Pilgrim
// License: MIT

mod app;
mod cli;
mod core;
mod config;
mod log;
mod managed;
mod paths;

fn main() -> std::process::ExitCode {
    app::run()
}

