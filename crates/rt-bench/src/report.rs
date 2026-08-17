use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use serde::Serialize;

use crate::env::Env;

/// Orden de campos y nombres calcados de `bench_sweep.py`: ambos drivers
/// escriben en el mismo `history.jsonl`, así que el esquema solo admite
/// campos nuevos, nunca renombrados.
#[derive(Serialize)]
pub struct Record {
    pub benchmark: String,
    pub config: String,
    pub width: u32,
    pub spp: u32,
    pub commit: String,
    pub commit_label: String,
    pub commit_subject: String,
    pub commit_date: String,
    pub profile: &'static str,
    pub rep: usize,
    pub wall_ms: u128,
    pub rays: Option<u64>,
    pub rays_per_sec: Option<f64>,
    pub node_visits: Option<u64>,
    pub prim_tests: Option<u64>,
    pub image_hash: Option<String>,
    pub mse: Option<f64>,
    pub cpu_mhz: Option<f64>,
    pub timestamp: String,
    pub build_ms: Option<f64>,
    pub env: Env,
}

pub struct Stats {
    pub median: f64,
    pub rsd_pct: f64,
    pub n: usize,
}

/// Desviación estándar relativa, no rango min-max: el rango de 3 muestras es
/// sistemáticamente menor que el de 5, así que no compara entre corridas con
/// distinto número de reps.
pub fn stats(samples: &[f64]) -> Stats {
    let n = samples.len();
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);

    let median = if n.is_multiple_of(2) {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    } else {
        sorted[n / 2]
    };

    let mean = sorted.iter().sum::<f64>() / n as f64;
    let rsd_pct = if n > 1 && median > 0.0 {
        let variance = sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        variance.sqrt() / median * 100.0
    } else {
        0.0
    };

    Stats { median, rsd_pct, n }
}

pub fn append(path: &Path, records: &[Record]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;

    for record in records {
        let line = serde_json::to_string(record)?;
        writeln!(file, "{line}").with_context(|| format!("writing to {}", path.display()))?;
    }

    Ok(())
}
