#!/usr/bin/env python3
"""
Roofline: where the renderer sits against the machine's ceilings.

FLOPs and bytes are counted analytically from the code, not read from hardware
counters — the VM does not expose the PMU, and using counters on a server but
counts on the VM would make the two machines incomparable. The counts are
estimates; the table below IS the model and is meant to be audited.

One property makes the main conclusion robust despite that. If the FLOP count is
wrong by a factor k, then intensity (FLOP/byte) and achieved GFLOP/s both scale
by k, so the point moves **parallel to the bandwidth roofline**: its distance to
the bandwidth ceiling does not change. Only the distance to the compute ceiling
does. So "bandwidth-bound or not" survives a bad count; "% of compute peak" does
not.

Caveat the plot cannot show: the bandwidth ceilings come from a sequential read,
where the prefetcher always wins. BVH traversal chases pointers, so the
achievable bandwidth for this access pattern is below the measured ceiling and
the real gap is smaller than it looks.

Prints the PNG path to stdout; the derivation table goes to stderr.

    python3 scripts/plot_roofline.py | xargs kitten icat
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
CEILINGS = REPO / "bench" / "ceilings"
OUT_DIR = REPO / "bench" / "plots"

LEGACY_HARDWARE = "gen0"
LEGACY_TRACER = "path"

# --- the model -------------------------------------------------------------
#
# Hand-counted from the source; the derivation is in crates/rt-bench/cli_guide.md
# so the numbers in the report are auditable. The byte sizes are NOT counted here
# — they are asserted against the types by test_layout.rs.

FLOPS_PER_AABB = 25      # 6 sub + 6 mul + 3 min + 3 max + 4 horizontal + 2 clamp + 1 cmp

# A test that misses exits early and costs about half of one that hits, so the
# two are counted separately against the measured prim_tests / prim_hits. The
# first version used the hit cost for everything and overestimated ~2x, because
# with ~10 tests per ray almost all of them miss.
FLOPS_SPHERE_MISS = 18   # oc 3, dot 5, length_squared 5, r^2 2, discriminant 2, cmp 1
FLOPS_SPHERE_HIT = 41    # + sqrt, root, surrounds, at(t) 6, normal 6, HitRecord 7

FLOPS_QUAD_MISS = 35     # 16 exiting at the interval check, 54 at is_interior; midpoint
FLOPS_QUAD_HIT = 61      # + at(t) 6, planar vector 3, 2 cross 18, 2 dot 10, HitRecord 7

FLOPS_PER_SCATTER = 31   # analytic random_unit_vector 10, add 3, near_zero 6, Ray::new 12

# Both asserted by crates/rt-scene/src/tests/test_layout.rs, not derived from
# alignment rules here. The primitive was assumed to be 32 and measured 48: a
# Vec3A centre stretches Sphere's 24 bytes of data to 32, and the enum tag
# rounds to 48.
BYTES_PER_NODE = 48
BYTES_PER_PRIMITIVE = 48

# B1 is a Cornell box: 18 quads and one glass sphere. B2 is all spheres.
PRIMITIVE_FLOPS = {
    "B1": (FLOPS_QUAD_MISS, FLOPS_QUAD_HIT),
    "B2": (FLOPS_SPHERE_MISS, FLOPS_SPHERE_HIT),
}

GIB = 1024 ** 3


def load_ceilings(hardware):
    path = CEILINGS / f"{hardware}.json"
    if not path.exists():
        raise SystemExit(
            f"error: no ceilings for {hardware} in {CEILINGS.relative_to(REPO)}. "
            f"Measure them with:\n  ./target/release/rt-bench ceilings"
        )
    return json.loads(path.read_text())


def load_records(hardware, tracer):
    groups = defaultdict(list)
    for line in HISTORY.open():
        if not line.strip():
            continue
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        if (r.get("hardware") or LEGACY_HARDWARE) != hardware:
            continue
        if (r.get("tracer") or LEGACY_TRACER) != tracer:
            continue
        if not all(r.get(k) for k in ("rays", "node_visits", "wall_ms")):
            continue
        if r.get("prim_hits") is None:
            continue
        groups[(r["benchmark"], r["config"])].append(r)
    return groups


def scene_sizes():
    """Primitive count per benchmark, read from the frozen scenes.

    Needed for the working set, which is what decides WHICH bandwidth ceiling
    binds. `node_visits` counts traversal work, not array size.
    """
    sizes = {}
    for manifest in sorted((REPO / "scenes" / "bench").glob("*/bench.toml")):
        ident = None
        for line in manifest.read_text().splitlines():
            if line.startswith("id"):
                ident = line.split("=")[1].strip().strip('"')
        scene = manifest.parent / "scene.json"
        if ident and scene.exists():
            sizes[ident] = len(json.loads(scene.read_text()).get("objects", []))
    return sizes


def working_set(primitives):
    """Bytes the traversal reads from, repeatedly. Leaves hold up to 4
    primitives, so a median-split tree is about 2*ceil(n/4)-1 nodes."""
    leaves = -(-primitives // 4)
    nodes = max(1, 2 * leaves - 1)
    return nodes * BYTES_PER_NODE + primitives * BYTES_PER_PRIMITIVE


def analyse(benchmark, records, tracer, primitives):
    """Median over reps, then the analytic counts. Returns one roofline point."""
    rays = st.median([r["rays"] for r in records])
    samples = st.median([r["samples"] or 0 for r in records])
    nodes = st.median([r["node_visits"] for r in records])
    prims = st.median([r["prim_tests"] or 0 for r in records])
    hits = st.median([r["prim_hits"] or 0 for r in records])
    seconds = st.median([r["wall_ms"] for r in records]) / 1000.0

    miss_flops, hit_flops = PRIMITIVE_FLOPS.get(
        benchmark, (FLOPS_SPHERE_MISS, FLOPS_SPHERE_HIT)
    )

    # A path of length L has L segments and L-1 scatters: the last one escapes
    # or is absorbed. So scatters = rays - samples, both of them recorded.
    scatters = max(0.0, rays - samples) if tracer == "path" else 0.0

    flops = (
        nodes * FLOPS_PER_AABB
        + (prims - hits) * miss_flops
        + hits * hit_flops
        + scatters * FLOPS_PER_SCATTER
    )
    byte_count = nodes * BYTES_PER_NODE + prims * BYTES_PER_PRIMITIVE

    return {
        "rays": rays,
        "nodes_per_ray": nodes / rays,
        "prims_per_ray": prims / rays,
        "hit_rate": hits / prims if prims else 0.0,
        "seconds": seconds,
        "flops": flops,
        "bytes": byte_count,
        "intensity": flops / byte_count,
        "gflops": flops / seconds / 1e9,
        "working_set": working_set(primitives),
        "primitives": primitives,
    }


def ceiling_lines(ceilings):
    """Peak compute per SIMD width, and the distinct bandwidth plateaus.

    The plateaus are found from the shape of the curve rather than from named
    cache sizes: the knees move per machine, which is why they get measured.
    """
    compute = {}
    at_max = max(ceilings["compute"], key=lambda p: p["threads"])
    compute["128-bit"] = at_max["gflops_128"]
    if at_max.get("gflops_256"):
        compute["256-bit"] = at_max["gflops_256"]

    full = sorted(
        (p for p in ceilings["bandwidth"] if p["threads"] == at_max["threads"]),
        key=lambda p: p["bytes_per_thread"],
    )

    bandwidth = {}
    if full:
        bandwidth["cache"] = max(p["gib_per_sec"] for p in full)
        bandwidth["DRAM"] = min(p["gib_per_sec"] for p in full)
        # Anything more than 1.4x above DRAM but under half of cache is the
        # shared level; take the lowest such point so the line is conservative.
        middle = [
            p["gib_per_sec"]
            for p in full
            if bandwidth["DRAM"] * 1.4 < p["gib_per_sec"] < bandwidth["cache"] * 0.5
        ]
        if middle:
            bandwidth["L3"] = min(middle)

    curve = [(p["bytes_per_thread"], p["gib_per_sec"]) for p in full]
    return compute, bandwidth, curve


def binding_bandwidth(working_set, curve):
    """The bandwidth the kernel actually sees, from the measured curve at the
    point closest to its working set. Picking the global minimum instead would
    charge DRAM bandwidth to a kernel that never leaves L2."""
    for bytes_per_thread, gib in curve:
        if bytes_per_thread >= working_set:
            return f"{bytes_per_thread // 1024} KiB", gib
    return "DRAM", curve[-1][1]


def report(points, compute, bandwidth, curve, ceilings, hardware, tracer):
    print(f"\n== roofline, {hardware} / {tracer} ==", file=sys.stderr)
    print(f"  ceilings measured {ceilings['timestamp'][:19]}  ({ceilings['cpu']}, "
          f"{ceilings['cpu_threads']} threads)", file=sys.stderr)
    print("  FLOP counts are hand-derived; the model is in this script's header",
          file=sys.stderr)

    print(f"\n  {'bench':<12}{'nod/ray':>9}{'prim/ray':>10}{'hit rate':>10}"
          f"{'GFLOP':>10}{'GB':>9}{'FLOP/B':>9}{'GFLOP/s':>10}", file=sys.stderr)
    print(f"  {'-' * 79}", file=sys.stderr)
    for label, p in points.items():
        print(f"  {label:<12}{p['nodes_per_ray']:>9.2f}{p['prims_per_ray']:>10.2f}"
              f"{100 * p['hit_rate']:>9.1f}%{p['flops'] / 1e9:>10.1f}"
              f"{p['bytes'] / 1e9:>9.1f}{p['intensity']:>9.2f}"
              f"{p['gflops']:>10.1f}", file=sys.stderr)

    print(f"\n  {'bench':<12}{'working set':>13}{'level':>10}{'BW there':>12}"
          f"{'% of that':>12}{'% of peak':>12}", file=sys.stderr)
    print(f"  {'-' * 71}", file=sys.stderr)

    peak = max(compute.values())
    for label, p in points.items():
        level, gib = binding_bandwidth(p["working_set"], curve)
        # The binding ceiling is the lower of compute and the bandwidth of the
        # level the working set actually lives in. Taking the minimum over ALL
        # bandwidth levels would pick DRAM, which this kernel never touches.
        at_intensity = gib * GIB * p["intensity"] / 1e9
        limit = min(min(compute.values()), at_intensity)
        p["limit"] = limit
        print(f"  {label:<12}{p['working_set'] / 1024:>11.1f} KiB{level:>10}"
              f"{gib:>9.0f} GiB/s{100 * p['gflops'] / limit:>11.1f}%"
              f"{100 * p['gflops'] / peak:>11.1f}%", file=sys.stderr)

    cache = bandwidth.get("cache")
    if cache:
        for name, value in sorted(compute.items()):
            print(f"\n  ridge point, {name} against cache bandwidth: "
                  f"{value / (cache * GIB / 1e9):.2f} FLOP/byte", file=sys.stderr)


def plot(points, compute, bandwidth, hardware, tracer):
    fig, ax = plt.subplots(figsize=(9, 6.5))

    intensities = [p["intensity"] for p in points.values()]
    lo = min(intensities) / 8
    hi = max(max(intensities) * 8, max(compute.values()) / (min(bandwidth.values()) * GIB / 1e9))
    x = [lo, hi]

    names = {}
    for name, gib in sorted(bandwidth.items(), key=lambda kv: -kv[1]):
        gflops = [gib * GIB * value / 1e9 for value in x]
        ax.plot(x, gflops, linestyle="--", linewidth=1.2, alpha=0.7,
                label=f"{names.get(name, name)} BW — {gib:.0f} GiB/s")

    for name, value in sorted(compute.items(), key=lambda kv: -kv[1]):
        ax.axhline(value, linestyle="-", linewidth=1.2, alpha=0.8,
                   label=f"{name} peak — {value:.0f} GFLOP/s")

    for label, p in points.items():
        # The full numbers live in the legend; the on-plot text is just the id,
        # because the two points sit close enough that longer labels collide.
        point, = ax.plot(
            p["intensity"], p["gflops"], marker="o", markersize=9, zorder=5,
            label=f"{label} — {p['intensity']:.2f} FLOP/B, {p['gflops']:.0f} GFLOP/s",
        )
        ax.axvline(p["intensity"], color=point.get_color(), linestyle=":",
                   linewidth=1.0, alpha=0.5, zorder=1)
        ax.annotate(label, (p["intensity"], p["gflops"]), fontsize=7,
                    textcoords="offset points", xytext=(9, -3), zorder=6)

    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("arithmetic intensity (FLOP / byte)")
    ax.set_ylabel("performance (GFLOP/s)")
    ax.set_title(f"Roofline — {hardware} / {tracer} — "
                 f"{datetime.now().strftime('%Y-%m-%d %H:%M')}")
    ax.grid(which="both", alpha=0.25)
    ax.legend(fontsize=7.5, loc="lower right")

    fig.text(0.01, 0.012,
             "FLOPs and bytes are hand-counted. An error of factor k moves the point "
             "PARALLEL to the diagonals, so its distance to the bandwidth ceiling is "
             "unchanged — only the distance to the compute ceiling moves. The diagonals "
             "come from a sequential read; BVH traversal chases pointers, so its real "
             "ceiling sits lower.",
             fontsize=6.5, alpha=0.7, wrap=True)
    fig.tight_layout(rect=(0, 0.05, 1, 1))

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"roofline-{datetime.now().strftime('%Y%m%d-%H%M%S')}.png"
    fig.savefig(out, dpi=130)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--hardware", default=None,
                        help="hardware generation; defaults to the newest ceilings file")
    parser.add_argument("--tracer", default="path", choices=["path", "normal"])
    parser.add_argument("--config", default=None, choices=["quick", "full"],
                        help="both by default")
    parser.add_argument("--table-only", action="store_true")
    args = parser.parse_args()

    available = sorted(CEILINGS.glob("*.json")) if CEILINGS.exists() else []
    if not available:
        raise SystemExit(
            f"error: no ceilings in {CEILINGS.relative_to(REPO)}. "
            f"Measure them with:\n  ./target/release/rt-bench ceilings"
        )

    hardware = args.hardware or max(available, key=lambda p: p.stat().st_mtime).stem
    ceilings = load_ceilings(hardware)
    groups = load_records(hardware, args.tracer)

    if args.config:
        groups = {k: v for k, v in groups.items() if k[1] == args.config}
    if not groups:
        raise SystemExit(
            f"error: no usable records for {hardware} with tracer {args.tracer}.\n"
            f"       The roofline needs node_visits, prim_tests and prim_hits, which\n"
            f"       only rt-bench records — the sweep measures `standalone`, which is\n"
            f"       not instrumented. Record a run with:\n"
            f"         ./target/release/rt-bench run --config quick --reps 3\n"
            f"         ./target/release/rt-bench run --config full  --reps 3")

    primitives = scene_sizes()
    points = {
        f"{bench} {config}": analyse(bench, records, args.tracer,
                                     primitives.get(bench, 0))
        for (bench, config), records in sorted(groups.items())
    }

    compute, bandwidth, curve = ceiling_lines(ceilings)
    if not compute or not bandwidth:
        raise SystemExit(f"error: the ceilings for {hardware} are incomplete")

    report(points, compute, bandwidth, curve, ceilings, hardware, args.tracer)
    if not args.table_only:
        print(plot(points, compute, bandwidth, hardware, args.tracer))


if __name__ == "__main__":
    main()
