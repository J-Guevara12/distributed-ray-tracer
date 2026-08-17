use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::manifest::Benchmark;

#[derive(Serialize, Clone)]
pub struct Env {
    pub rustc: String,
    pub cpu: String,
    pub cpu_threads: usize,
    pub platform: String,
    pub scene_hashes: BTreeMap<String, SceneHashes>,
    pub dirty: bool,
    pub driver: &'static str,
    pub max_depth: u32,
    pub tile_size: u32,
}

#[derive(Serialize, Clone)]
pub struct SceneHashes {
    pub scene: String,
    pub camera: String,
    pub manifest: String,
}

pub struct Commit {
    pub sha: String,
    pub subject: String,
    pub date: String,
}

fn git(args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .with_context(|| format!("running git {}", args.join(" ")))?;

    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn head_commit() -> anyhow::Result<Commit> {
    let raw = git(&["show", "-s", "--format=%H%x1f%s%x1f%cI", "HEAD"])?;
    let mut parts = raw.split('\u{1f}');

    Ok(Commit {
        sha: parts.next().unwrap_or_default().to_string(),
        subject: parts.next().unwrap_or_default().to_string(),
        date: parts.next().unwrap_or_default().to_string(),
    })
}

pub fn is_dirty() -> anyhow::Result<bool> {
    Ok(!git(&["status", "--porcelain"])?.is_empty())
}

fn rustc_version() -> String {
    Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| std::env::consts::ARCH.to_string())
}

pub fn cpu_mhz() -> Option<f64> {
    let dir = fs::read_dir("/sys/devices/system/cpu").ok()?;
    let mut sum = 0.0;
    let mut count = 0;

    for entry in dir.flatten() {
        let path = entry.path().join("cpufreq/scaling_cur_freq");
        if let Ok(text) = fs::read_to_string(&path)
            && let Ok(khz) = text.trim().parse::<f64>() {
                sum += khz / 1000.0;
                count += 1;
            }
    }

    (count > 0).then(|| (sum / count as f64 * 10.0).round() / 10.0)
}

fn file_sha(path: &Path) -> anyhow::Result<String> {
    let bytes = fs::read(path).with_context(|| format!("hashing {}", path.display()))?;
    Ok(hex16(&Sha256::digest(&bytes)))
}

fn hex16(digest: &[u8]) -> String {
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

pub fn collect(
    benches: &[Benchmark],
    dirty: bool,
    max_depth: u32,
    tile_size: u32,
) -> anyhow::Result<Env> {
    let mut scene_hashes = BTreeMap::new();

    for bench in benches {
        scene_hashes.insert(
            bench.manifest.name.clone(),
            SceneHashes {
                scene: file_sha(&bench.scene_path())?,
                camera: file_sha(&bench.camera_path())?,
                manifest: file_sha(&bench.path)?,
            },
        );
    }

    Ok(Env {
        rustc: rustc_version(),
        cpu: cpu_model(),
        cpu_threads: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(0),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        scene_hashes,
        dirty,
        driver: "rt-bench",
        max_depth,
        tile_size,
    })
}
