#!/usr/bin/env python3
"""
Grafica la evolución del tiempo de render por commit.

Línea = mediana, sombra = rango min-max de las repeticiones.
Imprime la ruta del PNG por stdout; las notas van por stderr.
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

# Cargas de las corridas anteriores a que el registro trajera width/spp.
# (hash de bench.toml, config) -> (width, spp)
LEGACY_WORKLOADS = {
    ("93f81f40190c645c", "quick"): (640, 64),
    ("80777a51f6822816", "quick"): (1920, 20),
    ("80777a51f6822816", "full"): (1920, 250),
    ("0045c38302d84891", "quick"): (1024, 8),
    ("0045c38302d84891", "full"): (1024, 200),
}


def bench_names():
    names = {}
    for manifest in (REPO / "scenes" / "bench").glob("*/bench.toml"):
        ident = name = None
        for line in manifest.read_text().splitlines():
            if line.startswith("id"):
                ident = line.split("=")[1].strip().strip('"')
            elif line.startswith("name"):
                name = line.split("=")[1].strip().strip('"')
        if ident and name:
            names[ident] = name
    return names


def load():
    """Agrupa por la carga real, no por el hash del bench.toml: ese cambia con
    cualquier edición del archivo aunque la carga sea la misma."""
    names = bench_names()
    rows, bad = [], []

    for number, line in enumerate(HISTORY.open(), start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
            record["env"]["scene_hashes"]
        except (json.JSONDecodeError, KeyError, TypeError):
            bad.append(number)
            continue
        rows.append(record)

    if bad:
        print(f"aviso: {len(bad)} líneas ilegibles en history.jsonl "
              f"(líneas {', '.join(map(str, bad[:5]))}...)", file=sys.stderr)

    unknown = set()

    for r in rows:
        hashes = r["env"]["scene_hashes"].get(names.get(r["benchmark"], ""), {})

        if r.get("width") and r.get("spp"):
            workload = (r["width"], r["spp"])
        else:
            key = (hashes.get("manifest", "?"), r["config"])
            workload = LEGACY_WORKLOADS.get(key)
            if workload is None:
                unknown.add(key)

        r["_key"] = (hashes.get("scene"), hashes.get("camera"), workload)
        r["_workload"] = workload

    if unknown:
        for manifest, config in sorted(unknown):
            print(f"aviso: carga desconocida para bench.toml {manifest[:8]} "
                  f"({config}); agrégala a LEGACY_WORKLOADS", file=sys.stderr)

    return rows


def pick_workload(rows):
    """Por (config, benchmark), la carga con más commits distintos."""
    commits = defaultdict(set)
    newest = defaultdict(str)
    for r in rows:
        key = (r["config"], r["benchmark"], r["_key"])
        commits[key].add(r["commit_label"])
        newest[key] = max(newest[key], r["timestamp"])

    best = {}
    for (config, bench, key), labels in commits.items():
        score = (len(labels), newest[(config, bench, key)])
        if (config, bench) not in best or score > best[(config, bench)][1]:
            best[(config, bench)] = (key, score)
    return {k: v[0] for k, v in best.items()}


def select_labels(rows, start, back):
    """Etiquetas a graficar, en orden cronológico. `start` acepta un
    `commit_label` o un prefijo de SHA."""
    order = {}
    for r in rows:
        order.setdefault(r["commit_label"], r["commit_date"])
    labels = sorted(order, key=lambda l: order[l])

    if start:
        shas = defaultdict(set)
        for r in rows:
            shas[r["commit_label"]].add(r["commit"])

        position = next(
            (
                i
                for i, label in enumerate(labels)
                if label == start or any(sha.startswith(start) for sha in shas[label])
            ),
            None,
        )
        if position is None:
            raise SystemExit(
                f"error: --from {start!r} no coincide con ningún label ni commit.\n"
                f"disponibles: {', '.join(labels)}"
            )
        labels = labels[position:]

    if back:
        labels = labels[-back:]

    return labels


def series(rows, config, benchmark, key):
    by_label = defaultdict(list)
    order = {}
    for r in rows:
        if r["config"] != config or r["benchmark"] != benchmark or r["_key"] != key:
            continue
        by_label[r["commit_label"]].append(r["wall_ms"] / 1000.0)
        order.setdefault(r["commit_label"], r["commit_date"])

    labels = sorted(by_label, key=lambda l: order[l])
    return (
        labels,
        [st.median(by_label[l]) for l in labels],
        [min(by_label[l]) for l in labels],
        [max(by_label[l]) for l in labels],
        [len(by_label[l]) for l in labels],
    )


def parse_args():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--from",
        dest="start",
        metavar="LABEL|SHA",
        help="graficar desde este punto en adelante",
    )
    parser.add_argument(
        "-b",
        "--back",
        type=int,
        metavar="N",
        help="quedarse solo con los últimos N commits",
    )
    return parser.parse_args()


def main():
    args = parse_args()
    rows = load()

    # El filtro va antes de elegir la carga: si pides los últimos N commits,
    # quieres la revisión de manifiesto que esos N comparten.
    selected = select_labels(rows, args.start, args.back)
    rows = [r for r in rows if r["commit_label"] in set(selected)]
    if not rows:
        raise SystemExit("error: el filtro no dejó ningún registro")

    keep = pick_workload(rows)

    configs = ["quick", "full"]
    benchmarks = sorted({r["benchmark"] for r in rows})

    fig, axes = plt.subplots(len(configs), len(benchmarks), figsize=(13, 8), squeeze=False)
    dropped = defaultdict(int)

    for row, config in enumerate(configs):
        for col, bench in enumerate(benchmarks):
            ax = axes[row][col]
            key = keep.get((config, bench))
            if key is None:
                ax.set_visible(False)
                continue

            labels, median, low, high, n = series(rows, config, bench, key)

            for r in rows:
                if r["config"] == config and r["benchmark"] == bench and r["_key"] != key:
                    dropped[(bench, config, r["_workload"])] += 1

            x = range(len(labels))
            ax.fill_between(x, low, high, alpha=0.25, label="min-max")
            ax.plot(x, median, marker="o", label="mediana")

            ax.set_xticks(list(x))
            ax.set_xticklabels(labels, rotation=35, ha="right", fontsize=8)
            width, spp = key[2] if key[2] else ("?", "?")
            ax.set_title(f"{bench} — {config}  {width}px / {spp} spp  (n={n[0] if n else 0})")
            ax.set_ylabel("segundos")
            ax.grid(alpha=0.3)
            ax.legend(fontsize=8)

            for xi, (m, lo, hi) in enumerate(zip(median, low, high)):
                spread = (hi - lo) / m * 100 if m else 0
                ax.annotate(
                    f"{m:.2f}s\n±{spread:.0f}%",
                    (xi, m),
                    textcoords="offset points",
                    xytext=(0, 9),
                    ha="center",
                    fontsize=7,
                )

    stamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    scope = ""
    if args.start:
        scope += f"  desde {args.start}"
    if args.back:
        scope += f"  últimos {args.back}"
    fig.suptitle(f"Evolución del tiempo de render — {stamp}{scope}", fontsize=13)
    fig.tight_layout(rect=(0, 0, 1, 0.96))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"evolution-{datetime.now().strftime('%Y%m%d-%H%M%S')}.png"
    fig.savefig(out, dpi=130)

    if dropped:
        print("nota: omitidos por medir otra carga de trabajo:", file=sys.stderr)
        for (bench, config, workload), count in sorted(dropped.items(), key=str):
            print(f"  {bench} {config:<6} {workload}  ({count} registros)", file=sys.stderr)

    print(out)


if __name__ == "__main__":
    main()
