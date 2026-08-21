use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::build;
use crate::hardware;
use crate::manifest::{self, Tracer, WorkloadKind, discover_benches, parse_bench_config, select};
use crate::reference::{self, ReferenceOptions};
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
    /// Creates a high accuracy copy of the current benchmark images, saves it in EXR format
    Reference(ReferenceArgs),
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
    /// 15 until 2026-08-20, when Russian roulette landed. With roulette the
    /// depth stops being the termination mechanism and becomes a safety net for
    /// pathological paths (total internal reflection inside the glass sphere),
    /// so a low cap only adds truncation bias. A diffuse path reaching 64 has
    /// probability ~1e-7. `standalone` still hardcodes 15 for the historical
    /// sweep; `env.max_depth` tells the two eras apart.
    #[arg(long, default_value_t=64)]
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
    /// Directory holding the reference EXRs. Enables MSE and efficiency.
    #[arg(long)]
    pub reference: Option<PathBuf>,
    /// Overrides `current` in bench/hardware.toml for this run.
    #[arg(long)]
    pub hardware: Option<String>,
    #[arg(long, default_value_t=hardware::DEFAULT_PATH.to_string())]
    pub hardware_file: String,
    /// Rebuild in release and re-exec before measuring. Without it, a binary
    /// older than the sources is refused rather than measured.
    #[arg(long)]
    pub build: bool,
    #[arg(long)]
    pub allow_dirty: bool,
    /// Describes the tipe of tracer to use:
    /// Normal: Traces a normal map, no bounces
    /// Path: Traces a full ray with bounces and all the optic properties implemented
    #[arg(long, value_enum, default_value_t=Tracer::Path)]
    pub tracer: Tracer,
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
#[derive(Args)]
pub struct ReferenceArgs {
    #[arg(default_value_t="./scenes/bench".to_string())]
    #[arg(short, long)]
    pub base_dir: String,
    #[arg(default_value_t="bench.toml".to_string())]
    #[arg(short, long)]
    pub file_name: String,
    /// Measure only these benchmarks, by id or name: --only B1 B2 / --only B1,B2
    #[arg(long, num_args = 1.., value_delimiter = ',')]
    pub only: Vec<String>,
    /// Which workload's resolution to render. The reference is tied to the
    /// width, so `quick` and `full` need separate files when they differ.
    #[arg(long, value_enum, default_value_t=WorkloadKind::Full)]
    pub config: WorkloadKind,
    /// High on purpose: a fixed depth truncates the path and loses energy, so a
    /// shallow reference bakes that bias in and `run` would measure it as error.
    #[arg(long, default_value_t=100)]
    pub max_depth: u32,
    #[arg(long, default_value_t=32)]
    pub tile_size: u32,
    /// Reference noise adds to the measured MSE as a floor. Variance goes as
    /// 1/spp, so ~100x the measured spp puts that floor at 1%. Below 50x the
    /// command warns.
    #[arg(long, default_value_t=16384)]
    pub spp: u32,
    #[arg(long, default_value_t="./bench/reference".to_string())]
    pub out_dir: String,
    /// Rebuild in release and re-exec before rendering. Generating a reference
    /// from a stale binary wastes an hour and produces a wrong ground truth.
    #[arg(long)]
    pub build: bool,
    #[arg(long)]
    pub allow_dirty: bool,
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
                if args.build {
                    if build::ensure_fresh(Path::new(".")).is_err() {
                        build::rebuild_and_reexec()?;
                    }
                } else {
                    build::ensure_fresh(Path::new("."))?;
                }

                let hardware =
                    hardware::load(Path::new(&args.hardware_file), args.hardware.as_deref())?;

                let base_dir = Path::new(&args.base_dir);
                let config_files = discover_benches(base_dir, &args.file_name)?;
                let benches = select(parse_bench_config(config_files)?, &args.only)?;

                let opts = RunOptions {
                    kind: args.config,
                    reps: args.reps,
                    cooldown: Duration::from_secs(args.cooldown),
                    label: args.label.clone(),
                    max_depth: args.max_depth,
                    tile_size: args.tile_size,
                    out: (!args.no_record).then(|| args.out.clone()),
                    allow_dirty: args.allow_dirty,
                    reference_dir: args.reference.clone(),
                    tracer: args.tracer,
                    hardware,
                };

                runner::run(&benches, &opts)?;
            }

            Commands::Reference(args) => {
                if args.build {
                    if build::ensure_fresh(Path::new(".")).is_err() {
                        build::rebuild_and_reexec()?;
                    }
                } else {
                    build::ensure_fresh(Path::new("."))?;
                }

                let base_dir = Path::new(&args.base_dir);
                let config_files = discover_benches(base_dir, &args.file_name)?;
                let benches = select(parse_bench_config(config_files)?, &args.only)?;

                let opts = ReferenceOptions {
                    kind: args.config,
                    spp: args.spp,
                    max_depth: args.max_depth,
                    tile_size: args.tile_size,
                    out_dir: PathBuf::from(&args.out_dir),
                    allow_dirty: args.allow_dirty,
                };

                reference::generate(&benches, &opts)?;
            }
        }

        Ok(())
    }
}
