// Author Dustin Pilgrim
// License: MIT

mod app;
mod cli;
mod config;
mod log;
mod managed;
mod paths;
mod ops;

fn main() -> std::process::ExitCode {
    app::run()
}

