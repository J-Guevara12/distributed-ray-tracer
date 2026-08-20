//! Stale binary detection.
//!
//! `rt-bench` measures the renderer linked *into itself*, so running an old
//! binary measures old code under the new commit's label. That already ruined a
//! full round of F0.7 measurements.
//!
//! Recompiling from the running process fixes nothing, since the old code is
//! already loaded. Hence two mechanisms: the guard always refuses to measure a
//! binary older than the sources, and `--build` rebuilds and re-execs, which is
//! the only thing that can actually measure the new code.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use anyhow::{Context, bail};

/// Re-exec marker, to avoid a build loop.
const REBUILT: &str = "RT_BENCH_REBUILT";

/// `.cargo/config.toml` is here because it carries `target-cpu=native`.
const EXTRA_INPUTS: &[&str] = &["Cargo.toml", "Cargo.lock", ".cargo/config.toml"];

fn newest_source(root: &Path) -> anyhow::Result<Option<(PathBuf, SystemTime)>> {
    let mut newest: Option<(PathBuf, SystemTime)> = None;

    let mut consider = |path: PathBuf| -> anyhow::Result<()> {
        let modified = path.metadata().and_then(|m| m.modified());
        if let Ok(time) = modified
            && newest.as_ref().is_none_or(|(_, best)| time > *best)
        {
            newest = Some((path, time));
        }
        Ok(())
    };

    let mut stack = vec![root.join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            if name == "target" || name == ".git" {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "toml") {
                consider(path)?;
            }
        }
    }

    for extra in EXTRA_INPUTS {
        let path = root.join(extra);
        if path.exists() {
            consider(path)?;
        }
    }

    Ok(newest)
}

/// Refuses to measure with a binary older than the sources.
pub fn ensure_fresh(root: &Path) -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("locating the running binary")?;
    let built = exe
        .metadata()
        .and_then(|m| m.modified())
        .context("reading the binary's timestamp")?;

    let Some((source, changed)) = newest_source(root)? else {
        return Ok(());
    };

    if changed > built {
        let relative = source.strip_prefix(root).unwrap_or(&source);
        bail!(
            "the binary is older than {}: you would be measuring code that is not compiled.\n\
             Rebuild with `cargo build --release -p rt-bench`, or pass --build to have \
             rt-bench do it and re-exec.",
            relative.display()
        );
    }

    Ok(())
}

/// Rebuilds and re-execs with the same arguments. Does not return on success.
pub fn rebuild_and_reexec() -> anyhow::Result<()> {
    if std::env::var_os(REBUILT).is_some() {
        bail!(
            "already rebuilt once and the binary is still stale. \
             Check that --release and the package are the right ones."
        );
    }

    // Captured before the build: cargo unlinks the old binary, after which
    // /proc/self/exe reads "<path> (deleted)", which does not exist.
    let exe = std::env::current_exe().context("locating the running binary")?;

    println!("building rt-bench in release...");
    let status = Command::new("cargo")
        .args(["build", "--release", "--bin", "rt-bench"])
        .status()
        .context("launching cargo build")?;

    if !status.success() {
        bail!("cargo build failed; nothing measured");
    }

    let args: Vec<String> = std::env::args().skip(1).collect();

    println!("re-executing the new binary\n");
    let status = Command::new(&exe)
        .args(&args)
        .env(REBUILT, "1")
        .status()
        .with_context(|| format!("re-executing {}", exe.display()))?;

    std::process::exit(status.code().unwrap_or(1));
}
