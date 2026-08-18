#!/usr/bin/env python3
"""
Barrida histórica de rendimiento.

Mide el renderer en varios commits SIN modificar su código fuente. Para cada
commit crea un worktree, inyecta los archivos de escena congelados (que son
datos, no código) y el perfil de compilación, compila `standalone` y parsea el
tiempo que ya imprime por stdout.

Por eso este driver vive FUERA del código medido y está escrito en Python: en
los commits viejos el crate `rt-bench` todavía no existe, así que la barrida no
puede depender de él. `rt-bench` se usa de HEAD en adelante; este script se usa
para reconstruir el pasado.

Notas de diseño:

  * El timer de `standalone` envuelve solo `render_scene`, así que el parseo de
    la escena y la construcción del BVH quedan FUERA de la medición.
  * Las corridas se INTERCALAN entre commits en lugar de agrupar todas las
    repeticiones de cada uno. En un portátil el chip se calienta durante la
    barrida; agrupando, los commits tardíos parecerían más lentos por
    termodinámica y no por código.
  * Cada binario se copia fuera de su worktree y el worktree se destruye, para
    no acumular decenas de GB de directorios `target/`.

Uso:
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

# Rutas que `standalone` tiene hardcodeadas. El driver copia los archivos del
# benchmark a estas rutas dentro del worktree.
STANDALONE_SCENE = "scenes/spheres_scene.json"
STANDALONE_CAMERA = "scenes/spheres_camera.json"

TIME_RE = re.compile(r"Procesado en (\d+) ms")

# Los commits que cambian rendimiento. Los intermedios que implementan el BVH
# pero todavía no lo usan se omiten a propósito: rinden igual que el anterior.
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
]

# El perfil hay que inyectarlo siempre: los commits viejos no traen
# `[profile.release]` ni `.cargo/config.toml`, así que sin esto cada commit se
# compilaría con flags distintos y la comparación no valdría.
#
# Solo se mide `optimized`. La barrida del 2026-08-13 midió también `default`
# (lto/cu/target-cpu desactivados) y la diferencia salió ~0% incluso en las
# filas de baja varianza — ese resultado ya está en history.jsonl. Se puede
# reproducir con `--profiles default optimized`.
PROFILES = ["optimized"]

OPTIMIZED_PROFILE = """
[profile.release]
lto = "fat"
codegen-units = 1
"""

OPTIMIZED_CARGO_CONFIG = """[build]
rustflags = ["-C", "target-cpu=native"]
"""

# `standalone` hardcodeaba el gradiente de cielo y descartaba el background que
# declara la escena. Para B2 da igual (declara justo ese gradiente), pero para
# un Cornell box es fatal: el frente está abierto, así que un cielo brillante
# entra como segunda fuente de luz y la escena deja de ser "luz pequeña".
#
# Es la ÚNICA inyección de código de toda la barrida, y se permite porque:
#   * la línea es byte-idéntica en los 6 commits, así que la sustitución es
#     mecánica y no requiere resolver ningún conflicto;
#   * no toca el bucle de render — cambia qué valor se pasa, no código que
#     corra por rayo;
#   * se aplica IGUAL en todos los commits, así que no sesga la comparación.
# El driver verifica que haya exactamente una coincidencia antes de sustituir.
BACKGROUND_HARDCODED = (
    "let background = Background::new_gradient("
    "Color::new(0.5, 0.7, 1.0), Color::new(1.0, 1.0, 1.0));"
)
BACKGROUND_FROM_SCENE = "let background = scene_payload.background.clone();"

STANDALONE_SRC = "crates/rt-renderer/src/bin/standalone.rs"


@dataclass
class Result:
    benchmark: str
    config: str
    # La carga real, explícita. El nombre de la config NO basta: `quick` de B2
    # fue 640/64 antes del 2026-08-13 y 1920/20 después, y agregar por
    # (benchmark, config) mezclaría datos incomparables. Con estos dos campos
    # el registro se explica solo sin tener que mirar el hash del manifiesto.
    width: int
    spp: int
    commit: str
    commit_label: str
    commit_subject: str
    commit_date: str
    profile: str
    rep: int
    wall_ms: int
    # Campos que solo `rt-bench` puede llenar (necesitan instrumentación
    # interna). Se dejan explícitos en null para que el esquema sea el mismo.
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
            f"comando falló: {' '.join(cmd)}\n"
            f"cwd={cwd}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    return proc.stdout


def git(*args: str, cwd: Path | None = None) -> str:
    return run(["git", *args], cwd=cwd or REPO).strip()


def file_sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()[:16]


def read_cpu_model() -> str:
    """`platform.processor()` en Linux suele venir vacío; el modelo real
    importa para poder atribuir mediciones entre máquinas."""
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or platform.machine()


def read_cpu_mhz() -> float | None:
    """Mejor esfuerzo: dentro de una VM cpufreq suele no estar expuesto."""
    freqs = list(Path("/sys/devices/system/cpu").glob("cpu*/cpufreq/scaling_cur_freq"))
    if not freqs:
        return None
    try:
        values = [int(f.read_text()) for f in freqs]
        return round(statistics.mean(values) / 1000.0, 1)
    except OSError:
        return None


def strip_release_profile(text: str) -> str:
    """Elimina la sección [profile.release] preservando el resto del archivo."""
    lines = text.splitlines(keepends=True)
    out: list[str] = []
    skipping = False
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("["):
            # Cualquier encabezado nuevo termina la sección que estemos saltando.
            skipping = stripped == "[profile.release]"
        if not skipping:
            out.append(line)
    return "".join(out)


def apply_profile(worktree: Path, profile: str) -> None:
    cargo_toml = worktree / "Cargo.toml"
    cargo_config = worktree / ".cargo" / "config.toml"

    # Partimos siempre de un Cargo.toml sin perfil: los commits recientes ya lo
    # traen y hay que quitarlo para medir el perfil `default`.
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
    Hace que `standalone` respete el background declarado por la escena.

    Devuelve True si hubo que parchar, False si el commit ya lo hacía bien.
    Lanza si encuentra algo inesperado, para no medir en silencio una escena
    distinta de la que se pidió.
    """
    src = worktree / STANDALONE_SRC
    text = src.read_text()

    hits = text.count(BACKGROUND_HARDCODED)
    if hits == 1:
        src.write_text(text.replace(BACKGROUND_HARDCODED, BACKGROUND_FROM_SCENE))
        return True
    if hits == 0 and "scene_payload.background" in text:
        return False  # el commit ya lee el background de la escena
    raise RuntimeError(
        f"{src}: se esperaba 1 coincidencia del background hardcodeado y se "
        f"encontraron {hits}, sin que el archivo lea `scene_payload.background`. "
        f"La barrida mediría una escena distinta de la declarada."
    )


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
    Materializa un directorio de trabajo con los archivos de escena que
    `standalone` espera en sus rutas hardcodeadas.

    El `camera.json` se genera parchando width y spp desde bench.toml, de modo
    que los valores commiteados en camera.json nunca definen la carga.
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
    """Compila un binario por (commit, perfil) y lo copia fuera del worktree."""
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
            # El target/ de un build con lto=fat pesa GB; el binario ya está a
            # salvo, así que el worktree completo se puede tirar.
            shutil.rmtree(worktree / "target", ignore_errors=True)
            git("worktree", "remove", "--force", str(worktree))

    return binaries


def measure(binary: Path, stage: Path) -> int:
    stdout = run([str(binary)], cwd=stage)
    match = TIME_RE.search(stdout)
    if not match:
        raise RuntimeError(
            f"no se encontró el tiempo en la salida de {binary.name}:\n{stdout}"
        )
    return int(match.group(1))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", default="quick", choices=["quick", "full"])
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

    # Cada unidad de trabajo es una combinación completa. Iteramos las
    # repeticiones por fuera para intercalar y decorrelacionar la deriva
    # térmica del orden de los commits.
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
        # Única inyección de código de la barrida; queda registrada para que
        # los resultados sean autoexplicativos.
        "standalone_background_patch": True,
        # `standalone` los tiene hardcodeados e idénticos en los 6 commits.
        # `rt-bench` los va a hacer configurables, así que hay que dejar
        # constancia de con cuáles se tomaron estas mediciones.
        "max_depth": 15,
        "tile_size": 128,
    }

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
            # Spread relativo: si supera ~10% el dato no resuelve diferencias
            # pequeñas y conviene subir la resolución o las repeticiones.
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
            fh.write(json.dumps({**asdict(r), "env": env}) + "\n")
    print(f"\n{len(results)} resultados añadidos a {HISTORY.relative_to(REPO)}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
