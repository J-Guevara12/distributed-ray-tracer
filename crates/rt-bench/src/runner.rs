use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::bail;
use rt_core::camera::Camera;
use rt_core::display::resolve;
use rt_renderer::exr_io::Comparison;
use rt_renderer::framebuffer::FrameBuffer;
use rt_renderer::render::render_scene;
use rt_renderer::tiles::TileResult;
use rt_renderer::tracers::PathTracer;
use rt_scene::bvh::Bvh;
use rt_scene::hittable_list::SceneData;
use rt_scene::Scene;

use crate::env;
use crate::manifest::{Benchmark, WorkloadKind};
use crate::hardware::Hardware;
use crate::reference::{self, Reference};
use crate::report::{self, Record, Stats, TileSummary, stats};

pub struct RunOptions {
    pub kind: WorkloadKind,
    pub reps: usize,
    pub cooldown: Duration,
    pub label: Option<String>,
    pub max_depth: u32,
    pub tile_size: u32,
    pub out: Option<String>,
    pub allow_dirty: bool,
    /// Directorio con las referencias EXR. Sin esto no se calcula MSE.
    pub reference_dir: Option<PathBuf>,
    pub hardware: Hardware,
}

struct Timing {
    comparison: Option<Comparison>,
    build_ms: f64,
    wall_ms: u128,
    rays: u64,
    samples: u64,
    node_visits: u64,
    prim_tests: u64,
    tile_ms: Vec<f64>,
    image_hash: String,
}

impl Timing {
    fn secs(&self) -> f64 {
        self.wall_ms as f64 / 1000.0
    }

    fn nodes_per_ray(&self) -> f64 {
        if self.rays == 0 { 0.0 } else { self.node_visits as f64 / self.rays as f64 }
    }

    fn prims_per_ray(&self) -> f64 {
        if self.rays == 0 { 0.0 } else { self.prim_tests as f64 / self.rays as f64 }
    }

    fn rays_per_sec(&self) -> f64 {
        if self.wall_ms == 0 { 0.0 } else { self.rays as f64 / self.secs() }
    }

    fn samples_per_sec(&self) -> f64 {
        if self.wall_ms == 0 { 0.0 } else { self.samples as f64 / self.secs() }
    }
}

fn load_references(
    benches: &[Benchmark],
    opts: &RunOptions,
) -> anyhow::Result<HashMap<String, Reference>> {
    let Some(dir) = &opts.reference_dir else {
        return Ok(HashMap::new());
    };

    println!("\n== references ({}) ==", dir.display());
    let mut loaded = HashMap::new();

    for bench in benches {
        let workload = bench.workload(opts.kind);
        let width = workload.width;
        let height = bench.height(width);

        let reference =
            reference::load(dir, bench, width, height, opts.max_depth, workload.spp)?;
        println!(
            "  {} {width}x{height}  {} spp, max_depth {}  ({})",
            bench.manifest.id, reference.meta.spp, reference.meta.max_depth, reference.meta.commit
        );
        loaded.insert(bench.manifest.id.clone(), reference);
    }

    Ok(loaded)
}

fn measure(bench: &Benchmark, opts: &RunOptions, reference: Option<&Reference>) -> Timing {
    let workload = bench.workload(opts.kind);

    let mut config = bench.camera;
    config.image_width = workload.width;
    config.samples_per_pixel = workload.spp;
    let camera = Camera::new(config);

    let build_start = Instant::now();
    let data = SceneData::from(&bench.scene);
    let scene = Scene {
        world: Arc::new(Bvh::build(data.objects)),
        materials: data.materials,
        background: bench.scene.background.clone(),
    };
    let build_ms = build_start.elapsed().as_secs_f64() * 1000.0;

    let (width, height) = (camera.width, camera.height);

    let framebuffer = Arc::new(FrameBuffer::new(width, height));
    let snapshot_source = Arc::clone(&framebuffer);
    let tracer = Arc::new(PathTracer::new(opts.max_depth));
    let on_tile = |_: &TileResult| {};

    let render_start = Instant::now();
    let stats = render_scene(
        Arc::new(camera),
        tracer,
        framebuffer,
        &on_tile,
        opts.tile_size,
        &scene,
    );

    let wall_ms = render_start.elapsed().as_millis();
    let snapshot = snapshot_source.get_snapshot();

    let comparison = reference.map(|reference| {
        reference::compare(&resolve(&snapshot), reference)
            .expect("la referencia ya fue validada al cargarla")
    });

    Timing {
        comparison,
        build_ms,
        wall_ms,
        rays: stats.rays,
        samples: width as u64 * height as u64 * workload.spp as u64,
        node_visits: stats.traversal.node_visits,
        prim_tests: stats.traversal.prim_tests,
        tile_ms: stats.tile_ms,
        image_hash: env::image_sha(&snapshot),
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

    let environment = env::collect(
        benches,
        dirty,
        opts.max_depth,
        opts.tile_size,
        opts.hardware.clone(),
    )?;

    println!(
        "hardware={}  config={}  reps={}  cooldown={}s  max_depth={}  tile_size={}  label={label}",
        opts.hardware.id,
        opts.kind.as_str(),
        opts.reps,
        opts.cooldown.as_secs(),
        opts.max_depth,
        opts.tile_size,
    );
    if dirty {
        println!("WARNING: dirty working tree, results are not attributable to a commit");
    }

    // Antes del warmup: si una referencia está obsoleta o falta, es mejor
    // enterarse ahora que después de veinte minutos de mediciones.
    let references = load_references(benches, opts)?;
    let reference_of = |bench: &Benchmark| references.get(&bench.manifest.id);

    println!("\n== warmup ({} runs, not recorded) ==", benches.len());
    for bench in benches {
        let timing = measure(bench, opts, None);
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
            let reference = reference_of(bench);
            let timing = measure(bench, opts, reference);
            let workload = bench.workload(opts.kind);

            println!(
                "  [rep {rep}] {} {} ms  {:.2} Mray/s  {:.2} Msmp/s  {:.1} nod/ray  {:.2} prim/ray (build {:.2} ms){}",
                bench.manifest.id,
                timing.wall_ms,
                timing.rays_per_sec() / 1e6,
                timing.samples_per_sec() / 1e6,
                timing.nodes_per_ray(),
                timing.prims_per_ray(),
                timing.build_ms,
                mhz.map(|m| format!("  {m:.0} MHz")).unwrap_or_default(),
            );
            if let Some(comparison) = &timing.comparison {
                println!(
                    "            mse {:.3e}  rel {:.3e}  rmse {:.3e}  efic {:.4}",
                    comparison.mse,
                    comparison.relative_mse,
                    comparison.rmse,
                    rt_renderer::exr_io::efficiency(comparison.mse, timing.secs()),
                );
            }

            records.push(Record {
                benchmark: bench.manifest.id.clone(),
                config: opts.kind.as_str().to_string(),
                width: workload.width,
                height: bench.height(workload.width),
                spp: workload.spp,
                hardware: opts.hardware.id.clone(),
                commit: commit.sha[..12].to_string(),
                commit_label: label.clone(),
                commit_subject: commit.subject.clone(),
                commit_date: commit.date.clone(),
                profile: "optimized",
                rep,
                wall_ms: timing.wall_ms,
                rays: Some(timing.rays),
                rays_per_sec: Some(timing.rays_per_sec()),
                samples: Some(timing.samples),
                samples_per_sec: Some(timing.samples_per_sec()),
                node_visits: Some(timing.node_visits),
                prim_tests: Some(timing.prim_tests),
                image_hash: Some(timing.image_hash.clone()),
                mse: timing.comparison.map(|c| c.mse),
                relative_mse: timing.comparison.map(|c| c.relative_mse),
                efficiency: timing
                    .comparison
                    .map(|c| rt_renderer::exr_io::efficiency(c.mse, timing.secs())),
                reference_spp: reference.map(|r| r.meta.spp),
                reference_max_depth: reference.map(|r| r.meta.max_depth),
                cpu_mhz: mhz,
                timestamp: timestamp.clone(),
                build_ms: Some(timing.build_ms),
                tiles: TileSummary::new(&timing.tile_ms),
                env: environment.clone(),
            });
        }
    }

    check_determinism(benches, &records);
    print_summary(benches, &records, opts);
    print_quality(benches, &records);

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

/// El hash de la imagen debe ser idéntico entre repeticiones. Si no lo es,
/// queda algo no determinista y cualquier medición fina es sospechosa.
/// Solo se imprime si la corrida usó `--reference`. La eficiencia es la métrica
/// que decide: un cambio que baja el tiempo subiendo el ruido no es una mejora,
/// y el reloj por sí solo no lo distingue.
fn print_quality(benches: &[Benchmark], records: &[Record]) {
    if !records.iter().any(|r| r.mse.is_some()) {
        return;
    }

    println!("\n== quality (vs reference) ==");
    println!(
        "  {:<5} {:>12} {:>12} {:>12} {:>10} {:>16}",
        "ID", "mse", "rel_mse", "rmse", "eficiencia", "referencia"
    );
    println!("  {}", "-".repeat(72));

    for bench in benches {
        let id = &bench.manifest.id;
        let rows: Vec<&Record> = records
            .iter()
            .filter(|r| &r.benchmark == id && r.mse.is_some())
            .collect();
        if rows.is_empty() {
            continue;
        }

        let median = |values: Vec<f64>| stats(&values).median;
        let mse = median(rows.iter().filter_map(|r| r.mse).collect());
        let rel = median(rows.iter().filter_map(|r| r.relative_mse).collect());
        let efficiency = median(rows.iter().filter_map(|r| r.efficiency).collect());
        let spp = rows[0].reference_spp.unwrap_or(0);
        let depth = rows[0].reference_max_depth.unwrap_or(0);

        println!(
            "  {:<5} {:>12.4e} {:>12.4e} {:>12.4e} {:>10.4} {:>16}",
            id,
            mse,
            rel,
            mse.sqrt(),
            efficiency,
            format!("{spp} spp / d{depth}"),
        );
    }
}

fn check_determinism(benches: &[Benchmark], records: &[Record]) {
    for bench in benches {
        let hashes: Vec<&str> = records
            .iter()
            .filter(|r| r.benchmark == bench.manifest.id)
            .filter_map(|r| r.image_hash.as_deref())
            .collect();

        let Some(first) = hashes.first() else { continue };
        if hashes.iter().any(|h| h != first) {
            let mut distinct: Vec<&&str> = hashes.iter().collect();
            distinct.sort_unstable();
            distinct.dedup();
            println!(
                "\nWARNING: {} produjo {} imágenes distintas en {} repeticiones \
                 — el render no es determinista",
                bench.manifest.id,
                distinct.len(),
                hashes.len()
            );
        }
    }
}

fn print_summary(benches: &[Benchmark], records: &[Record], opts: &RunOptions) {
    println!("\n== summary ==");
    println!(
        "  {:<5} {:<15} {:>11} {:>5} {:>10} {:>6} {:>3} {:>8} {:>8} {:>7} {:>8} {:>8} {:>6} {:>10}",
        "ID", "name", "resolution", "spp", "render", "rsd", "n", "Mray/s", "Msmp/s", "ray/smp",
        "nod/ray", "prim/ray", "imbal", "image"
    );
    println!("  {}", "-".repeat(129));

    for bench in benches {
        let id = &bench.manifest.id;
        let workload = bench.workload(opts.kind);

        let wall: Vec<f64> = records
            .iter()
            .filter(|r| &r.benchmark == id)
            .map(|r| r.wall_ms as f64)
            .collect();
        if wall.is_empty() {
            continue;
        }

        let Stats { median, rsd_pct, n } = stats(&wall);

        let of = |f: fn(&Record) -> Option<f64>| -> Vec<f64> {
            records
                .iter()
                .filter(|r| &r.benchmark == id)
                .filter_map(f)
                .collect()
        };

        let mray = of(|r| r.rays_per_sec.map(|v| v / 1e6));
        let msmp = of(|r| r.samples_per_sec.map(|v| v / 1e6));
        let per_sample = of(|r| match (r.rays, r.samples) {
            (Some(rays), Some(samples)) if samples > 0 => Some(rays as f64 / samples as f64),
            _ => None,
        });
        let imbalance = of(|r| r.tiles.as_ref().map(|t| t.imbalance));
        let per_ray = |pick: fn(&Record) -> Option<u64>| -> Vec<f64> {
            records
                .iter()
                .filter(|r| &r.benchmark == id)
                .filter_map(|r| match (pick(r), r.rays) {
                    (Some(count), Some(rays)) if rays > 0 => Some(count as f64 / rays as f64),
                    _ => None,
                })
                .collect()
        };
        let nodes_per_ray = per_ray(|r| r.node_visits);
        let prims_per_ray = per_ray(|r| r.prim_tests);

        println!(
            "  {:<5} {:<15} {:>11} {:>5} {:>7.0} ms {:>5.1}% {:>3} {:>8.2} {:>8.2} {:>7.2} {:>8.2} {:>8.2} {:>6.2} {:>10}",
            id,
            bench.manifest.name,
            format!("{}x{}", workload.width, bench.height(workload.width)),
            workload.spp,
            median,
            rsd_pct,
            n,
            stats(&mray).median,
            stats(&msmp).median,
            stats(&per_sample).median,
            stats(&nodes_per_ray).median,
            stats(&prims_per_ray).median,
            stats(&imbalance).median,
            records
                .iter()
                .find(|r| &r.benchmark == id)
                .and_then(|r| r.image_hash.as_deref())
                .map(|h| &h[..8])
                .unwrap_or("-"),
        );
    }
}
