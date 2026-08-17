use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::bail;
use rt_core::camera::Camera;
use rt_renderer::framebuffer::FrameBuffer;
use rt_renderer::render::render_scene;
use rt_renderer::tiles::TileResult;
use rt_renderer::tracers::PathTracer;
use rt_scene::bvh::BvhNode;
use rt_scene::hittable_list::HittableList;

use crate::env;
use crate::manifest::{Benchmark, WorkloadKind};
use crate::report::{self, Record, Stats, stats};

pub struct RunOptions {
    pub kind: WorkloadKind,
    pub reps: usize,
    pub cooldown: Duration,
    pub label: Option<String>,
    pub max_depth: u32,
    pub tile_size: u32,
    pub out: Option<String>,
    pub allow_dirty: bool,
}

struct Timing {
    build_ms: f64,
    wall_ms: u128,
}

fn measure(bench: &Benchmark, opts: &RunOptions) -> Timing {
    let workload = bench.workload(opts.kind);

    let mut config = bench.camera;
    config.image_width = workload.width;
    config.samples_per_pixel = workload.spp;
    let camera = Camera::new(config);

    let build_start = Instant::now();
    let list = HittableList::from(&bench.scene);
    let world = BvhNode::new(list.objects);
    let build_ms = build_start.elapsed().as_secs_f64() * 1000.0;

    let framebuffer = Arc::new(FrameBuffer::new(camera.width, camera.height));
    let tracer = Arc::new(PathTracer::new(opts.max_depth));
    let on_tile = |_: &TileResult| {};

    let render_start = Instant::now();
    render_scene(
        Arc::new(camera),
        tracer,
        framebuffer,
        &on_tile,
        opts.tile_size,
        &world,
        &bench.scene.background,
    );

    Timing {
        build_ms,
        wall_ms: render_start.elapsed().as_millis(),
    }
}

pub fn run(benches: &[Benchmark], opts: &RunOptions) -> anyhow::Result<()> {
    if cfg!(debug_assertions) {
        bail!("rt-bench must be built in release mode; a debug build would pollute history.jsonl");
    }

    let dirty = env::is_dirty()?;
    if dirty && !opts.allow_dirty {
        bail!(
            "working tree is dirty; a measurement from uncommitted code is not attributable \
             (use --allow-dirty to override)"
        );
    }

    let commit = env::head_commit()?;
    let label = opts
        .label
        .clone()
        .unwrap_or_else(|| if dirty { "workdir".to_string() } else { commit.sha[..12].to_string() });

    let environment = env::collect(benches, dirty, opts.max_depth, opts.tile_size)?;

    println!(
        "config={}  reps={}  cooldown={}s  max_depth={}  tile_size={}  label={label}",
        opts.kind.as_str(),
        opts.reps,
        opts.cooldown.as_secs(),
        opts.max_depth,
        opts.tile_size,
    );
    if dirty {
        println!("WARNING: dirty working tree, results are not attributable to a commit");
    }

    println!("\n== warmup ({} runs, not recorded) ==", benches.len());
    for bench in benches {
        let timing = measure(bench, opts);
        println!("  {} {} ms", bench.manifest.id, timing.wall_ms);
    }

    println!(
        "\n== measuring ({} reps x {} benchmarks) ==",
        opts.reps,
        benches.len()
    );

    let timestamp = chrono::Utc::now().to_rfc3339();
    let mut records = Vec::new();

    for rep in 1..=opts.reps {
        for bench in benches {
            std::thread::sleep(opts.cooldown);

            let mhz = env::cpu_mhz();
            let timing = measure(bench, opts);
            let workload = bench.workload(opts.kind);

            println!(
                "  [rep {rep}] {} {} ms (build {:.2} ms){}",
                bench.manifest.id,
                timing.wall_ms,
                timing.build_ms,
                mhz.map(|m| format!("  {m:.0} MHz")).unwrap_or_default(),
            );

            records.push(Record {
                benchmark: bench.manifest.id.clone(),
                config: opts.kind.as_str().to_string(),
                width: workload.width,
                spp: workload.spp,
                commit: commit.sha[..12].to_string(),
                commit_label: label.clone(),
                commit_subject: commit.subject.clone(),
                commit_date: commit.date.clone(),
                profile: "optimized",
                rep,
                wall_ms: timing.wall_ms,
                rays: None,
                rays_per_sec: None,
                node_visits: None,
                prim_tests: None,
                image_hash: None,
                mse: None,
                cpu_mhz: mhz,
                timestamp: timestamp.clone(),
                build_ms: Some(timing.build_ms),
                env: environment.clone(),
            });
        }
    }

    print_summary(benches, &records, opts);

    match &opts.out {
        Some(path) => {
            let path = Path::new(path);
            report::append(path, &records)?;
            println!("\n{} records appended to {}", records.len(), path.display());
        }
        None => println!("\n{} records measured (not recorded)", records.len()),
    }

    Ok(())
}

fn print_summary(benches: &[Benchmark], records: &[Record], opts: &RunOptions) {
    println!("\n== summary ==");
    println!(
        "  {:<5} {:<16} {:>13} {:>6} {:>11} {:>11} {:>7} {:>4}",
        "ID", "name", "resolution", "spp", "build", "render", "rsd", "n"
    );
    println!("  {}", "-".repeat(80));

    for bench in benches {
        let id = &bench.manifest.id;
        let workload = bench.workload(opts.kind);

        let wall: Vec<f64> = records
            .iter()
            .filter(|r| &r.benchmark == id)
            .map(|r| r.wall_ms as f64)
            .collect();
        let build: Vec<f64> = records
            .iter()
            .filter(|r| &r.benchmark == id)
            .filter_map(|r| r.build_ms)
            .collect();

        if wall.is_empty() {
            continue;
        }

        let Stats { median, rsd_pct, n } = stats(&wall);
        let build_median = stats(&build).median;

        println!(
            "  {:<5} {:<16} {:>13} {:>6} {:>8.2} ms {:>8.0} ms {:>6.1}% {:>4}",
            id,
            bench.manifest.name,
            format!("{}x{}", workload.width, bench.height(workload.width)),
            workload.spp,
            build_median,
            median,
            rsd_pct,
            n,
        );
    }
}
