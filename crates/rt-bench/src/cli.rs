use std::path::Path;
use std::time::Duration;

use anyhow::bail;
use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::manifest::{self, Benchmark, WorkloadKind, discover_benches, parse_bench_config};
use crate::runner::{self, RunOptions};

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
    /// Measures the current build against the benchmark suite
    Run (RunArgs),
}

#[derive(Args)]
pub struct RunArgs {
    #[arg(default_value_t="./scenes/bench".to_string())]
    #[arg(short, long)]
    pub base_dir: String,
    #[arg(default_value_t="bench.toml".to_string())]
    #[arg(short, long)]
    pub file_name: String,
    /// Workload to measure
    #[arg(long, value_enum, default_value_t=WorkloadKind::Quick)]
    pub config: WorkloadKind,
    /// Measure only these benchmarks, by id or name: --only B1 B2 / --only B1,B2
    #[arg(long, num_args = 1.., value_delimiter = ',')]
    pub only: Vec<String>,
    #[arg(long, default_value_t=5)]
    pub reps: usize,
    /// Seconds between runs, to let the CPU settle
    #[arg(long, default_value_t=20)]
    pub cooldown: u64,
    /// Defaults to the short HEAD sha, or "workdir" when the tree is dirty
    #[arg(long)]
    pub label: Option<String>,
    /// Matches the value hardcoded in `standalone`, which is what the
    /// historical sweep measured — changing it breaks comparability.
    #[arg(long, default_value_t=15)]
    pub max_depth: u32,
    /// 128 until 2026-08-18. The sweep measured 32 at 98.5% parallel efficiency
    /// against 83.5% for 128: with 24 threads, 128px tiles leave the last
    /// scheduling round short. Records before the change are not comparable;
    /// `env.tile_size` tells them apart.
    #[arg(long, default_value_t=32)]
    pub tile_size: u32,
    #[arg(long, default_value_t="./bench/history.jsonl".to_string())]
    pub out: String,
    /// Measure and print without writing to the history file
    #[arg(long)]
    pub no_record: bool,
    #[arg(long)]
    pub allow_dirty: bool,
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
    /// JSON (verbose uncompatible)
    Json,
}

impl Cli {
    pub fn match_command(&self) -> anyhow::Result<()> {
        match &self.command {
            Commands::List(args) => {
                let base_dir = Path::new(&args.base_dir);
                let file_name = &args.file_name;
                let verbose = args.verbose;

                let format = args.format;

                let config_files = discover_benches(base_dir, file_name)?;
                let configs = parse_bench_config(config_files)?;

                match format {
                    Format::Text => {
                        for config in configs {
                            config.print_pretty(verbose);
                        }
                    }
                    Format::Json => {
                        println!("{}", serde_json::to_string(&configs)?);
                    }
                    Format::Table => {
                        manifest::print_summary_table(&configs);
                    }
                }
            }

            Commands::Run(args) => {
                let base_dir = Path::new(&args.base_dir);
                let config_files = discover_benches(base_dir, &args.file_name)?;
                let mut benches = parse_bench_config(config_files)?;

                if !args.only.is_empty() {
                    let matches = |sel: &String, b: &Benchmark| {
                        &b.manifest.id == sel || &b.manifest.name == sel
                    };

                    let unknown: Vec<&String> = args
                        .only
                        .iter()
                        .filter(|sel| !benches.iter().any(|b| matches(sel, b)))
                        .collect();

                    if !unknown.is_empty() {
                        let available: Vec<&str> = benches
                            .iter()
                            .map(|b| b.manifest.id.as_str())
                            .collect();
                        bail!(
                            "unknown benchmark(s): {}. available: {}",
                            unknown.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "),
                            available.join(", ")
                        );
                    }

                    benches.retain(|b| args.only.iter().any(|sel| matches(sel, b)));
                }

                let opts = RunOptions {
                    kind: args.config,
                    reps: args.reps,
                    cooldown: Duration::from_secs(args.cooldown),
                    label: args.label.clone(),
                    max_depth: args.max_depth,
                    tile_size: args.tile_size,
                    out: (!args.no_record).then(|| args.out.clone()),
                    allow_dirty: args.allow_dirty,
                };

                runner::run(&benches, &opts)?;
            }
        }

        Ok(())
    }
}
