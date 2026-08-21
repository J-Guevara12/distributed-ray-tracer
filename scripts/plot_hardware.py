#!/usr/bin/env python3
"""
Per-commit evolution with one series per hardware generation.

Same as `plot_evolution.py` but overlaying the generations on shared axes, to
read the cut directly: parallel curves mean a uniform multiplier, and then ratios
measured inside one generation still hold across it. Curves that cross or diverge
mean the cut changed the ranking and no cross-generation comparison is valid.

The x axis keys on commit hash, not label: labels are free text and the same
commit was measured under different names in different runs.

Prints the PNG path to stdout; the ratio table goes to stderr.

    python3 scripts/plot_hardware.py | xargs kitten icat
"""

import argparse
import json
import statistics as st
import sys
from collections import Counter, defaultdict
from datetime import datetime
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

REPO = Path(__file__).resolve().parent.parent
HISTORY = REPO / "bench" / "history.jsonl"
OUT_DIR = REPO / "bench" / "plots"

LEGACY_HARDWARE = "gen0"

# Records before the `--tracer` flag existed are all path-traced. A normals
# render is one ray per sample, so mixing it into a timing series shows a
# spectacular drop that is not an optimisation.
LEGACY_TRACER = "path"

PALETTE = ["#4878cf", "#d1495b", "#3f9950", "#e08b1f", "#8d6cab"]


def load():
    rows, skipped = [], 0

    for line in HISTORY.open():
        if not line.strip():
            continue
        try:
            r = json.loads(line)
            env = r["env"]
        except (json.JSONDecodeError, KeyError, TypeError):
            skipped += 1
            continue
        if not r.get("width") or not r.get("spp"):
            skipped += 1
            continue

        rows.append({
            "hardware": r.get("hardware") or LEGACY_HARDWARE,
            "tracer": r.get("tracer") or LEGACY_TRACER,
            "commit": r["commit"][:12],
            "label": r["commit_label"],
            "date": r["commit_date"],
            "measured": r.get("timestamp", ""),
            "benchmark": r["benchmark"],
            "config": r["config"],
            "wall_ms": r["wall_ms"],
            "workload": (r["width"], r["spp"]),
            "tile_size": env.get("tile_size"),
        })

    if skipped:
        print(f"aviso: {skipped} registros sin width/spp, no comparables",
              file=sys.stderr)
    return rows


def dominant_workload(rows):
    """Per panel, the workload with the most distinct commits. One scene measured
    at two resolutions is not one series, it is two."""
    counts = defaultdict(Counter)
    for r in rows:
        counts[(r["config"], r["benchmark"])][r["workload"]] += 1
    return {panel: c.most_common(1)[0][0] for panel, c in counts.items()}


def commit_axis(rows):
    """Chronological commit order. The label comes from the most recent
    measurement, since the newer runs are the ones with useful names."""
    date, label, newest = {}, {}, {}
    for r in rows:
        date[r["commit"]] = r["date"]
        if r["measured"] >= newest.get(r["commit"], ""):
            newest[r["commit"]] = r["measured"]
            label[r["commit"]] = r["label"]
    order = sorted(date, key=lambda c: date[c])
    return order, [label[c] for c in order]


def series(rows, generation, commits):
    """Median, min and max per commit. `None` where that generation did not
    measure it, so the line breaks instead of interpolating over a gap."""
    samples = defaultdict(list)
    for r in rows:
        if r["hardware"] == generation:
            samples[r["commit"]].append(r["wall_ms"])

    median = [st.median(samples[c]) if samples[c] else None for c in commits]
    low = [min(samples[c]) if samples[c] else None for c in commits]
    high = [max(samples[c]) if samples[c] else None for c in commits]
    return median, low, high


def ratios(rows, generations, commits, labels):
    """Per-commit ratio against the newest generation, where both measured it.
    This is the number that says whether the cut is uniform."""
    if len(generations) < 2:
        return

    reference = generations[-1]
    per_generation = {g: series(rows, g, commits)[0] for g in generations}
    tiles = defaultdict(dict)
    for r in rows:
        tiles[r["commit"]][r["hardware"]] = r["tile_size"]

    for generation in generations[:-1]:
        collected = []
        print(f"\n  {generation} / {reference}", file=sys.stderr)
        print(f"    {'commit':<24}{generation:>11}{reference:>11}{'cociente':>11}",
              file=sys.stderr)
        print(f"    {'-' * 57}", file=sys.stderr)

        for commit, label, a, b in zip(commits, labels,
                                       per_generation[generation],
                                       per_generation[reference]):
            if a is None or b is None:
                continue
            flag = ""
            if tiles[commit].get(generation) != tiles[commit].get(reference):
                flag = f"  tile {tiles[commit].get(generation)}/{tiles[commit].get(reference)}"
            print(f"    {label[:23]:<24}{a:>9.0f}ms{b:>9.0f}ms{a / b:>10.2f}x{flag}",
                  file=sys.stderr)
            if not flag:
                collected.append(a / b)

        if len(collected) > 1:
            spread = st.stdev(collected) / st.mean(collected) * 100
            print(f"    {'mediana':<24}{'':>11}{'':>11}"
                  f"{st.median(collected):>10.2f}x   (dispersión {spread:.1f}%, "
                  f"n={len(collected)})", file=sys.stderr)
            if spread < 10:
                print(f"    Uniforme: los cocientes medidos dentro de {generation} "
                      f"siguen valiendo.", file=sys.stderr)
            else:
                print(f"    NO uniforme: el corte cambió el ranking, ninguna "
                      f"comparación cruzada sirve.", file=sys.stderr)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hardware", nargs="*", metavar="GEN",
                        help="generaciones a superponer; por defecto todas")
    parser.add_argument("--tracer", default="path", choices=["path", "normal", "all"],
                        help="qué tracer graficar; 'normal' traza un rayo por muestra "
                             "y mezclarlo inventa una caída (default: path)")
    parser.add_argument("--linear", action="store_true",
                        help="eje y lineal en vez de logarítmico")
    args = parser.parse_args()

    rows = load()
    if not rows:
        raise SystemExit("error: no hay registros comparables en history.jsonl")

    if args.tracer != "all":
        dropped = sum(1 for r in rows if r["tracer"] != args.tracer)
        rows = [r for r in rows if r["tracer"] == args.tracer]
        if dropped:
            print(f"aviso: {dropped} registros de otro tracer excluidos "
                  f"(--tracer all para incluirlos)", file=sys.stderr)
        if not rows:
            raise SystemExit(f"error: no hay registros con tracer {args.tracer}")

    present = sorted({r["hardware"] for r in rows},
                     key=lambda g: min(r["measured"] for r in rows if r["hardware"] == g))

    if args.hardware:
        unknown = [g for g in args.hardware if g not in present]
        if unknown:
            raise SystemExit(f"error: no hay registros de {', '.join(unknown)}. "
                             f"Presentes: {', '.join(present)}")
        generations = [g for g in present if g in args.hardware]
    else:
        generations = present

    if len(generations) < 2:
        print(f"aviso: solo hay una generación ({generations[0]}); el gráfico va a "
              f"tener una sola serie", file=sys.stderr)

    rows = [r for r in rows if r["hardware"] in generations]
    keep = dominant_workload(rows)
    rows = [r for r in rows if r["workload"] == keep[(r["config"], r["benchmark"])]]

    configs = sorted({r["config"] for r in rows})
    benchmarks = sorted({r["benchmark"] for r in rows})

    fig, axes = plt.subplots(len(configs), len(benchmarks),
                             figsize=(7 * len(benchmarks), 4.5 * len(configs)),
                             squeeze=False)

    for row, config in enumerate(configs):
        for col, bench in enumerate(benchmarks):
            ax = axes[row][col]
            panel = [r for r in rows if r["config"] == config and r["benchmark"] == bench]
            if not panel:
                ax.set_axis_off()
                continue

            commits, labels = commit_axis(panel)
            for index, generation in enumerate(generations):
                median, low, high = series(panel, generation, commits)
                if not any(m is not None for m in median):
                    continue
                color = PALETTE[index % len(PALETTE)]
                x = range(len(commits))
                ax.plot(x, median, marker="o", markersize=3.5, linewidth=1.6,
                        color=color, label=generation)
                drawn = [(i, l, h) for i, (l, h) in enumerate(zip(low, high))
                         if l is not None]
                if drawn:
                    ax.fill_between([d[0] for d in drawn], [d[1] for d in drawn],
                                    [d[2] for d in drawn], color=color, alpha=0.18)

            width, spp = keep[(config, bench)]
            ax.set_xticks(range(len(commits)))
            ax.set_xticklabels(labels, rotation=45, ha="right", fontsize=7)
            ax.set_ylabel("tiempo de render (ms)")
            if not args.linear:
                ax.set_yscale("log")
            ax.set_title(f"{bench} — {config}  {width}px / {spp} spp")
            ax.grid(alpha=0.3)
            ax.legend(fontsize=8)

    stamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    fig.suptitle(f"Evolución por generación de hardware — "
                 f"{', '.join(generations)} — {stamp}", fontsize=13)
    fig.tight_layout(rect=(0, 0, 1, 0.96))

    if len(generations) > 1:
        print("\n== cocientes por commit, donde las dos generaciones lo midieron ==",
              file=sys.stderr)
        for config in configs:
            for bench in benchmarks:
                panel = [r for r in rows
                         if r["config"] == config and r["benchmark"] == bench]
                if not panel:
                    continue
                commits, labels = commit_axis(panel)
                print(f"\n  {bench} {config}", file=sys.stderr)
                ratios(panel, generations, commits, labels)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"hardware-{datetime.now().strftime('%Y%m%d-%H%M%S')}.png"
    fig.savefig(out, dpi=130)
    print(out)


if __name__ == "__main__":
    main()
