#!/usr/bin/env python3
"""
Barrida de `tile_size` sobre la suite de benchmarks.

A diferencia de `bench_sweep.py`, esto no viaja por el historial: mide un único
build en el commit actual variando solo el tamaño de tile. Por eso usa `rt-bench`
directamente en vez de `standalone` — ya expone `--tile-size` y registra el
resumen por tile (min/mediana/p95/max/desbalance), que es justo el mecanismo
bajo prueba.

El agrupamiento sale de `env.tile_size`, que `rt-bench` ya registra. Las corridas
igual se etiquetan `--label tile-<N>` para poder distinguirlas a ojo en el JSONL
y para filtrar lo que escribió esta barrida. Los resultados van a un archivo
aparte para no ensuciar `bench/history.jsonl`.

Dos invariantes se verifican solos:

  * El `image_hash` NO puede cambiar con el tamaño de tile. El RNG está sembrado
    por (píxel, sample), así que la partición en tiles es irrelevante para el
    resultado. Si cambia, hay un bug de particionado, no una medición.
  * `node_visits/rayo` tampoco puede cambiar: es la misma geometría y los mismos
    rayos. Si se mueve, algo depende del orden de los tiles.

Metodología (ver LEARNED_LESSONS):

  * Corridas INTERCALADAS entre tamaños, no agrupadas: en un portátil el chip se
    calienta durante la barrida y agrupar le regala el primer lugar al primero.
  * Dispersión reportada como desviación estándar relativa, nunca como rango
    min-max, que crece con n y no es comparable entre variantes.
  * Se declara ganador solo si la diferencia supera el ruido de las dos.

Uso:

    python3 scripts/tile_sweep.py --config full --reps 3
    python3 scripts/tile_sweep.py --config full --sizes 32 64 128 --only B2
"""

import argparse
import json
import os
import statistics
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_SIZES = [16, 32, 64, 128, 256]
BASELINE = 128           # el default de rt-bench hasta 2026-08-18; hoy es 32
OUT = REPO / "bench" / "tile_sweep.jsonl"

# Segundos por corrida de cada benchmark, para la estimación previa. Sacados de
# `quick` escalado por spp; solo sirven para decidir si te da tiempo un café.
COST_HINT = {("B1", "full"): 18, ("B2", "full"): 22,
             ("B1", "quick"): 1, ("B2", "quick"): 2}


class Colors:
    def __init__(self, enabled):
        self.dim = "\033[2m" if enabled else ""
        self.bold = "\033[1m" if enabled else ""
        self.green = "\033[32m" if enabled else ""
        self.red = "\033[31m" if enabled else ""
        self.yellow = "\033[33m" if enabled else ""
        self.off = "\033[0m" if enabled else ""


def run(cmd, **kwargs):
    proc = subprocess.run(cmd, cwd=REPO, text=True, **kwargs)
    if proc.returncode != 0:
        sys.exit(f"falló: {' '.join(str(c) for c in cmd)}")
    return proc


def measure(size, rep, args):
    """Una corrida de rt-bench con --reps 1; el intercalado lo maneja el llamador."""
    cmd = [
        "./target/release/rt-bench", "run",
        "--config", args.config,
        "--tile-size", str(size),
        "--reps", "1",
        "--cooldown", str(args.cooldown),
        "--label", f"tile-{size}",
        "--out", str(OUT),
    ]
    if args.only:
        cmd += ["--only", ",".join(args.only)]
    if args.allow_dirty:
        cmd.append("--allow-dirty")

    run(cmd, stdout=subprocess.DEVNULL if args.quiet else None)


def load(started_at):
    """Solo los registros que escribió esta barrida."""
    if not OUT.exists():
        sys.exit(f"no se escribió nada en {OUT}")

    records = []
    for line in OUT.read_text().splitlines():
        if not line.strip():
            continue
        record = json.loads(line)
        if not record.get("commit_label", "").startswith("tile-"):
            continue
        if record.get("timestamp", "") < started_at:
            continue
        records.append(record)
    return records


def size_of(record):
    """`env.tile_size` es la fuente de verdad; el label es solo para leerlo a ojo."""
    return record["env"]["tile_size"]


def spread(values):
    """Mediana y desviación estándar relativa. El rango min-max no sirve: crece
    con n y no es comparable entre variantes con distinta cantidad de muestras."""
    median = statistics.median(values)
    if len(values) < 2 or median == 0:
        return median, 0.0
    return median, statistics.stdev(values) / median * 100


def check_invariants(records, c):
    """El tamaño de tile no puede cambiar ni la imagen ni el trabajo de recorrido."""
    problems = []

    for bench in sorted({r["benchmark"] for r in records}):
        rows = [r for r in records if r["benchmark"] == bench]

        hashes = {r.get("image_hash") for r in rows if r.get("image_hash")}
        if len(hashes) > 1:
            by_size = {}
            for r in rows:
                by_size.setdefault(size_of(r), set()).add(r.get("image_hash"))
            problems.append(
                f"{bench}: la imagen CAMBIA con el tamaño de tile — "
                + ", ".join(f"{k}={sorted(v)[0][:8]}" for k, v in sorted(by_size.items()))
            )

        per_ray = {
            size_of(r): round(r["node_visits"] / r["rays"], 4)
            for r in rows
            if r.get("node_visits") and r.get("rays")
        }
        if len(set(per_ray.values())) > 1:
            problems.append(f"{bench}: nodos/rayo cambia con el tile — {per_ray}")

    if problems:
        print(f"\n{c.red}{c.bold}== INVARIANTES ROTOS =={c.off}")
        for p in problems:
            print(f"  {c.red}{p}{c.off}")
        print(f"  {c.dim}Esto es un bug de particionado, no un resultado de rendimiento.{c.off}")
    return not problems


def efficiency(record, threads):
    """Fracción del tiempo de pared en que los hilos tuvieron trabajo.

    `imbalance` no sirve para esto: está definido como (max - media) / media, o
    sea cuánto se despega el peor tile del promedio. Eso mide heterogeneidad del
    CONTENIDO y sube con tiles chicos por construcción, justo al revés de lo que
    uno querría leer. Lo que importa es si sobran hilos ociosos:

        eficiencia = suma(tiempos de tile) / (hilos × tiempo de pared)

    La suma se recupera del resumen: media = max / (1 + imbalance).
    """
    tiles = record.get("tiles")
    if not tiles or not record.get("wall_ms"):
        return None
    mean = tiles["max_ms"] / (1.0 + tiles["imbalance"])
    return mean * tiles["count"] / (threads * record["wall_ms"])


def report(records, c, threads):
    for bench in sorted({r["benchmark"] for r in records}):
        rows = [r for r in records if r["benchmark"] == bench]
        head = rows[0]
        print(f"\n{c.bold}{bench}{c.off}  {head['width']}x{head['height']}  "
              f"{head['spp']} spp  {c.dim}({head['config']}, {threads} hilos){c.off}")

        sizes = sorted({size_of(r) for r in rows})

        # Primera pasada: hay que tener la base antes de poder comparar contra
        # ella, y 128 no es el tamaño más chico.
        summary = {}
        for size in sizes:
            group = [r for r in rows if size_of(r) == size]
            median, rsd = spread([r["wall_ms"] for r in group])
            summary[size] = {
                "median": median,
                "rsd": rsd,
                "n": len(group),
                "mray": statistics.median([r["rays_per_sec"] / 1e6 for r in group
                                           if r.get("rays_per_sec")] or [0]),
                "count": next((r["tiles"]["count"] for r in group if r.get("tiles")), 0),
                "eff": statistics.median(
                    [e for e in (efficiency(r, threads) for r in group) if e] or [0]),
            }

        base = summary.get(BASELINE, {}).get("median")

        print(f"  {'tile':>5} {'render':>10} {'rsd':>7} {'n':>3} {'Mray/s':>8} "
              f"{'tiles':>7} {'eficiencia':>11} {'vs ' + str(BASELINE):>10}")
        print(f"  {'-' * 70}")

        for size in sizes:
            row = summary[size]
            if size == BASELINE:
                change = f"{c.dim}base{c.off}"
            elif base:
                delta = (row["median"] - base) / base * 100
                tint = c.green if delta < 0 else c.red
                change = f"{tint}{delta:+.1f}%{c.off}"
            else:
                change = "—"

            eff = row["eff"] * 100
            tint = c.red if eff < 90 else (c.dim if eff > 98 else "")
            print(f"  {size:>5} {row['median']:>7.0f} ms {row['rsd']:>6.1f}% "
                  f"{row['n']:>3} {row['mray']:>8.2f} {row['count']:>7} "
                  f"{tint}{eff:>10.1f}%{c.off} {change:>10}")

        verdict({k: (v["median"], v["rsd"]) for k, v in summary.items()}, c)


def verdict(summary, c):
    """Un ganador solo cuenta si le saca más que el ruido de las dos medidas."""
    if BASELINE not in summary:
        return

    base_median, base_rsd = summary[BASELINE]
    best = min(summary.items(), key=lambda kv: kv[1][0])
    size, (median, rsd) = best

    if size == BASELINE:
        print(f"  {c.dim}El default de {BASELINE} sigue siendo el mejor.{c.off}")
        return

    gain = (base_median - median) / base_median * 100
    noise = 2 * max(base_rsd, rsd)

    if gain > noise:
        # Entre tamaños que empatan dentro del ruido, gana el más grande: menos
        # tiles significa menos locks del framebuffer y menos reservas de Vec.
        tied = [s for s, (m, r) in summary.items()
                if abs(m - median) / median * 100 <= 2 * max(r, rsd)]
        pick = max(tied)
        extra = (f" {c.dim}(empata con {sorted(tied)}; se elige el mayor){c.off}"
                 if len(tied) > 1 else "")
        print(f"  {c.green}{c.bold}tile_size {pick}: {gain:.1f}% mejor que {BASELINE}"
              f"{c.off} {c.dim}(ruido {noise:.1f}%){c.off}{extra}")
    else:
        print(f"  {c.yellow}Sin ganador: la mejor diferencia ({gain:.1f}%) no supera "
              f"el ruido ({noise:.1f}%).{c.off} {c.dim}Subí --reps.{c.off}")


def main():
    parser = argparse.ArgumentParser(description="Barrida de tile_size con rt-bench")
    parser.add_argument("--sizes", type=int, nargs="+", default=DEFAULT_SIZES)
    parser.add_argument("--reps", type=int, default=3)
    parser.add_argument("--cooldown", type=int, default=20,
                        help="segundos entre corridas, los aplica rt-bench")
    parser.add_argument("--config", choices=["quick", "full"], default="full")
    parser.add_argument("--only", nargs="*", default=[], help="ids: B1 B2")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--report-only", action="store_true",
                        help="no mide, solo re-imprime el último JSONL")
    parser.add_argument("--quiet", action="store_true", help="silencia rt-bench")
    parser.add_argument("--threads", type=int, default=os.cpu_count(),
                        help="para la columna de eficiencia paralela")
    parser.add_argument("--no-color", action="store_true")
    args = parser.parse_args()

    c = Colors(not args.no_color and sys.stdout.isatty())

    if args.report_only:
        records = load("")
        if records:
            check_invariants(records, c)
            report(records, c, args.threads)
        return

    if BASELINE not in args.sizes:
        sys.exit(f"--sizes tiene que incluir {BASELINE}: es la línea base de comparación.\n"
                 f"Si querés comparar contra otro, cambiá BASELINE en el script.")

    if not args.skip_build:
        print(f"{c.dim}compilando rt-bench en release...{c.off}")
        run(["cargo", "build", "--release", "--bin", "rt-bench"])

    benches = args.only or ["B1", "B2"]
    runs = len(args.sizes) * args.reps
    seconds = sum(COST_HINT.get((b, args.config), 20) for b in benches) * runs
    seconds += args.cooldown * len(benches) * runs

    print(f"\n{c.bold}Barrida de tile_size{c.off}")
    print(f"  config    {args.config}")
    print(f"  tamaños   {args.sizes}")
    print(f"  reps      {args.reps}  ({runs} corridas, intercaladas)")
    print(f"  salida    {OUT.relative_to(REPO)}")
    print(f"  estimado  ~{seconds // 60} min\n")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    started_at = time.strftime("%Y-%m-%dT%H:%M:%S")
    clock = time.time()

    done = 0
    for rep in range(1, args.reps + 1):
        for size in args.sizes:      # intercalado: rep externo, tamaño interno
            done += 1
            elapsed = time.time() - clock
            eta = elapsed / done * (runs - done) if done else 0
            print(f"{c.dim}[{done}/{runs}]{c.off} rep {rep}, tile {size}"
                  f"{c.dim}  (quedan ~{eta / 60:.0f} min){c.off}")
            measure(size, rep, args)

    records = load(started_at)
    if not records:
        sys.exit("la barrida no dejó registros; ¿rt-bench falló en silencio?")

    print(f"\n{c.bold}== Resultados =={c.off}")
    check_invariants(records, c)
    report(records, c, args.threads)
    print(f"\n{c.dim}Registros en {OUT.relative_to(REPO)}. "
          f"Re-imprimir: python3 scripts/tile_sweep.py --report-only{c.off}")


if __name__ == "__main__":
    main()
