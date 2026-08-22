#!/usr/bin/env python3
"""
MSE against time: the figure Phase 1 is measured by.

Equal time, not equal samples. An integrator that costs twice per sample and
needs a tenth of them wins, and the wall clock alone says it got slower. On
log-log axes that comparison is read off the diagonals: a line of slope -1 is
constant `MSE x time`, which is constant efficiency, so two curves sitting on the
same diagonal are equally good no matter where you stop rendering.

The slope is a diagnostic in itself. Noise falls as 1/spp and time rises as spp,
so an unbiased integrator traces a straight line of slope -1. A curve that
flattens has hit a bias floor — truncation, clamping, or a stale reference.

Data comes from `rt-bench converge`, keyed by hardware and commit: MSE depends on
the code and time depends on the machine, so a curve only compares within one of
each.

Prints the PNG path to stdout; the table goes to stderr.

    python3 scripts/plot_convergence.py | xargs kitten icat
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
DATA = REPO / "bench" / "convergence.jsonl"
OUT_DIR = REPO / "bench" / "plots"

PALETTE = ["#4878cf", "#d1495b", "#3f9950", "#e08b1f", "#8d6cab", "#00a0a0"]


def load():
    if not DATA.exists():
        raise SystemExit(
            f"error: {DATA.relative_to(REPO)} does not exist. Build a curve with:\n"
            f"  ./target/release/rt-bench converge --only B1"
        )

    rows = []
    for line in DATA.open():
        if line.strip():
            rows.append(json.loads(line))
    if not rows:
        raise SystemExit(f"error: {DATA.relative_to(REPO)} is empty")
    return rows


def curves(rows):
    """Median per (curve, spp). The curve identity is what separates the lines:
    benchmark, integrator and commit."""
    grouped = defaultdict(lambda: defaultdict(list))
    for r in rows:
        key = (r["benchmark"], r["integrator"], r["commit_label"])
        grouped[key][r["spp"]].append(r)

    out = {}
    for key, by_spp in grouped.items():
        points = []
        for spp in sorted(by_spp):
            group = by_spp[spp]
            points.append({
                "spp": spp,
                "seconds": st.median([g["wall_ms"] for g in group]) / 1000.0,
                "mse": st.median([g["mse"] for g in group]),
                "relative_mse": st.median([g["relative_mse"] for g in group]),
                "efficiency": st.median([g["efficiency"] for g in group]),
                "n": len(group),
            })
        out[key] = points
    return out


def slope(points):
    """Least-squares slope of log(mse) against log(time). -1 is the unbiased
    ideal; flatter means a bias floor is being approached."""
    import math

    if len(points) < 2:
        return None
    xs = [math.log(p["seconds"]) for p in points]
    ys = [math.log(p["mse"]) for p in points]
    mx, my = sum(xs) / len(xs), sum(ys) / len(ys)
    denominator = sum((x - mx) ** 2 for x in xs)
    if denominator == 0:
        return None
    return sum((x - mx) * (y - my) for x, y in zip(xs, ys)) / denominator


def report(data):
    for key, points in sorted(data.items()):
        bench, integrator, commit = key
        fitted = slope(points)
        print(f"\n  {bench} / {integrator} / {commit}", file=sys.stderr)
        print(f"    {'spp':>6}{'time':>10}{'mse':>13}{'rel_mse':>13}"
              f"{'efficiency':>13}{'n':>4}", file=sys.stderr)
        print(f"    {'-' * 59}", file=sys.stderr)
        for p in points:
            print(f"    {p['spp']:>6}{p['seconds']:>9.3f}s{p['mse']:>13.4e}"
                  f"{p['relative_mse']:>13.4e}{p['efficiency']:>13.4f}"
                  f"{p['n']:>4}", file=sys.stderr)

        if fitted is not None:
            verdict = "unbiased" if abs(fitted + 1.0) < 0.15 else "FLATTER THAN -1"
            print(f"    slope {fitted:+.3f}  ({verdict}; -1 is the ideal)",
                  file=sys.stderr)
            if abs(fitted + 1.0) >= 0.15:
                print("    a flat tail means a bias floor: truncation, clamping, "
                      "or a reference that is not converged enough", file=sys.stderr)


def plot(data, hardware):
    benches = sorted({key[0] for key in data})
    fig, axes = plt.subplots(1, len(benches), figsize=(7 * len(benches), 5.5),
                             squeeze=False)

    for ax, bench in zip(axes[0], benches):
        selected = {k: v for k, v in data.items() if k[0] == bench}
        everything = [p for points in selected.values() for p in points]
        if not everything:
            ax.set_axis_off()
            continue

        lo_t = min(p["seconds"] for p in everything) / 3
        hi_t = max(p["seconds"] for p in everything) * 3

        # Iso-efficiency diagonals: mse * t = k, so slope -1 on log-log. Two
        # curves on the same diagonal are equally efficient.
        best = max(p["efficiency"] for p in everything)
        for factor, style in ((1.0, "-"), (0.5, "--"), (0.25, ":")):
            k = 1.0 / (best * factor)
            ax.plot([lo_t, hi_t], [k / lo_t, k / hi_t], style, color="#999",
                    linewidth=0.9, alpha=0.55, zorder=1,
                    label=f"efficiency {best * factor:.0f}" if factor == 1.0 else None)

        for index, (key, points) in enumerate(sorted(selected.items())):
            colour = PALETTE[index % len(PALETTE)]
            ax.plot([p["seconds"] for p in points], [p["mse"] for p in points],
                    marker="o", markersize=4.5, linewidth=1.6, color=colour,
                    zorder=5, label=f"{key[1]} / {key[2]}")
            for p in points:
                ax.annotate(f"{p['spp']}", (p["seconds"], p["mse"]), fontsize=6,
                            textcoords="offset points", xytext=(4, 4), alpha=0.8)

        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xlabel("render time (s)")
        ax.set_ylabel("MSE against reference (linear space)")
        ax.set_title(f"{bench}")
        ax.grid(which="both", alpha=0.25)
        ax.legend(fontsize=7.5, loc="upper right")

    stamp = datetime.now().strftime("%Y-%m-%d %H:%M")
    fig.suptitle(f"Convergence — MSE against time — {hardware} — {stamp}",
                 fontsize=13)
    fig.text(0.01, 0.012,
             "Labels are spp. Grey diagonals are constant MSE x time, i.e. constant "
             "efficiency: a curve on a lower diagonal is better at every stopping "
             "point. An unbiased integrator has slope -1; a flat tail is a bias floor.",
             fontsize=6.5, alpha=0.7, wrap=True)
    fig.tight_layout(rect=(0, 0.05, 1, 0.95))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"convergence-{datetime.now().strftime('%Y%m%d-%H%M%S')}.png"
    fig.savefig(out, dpi=130)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hardware", default=None,
                        help="generation to plot; defaults to the most recent")
    parser.add_argument("--commit", nargs="*", metavar="LABEL",
                        help="commit labels to overlay; defaults to all present")
    parser.add_argument("--table-only", action="store_true")
    args = parser.parse_args()

    rows = load()

    present = {r["hardware"] for r in rows}
    hardware = args.hardware or max(rows, key=lambda r: r["timestamp"])["hardware"]
    if hardware not in present:
        raise SystemExit(f"error: no records for {hardware}. "
                         f"Present: {', '.join(sorted(present))}")
    rows = [r for r in rows if r["hardware"] == hardware]

    if args.commit:
        labels = set(args.commit)
        unknown = labels - {r["commit_label"] for r in rows}
        if unknown:
            raise SystemExit(f"error: no records for {', '.join(sorted(unknown))}")
        rows = [r for r in rows if r["commit_label"] in labels]

    data = curves(rows)
    if not data:
        raise SystemExit("error: the filters left no records")

    print(f"\n== convergence, {hardware} ==", file=sys.stderr)
    report(data)
    if not args.table_only:
        print(plot(data, hardware))


if __name__ == "__main__":
    main()
