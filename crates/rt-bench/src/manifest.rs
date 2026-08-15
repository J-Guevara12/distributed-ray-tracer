use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::fs;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct BenchConfig {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub quick: ProfileConfig,
    pub full: ProfileConfig,
}

#[derive(Serialize, Deserialize)]
pub struct ProfileConfig {
    pub width: u32,
    pub spp: u32
}

// Estilos y Colores ANSI
const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const GRAY: &str = "\x1b[90m";

impl BenchConfig {
    pub fn load(path: &Path) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: BenchConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn print_pretty(&self, verbose: bool) {
        // Encabezado principal: [ID] Nombre
        println!(
            "{}[{}]{} {}{}{}",
            BOLD, self.id, RESET, CYAN, self.name, RESET
        );

        // Perfiles de ejecución
        println!(
            "  {}Perfil Quick:{} {}x{}px @ {}{} spp{}",
            DIM, RESET, self.quick.width, self.quick.width, GREEN, self.quick.spp, RESET
        );
        println!(
            "  {}Perfil Full: {} {}x{}px @ {}{} spp{}",
            DIM, RESET, self.full.width, self.full.width, GREEN, self.full.spp, RESET
        );

        // Modo Verbose: Muestra las notas formateadas e indentadas
        if verbose && !self.notes.trim().is_empty() {
            println!("  {}Notas:{}", BOLD, RESET);
            for line in self.notes.trim().lines() {
                println!("    {}{}{}", GRAY, line, RESET);
            }
        }

        // Separador visual
        println!("{}", "-".repeat(60));
    }
}

pub fn discover_benches(base_dir: &Path, file_name: &str) -> Result<Vec<PathBuf>, anyhow::Error>{
    let mut files = vec![];
    if !base_dir.is_dir() {
        return Err(std::io::Error::new(ErrorKind::InvalidInput, "base_dir Must be a directory").into());
    }
    if !base_dir.exists() {
        return Err(std::io::Error::new(ErrorKind::InvalidInput, "base_dir Must be a directory").into());
    }
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            let mut filepath = path.to_path_buf();
            filepath.push(file_name);
            if fs::exists(&filepath)? {
                files.push(filepath);
            }
        }
    }

    files.sort();

    Ok(files)

}

pub fn parse_bench_config(files: Vec<PathBuf>) -> Result<Vec<BenchConfig>, anyhow::Error>{
    let mut configs = vec![];
    for file in files {
        if !file.is_file() {
            return Err(std::io::Error::new(ErrorKind::InvalidInput, "base_dir Must be a directory").into());
        }
        let config = BenchConfig::load(&file)?;
        configs.push(config)
    }

    Ok(configs)
}

pub fn print_summary_table(benches: &[BenchConfig]) {
    println!("{:<6} {:<20} {:<15} {:<15}", "ID", "Nombre", "Quick (W/SPP)", "Full (W/SPP)");
    println!("{}", "=".repeat(60));

    for bench in benches {
        let quick_str = format!("{}x{} / {}spp", bench.quick.width, bench.quick.width, bench.quick.spp);
        let full_str = format!("{}x{} / {}spp", bench.full.width, bench.full.width, bench.full.spp);

        println!(
            "\x1b[1m{:<6}\x1b[0m {:<20} {:<15} {:<15}",
            bench.id, bench.name, quick_str, full_str
        );
    }
}

