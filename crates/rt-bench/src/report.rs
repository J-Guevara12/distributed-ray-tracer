use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use anyhow::Context;
use serde::Serialize;

use crate::env::Env;

#[derive(Serialize)]
pub struct Record {
    pub benchmark: String,
    pub config: String,
    /// `path` or `normal`. A normals render is one ray per sample, so its
    /// Mray/s is a different quantity: filter on this before comparing.
    pub tracer: String,
    pub width: u32,
    pub height: u32,
    pub spp: u32,
    pub hardware: String,
    pub commit: String,
    pub commit_label: String,
    pub commit_subject: String,
    pub commit_date: String,
    pub profile: &'static str,
    pub rep: usize,
    pub wall_ms: u128,
    pub rays: Option<u64>,
    pub rays_per_sec: Option<f64>,
    pub samples: Option<u64>,
    pub samples_per_sec: Option<f64>,
    pub node_visits: Option<u64>,
    pub prim_tests: Option<u64>,
    /// De `prim_tests`, los que acertaron. Un test que falla sale temprano y
    /// cuesta la mitad; el roofline necesita separarlos.
    pub prim_hits: Option<u64>,
    pub image_hash: Option<String>,
    /// Error contra la imagen de referencia, en espacio lineal. `None` si la
    /// corrida no usó `--reference`.
    pub mse: Option<f64>,
    /// MSE dividido por la referencia. El plano lo domina la región brillante.
    pub relative_mse: Option<f64>,
    /// `1 / (mse × segundos)`. La única métrica que decide si un cambio que
    /// baja el tiempo a costa de ruido es realmente una mejora.
    pub efficiency: Option<f64>,
    /// spp y profundidad con que se generó la referencia, para poder auditar el
    /// piso de ruido del MSE sin abrir el sidecar.
    pub reference_spp: Option<u32>,
    pub reference_max_depth: Option<u32>,
    pub cpu_mhz: Option<f64>,
    pub timestamp: String,
    pub build_ms: Option<f64>,
    pub tiles: Option<TileSummary>,
    pub env: Env,
}

#[derive(Serialize, Clone)]
pub struct TileSummary {
    pub count: usize,
    pub min_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub imbalance: f64,
}

impl TileSummary {
    pub fn new(tile_ms: &[f64]) -> Option<Self> {
        if tile_ms.is_empty() {
            return None;
        }

        let mut sorted = tile_ms.to_vec();
        sorted.sort_by(f64::total_cmp);

        let n = sorted.len();
        let mean = sorted.iter().sum::<f64>() / n as f64;
        let max = sorted[n - 1];

        Some(Self {
            count: n,
            min_ms: sorted[0],
            median_ms: stats(&sorted).median,
            p95_ms: sorted[(((n - 1) as f64) * 0.95).round() as usize],
            max_ms: max,
            imbalance: if mean > 0.0 { (max - mean) / mean } else { 0.0 },
        })
    }
}

pub struct Stats {
    pub median: f64,
    pub rsd_pct: f64,
    pub n: usize,
}

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
