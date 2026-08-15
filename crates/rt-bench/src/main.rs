use clap::Parser;

mod manifest;
mod cli;

fn main() {
    let cli = cli::Cli::parse();
    cli.match_command();
}
