use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Benchmark {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub quick: Workload,
    pub full: Workload,
}

/// Carga de trabajo de una corrida.
#[derive(Serialize, Deserialize)]
pub struct Workload {
    pub width: u32,
    pub spp: u32,
}

struct Palette {
    reset: &'static str,
    bold: &'static str,
    dim: &'static str,
    cyan: &'static str,
    green: &'static str,
    gray: &'static str,
}

impl Palette {
    const PLAIN: Palette = Palette {
        reset: "",
        bold: "",
        dim: "",
        cyan: "",
        green: "",
        gray: "",
    };

    const ANSI: Palette = Palette {
        reset: "\x1b[0m",
        bold: "\x1b[1m",
        dim: "\x1b[2m",
        cyan: "\x1b[36m",
        green: "\x1b[32m",
        gray: "\x1b[90m",
    };
}

fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();
    PALETTE.get_or_init(|| {
        // NO_COLOR es la convención estándar para desactivar color por entorno.
        let colored = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        if colored { Palette::ANSI } else { Palette::PLAIN }
    })
}

impl Benchmark {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;

        toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn print_pretty(&self, verbose: bool) {
        let p = palette();

        println!(
            "{}[{}]{} {}{}{}",
            p.bold, self.id, p.reset, p.cyan, self.name, p.reset
        );

        println!(
            "  {} Quick profile:{} {}x{}px @ {}{} spp{}",
            p.dim, p.reset, self.quick.width, self.quick.width, p.green, self.quick.spp, p.reset
        );
        println!(
            "  {} Full profile: {} {}x{}px @ {}{} spp{}",
            p.dim, p.reset, self.full.width, self.full.width, p.green, self.full.spp, p.reset
        );

        if verbose && !self.notes.trim().is_empty() {
            println!("  {}Notes:{}", p.bold, p.reset);
            for line in self.notes.trim().lines() {
                println!("    {}{}{}", p.gray, line, p.reset);
            }
        }

        println!("{}", "-".repeat(60));
    }
}

pub fn discover_benches(base_dir: &Path, file_name: &str) -> anyhow::Result<Vec<PathBuf>> {
    if !base_dir.exists() {
        bail!("Benchmark directory does not exist: {}", base_dir.display());
    }
    if !base_dir.is_dir() {
        bail!("{} is not a directory", base_dir.display());
    }

    let mut files = vec![];
    let entries = fs::read_dir(base_dir)
        .with_context(|| format!("Listing {}", base_dir.display()))?;

    for entry in entries {
        let path = entry
            .with_context(|| format!("reading an entry of {}", base_dir.display()))?
            .path();

        if path.is_dir() {
            let manifest = path.join(file_name);
            if manifest.is_file() {
                files.push(manifest);
            }
        }
    }

    if files.is_empty() {
        bail!(
            "{} Not found in {}",
            file_name,
            base_dir.display()
        );
    }

    files.sort();

    Ok(files)
}

pub fn parse_bench_config(files: Vec<PathBuf>) -> anyhow::Result<Vec<Benchmark>> {
    files.iter().map(|file| Benchmark::load(file)).collect()
}

pub fn print_summary_table(benches: &[Benchmark]) {
    let p = palette();

    println!(
        "{:<6} {:<20} {:<15} {:<15}",
        "ID", "Nombre", "Quick (W/SPP)", "Full (W/SPP)"
    );
    println!("{}", "=".repeat(60));

    for bench in benches {
        let quick_str = format!(
            "{}x{} / {}spp",
            bench.quick.width, bench.quick.width, bench.quick.spp
        );
        let full_str = format!(
            "{}x{} / {}spp",
            bench.full.width, bench.full.width, bench.full.spp
        );

        println!(
            "{}{:<6}{} {:<20} {:<15} {:<15}",
            p.bold, bench.id, p.reset, bench.name, quick_str, full_str
        );
    }
}
