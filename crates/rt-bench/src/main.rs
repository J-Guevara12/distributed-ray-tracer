use std::process::ExitCode;

use clap::Parser;

mod cli;
mod manifest;

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    if let Err(err) = cli.match_command() {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
