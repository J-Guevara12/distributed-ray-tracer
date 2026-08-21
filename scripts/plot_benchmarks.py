#!/usr/bin/env python3
"""
B1 y B2 sobre los mismos ejes, normalizados contra el primer commit.

`plot_evolution.py` grafica milisegundos, y en ms las dos escenas no se pueden
comparar: B2 arranca en 11 s y B1 en 0.66 s. Normalizando a "veces más rápido
que el primer commit" quedan en la misma escala y se ve QUÉ optimización sirvió
para CUÁL escena, que es donde están los hallazgos: el BVH multiplicó por 3 a B2
y dejó a B1 peor que antes, y la ruleta rusa hizo lo contrario.

Imprime la ruta del PNG por stdout; las notas van por stderr.

    python3 scripts/plot_benchmarks.py --hardware gen1 | xargs kitten icat
"""

import argparse
import json
import statistics as st
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt

REPO = Path(__file__).resolve().parent.parent
HISTORY = REPO / "bench" / "history.jsonl"
OUT_DIR = REPO / "bench" / "plots"

LEGACY_HARDWARE = "gen0"
COLORS = {"B1": "#d1495b", "B2": "#4878cf"}


def load(hardware):
    rows = []
    for line in HISTORY.open():
        if not line.strip():
            continue
        try:
            r = json.loads(line)
            r["env"]
        except (json.JSONDecodeError, KeyError, TypeError):
            continue
        if (r.get("hardware") or LEGACY_HARDWARE) != hardware:
            continue
        if not r.get("width") or not r.get("spp"):
            continue
        rows.append(r)
    return rows


def series(rows, config, bench):
    """Mediana por commit, en orden cronológico de commit."""
    by_commit = defaultdict(list)
    meta = {}
    for r in rows:
        if r["config"] != config or r["benchmark"] != bench:
            continue
        by_commit[r["commit"][:12]].append(r["wall_ms"])
        meta[r["commit"][:12]] = (r["commit_date"], r["commit_label"])

    order = sorted(by_commit, key=lambda c: meta[c][0])
    return (
        [meta[c][1] for c in order],
        [st.median(by_commit[c]) for c in order],
        [min(by_commit[c]) for c in order],
        [max(by_commit[c]) for c in order],
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hardware", default=None,
                        help="generación a graficar; por defecto la más reciente")
    parser.add_argument("--absolute", action="store_true",
                        help="milisegundos en vez de aceleración normalizada")
    args = parser.parse_args()

    all_rows = []
    for line in HISTORY.open():
        if line.strip():
            try:
                all_rows.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    present = {r.get("hardware") or LEGACY_HARDWARE for r in all_rows}

    newest = max(all_rows, key=lambda r: r["timestamp"])
    hardware = args.hardware or newest.get("hardware") or LEGACY_HARDWARE
    if hardware not in present:
        raise SystemExit(f"error: no hay registros de {hardware}. "
                         f"Presentes: {', '.join(sorted(present))}")

    rows = load(hardware)
    if not rows:
        raise SystemExit(f"error: {hardware} no dejó registros comparables")

    configs = sorted({r["config"] for r in rows})
    benchmarks = sorted({r["benchmark"] for r in rows})

    fig, axes = plt.subplots(1, len(configs), figsize=(7 * len(configs), 5.5),
                             squeeze=False)

    for ax, config in zip(axes[0], configs):
        ticks = None
        for bench in benchmarks:
            labels, median, low, high = series(rows, config, bench)
            if not median:
                continue

            if args.absolute:
                y, lo, hi = median, low, high
            else:
                # Aceleración contra el primer commit: invertida, porque menos
                # tiempo es mejor y una curva que sube se lee sola.
                base = median[0]
                y = [base / m for m in median]
                lo = [base / h for h in high]
                hi = [base / l for l in low]

            x = range(len(y))
            ax.plot(x, y, marker="o", markersize=3.5, linewidth=1.6,
                    color=COLORS.get(bench, None), label=bench)
            ax.fill_between(x, lo, hi, color=COLORS.get(bench, None), alpha=0.18)

            if ticks is None or len(labels) > len(ticks):
                ticks = labels

            print(f"  {bench} {config}: {len(y)} commits, "
                  f"{'x%.2f acumulado' % y[-1] if not args.absolute else '%.0f ms' % y[-1]}",
                  file=sys.stderr)

        ax.set_xticks(range(len(ticks or [])))
        ax.set_xticklabels(ticks or [], rotation=45, ha="right", fontsize=7)
        ax.grid(alpha=0.3)
        ax.legend()

        if args.absolute:
            ax.set_ylabel("tiempo de render (ms)")
            ax.set_yscale("log")
        else:
            ax.set_ylabel("veces más rápido que el primer commit")
            ax.set_yscale("log")
            ax.axhline(1.0, color="#888", linewidth=0.8)
        ax.set_title(f"{config}")

    stamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    kind = "tiempo absoluto" if args.absolute else "aceleración normalizada"
    fig.suptitle(f"B1 contra B2 — {kind} — {hardware} — {stamp}", fontsize=13)
    fig.tight_layout(rect=(0, 0, 1, 0.94))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"benchmarks-{datetime.now().strftime('%Y%m%d-%H%M%S')}.png"
    fig.savefig(out, dpi=130)
    print(out)


if __name__ == "__main__":
    main()
