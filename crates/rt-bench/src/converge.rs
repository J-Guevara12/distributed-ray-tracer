//! Convergence curve: MSE against time as the sample count grows.
//!
//! This is the instrument Phase 1 needs. `run --reference` gives one MSE point
//! per config, which tracks efficiency per commit but cannot answer the question
//! F1 is about: does this integrator reach a given error faster than that one.
//! For that you need several points along the spp axis, at equal *time*, not at
//! equal samples — an integrator that costs twice per sample and needs a tenth
//! of them wins, and the clock alone says it got slower.
//!
//! Separate subcommand and separate file on purpose. `bench.toml` stays the
//! single source of truth for the workload of `run`, so sweeping spp here cannot
//! contaminate `history.jsonl` with records whose spp did not come from the
//! manifest.
//!
//! Records carry the commit and the hardware generation because MSE depends on
//! the code and time depends on the machine, so a curve is only comparable
//! within one of each.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};
use serde::Serialize;

use rt_core::display::resolve;
use rt_renderer::camera::Camera;
use rt_renderer::exr_io;
use rt_renderer::framebuffer::FrameBuffer;
use rt_renderer::render::render_scene;
use rt_renderer::tiles::TileResult;
use rt_scene::hittable_list::SceneData;
use rt_scene::{Scene, bvh::Bvh};

use crate::env;
use crate::hardware::Hardware;
use crate::manifest::{Benchmark, Tracer, WorkloadKind};
use crate::reference;

/// Powers of two: the noise floor falls as `1/sqrt(spp)`, so a geometric ladder
/// puts the points evenly on the log-log plot the curve is read on.
pub const DEFAULT_SPP: &[u32] = &[1, 2, 4, 8, 16, 32, 64, 128, 256];

pub struct ConvergeOptions {
    pub kind: WorkloadKind,
    pub tracer: Tracer,
    pub spp: Vec<u32>,
    pub reps: usize,
    pub cooldown: std::time::Duration,
    pub max_depth: u32,
    pub tile_size: u32,
    pub reference_dir: PathBuf,
    pub out: PathBuf,
    pub hardware: Hardware,
    pub allow_dirty: bool,
}

#[derive(Serialize)]
struct Point {
    hardware: String,
    benchmark: String,
    tracer: String,
    /// Which estimator ran. This is what separates the curves once there is
    /// more than one; `tracer` stays as the CLI selection that produced it.
    integrator: String,
    commit: String,
    commit_label: String,
    commit_date: String,
    dirty: bool,
    width: u32,
    height: u32,
    spp: u32,
    rep: usize,
    wall_ms: u128,
    mse: f64,
    relative_mse: f64,
    rmse: f64,
    efficiency: f64,
    rays: u64,
    node_visits: u64,
    prim_tests: u64,
    prim_hits: u64,
    reference_spp: u32,
    reference_max_depth: u32,
    max_depth: u32,
    tile_size: u32,
    timestamp: String,
}

pub fn run(benches: &[Benchmark], opts: &ConvergeOptions) -> anyhow::Result<()> {
    if cfg!(debug_assertions) {
        bail!("rt-bench must be built in release mode");
    }
    if opts.tracer != Tracer::Path {
        bail!(
            "convergence only makes sense with --tracer path; a {} render has \
             nothing to converge to",
            opts.tracer.as_str()
        );
    }
    if opts.spp.is_empty() {
        bail!("--spp cannot be empty");
    }

    let dirty = env::is_dirty()?;
    if dirty && !opts.allow_dirty {
        bail!(
            "working tree is dirty; a convergence curve from uncommitted code is \
             not attributable (use --allow-dirty to override)"
        );
    }

    let commit = env::head_commit()?;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let integrator = Arc::new(opts.tracer.build(opts.max_depth));
    let mut points = Vec::new();

    println!(
        "hardware={}  config={}  tracer={}  reps={}  spp={:?}",
        opts.hardware.id,
        opts.kind.as_str(),
        opts.tracer.as_str(),
        opts.reps,
        opts.spp
    );

    for bench in benches {
        let workload = bench.workload(opts.kind);
        let width = workload.width;
        let height = bench.height(width);

        // Loaded before rendering anything: a stale reference should cost
        // seconds, not the whole sweep.
        let largest = *opts.spp.iter().max().unwrap();
        let reference = reference::load(
            &opts.reference_dir,
            bench,
            width,
            height,
            opts.max_depth,
            largest,
        )?;

        println!(
            "\n== {} {width}x{height} ==  reference {} spp / d{}",
            bench.manifest.id, reference.meta.spp, reference.meta.max_depth
        );
        println!(
            "  {:>6} {:>10} {:>12} {:>12} {:>12}",
            "spp", "render", "mse", "rel_mse", "efficiency"
        );

        for &spp in &opts.spp {
            for rep in 1..=opts.reps {
                std::thread::sleep(opts.cooldown);

                let mut config = bench.camera;
                config.image_width = width;
                config.samples_per_pixel = spp;
                let camera = Camera::new(config);

                let data = SceneData::from(&bench.scene);
                let scene = Scene {
                    world: Arc::new(Bvh::build(data.objects)),
                    materials: data.materials,
                    background: bench.scene.background.clone(),
                };

                let framebuffer = Arc::new(FrameBuffer::new(width, height));
                let snapshot_source = Arc::clone(&framebuffer);

                let started = std::time::Instant::now();
                let stats = render_scene(
                    Arc::new(camera),
                    Arc::clone(&integrator),
                    framebuffer,
                    &|_: &TileResult| {},
                    opts.tile_size,
                    &scene,
                );
                let wall_ms = started.elapsed().as_millis();

                let pixels = resolve(&snapshot_source.get_snapshot());
                let comparison = reference::compare(&pixels, &reference)?;
                let seconds = wall_ms as f64 / 1000.0;
                let efficiency = exr_io::efficiency(comparison.mse, seconds);

                if rep == 1 {
                    println!(
                        "  {spp:>6} {wall_ms:>7} ms {:>12.4e} {:>12.4e} {efficiency:>12.4}",
                        comparison.mse, comparison.relative_mse
                    );
                }

                points.push(Point {
                    hardware: opts.hardware.id.clone(),
                    benchmark: bench.manifest.id.clone(),
                    tracer: opts.tracer.as_str().to_string(),
                    integrator: integrator.name().to_string(),
                    commit: commit.sha[..12].to_string(),
                    commit_label: if dirty {
                        "workdir".to_string()
                    } else {
                        commit.sha[..12].to_string()
                    },
                    commit_date: commit.date.clone(),
                    dirty,
                    width,
                    height,
                    spp,
                    rep,
                    wall_ms,
                    mse: comparison.mse,
                    relative_mse: comparison.relative_mse,
                    rmse: comparison.rmse,
                    efficiency,
                    rays: stats.rays,
                    node_visits: stats.traversal.node_visits,
                    prim_tests: stats.traversal.prim_tests,
                    prim_hits: stats.traversal.prim_hits,
                    reference_spp: reference.meta.spp,
                    reference_max_depth: reference.meta.max_depth,
                    max_depth: opts.max_depth,
                    tile_size: opts.tile_size,
                    timestamp: timestamp.clone(),
                });
            }
        }
    }

    if let Some(parent) = opts.out.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&opts.out)
        .with_context(|| format!("opening {}", opts.out.display()))?;

    use std::io::Write;
    for point in &points {
        writeln!(file, "{}", serde_json::to_string(point)?)?;
    }

    println!("\n{} points appended to {}", points.len(), opts.out.display());
    Ok(())
}
