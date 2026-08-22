//! Machine ceilings: peak FLOP/s and the memory bandwidth hierarchy.
//!
//! These are properties of the machine, not of the project, so they are stored
//! per hardware generation. A roofline plotted against another generation's
//! ceilings is meaningless.
//!
//! Both are measured rather than looked up, for two reasons. Vendor figures do
//! not describe a throttled VM, and this CPU is hybrid: 8 P-cores and 12
//! E-cores with different FMA throughput, so `threads * clock * lanes * 2` does
//! not apply. Measuring handles that on its own, and keeps the VM and a
//! dedicated server directly comparable.
//!
//! Everything here is analytic-count friendly on purpose: no PMU is used, so
//! the same method works where hardware counters are not exposed.
//!
//! Second use: this doubles as a machine calibration probe. Running it before a
//! sweep says whether the machine is in the state you think, which is what the
//! `cpu_mhz` field failed to do when the host had power saving enabled.

use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, bail};
use serde::Serialize;

use crate::env;
use crate::hardware::Hardware;

/// Minimum time per data point. Long enough that the clock and the thread pool
/// spin-up do not dominate, short enough that the whole sweep stays quick.
const MIN_SAMPLE: Duration = Duration::from_millis(250);

/// Independent accumulators in the FMA loop. An FMA has ~4 cycles of latency
/// and 2/cycle of throughput, so fewer chains than this measures latency
/// instead of peak.
const CHAINS: usize = 16;

/// Per-thread working sets, from well inside L1 to well past L3. Deliberately
/// not named after cache levels: the knees in the curve are the boundaries, and
/// they differ per machine — which is the point of measuring instead of
/// hardcoding.
const WORKING_SETS_KIB: &[usize] = &[4, 16, 64, 256, 1024, 4096, 16384, 65536];

pub struct CeilingOptions {
    pub out_dir: PathBuf,
    pub threads: Vec<usize>,
    pub hardware: Hardware,
}

#[derive(Serialize, Clone)]
pub struct ComputePoint {
    pub threads: usize,
    pub gflops_128: f64,
    pub gflops_256: Option<f64>,
}

#[derive(Serialize, Clone)]
pub struct BandwidthPoint {
    pub threads: usize,
    pub bytes_per_thread: usize,
    pub gib_per_sec: f64,
}

#[derive(Serialize, Clone)]
pub struct Ceilings {
    pub hardware: String,
    pub description: String,
    pub cpu: String,
    pub cpu_threads: usize,
    pub commit: String,
    pub dirty: bool,
    /// Which FMA width the 256-bit numbers came from, or why they are absent.
    pub simd: String,
    pub compute: Vec<ComputePoint>,
    pub bandwidth: Vec<BandwidthPoint>,
    pub timestamp: String,
}

/// Repeats `work` until `MIN_SAMPLE` elapses and returns operations per second.
/// `work` must return something so the optimiser cannot delete the loop.
fn rate<T>(ops_per_call: f64, mut work: impl FnMut() -> T) -> f64 {
    let mut calls = 0u64;
    let started = Instant::now();

    loop {
        black_box(work());
        calls += 1;
        if started.elapsed() >= MIN_SAMPLE {
            break;
        }
    }

    let seconds = started.elapsed().as_secs_f64();
    ops_per_call * calls as f64 / seconds
}

/// 128-bit FMA chain. `glam::Vec4` is what the renderer itself uses, so this is
/// the ceiling the current code could actually reach.
fn fma_128(iterations: usize) -> f32 {
    use rt_core::Vec4;

    let mut acc = [Vec4::splat(1.000_001); CHAINS];
    let multiplier = Vec4::splat(1.000_000_1);
    let addend = Vec4::splat(0.000_000_1);

    for _ in 0..iterations {
        for slot in acc.iter_mut() {
            *slot = *slot * multiplier + addend;
        }
    }

    acc.iter().map(|v| v.x + v.y + v.z + v.w).sum()
}

/// 256-bit FMA chain, the machine's actual peak. Separate from `fma_128`
/// because the gap between them is informative: the renderer never issues
/// 256-bit vectors, so part of the distance to peak is unreachable by design.
///
/// # Safety
/// The caller must have checked `avx` and `fma` with `is_x86_feature_detected`.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx", enable = "fma")]
unsafe fn fma_256(iterations: usize) -> f32 {
    use std::arch::x86_64::*;

    {
        let mut acc = [_mm256_set1_ps(1.000_001); CHAINS];
        let multiplier = _mm256_set1_ps(1.000_000_1);
        let addend = _mm256_set1_ps(0.000_000_1);

        for _ in 0..iterations {
            for slot in acc.iter_mut() {
                *slot = _mm256_fmadd_ps(*slot, multiplier, addend);
            }
        }

        let mut out = [0.0f32; 8];
        let total = acc
            .iter()
            .fold(_mm256_setzero_ps(), |sum, v| _mm256_add_ps(sum, *v));
        // SAFETY: `out` is 8 f32 wide, exactly what the store writes.
        unsafe { _mm256_storeu_ps(out.as_mut_ptr(), total) };
        out.iter().sum()
    }
}

fn pool(threads: usize) -> anyhow::Result<rayon::ThreadPool> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .context("building the rayon pool")
}

/// FLOPs per FMA: one multiply and one add, times the lane count.
fn compute_at(threads: usize, avx: bool) -> anyhow::Result<ComputePoint> {
    // Sized so one call takes milliseconds. With a few thousand iterations the
    // rayon dispatch dominated and the curve collapsed past 8 threads.
    const ITERATIONS: usize = 1_000_000;
    let pool = pool(threads)?;

    let measure = |lanes: usize, kernel: fn(usize) -> f32| {
        let flops_per_call = (ITERATIONS * CHAINS * lanes * 2 * threads) as f64;
        // `broadcast` runs the closure exactly once per worker; `par_iter` over
        // N items can hand two to one worker and leave another idle.
        pool.install(|| {
            rate(flops_per_call, || {
                pool.broadcast(|_| kernel(ITERATIONS))
                    .into_iter()
                    .sum::<f32>()
            })
        }) / 1e9
    };

    Ok(ComputePoint {
        threads,
        gflops_128: measure(4, fma_128),
        #[cfg(target_arch = "x86_64")]
        // SAFETY: `avx` is the result of `is_x86_feature_detected` for avx+fma.
        gflops_256: avx.then(|| measure(8, |n| unsafe { fma_256(n) })),
        #[cfg(not(target_arch = "x86_64"))]
        gflops_256: None,
    })
}

/// Read bandwidth. The renderer's traffic is read-dominated — nodes and
/// primitives in, one framebuffer write per tile — so a read kernel is the
/// honest match. Each thread walks its own buffer, so the aggregate working set
/// is `threads * bytes_per_thread`: that is what makes the shared-cache knee
/// appear where it does.
fn bandwidth_at(threads: usize, kib_per_thread: usize) -> anyhow::Result<BandwidthPoint> {
    let bytes = kib_per_thread * 1024;
    let floats = bytes / std::mem::size_of::<f32>();
    let pool = pool(threads)?;

    let buffers: Vec<Vec<f32>> = (0..threads)
        .map(|t| (0..floats).map(|i| (i + t) as f32 * 1.000_001).collect())
        .collect();

    // Every point moves roughly the same total bytes per call, so the dispatch
    // overhead is amortised equally across working-set sizes. Without this the
    // curve came out monotonically increasing — small buffers were measuring
    // rayon, not the cache.
    const TARGET_PER_CALL: usize = 256 * 1024 * 1024;
    let passes = (TARGET_PER_CALL / bytes).max(1);

    let bytes_per_call = (bytes * passes * threads) as f64;
    let gib_per_sec = pool.install(|| {
        rate(bytes_per_call, || {
            pool.broadcast(|ctx| {
                let buffer = &buffers[ctx.index()];
                // Four accumulators so the adds are not the bottleneck.
                let mut acc = [0.0f32; 4];
                for _ in 0..passes {
                    for chunk in buffer.chunks_exact(4) {
                        for (slot, value) in acc.iter_mut().zip(chunk) {
                            *slot += *value;
                        }
                    }
                }
                acc.iter().sum::<f32>()
            })
            .into_iter()
            .sum::<f32>()
        })
    }) / (1024.0 * 1024.0 * 1024.0);

    Ok(BandwidthPoint {
        threads,
        bytes_per_thread: bytes,
        gib_per_sec,
    })
}

pub fn measure(opts: &CeilingOptions) -> anyhow::Result<()> {
    if cfg!(debug_assertions) {
        bail!("ceilings must be measured in release; a debug build measures nothing useful");
    }

    let available = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let mut threads: Vec<usize> = if opts.threads.is_empty() {
        let mut scan = vec![1usize];
        while *scan.last().unwrap() * 2 < available {
            scan.push(scan.last().unwrap() * 2);
        }
        scan.push(available);
        scan
    } else {
        opts.threads.clone()
    };
    threads.sort_unstable();
    threads.dedup();

    #[cfg(target_arch = "x86_64")]
    let avx = is_x86_feature_detected!("avx") && is_x86_feature_detected!("fma");
    #[cfg(not(target_arch = "x86_64"))]
    let avx = false;

    let simd = if avx {
        "128-bit (glam Vec4) and 256-bit (AVX2 FMA)".to_string()
    } else {
        "128-bit only; no AVX+FMA detected".to_string()
    };

    println!("ceilings for {} — {}", opts.hardware.id, simd);
    println!("  {} threads available\n", available);

    println!("  == peak compute ==");
    println!("  {:>8} {:>14} {:>14}", "threads", "GFLOP/s 128", "GFLOP/s 256");
    let mut compute = Vec::new();
    for &count in &threads {
        let point = compute_at(count, avx)?;
        println!(
            "  {:>8} {:>14.1} {:>14}",
            point.threads,
            point.gflops_128,
            point
                .gflops_256
                .map(|v| format!("{v:.1}"))
                .unwrap_or_else(|| "-".into())
        );
        compute.push(point);
    }

    println!("\n  == read bandwidth, {available} threads ==");
    println!("  {:>14} {:>10} {:>14}", "KiB/thread", "total MiB", "GiB/s");
    let mut bandwidth = Vec::new();
    for &kib in WORKING_SETS_KIB {
        let point = bandwidth_at(available, kib)?;
        println!(
            "  {:>14} {:>10.1} {:>14.1}",
            kib,
            (point.bytes_per_thread * available) as f64 / (1024.0 * 1024.0),
            point.gib_per_sec
        );
        bandwidth.push(point);
    }
    // One thread as well: the gap against the full-thread curve is where the
    // shared levels saturate.
    for &kib in WORKING_SETS_KIB {
        bandwidth.push(bandwidth_at(1, kib)?);
    }

    let commit = env::head_commit()?;
    let result = Ceilings {
        hardware: opts.hardware.id.clone(),
        description: opts.hardware.generation.description.clone(),
        cpu: env::cpu_model(),
        cpu_threads: available,
        commit: commit.sha[..12].to_string(),
        dirty: env::is_dirty()?,
        simd,
        compute,
        bandwidth,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    std::fs::create_dir_all(&opts.out_dir)
        .with_context(|| format!("creating {}", opts.out_dir.display()))?;
    let path = opts.out_dir.join(format!("{}.json", opts.hardware.id));
    std::fs::write(&path, serde_json::to_string_pretty(&result)?)
        .with_context(|| format!("writing {}", path.display()))?;

    println!("\n{}", path.display());
    Ok(())
}
