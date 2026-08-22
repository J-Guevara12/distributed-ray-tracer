use std::process::ExitCode;

use clap::Parser;

mod build;
mod ceilings;
mod cli;
mod converge;
mod env;
mod hardware;
mod manifest;
mod preview;
mod reference;
mod report;
mod runner;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    if let Err(err) = cli.match_command() {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
