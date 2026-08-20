use std::process::ExitCode;

use clap::Parser;

mod build;
mod cli;
mod env;
mod hardware;
mod manifest;
mod report;
mod reference;
mod runner;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    if let Err(err) = cli.match_command() {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
