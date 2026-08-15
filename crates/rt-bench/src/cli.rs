use std::path::Path;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::manifest::{self, BenchConfig, discover_benches, parse_bench_config};

#[derive(Parser)]
#[command(name="Raytracer benchmark tool")]
#[command(version = "1.0")]
#[command(about = "Performs and manages benchmarks on the system", long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands
}

#[derive(Subcommand)]
pub enum Commands {
    /// Lists all the available benchmarks
    List (ListArgs),
}
#[derive(Args)]
pub struct ListArgs {
    #[arg(default_value_t="./scenes/bench".to_string())]
    #[arg(short, long)]
    pub base_dir: String,
    #[arg(default_value_t="bench.toml".to_string())]
    #[arg(short, long)]
    pub file_name: String,
    #[arg(short, long)]
    pub verbose: bool,
    #[arg(value_enum)]
    #[arg(long)]
    #[arg(default_value_t=Format::Text)]
    pub format: Format
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Format {
    /// Full text
    Text,
    /// Table (verbose uncompatible)
    Table,
    /// JSON
    Json,
}

impl Cli {
    pub fn match_command(&self) {
        match &self.command {
            Commands::List(args) => {
                let base_dir = Path::new(&args.base_dir);
                let file_name = &args.file_name;
                let verbose = args.verbose;

                let format = args.format;

                let config_files = discover_benches(base_dir, file_name).unwrap();
                let configs = parse_bench_config(config_files).unwrap();

                match format {
                    Format::Text => {
                        for config in configs {
                            config.print_pretty(verbose);
                        }
                    },
                    Format::Json => {
                        println!("{}", serde_json::to_string(&configs).unwrap());
                    }
                    Format::Table => {
                        manifest::print_summary_table(&configs);
                    }
                }
            },
        }

    }
}
