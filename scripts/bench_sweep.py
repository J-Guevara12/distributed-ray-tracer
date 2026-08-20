#!/usr/bin/env python3
"""
Historical performance sweep.

Measures the renderer across several commits WITHOUT modifying their source. For
each commit it creates a worktree, injects the frozen scene files (data, not
code) and the build profile, compiles `standalone` and parses the time it
already prints to stdout.

This driver lives OUTSIDE the measured code and is written in Python because the
`rt-bench` crate does not exist at the older commits, so the sweep cannot depend
on it. `rt-bench` covers HEAD onward; this script reconstructs the past.

Design notes:

  * `standalone`'s timer wraps only `render_scene`, so scene parsing and BVH
    construction fall OUTSIDE the measurement.
  * Runs are INTERLEAVED across commits rather than grouped per commit. On a
    laptop the chip heats up during the sweep; grouped, the later commits would
    look slower for thermal reasons rather than code reasons.
  * Each binary is copied out of its worktree and the worktree destroyed, to
    avoid accumulating tens of GB of `target/` directories.

Usage:
    python3 scripts/bench_sweep.py --config quick
    python3 scripts/bench_sweep.py --config full --reps 3 --cooldown 45
"""


import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import statistics
import sys
import time
import tomllib
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
BENCH_DIR = REPO / "bench"
BIN_DIR = BENCH_DIR / "bin"
STAGE_DIR = BENCH_DIR / "stage"
HISTORY = BENCH_DIR / "history.jsonl"
WORKTREE_ROOT = REPO.parent / ".rt-bench-worktrees"

# Paths `standalone` hardcodes. The driver copies the benchmark files here
# inside each worktree.
STANDALONE_SCENE = "scenes/spheres_scene.json"
STANDALONE_CAMERA = "scenes/spheres_camera.json"

TIME_RE = re.compile(r"Procesado en (\d+) ms")

# Commits that change performance. The intermediate ones that implement the BVH
# without using it yet are skipped: they perform the same as their predecessor.
COMMITS = [
    ("0aa2a51", "pre-bvh"),
    ("1efe2e8", "bvh-enabled"),
    ("da1df30", "release-profile"),
    ("cf36400", "bvh-axis-fix"),
    ("d50313d", "framebuffer-f32"),
    ("e80a41c", "pre-benchmark"),
    ("25618ca", "end-benchmark"),
    ("a386316", "f0.7-longest-axis"),
    ("d77f995", "f0.7-dedup-leaves"),
    ("59cc3df", "f0.7-front-to-back"),
    ("89e51db", "f0.7-sort-bbox-min"),
    ("99b68a4", "f0.4-determinism"),
    ("8f00613", "f0.5-parking-lot"),
    ("4948a99", "f0.6-material-enum"),
    ("f6fa908", "f0.8-flat-bvh"),
    ("03eecf6", "f0.8-aabb-simd"),
    ("c450c47", "f0.8-traversal-stats"),
    ("8d2160e", "f0.9-tile-size-32"),
    ("9d733a8", "f0.9-russian-roulette"),
]

STANDALONE_TILE_SIZE = 128
STANDALONE_MAX_DEPTH = 15

TILE_SIZE_OVERRIDES = {"8d2160e": 32}


def tile_size_for(sha: str) -> int:
    size = STANDALONE_TILE_SIZE
    for commit, _ in COMMITS:
        size = TILE_SIZE_OVERRIDES.get(commit, size)
        if commit == sha:
            break
    return size

# The profile always has to be injected: the older commits carry no
# `[profile.release]` nor `.cargo/config.toml`, so without this each commit would
# compile with different flags and the comparison would be worthless.
#
# Only `optimized` is measured. The 2026-08-13 sweep also measured `default`
# (lto/cu/target-cpu off) and the difference came out ~0% even in the low-variance
# rows; that result is already in history.jsonl. Reproduce it with
# `--profiles default optimized`.
PROFILES = ["optimized"]

OPTIMIZED_PROFILE = """
[profile.release]
lto = "fat"
codegen-units = 1
"""

OPTIMIZED_CARGO_CONFIG = """[build]
rustflags = ["-C", "target-cpu=native"]
"""

# `standalone` used to hardcode the sky gradient and discard the background the
# scene declares. Harmless for B2 (which declares exactly that gradient), fatal
# for a Cornell box: the front is open, so a bright sky enters as a second light
# source and the scene stops being "small light".
#
# Allowed because the line is byte-identical across those commits, it changes a
# value rather than code that runs per ray, and it is applied the SAME way
# everywhere, so it does not bias the comparison. The driver checks there is
# exactly one match before substituting.
BACKGROUND_HARDCODED = (
    "let background = Background::new_gradient("
    "Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));"
)
BACKGROUND_FROM_SCENE = "let background = scene_payload.background.clone();"

STANDALONE_SRC = "crates/rt-renderer/src/bin/standalone.rs"
HARDWARE_FILE = REPO / "bench" / "hardware.toml"


def read_hardware(override: str | None) -> tuple[str, dict]:
    """Same source of truth as `rt-bench`: bench/hardware.toml."""
    if not HARDWARE_FILE.exists():
        sys.exit(f"falta {HARDWARE_FILE}. Es lo que etiqueta la generación de "
                 f"hardware; sin él un cambio de máquina se confunde con un "
                 f"cambio de código.")

    data = tomllib.loads(HARDWARE_FILE.read_text())
    ident = override or data.get("current")
    generations = {k: v for k, v in data.items() if isinstance(v, dict)}

    if ident not in generations:
        sys.exit(f"{HARDWARE_FILE} no define la generación {ident!r}. "
                 f"Definidas: {', '.join(sorted(generations))}")

    return ident, {"id": ident, **generations[ident]}


@dataclass
class Result:
    benchmark: str
    config: str
    # The real workload, explicit. The config *name* is not enough: B2's `quick`
    # was 640/64 before 2026-08-13 and 1920/20 after, so grouping by
    # (benchmark, config) would mix incomparable data.
    width: int
    spp: int
    # Read from bench/hardware.toml, same as rt-bench. Without it a fresh sweep
    # would be indistinguishable from the old ones, which were taken with host
    # power saving on.
    hardware: str
    commit: str
    commit_label: str
    commit_subject: str
    commit_date: str
    profile: str
    rep: int
    wall_ms: int
    # Only `rt-bench` can fill these; they need internal instrumentation. Left
    # explicitly null so the schema stays identical.
    rays: int | None = None
    rays_per_sec: float | None = None
    node_visits: int | None = None
    prim_tests: int | None = None
    image_hash: str | None = None
    mse: float | None = None
    cpu_mhz: float | None = None
    timestamp: str = ""


def run(cmd: list[str], cwd: Path | None = None, check: bool = True) -> str:
    proc = subprocess.run(
        cmd, cwd=cwd, check=False, capture_output=True, text=True
    )
    if check and proc.returncode != 0:
        raise RuntimeError(
            f"command failed: {' '.join(cmd)}\n"
            f"cwd={cwd}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    return proc.stdout


def git(*args: str, cwd: Path | None = None) -> str:
    return run(["git", *args], cwd=cwd or REPO).strip()


def file_sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def read_cpu_model() -> str:
    """`platform.processor()` is usually empty on Linux, and the real model
    matters for attributing measurements across machines."""
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or platform.machine()


def read_cpu_mhz() -> float | None:
    """Best effort: inside a VM cpufreq is usually not exposed."""
    freqs = list(Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_cur_freq"))
    if not freqs:
        return None
    try:
        values = [int(f.read_text()) for f in freqs]
        return round(statistics.mean(values) / 1000.0, 1)
    except OSError:
        return None


def strip_release_profile(text: str) -> str:
    """Drops the [profile.release] section, preserving the rest of the file."""
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    skipping = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("["):
            # Any new header ends whatever section we are skipping.
            skipping = stripped == "[profile.release]"
        if not skipping:
            out.append(line)
    return "".join(out)


def apply_profile(worktree: Path, profile: str) -> None:
    cargo_toml = worktree / "Cargo.toml"
    cargo_config = worktree / ".cargo" / "config.toml"

    # Always start from a Cargo.toml with no profile: the recent commits ship
    # one, and it has to go to measure the `default` profile.
    text = strip_release_profile(cargo_toml.read_text())

    if profile == "optimized":
        cargo_toml.write_text(text.rstrip() + "\n" + OPTIMIZED_PROFILE)
        cargo_config.parent.mkdir(exist_ok=True)
        cargo_config.write_text(OPTIMIZED_CARGO_CONFIG)
    else:
        cargo_toml.write_text(text)
        if cargo_config.exists():
            cargo_config.unlink()


def patch_standalone(worktree: Path) -> bool:
    """
    Makes `standalone` respect the background the scene declares.

    True if it had to patch, False if the commit already did the right thing.
    Raises on anything unexpected, so it cannot silently measure a scene other
    than the one that was asked for.
    """
    src = worktree / STANDALONE_SRC
    text = src.read_text()

    hits = text.count(BACKGROUND_HARDCODED)
    if hits == 1:
        src.write_text(text.replace(BACKGROUND_HARDCODED, BACKGROUND_FROM_SCENE))
        return True
    if hits == 0 and "scene_payload.background" in text:
        return False  # the commit already reads the background from the scene
    raise RuntimeError(
        f"{src}: expected 1 match of the hardcoded background and found {hits}, "
        f"without the file reading `scene_payload.background`. The sweep would "
        f"measure a scene other than the declared one."
    )


def patch_tile_size(worktree: Path, size: int) -> bool:
    """
    Overrides `standalone`'s hardcoded tile size.

    True if it had to patch. Raises when the literal does not appear exactly
    once, so it cannot silently measure a configuration other than the one that
    was asked for.
    """
    if size == STANDALONE_TILE_SIZE:
        return False

    src = worktree / STANDALONE_SRC
    text = src.read_text()
    wanted = f"\n        {size},\n"

    if wanted in text:
        return False  # reused worktree, already patched

    literal = f"\n        {STANDALONE_TILE_SIZE},\n"
    hits = text.count(literal)
    if hits != 1:
        raise RuntimeError(
            f"{src}: expected 1 match of `{STANDALONE_TILE_SIZE},` as the tile size "
            f"argument and found {hits}. The sweep would measure a configuration "
            f"other than the declared one."
        )

    src.write_text(text.replace(literal, wanted))
    return True


def load_benchmarks(only: str | None) -> list[dict]:
    benchmarks = []
    for manifest in sorted((REPO / "scenes" / "bench").glob("*/bench.toml")):
        data = tomllib.loads(manifest.read_text())
        folder = manifest.parent
        data["_folder"] = folder
        data["_scene"] = folder / "scene.json"
        data["_camera"] = folder / "camera.json"
        if not data["_scene"].exists() or not data["_camera"].exists():
            print(f"  ! {folder.name}: falta scene.json o camera.json, se omite")
            continue
        if only and data["id"] != only and data["name"] != only:
            continue
        benchmarks.append(data)
    return benchmarks


def make_stage(bench: dict, config: str) -> Path:
    """
    Materialises a working directory with the scene files `standalone` expects
    at its hardcoded paths.

    `camera.json` is generated by patching width and spp from bench.toml, so the
    values committed in camera.json never define the workload.
    """
    cfg = bench[config]
    stage = STAGE_DIR / f"{bench['name']}-{config}"
    (stage / "scenes").mkdir(parents=True, exist_ok=True)

    shutil.copyfile(bench["_scene"], stage / STANDALONE_SCENE)

    camera = json.loads(bench["_camera"].read_text())
    camera["image_width"] = cfg["width"]
    camera["samples_per_pixel"] = cfg["spp"]
    (stage / STANDALONE_CAMERA).write_text(json.dumps(camera, indent=2))

    return stage


def build_all(commits, profiles, keep_worktrees: bool) -> dict[tuple[str, str], Path]:
    """Builds one binary per (commit, profile) and copies it out of the worktree."""
    BIN_DIR.mkdir(parents=True, exist_ok=True)
    WORKTREE_ROOT.mkdir(exist_ok=True)
    binaries: dict[tuple[str, str], Path] = {}

    for sha, label in commits:
        worktree = WORKTREE_ROOT / sha
        if worktree.exists():
            print(f"  worktree {sha} ya existe, se reutiliza")
        else:
            print(f"  creando worktree {sha} ({label})")
            git("worktree", "add", "--detach", str(worktree), sha)

        patched = patch_standalone(worktree)
        print(f"    background: {'parchado' if patched else 'ya correcto'}")

        tile_size = tile_size_for(sha)
        patch_tile_size(worktree, tile_size)
        print(f"    tile_size: {tile_size}")

        for profile in profiles:
            dest = BIN_DIR / f"{label}-{profile}"
            if dest.exists():
                print(f"  ✓ {label}/{profile} ya compilado")
                binaries[(sha, profile)] = dest
                continue

            print(f"  compilando {label}/{profile} …", end="", flush=True)
            apply_profile(worktree, profile)
            started = time.monotonic()
            run(["cargo", "build", "--release", "--bin", "standalone"], cwd=worktree)
            shutil.copyfile(worktree / "target" / "release" / "standalone", dest)
            dest.chmod(0o755)
            print(f" {time.monotonic() - started:.0f}s")
            binaries[(sha, profile)] = dest

        if not keep_worktrees:
            # The target/ of an lto=fat build weighs GB, and the binary is
            # already safe, so the whole worktree can go.
            shutil.rmtree(worktree / "target", ignore_errors=True)
            git("worktree", "remove", "--force", str(worktree))

    return binaries


def measure(binary: Path, stage: Path) -> int:
    stdout = run([str(binary)], cwd=stage)
    match = TIME_RE.search(stdout)
    if not match:
        raise RuntimeError(
            f"no timing found in the output of {binary.name}:\n{stdout}"
        )
    return int(match.group(1))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", default="quick", choices=["quick", "full"])
    parser.add_argument("--hardware", default=None,
                        help="sobreescribe `current` de bench/hardware.toml")
    parser.add_argument("--reps", type=int, default=None,
                        help="repeticiones grabadas (default: 5 quick / 3 full)")
    parser.add_argument("--cooldown", type=float, default=None,
                        help="segundos entre corridas (default: 20 quick / 45 full)")
    parser.add_argument("--only", help="medir solo este benchmark (id o nombre)")
    parser.add_argument("--profiles", nargs="+", default=PROFILES)
    parser.add_argument("--keep-worktrees", action="store_true",
                        help="no destruir los worktrees (útil para depurar)")
    parser.add_argument("--allow-dirty", action="store_true",
                        help="permitir barrida con el árbol sucio (no recomendado)")
    args = parser.parse_args()

    hardware_id, hardware_meta = read_hardware(args.hardware)
    reps = args.reps or (5 if args.config == "quick" else 3)
    cooldown = args.cooldown if args.cooldown is not None else (
        20.0 if args.config == "quick" else 45.0
    )

    dirty = bool(git("status", "--porcelain"))
    if dirty and not args.allow_dirty:
        print("El árbol de trabajo está sucio. Una medición desde código sin")
        print("commitear no es atribuible. Commitea o usa --allow-dirty.")
        return 1

    benchmarks = load_benchmarks(args.only)
    if not benchmarks:
        print("No se encontró ningún benchmark en scenes/bench/*/bench.toml")
        return 1

    print(f"Benchmarks: {', '.join(b['id'] + '/' + b['name'] for b in benchmarks)}")
    print(f"Commits:    {len(COMMITS)}   Perfiles: {', '.join(args.profiles)}")
    print(f"Hardware:   {hardware_id}  ({hardware_meta['description']})")
    print(f"Config:     {args.config}   reps={reps}  cooldown={cooldown}s")
    print()

    print("== Fase 1: compilación ==")
    binaries = build_all(COMMITS, args.profiles, args.keep_worktrees)

    print("\n== Fase 2: staging de escenas ==")
    stages = {b["name"]: make_stage(b, args.config) for b in benchmarks}
    for name, stage in stages.items():
        print(f"  {name} → {stage.relative_to(REPO)}")

    commit_info = {
        sha: git("show", "-s", "--format=%H%x1f%s%x1f%cI", sha).split("\x1f")
        for sha, _ in COMMITS
    }

    # One unit of work is a full combination. Repetitions loop on the outside so
    # the runs interleave and thermal drift decorrelates from commit order.
    units = [
        (bench, sha, label, profile)
        for bench in benchmarks
        for sha, label in COMMITS
        for profile in args.profiles
        if (sha, profile) in binaries
    ]

    print(f"\n== Fase 3: warmup ({len(units)} corridas, no se graban) ==")
    for bench, sha, label, profile in units:
        ms = measure(binaries[(sha, profile)], stages[bench["name"]])
        print(f"  {label}/{profile} {bench['id']}: {ms} ms")

    print(f"\n== Fase 4: medición ({reps} reps × {len(units)} corridas) ==")
    results: list[Result] = []
    for rep in range(1, reps + 1):
        for bench, sha, label, profile in units:
            time.sleep(cooldown)
            mhz = read_cpu_mhz()
            ms = measure(binaries[(sha, profile)], stages[bench["name"]])
            full_sha, subject, date = commit_info[sha]
            results.append(Result(
                benchmark=bench["id"],
                config=args.config,
                width=bench[args.config]["width"],
                spp=bench[args.config]["spp"],
                hardware=hardware_id,
                commit=full_sha[:12],
                commit_label=label,
                commit_subject=subject,
                commit_date=date,
                profile=profile,
                rep=rep,
                wall_ms=ms,
                cpu_mhz=mhz,
                timestamp=datetime.now(timezone.utc).isoformat(),
            ))
            freq = f"  {mhz:.0f} MHz" if mhz else ""
            print(f"  [rep {rep}] {label}/{profile} {bench['id']}: {ms} ms{freq}")

    print("\n== Resumen (mediana, min–max) ==")
    env = {
        "rustc": run(["rustc", "--version"]).strip(),
        "cpu": read_cpu_model(),
        "cpu_threads": os.cpu_count(),
        "platform": platform.platform(),
        "scene_hashes": {
            b["name"]: {
                "scene": file_sha(b["_scene"]),
                "camera": file_sha(b["_camera"]),
                "manifest": file_sha(b["_folder"] / "bench.toml"),
            }
            for b in benchmarks
        },
        "dirty": dirty,
        "driver": Path(__file__).name,
        # The sweep's code injections, recorded so the results explain
        # themselves.
        "standalone_background_patch": True,
        "max_depth": STANDALONE_MAX_DEPTH,
        "hardware": hardware_meta,
    }

    # `tile_size` cannot live in the shared env: the sweep varies it per commit,
    # so it is resolved per record at write time.
    tile_size_by_label = {label: tile_size_for(sha) for sha, label in COMMITS}

    for bench in benchmarks:
        cfg = bench[args.config]
        print(f"\n  {bench['id']} ({bench['name']}, {args.config}: "
              f"{cfg['width']}px / {cfg['spp']} spp)")
        print(f"    {'commit':<18}{'mediana':>10}{'spread':>9}{'vs anterior':>13}")
        print(f"    {'-' * 48}")

        previous = None
        for _, label in COMMITS:
            samples = [
                r.wall_ms for r in results
                if r.commit_label == label and r.benchmark == bench["id"]
            ]
            if not samples:
                print(f"    {label:<18}{'—':>10}")
                continue

            median = statistics.median(samples)
            # Relative spread: above ~10% the data cannot resolve small
            # differences and it is worth raising resolution or repetitions.
            spread = (max(samples) - min(samples)) / median * 100
            flag = " !" if spread > 10 else ""
            change = f"×{previous / median:.2f}" if previous else ""
            print(f"    {label:<18}{median:>7.0f} ms{spread:>8.1f}%{change:>13}{flag}")
            previous = median

        totals = [
            r.wall_ms for r in results
            if r.commit_label == COMMITS[0][1] and r.benchmark == bench["id"]
        ]
        if totals and previous:
            print(f"    {'total':<18}{'':>19}{'×' + f'{statistics.median(totals) / previous:.2f}':>13}")

    BENCH_DIR.mkdir(exist_ok=True)
    with HISTORY.open("a") as fh:
        for r in results:
            record_env = {**env, "tile_size": tile_size_by_label[r.commit_label]}
            fh.write(json.dumps({**asdict(r), "env": record_env}) + "\n")
    print(f"\n{len(results)} resultados añadidos a {HISTORY.relative_to(REPO)}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
