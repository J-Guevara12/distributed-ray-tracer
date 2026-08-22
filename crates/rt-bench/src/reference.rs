//! Imágenes de referencia y error medido contra ellas.
//!
//! El tiempo de pared solo no alcanza desde F0.9: la ruleta rusa y cualquier
//! cambio de muestreo no hacen el mismo trabajo más rápido, hacen menos trabajo
//! y producen más ruido. Para saber si eso es una mejora hace falta comparar
//! contra una imagen convergida.
//!
//! Dos cosas que definen si una referencia sirve:
//!
//!   * **`max_depth` alto.** Con profundidad fija el integrador es sesgado:
//!     trunca la serie y pierde energía. La ruleta rusa es insesgada, así que
//!     converge a otra imagen. Una referencia a profundidad 15 haría medir el
//!     sesgo como si fuera error.
//!   * **spp muy por encima del que se mide.** Los ruidos son independientes, o
//!     sea que el de la referencia se suma y queda como piso del MSE. La
//!     varianza va como `1/spp`, así que 100× de muestras deja el piso en 1%.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};

use rt_core::Vec3;
use rt_core::display::resolve;
use rt_renderer::camera::Camera;
use rt_renderer::exr_io::{self, Comparison};
use rt_renderer::framebuffer::FrameBuffer;
use rt_renderer::render::render_scene;
use rt_renderer::tiles::TileResult;
use rt_renderer::integrators::PathTracer;
use rt_scene::hittable_list::SceneData;
use rt_scene::{Scene, bvh::Bvh};

use crate::env;
use crate::manifest::{Benchmark, WorkloadKind};

/// Por debajo de esto el ruido de la referencia deja de ser despreciable.
const MIN_SPP_RATIO: u32 = 50;

pub struct ReferenceOptions {
    pub kind: WorkloadKind,
    pub spp: u32,
    pub max_depth: u32,
    pub tile_size: u32,
    pub out_dir: PathBuf,
    pub allow_dirty: bool,
}

/// Acompaña al EXR. Sin esto no hay forma de saber si una referencia quedó
/// obsoleta, y comparar contra una obsoleta da un número que parece válido.
#[derive(Serialize, Deserialize, Clone)]
pub struct ReferenceMeta {
    pub benchmark: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub spp: u32,
    pub max_depth: u32,
    pub tile_size: u32,
    pub commit: String,
    pub scene_hash: String,
    pub camera_hash: String,
    pub render_seconds: f64,
    pub timestamp: String,
}

pub struct Reference {
    pub pixels: Vec<Vec3>,
    pub meta: ReferenceMeta,
}

fn exr_path(dir: &Path, id: &str, width: u32) -> PathBuf {
    // El ancho va en el nombre: `quick` y `full` pueden tener resoluciones
    // distintas y las dos referencias tienen que poder coexistir.
    dir.join(format!("{id}-{width}.exr"))
}

fn meta_path(dir: &Path, id: &str, width: u32) -> PathBuf {
    dir.join(format!("{id}-{width}.json"))
}

fn build_scene(bench: &Benchmark) -> Scene {
    let data = SceneData::from(&bench.scene);
    Scene {
        world: Arc::new(Bvh::build(data.objects)),
        materials: data.materials,
        background: bench.scene.background.clone(),
    }
}

/// Renderiza y guarda las referencias de los benchmarks seleccionados.
pub fn generate(benches: &[Benchmark], opts: &ReferenceOptions) -> anyhow::Result<()> {
    if cfg!(debug_assertions) {
        bail!("rt-bench debe compilarse en release; un build debug tardaría horas de más");
    }

    let dirty = env::is_dirty()?;
    if dirty && !opts.allow_dirty {
        bail!("el árbol tiene cambios sin commitear; usá --allow-dirty si es a propósito");
    }

    let commit = env::head_commit()?;
    std::fs::create_dir_all(&opts.out_dir)
        .with_context(|| format!("creando {}", opts.out_dir.display()))?;

    println!(
        "Referencias: {} spp, max_depth {}, tile_size {}  →  {}",
        opts.spp,
        opts.max_depth,
        opts.tile_size,
        opts.out_dir.display()
    );

    for bench in benches {
        let workload = bench.workload(opts.kind);
        let ratio = opts.spp / workload.spp.max(1);
        if ratio < MIN_SPP_RATIO {
            println!(
                "  AVISO {}: {} spp es solo {}× el spp medido ({}). El ruido de la \
                 referencia va a ser {}% del MSE en vez de despreciable.",
                bench.manifest.id,
                opts.spp,
                ratio,
                workload.spp,
                100 / ratio.max(1)
            );
        }
    }

    for bench in benches {
        let workload = bench.workload(opts.kind);
        let width = workload.width;

        let mut config = bench.camera;
        config.image_width = width;
        config.samples_per_pixel = opts.spp;
        let camera = Camera::new(config);
        let (width, height) = (camera.width, camera.height);

        let tiles_across = width.div_ceil(opts.tile_size);
        let tiles_down = height.div_ceil(opts.tile_size);
        let total_tiles = (tiles_across * tiles_down) as usize;

        println!(
            "\n  {} ({}) {width}x{height}, {} tiles",
            bench.manifest.id, bench.manifest.name, total_tiles
        );

        let scene = build_scene(bench);
        let framebuffer = Arc::new(FrameBuffer::new(width, height));
        let snapshot_source = Arc::clone(&framebuffer);

        // Con `\r` la barra queda ilegible al redirigir la salida a un archivo.
        let interactive = std::io::stdout().is_terminal();
        let done = AtomicUsize::new(0);
        let started = Instant::now();
        let on_tile = |_: &TileResult| {
            let n = done.fetch_add(1, Ordering::Relaxed) + 1;
            // Un tile de cada 5% — con miles de tiles, imprimir cada uno sería
            // más costoso que renderizar.
            let step = (total_tiles / 20).max(1);
            if n.is_multiple_of(step) || n == total_tiles {
                let elapsed = started.elapsed().as_secs_f64();
                let eta = elapsed / n as f64 * (total_tiles - n) as f64;
                let line = format!(
                    "    {:>3}%  {:>5.0}s transcurridos, ~{:>5.0}s restantes",
                    100 * n / total_tiles,
                    elapsed,
                    eta
                );
                if interactive {
                    use std::io::Write;
                    print!("\r{line}");
                    let _ = std::io::stdout().flush();
                } else {
                    println!("{line}");
                }
            }
        };

        render_scene(
            Arc::new(camera),
            Arc::new(PathTracer::new(opts.max_depth)),
            framebuffer,
            &on_tile,
            opts.tile_size,
            &scene,
        );
        let render_seconds = started.elapsed().as_secs_f64();
        if interactive {
            println!();
        }

        let snapshot = snapshot_source.get_snapshot();
        let pixels = resolve(&snapshot);

        let exr = exr_path(&opts.out_dir, &bench.manifest.id, width);
        exr_io::write(&exr, &pixels, width, height)
            .with_context(|| format!("escribiendo {}", exr.display()))?;

        let meta = ReferenceMeta {
            benchmark: bench.manifest.id.clone(),
            name: bench.manifest.name.clone(),
            width,
            height,
            spp: opts.spp,
            max_depth: opts.max_depth,
            tile_size: opts.tile_size,
            commit: commit.sha[..12].to_string(),
            scene_hash: env::file_sha(&bench.scene_path())?,
            camera_hash: env::file_sha(&bench.camera_path())?,
            render_seconds,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        let json = meta_path(&opts.out_dir, &bench.manifest.id, width);
        std::fs::write(&json, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("escribiendo {}", json.display()))?;

        println!(
            "    {} ({:.0}s, {:.1} MB)",
            exr.display(),
            render_seconds,
            std::fs::metadata(&exr)
                .map(|m| m.len() as f64 / 1e6)
                .unwrap_or(0.0)
        );
    }

    Ok(())
}

/// Carga la referencia de un benchmark y verifica que siga siendo válida.
///
/// Falla en vez de avisar: comparar contra una referencia obsoleta produce un
/// MSE con la forma correcta y el valor equivocado, que es peor que no tenerlo.
pub fn load(
    dir: &Path,
    bench: &Benchmark,
    width: u32,
    height: u32,
    run_max_depth: u32,
    run_spp: u32,
) -> anyhow::Result<Reference> {
    let id = &bench.manifest.id;
    let json = meta_path(dir, id, width);
    let exr = exr_path(dir, id, width);

    if !exr.exists() {
        bail!(
            "no hay referencia para {id} a {width} px en {}. Generala con:\n  \
             rt-bench reference --only {id}",
            dir.display()
        );
    }

    let meta: ReferenceMeta = serde_json::from_str(
        &std::fs::read_to_string(&json).with_context(|| format!("leyendo {}", json.display()))?,
    )
    .with_context(|| format!("parseando {}", json.display()))?;

    let scene_hash = env::file_sha(&bench.scene_path())?;
    let camera_hash = env::file_sha(&bench.camera_path())?;

    if meta.scene_hash != scene_hash {
        bail!(
            "la referencia de {id} es obsoleta: la escena cambió desde que se generó \
             (esperaba {}, es {scene_hash}). Regenerala.",
            meta.scene_hash
        );
    }
    if meta.camera_hash != camera_hash {
        bail!(
            "la referencia de {id} es obsoleta: la cámara cambió desde que se generó \
             (esperaba {}, es {camera_hash}). Regenerala.",
            meta.camera_hash
        );
    }

    let image = exr_io::read(&exr).with_context(|| format!("leyendo {}", exr.display()))?;
    if image.dimensions() != (width, height) {
        bail!(
            "la referencia de {id} es de {}x{} y la corrida es de {width}x{height}",
            image.width,
            image.height
        );
    }

    if meta.max_depth < run_max_depth {
        println!(
            "  AVISO {id}: la referencia se generó con max_depth {} y la corrida usa {run_max_depth}. \
             El MSE va a incluir el sesgo de truncamiento de la referencia.",
            meta.max_depth
        );
    }
    let ratio = meta.spp / run_spp.max(1);
    if ratio < MIN_SPP_RATIO {
        println!(
            "  AVISO {id}: la referencia tiene {} spp, solo {ratio}× los {run_spp} de la corrida. \
             El piso de ruido es ~{}% del MSE.",
            meta.spp,
            100 / ratio.max(1)
        );
    }

    Ok(Reference {
        pixels: image.pixels,
        meta,
    })
}

/// Error de un render contra su referencia. Los dos en radiancia lineal.
pub fn compare(render: &[Vec3], reference: &Reference) -> anyhow::Result<Comparison> {
    exr_io::compare(render, &reference.pixels).with_context(|| {
        format!(
            "comparando contra la referencia de {}",
            reference.meta.benchmark
        )
    })
}
