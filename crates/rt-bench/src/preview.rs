//! Low-resolution previews of the benchmark scenes.
//!
//! Exists to answer "is the scene I am about to measure the scene I think it
//! is?" before spending machine time. A reference render costs an hour and a
//! historical sweep three; a 400px preview costs 10 ms.
//!
//! Follows the convention of the plotting scripts: only the file paths go to
//! stdout, so `rt-bench preview | xargs kitten icat` works. Notes go to stderr.
//!
//! Provenance travels inside the PNG as tEXt chunks, because a preview that has
//! been moved or shared is worthless if you cannot tell which commit produced
//! it or whether the tree was dirty.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, bail};

use rt_core::display::DisplayParams;
use rt_renderer::camera::Camera;
use rt_renderer::framebuffer::FrameBuffer;
use rt_renderer::render::render_scene;
use rt_renderer::tiles::TileResult;
use rt_scene::hittable_list::SceneData;
use rt_scene::{Scene, bvh::Bvh};

use crate::env;
use crate::manifest::{Benchmark, Tracer, WorkloadKind};

pub struct PreviewOptions {
    pub kind: WorkloadKind,
    pub tracer: Tracer,
    pub width: u32,
    pub spp: u32,
    pub max_depth: u32,
    pub tile_size: u32,
    pub out_dir: PathBuf,
}

pub fn generate(benches: &[Benchmark], opts: &PreviewOptions) -> anyhow::Result<()> {
    if benches.is_empty() {
        bail!("no benchmarks selected");
    }

    let commit = env::head_commit()?;
    let dirty = env::is_dirty()?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();

    let dir = opts.out_dir.join(&stamp);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    eprintln!(
        "preview: {} px, {} spp, tracer {}, commit {}{}",
        opts.width,
        opts.spp,
        opts.tracer.as_str(),
        &commit.sha[..12],
        if dirty { " (dirty)" } else { "" }
    );

    for bench in benches {
        let mut config = bench.camera;
        config.image_width = opts.width;
        config.samples_per_pixel = opts.spp;
        let camera = Camera::new(config);
        let (width, height) = (camera.width, camera.height);

        let data = SceneData::from(&bench.scene);
        let scene = Scene {
            world: Arc::new(Bvh::build(data.objects)),
            materials: data.materials,
            background: bench.scene.background.clone(),
        };

        let framebuffer = Arc::new(FrameBuffer::new(width, height));
        let output = Arc::clone(&framebuffer);

        let started = std::time::Instant::now();
        render_scene(
            Arc::new(camera),
            Arc::new(opts.tracer.build(opts.max_depth)),
            framebuffer,
            &|_: &TileResult| {},
            opts.tile_size,
            &scene,
        );
        let elapsed = started.elapsed().as_secs_f64() * 1000.0;

        let metadata = vec![
            ("Benchmark", bench.manifest.id.clone()),
            ("Scene", bench.manifest.name.clone()),
            ("Commit", commit.sha.clone()),
            ("CommitSubject", commit.subject.clone()),
            ("Dirty", dirty.to_string()),
            ("Tracer", opts.tracer.as_str().to_string()),
            ("Workload", opts.kind.as_str().to_string()),
            ("Resolution", format!("{width}x{height}")),
            ("Spp", opts.spp.to_string()),
            ("MaxDepth", opts.max_depth.to_string()),
            ("TileSize", opts.tile_size.to_string()),
            ("SceneHash", env::file_sha(&bench.scene_path())?),
            ("CameraHash", env::file_sha(&bench.camera_path())?),
            ("RenderMs", format!("{elapsed:.1}")),
            ("Rendered", chrono::Local::now().to_rfc3339()),
        ];

        let path = dir.join(format!("{}.png", bench.manifest.id));
        output
            .save_png_annotated(&path, &DisplayParams::default(), &metadata)
            .with_context(|| format!("writing {}", path.display()))?;

        eprintln!("  {} {width}x{height}  {elapsed:.0} ms", bench.manifest.id);
        println!("{}", path.display());
    }

    Ok(())
}
