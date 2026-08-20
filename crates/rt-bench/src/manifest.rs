use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Context, bail};
use rt_core::camera::CameraConfig;
use rt_core::dto::ScenePayload;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub quick: Workload,
    pub full: Workload,
}

#[derive(Serialize, Deserialize)]
pub struct Benchmark {
    pub path: PathBuf,
    pub manifest: Manifest,
    #[serde(skip_serializing)]
    pub camera: CameraConfig,
    #[serde(skip_serializing)]
    pub scene: ScenePayload,
}

/// Carga de trabajo de una corrida.
#[derive(Serialize, Deserialize)]
pub struct Workload {
    pub width: u32,
    pub spp: u32,
}

#[derive(Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum WorkloadKind {
    Quick,
    Full,
}

impl WorkloadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkloadKind::Quick => "quick",
            WorkloadKind::Full => "full",
        }
    }
}

struct Palette {
    reset: &'static str,
    bold: &'static str,
    dim: &'static str,
    cyan: &'static str,
    green: &'static str,
    gray: &'static str,
    purple: &'static str,
}

impl Palette {
    const PLAIN: Palette = Palette {
        reset: "",
        bold: "",
        dim: "",
        cyan: "",
        green: "",
        gray: "",
        purple: "",
    };

    const ANSI: Palette = Palette {
        reset: "\x1b[0m",
        bold: "\x1b[1m",
        dim: "\x1b[2m",
        cyan: "\x1b[36m",
        green: "\x1b[32m",
        gray: "\x1b[90m",
        purple: "\x1b[35m",
    };
}

fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();
    PALETTE.get_or_init(|| {
        let colored = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
        if colored { Palette::ANSI } else { Palette::PLAIN }
    })
}

impl Benchmark {
    pub fn scene_path(&self) -> PathBuf {
        self.path.with_file_name("scene.json")
    }

    pub fn camera_path(&self) -> PathBuf {
        self.path.with_file_name("camera.json")
    }

    pub fn workload(&self, kind: WorkloadKind) -> &Workload {
        match kind {
            WorkloadKind::Quick => &self.manifest.quick,
            WorkloadKind::Full => &self.manifest.full,
        }
    }

    pub fn height(&self, width: u32) -> u32 {
        (width as f32 / self.camera.aspect_ratio) as u32
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let original_path = path.to_path_buf();
        let content = fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;

        let manifest = toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;

        let path = path.with_file_name("camera.json");
        let content = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let camera = serde_json::de::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;

        let path = path.with_file_name("scene.json");
        let content = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;

        let scene: ScenePayload = serde_json::de::from_str(&content).with_context(|| format!("parsing {}", path.display()))?;
        let path = original_path;

        Ok(Self{ path, manifest, camera, scene  })
    }

    pub fn print_pretty(&self, verbose: bool) {
        let p = palette();
        let quick_height = (self.manifest.quick.width as f32 / self.camera.aspect_ratio) as i32;
        let full_height = (self.manifest.full.width as f32 / self.camera.aspect_ratio) as i32;

        println!(
            "{}[{}]{} {}{}{}",
            p.bold, self.manifest.id, p.reset, p.cyan, self.manifest.name, p.reset
        );

        println!(
            "  {} Quick profile:{}   {}x{}px @ {}{} spp{}",
            p.dim, p.reset, self.manifest.quick.width, quick_height, p.green, self.manifest.quick.spp, p.reset
        );
        println!(
            "  {} Full profile:{}    {}x{}px @ {}{} spp{}",
            p.dim, p.reset, self.manifest.full.width, full_height, p.green, self.manifest.full.spp, p.reset
        );
        println!(
            "  {} No. of Objects:{}  {}{}{}",
            p.dim, p.reset, p.purple, self.scene.objects.len(), p.reset
        );
        println!(
            "  {} No. of Materials:{} {}{}{}",
            p.dim, p.reset, p.purple, self.scene.materials.len(), p.reset
        );


        if verbose && !self.manifest.notes.trim().is_empty() {
            println!("  {}Notes:{}", p.bold, p.reset);
            for line in self.manifest.notes.trim().lines() {
                println!("    {}{}{}", p.gray, line, p.reset);
            }
        }

        println!("{}", "-".repeat(60));
    }
}

/// Filtra por id o nombre. Falla listando lo disponible en vez de devolver una
/// lista vacía en silencio, que se ve igual que "no hay benchmarks".
pub fn select(mut benches: Vec<Benchmark>, only: &[String]) -> anyhow::Result<Vec<Benchmark>> {
    if only.is_empty() {
        return Ok(benches);
    }

    let matches = |sel: &String, b: &Benchmark| &b.manifest.id == sel || &b.manifest.name == sel;

    let unknown: Vec<&str> = only
        .iter()
        .filter(|sel| !benches.iter().any(|b| matches(sel, b)))
        .map(|s| s.as_str())
        .collect();

    if !unknown.is_empty() {
        let available: Vec<&str> = benches.iter().map(|b| b.manifest.id.as_str()).collect();
        bail!(
            "unknown benchmark(s): {}. available: {}",
            unknown.join(", "),
            available.join(", ")
        );
    }

    benches.retain(|b| only.iter().any(|sel| matches(sel, b)));
    Ok(benches)
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
        "{}{:<6} {:<20} {:<25} {:<25} {:<15} {:<15}{}",
        p.bold, "ID", "Nombre", "Quick (W/SPP)", "Full (W/SPP)", "No. Objects", "No. Materials", p.reset
    );
    println!("{}{}{}", p.cyan, "=".repeat(109), p.reset);

    for bench in benches {
        let quick_height = (bench.manifest.quick.width as f32 / bench.camera.aspect_ratio) as i32;
        let full_height = (bench.manifest.full.width as f32 / bench.camera.aspect_ratio) as i32;

        let manifest = &bench.manifest;
        let quick_str = format!(
            "{}x{} / {}spp",
            manifest.quick.width, quick_height, manifest.quick.spp
        );
        let full_str = format!(
            "{}x{} / {}spp",
            manifest.full.width, full_height, manifest.full.spp
        );

        println!(
            "{}{:<6}{} {:<20} {:<25} {:<25} {:<15} {:<15}",
            p.bold, manifest.id, p.reset, manifest.name, quick_str, full_str, bench.scene.objects.len(), bench.scene.materials.len()
        );
    }
}
