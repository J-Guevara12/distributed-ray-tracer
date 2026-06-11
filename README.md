# Raytracer

A CPU path tracer written in Rust, featuring a tile-based parallel renderer exposed as an HTTP API with real-time streaming visualization.

![Example render](scenes/result.png)

## Features

- **Path tracing** — recursive Monte Carlo integration with configurable ray depth
- **Materials** — Lambertian diffuse, Metal (with roughness), and Dielectric (refraction + Fresnel)
- **Camera** — configurable FOV, position/orientation, depth of field, and multi-sample anti-aliasing
- **Parallel rendering** — tile-based CPU parallelization via Rayon
- **Live preview** — tiles streamed to the browser in real time via Server-Sent Events (SSE)
- **HTTP API** — load scenes, configure the camera, and start renders over REST
- **PNG export** — save the finished framebuffer to disk

## Project Structure

This is a Cargo workspace with five crates:

| Crate | Role |
|-------|------|
| `rt-core` | Shared types: `Vec3`, `Color`, `Ray`, `Camera`, `Job`, DTOs |
| `rt-scene` | Geometry (`Sphere`), materials, `Hittable` / `Material` traits |
| `rt-renderer` | Tile renderer, path tracer, normal tracer, framebuffer, PNG export |
| `rt-server` | Axum HTTP server — scene/camera/render endpoints + SSE stream |
| `rt-worker` | Distributed worker skeleton (work in progress) |

## Getting Started

### Prerequisites

- Rust toolchain (edition 2024, stable or nightly)
- Python 3 with `requests` (only needed for the scene generator script)

### Build

```bash
cargo build --release
```

### Run the server

```bash
cargo run -p rt-server
# Listening on http://127.0.1.1:3000
```

### Generate a scene and render

In a second terminal, run the bundled Python script to POST a procedurally generated scene (484 spheres) and kick off the render:

```bash
cd scripts
python generate_scene.py
```

Open `index.html` in a browser to watch tiles arrive in real time.

## API

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Liveness check |
| `GET` | `/camera` | Current camera configuration |
| `PUT` | `/camera` | Update camera configuration |
| `GET` | `/scene` | Current scene (materials + objects) |
| `POST` | `/scene` | Load a new scene |
| `POST` | `/render` | Start rendering (`tile_size`, `max_depth` params) |
| `GET` | `/render/stream` | SSE stream of rendered tiles |

### Example: load a scene

```bash
curl -X POST http://127.0.1.1:3000/scene \
  -H 'Content-Type: application/json' \
  -d @scenes/spheres_scene.json
```

### Example: start a render

```bash
curl -X POST http://127.0.1.1:3000/render \
  -H 'Content-Type: application/json' \
  -d '{"tile_size": 64, "max_depth": 50}'
```

## Scene Format

Scenes are JSON documents with a `materials` map and an `objects` list:

```json
{
  "materials": {
    "ground": { "type": "Lambertian", "albedo": [0.5, 0.5, 0.5] },
    "mirror":  { "type": "Metal",      "albedo": [0.8, 0.8, 0.8], "fuzz": 0.0 },
    "glass":   { "type": "Dielectric", "refraction_index": 1.5 }
  },
  "objects": [
    { "type": "Sphere", "center": [0, -1000, 0], "radius": 1000, "material": "ground" },
    { "type": "Sphere", "center": [0,  1,    0], "radius": 1,    "material": "glass"  }
  ]
}
```

Camera configuration is a separate JSON document sent to `PUT /camera`.

## Architecture Notes

- The renderer runs on a dedicated thread pool (Rayon). Each tile is processed independently and its result is broadcast over a Tokio channel.
- The SSE handler subscribes to that broadcast channel and forwards tiles as JSON events.
- `AppState` uses `Arc<RwLock<_>>` for scene/camera and an `Arc<FrameBuffer>` (internally `Arc<RwLock<Vec<u8>>>`) for the pixel buffer.
- The `rt-worker` crate is a placeholder for future distributed rendering over a job queue.
