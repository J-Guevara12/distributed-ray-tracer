# `rt-bench` CLI guide

`rt-bench` measures the current build of the renderer against a fixed benchmark
suite and appends the results to `bench/history.jsonl`.

## What it is and is not

There are two measurement drivers in this repo, and they do different jobs:

| | `rt-bench` | `scripts/bench_sweep.py` |
|---|---|---|
| Measures | the current working tree, going forward | past commits, retroactively |
| How | links the renderer directly | builds `standalone` in a git worktree and parses its stdout |
| Can report | wall time, BVH build time, (later) rays/s and traversal stats | wall time only |
| Lives | in the repo, evolves with the code | outside the measured code, version-agnostic |

The sweep cannot use `rt-bench` because `rt-bench` does not exist at the older
commits. Both write to the same `bench/history.jsonl` with the same schema, so
records from either driver can be read together — tell them apart by
`env.driver`.

## Before you run

Three things, all enforced by the tool:

1. **Build in release.** A debug build would be 10–50× slower and would poison
   the history file. `rt-bench` refuses to run when built with
   `debug_assertions`.
2. **Commit your work.** A measurement taken from uncommitted code cannot be
   attributed to a commit. Override with `--allow-dirty` when you are just
   iterating; the record is flagged with `env.dirty: true`.
3. **Run from the repo root.** `--base-dir` and `--out` are resolved against the
   current working directory.

```bash
cargo build --release -p rt-bench
```

---

## `list`

Prints the benchmark suite discovered under `--base-dir`. Each benchmark is a
directory containing `bench.toml`, `scene.json` and `camera.json`.

```
Usage: rt-bench list [OPTIONS]

  -b, --base-dir <BASE_DIR>    [default: ./scenes/bench]
  -f, --file-name <FILE_NAME>  [default: bench.toml]
  -v, --verbose                include the notes from bench.toml
      --format <FORMAT>        text | table | json   [default: text]
```

`--verbose` only affects `text`; `table` and `json` ignore it.

`text` and `table` show the object and material counts read from `scene.json`,
and the resolution with the height resolved against the camera's aspect ratio.
`json` currently emits the manifest as-is:

```json
[
  {
    "path": "./scenes/bench/cornell/bench.toml",
    "manifest": {
      "id": "B1",
      "name": "cornell-glass",
      "notes": "...",
      "quick": { "width": 1024, "spp": 8 },
      "full":  { "width": 1024, "spp": 200 }
    }
  }
]
```

> Not yet implemented: object/material counts in the JSON output, and omitting
> `notes` unless `--verbose` is passed. Both need a dedicated serializable view
> struct, since `skip_serializing_if` cannot see a runtime flag.

### Examples

```bash
# Default: one block per benchmark
rt-bench list

# Same, plus the notes explaining what each benchmark stresses
rt-bench list --verbose

# Compact overview of the whole suite
rt-bench list --format table

# Machine-readable, for scripts
rt-bench list --format json | jq '.[] | {id: .manifest.id, full: .manifest.full}'
```

Colors are emitted only when stdout is a terminal, and are suppressed when
`NO_COLOR` is set — so `rt-bench list > file.txt` stays clean.

---

## `run`

Measures each selected benchmark and reports median, relative standard
deviation and sample count.

```
Usage: rt-bench run [OPTIONS]

  -b, --base-dir <BASE_DIR>    [default: ./scenes/bench]
  -f, --file-name <FILE_NAME>  [default: bench.toml]
      --config <CONFIG>        quick | full            [default: quick]
      --only <ONLY>...         benchmarks by id or name  [default: all]
      --reps <REPS>            recorded repetitions    [default: 5]
      --cooldown <COOLDOWN>    seconds between runs    [default: 20]
      --label <LABEL>          [default: short HEAD sha, or "workdir" if dirty]
      --max-depth <MAX_DEPTH>  [default: 15]
      --tile-size <TILE_SIZE>  [default: 128]
      --out <OUT>              [default: ./bench/history.jsonl]
      --no-record              measure and print without writing
      --allow-dirty            allow measuring an uncommitted tree
```

### Options that need care

**`--max-depth` and `--tile-size`.** The defaults (15 and 128) are the values
hardcoded in `standalone`, which is what the historical sweep measured.
Changing them produces numbers that cannot be compared against the 96 existing
historical records. Change them deliberately or not at all.

**`--cooldown`.** This is a laptop with hybrid P/E cores that throttles under
sustained load. Dropping the cooldown from 20 s to 5 s raised B2's relative
standard deviation from 3.6% to 6.5%. The default earns its keep.

**`--config`.** `quick` and `full` come from each benchmark's `bench.toml`, not
from `camera.json`. The manifest is the single source of truth for the
workload: `rt-bench` loads `camera.json` for the framing and then overwrites
`image_width` and `samples_per_pixel`. That is what makes the scene files
immutable — editing `camera.json` cannot silently change a measurement.

### Measurement protocol

Applied automatically:

- One **warmup run per benchmark**, discarded.
- Reps are **interleaved** across benchmarks (`B1 B2 B1 B2 …`), not grouped. On
  a laptop the chip heats up during a run; grouping would make whichever
  benchmark ran last look slower for thermal reasons rather than code reasons.
- A cooldown before every measured run.
- The BVH is **rebuilt on every rep** and timed separately. Building once and
  reusing it would give tighter numbers but would hide real variance: the split
  axis is currently chosen with `fastrand`, seeded per process, so every run
  builds a structurally different tree.
- The render timer excludes scene parsing and BVH construction, matching what
  `standalone` measures.

### Reading the summary

```
== summary ==
  ID    name                resolution    spp       build      render     rsd    n
  --------------------------------------------------------------------------------
  B1    cornell-glass        1024x1024      8     0.01 ms     1152 ms    9.3%    3
  B2    rtow-spheres         1920x1080     20     0.58 ms     3493 ms    6.5%    3
```

`rsd` is the **relative standard deviation** (stdev / median), not the min–max
range. The range of 3 samples is systematically smaller than the range of 5, so
a range-based figure cannot be compared across runs with different `--reps`.

An `rsd` above roughly 10% means the run cannot resolve differences smaller
than that — which is currently the case for benchmarks that use the BVH,
because the tree is rebuilt at random each process. Until that is fixed, treat
sub-10% improvements as unmeasured rather than absent.

### Examples

```bash
# Standard run: quick config, whole suite, recorded. ~4 min.
rt-bench run

# Figures for the report. ~10 min.
rt-bench run --config full

# While iterating on an optimization: fast, and does not touch the history
rt-bench run --only B2 --reps 3 --no-record

# Several benchmarks — by id or name, space- or comma-separated
rt-bench run --only B1 B2
rt-bench run --only B1,B2
rt-bench run --only rtow-spheres

# Measuring uncommitted work; the record is flagged env.dirty = true
rt-bench run --only B1 --reps 3 --allow-dirty

# Tag a measurement with something meaningful instead of the sha
rt-bench run --config full --label before-flat-bvh

# Noisy machine: more reps and a longer cooldown
rt-bench run --config full --reps 9 --cooldown 45

# Scratch file instead of the shared history
rt-bench run --out /tmp/experiment.jsonl
```

---

## Output schema

One JSON object per repetition, appended to `--out`. Field names and order
match `scripts/bench_sweep.py`: both drivers write to the same file, so the
schema admits **new** fields only, never renamed or repurposed ones.

| Field | Notes |
|---|---|
| `benchmark`, `config` | e.g. `"B2"`, `"quick"` |
| `width`, `spp` | the actual workload. The config *name* is not enough — B2's `quick` was 640/64 before 2026-08-13 and 1920/20 after |
| `commit`, `commit_label`, `commit_subject`, `commit_date` | provenance |
| `profile` | `"optimized"` |
| `rep`, `wall_ms` | render time only |
| `build_ms` | scene + BVH construction. `rt-bench` only — the sweep cannot measure it |
| `rays`, `rays_per_sec`, `node_visits`, `prim_tests`, `image_hash`, `mse` | explicit `null` until F0.3b / F0.8 / F0.3c land |
| `cpu_mhz` | best effort; `null` inside a VM where cpufreq is not exposed |
| `timestamp` | RFC 3339, UTC |
| `env` | rustc, cpu, thread count, platform, scene hashes, dirty flag, driver, `max_depth`, `tile_size` |

`env.scene_hashes` holds a truncated sha256 of `scene.json`, `camera.json` and
`bench.toml` per benchmark, computed identically to the Python driver. If a
scene is ever edited, its hash changes and older records become visibly
incomparable without anyone having to police it.

---

## The benchmark suite

| | Name | Workload (quick / full) | Isolates |
|---|---|---|---|
| **B1** | `cornell-glass` | 1024² @ 8 / 200 spp | **Light transport.** Closed 555³ Cornell box, small light, two rotated boxes, one glass sphere. Only 19 objects, so BVH cost is negligible and the number reflects the integrator and sampler almost in isolation. Rays run to `max_depth` because almost none escape. |
| **B2** | `rtow-spheres` | 1920×1080 @ 20 / 250 spp | **Geometry and incoherent rays.** 521 spheres, depth of field. The scene that links back to the commit history — it must never change. |

Adding a benchmark is `mkdir scenes/bench/<name>/` plus `bench.toml`,
`scene.json` and `camera.json`. Both drivers discover it by glob; no code
change needed.

B1's geometry is generated by `scripts/gen_cornell.py`, which also verifies
that nothing intersects — with the boxes rotated, that is no longer obvious by
eye.
