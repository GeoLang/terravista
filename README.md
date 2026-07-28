# TerraVista

[![CI](https://github.com/GeoLang/terravista/actions/workflows/ci.yml/badge.svg)](https://github.com/GeoLang/terravista/actions)

**Mobile map SDK core for the GeoLang ecosystem**: camera and viewport math, gesture recognition, tile caching, offline feature storage, and on-device turn-by-turn navigation, exposed to iOS and Android over a flat C FFI.

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg)](https://www.rust-lang.org/)

> Part of the [GeoLang](https://github.com/GeoLang) geospatial platform.

---

## Overview

TerraVista is a cross-platform mobile mapping core written in Rust, designed to be consumed from Swift (iOS) and Kotlin (Android) via a flat C FFI. It holds map state and does the map math. It does not talk to the network and it does not touch the GPU.

### Status

This is v0.1. TerraVista is not yet a drop-in replacement for Mapbox or Google Maps: it
cannot fetch or draw a tile on its own.

**What works today**

- Camera and viewport: Web Mercator projection, zoom, bearing, pitch, visible bounds, tile range
- Gesture recognition: pan, pinch-zoom, rotate, tilt state machine
- Tile cache: in-memory LRU keyed by tile coordinate, with XYZ URL template building
- Offline vector store: in-memory feature CRUD with sync status tracking
- Style engine: parses Mapbox GL style JSON and interpolates properties by zoom
- Turn-by-turn navigation over a pre-computed route
- Location model: coordinates, Haversine distance, bearing, tracking modes
- Render command buffer: describes what to draw, in screen coordinates
- Tile package format: a custom TVPK binary archive
- C FFI covering map lifecycle, camera, gestures, and cache

**What the host app must supply**

- **HTTP tile fetching.** TerraVista builds tile URLs, it does not request them. There is no HTTP client in the dependency tree.
- **MVT decoding.** Nothing decodes Mapbox Vector Tiles. The cache stores opaque bytes and the renderer expects features you have already decoded.
- **GPU rendering.** The renderer emits `RenderCommand` objects. Executing them against Metal or Vulkan is the platform layer's job. No shaders ship here.
- **Routing.** The navigator tracks progress along a route you computed elsewhere, for example with [Itinera](https://github.com/GeoLang/itinera).
- **GPS.** `LocationProvider` is a trait for the platform to implement.

HTTP fetching and MVT decoding are targeted for v0.2, the rendering backends for v0.3. See the [Roadmap](#roadmap).

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                Platform Layer                                 │
│        Swift (iOS/macOS)  │  Kotlin (Android)                │
├──────────────────────────────────────────────────────────────┤
│              terravista-ffi (C ABI)                           │
│        staticlib (iOS) + cdylib (Android)                    │
├──────────────────────────────────────────────────────────────┤
│                    terravista-core                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │  Camera  │  │  Tile    │  │ Offline  │  │  Location  │  │
│  │  Model   │  │  Cache   │  │  Store   │  │  Service   │  │
│  │          │  │  (LRU)   │  │  (Sync)  │  │  (GPS)     │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Gesture  │  │ Renderer │  │  Style   │  │   Route    │  │
│  │ Recogn.  │  │ Pipeline │  │  Engine  │  │  Engine    │  │
│  │          │  │          │  │          │  │  (Nav)     │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `terravista-core` | Pure Rust map engine — camera, tiles, styles, navigation |
| `terravista-ffi` | C ABI bindings for mobile platform consumption |

---

## Features

### 🗺️ Camera & Viewport

- Continuous zoom levels 0–22 with smooth interpolation
- Bearing (rotation) and pitch (tilt) for 3D perspective
- Web Mercator projection with tile coordinate calculation
- Viewport-aware visible bounds and tile range computation

### 👆 Gesture Recognition

- Multi-touch state machine: Idle → Pan → PinchZoom → Rotate
- Touch event processing with configurable thresholds
- Camera delta output (pan pixels, zoom delta, rotation degrees, pitch delta)
- Platform-agnostic — works with any touch input system

### 📦 Offline Tile Cache

- In-memory LRU eviction with configurable max size (default 256 MB) and tile count (50,000)
- `missing_tiles` computes what a region still needs, so the host can pre-fetch it
- URL template system (`{z}/{x}/{y}` substitution), building only, the host does the request
- Tile metadata tracking (format, size, timestamps)

### 🔄 Offline Vector Store

- On-device feature CRUD with GeoJSON geometry
- Sync status tracking: `Synced`, `PendingCreate`, `PendingUpdate`, `PendingDelete`, `Conflict`
- Bounding-box spatial queries
- GeoJSON export for sync with remote servers

### 🎨 Style Engine

- Mapbox GL JSON-compatible style definitions
- Zoom-level interpolated properties (colors, widths, opacity)
- Layer types: Fill, Line, Symbol, Circle, Raster
- Source definitions with tile URL templates and zoom ranges

### 🧭 Turn-by-Turn Navigation

- On-device route tracking (no cloud dependency)
- Step-by-step maneuver instructions
- Off-route detection (configurable threshold, default 50m)
- Distance-to-next-step and total distance remaining
- Arrival detection
- Maneuver types: Depart, Turn L/R, Slight L/R, Sharp L/R, U-Turn, Merge, Ramp, Roundabout, Arrive

### 📍 Location Service

- GPS coordinate model with altitude, accuracy, speed, course
- Haversine distance and bearing calculations
- Tracking modes: None, Follow, FollowWithHeading, FollowWithCourse
- Abstract `LocationProvider` trait for platform implementation

### 🖼️ Render Pipeline

- Frame-based command buffer: Clear, DrawRasterTile, DrawVectorLayer, DrawLocationMarker, DrawRoute
- Visible tile calculation with screen-space placement
- Device pixel ratio awareness for Retina/HiDPI displays
- Describes the frame, it does not draw it. The platform layer executes the commands against Metal (iOS) or Vulkan (Android). Those backends are v0.3.

### 📦 Offline Tile Packages

- Custom TVPK binary archive format for fully disconnected use, not MBTiles or SQLite
- `PackageDefinition` with bounding box, zoom range, tile source URL
- Tile count estimation before download
- Serialize and deserialize a package, with a magic-byte header and bbox validation
- Region-based tile enumeration across zoom levels
- Held in memory, and populated by the host since there is no downloader here

---

## Building

### Prerequisites

- Rust 1.85+ (2024 edition)
- For iOS: Xcode + `aarch64-apple-ios` target
- For Android: Android NDK + `aarch64-linux-android` target

### Development

```bash
# Build all crates
cargo build

# Run tests (58 tests: 35 unit, 23 integration)
cargo test

# Lint
cargo clippy --all-targets -- -D warnings

# Format
cargo fmt --all
```

### iOS (Static Library)

```bash
rustup target add aarch64-apple-ios
cargo build --target aarch64-apple-ios -p terravista-ffi --release

# Output: target/aarch64-apple-ios/release/libterravista_ffi.a
```

### Android (Shared Library)

```bash
rustup target add aarch64-linux-android
# Requires ANDROID_NDK_HOME set and a cargo config for the linker
cargo build --target aarch64-linux-android -p terravista-ffi --release

# Output: target/aarch64-linux-android/release/libterravista_ffi.so
```

### All Mobile Targets

```bash
# iOS (arm64 + simulator)
cargo build --target aarch64-apple-ios -p terravista-ffi --release
cargo build --target aarch64-apple-ios-sim -p terravista-ffi --release

# Android (arm64, armv7, x86_64)
cargo build --target aarch64-linux-android -p terravista-ffi --release
cargo build --target armv7-linux-androideabi -p terravista-ffi --release
cargo build --target x86_64-linux-android -p terravista-ffi --release
```

---

## FFI API Reference

All functions use the `tv_` prefix and follow C naming conventions. Opaque pointers must be freed with their corresponding `_destroy` function.

The FFI covers map state and camera math. There is no `tv_` call that fetches or draws a tile, because neither exists yet.

### Map Lifecycle

```c
// Create a map state
TvMapState* tv_map_create(uint32_t width, uint32_t height, float device_pixel_ratio);

// Destroy a map state
void tv_map_destroy(TvMapState* state);
```

### Camera Control

```c
void tv_map_set_center(TvMapState* state, double latitude, double longitude);
void tv_map_set_zoom(TvMapState* state, double zoom);       // 0.0–22.0
void tv_map_set_bearing(TvMapState* state, double bearing); // 0–360°
void tv_map_set_pitch(TvMapState* state, double pitch);     // 0–60°
void tv_map_set_viewport(TvMapState* state, uint32_t width, uint32_t height, float dpr);

double tv_map_get_zoom(const TvMapState* state);
double tv_map_get_center_lat(const TvMapState* state);
double tv_map_get_center_lon(const TvMapState* state);
```

### Gesture Input

```c
void tv_map_pan(TvMapState* state, double dx, double dy);
void tv_map_zoom_by(TvMapState* state, double delta);
```

### Tile Cache

```c
void tv_map_set_tile_url(TvMapState* state, const char* url_template);
uint32_t tv_cache_tile_count(const TvMapState* state);
uint64_t tv_cache_size_bytes(const TvMapState* state);
void tv_cache_clear(TvMapState* state);
```

### Utility

```c
char* tv_version(void);        // Returns SDK version (caller must free)
void tv_string_free(char* ptr); // Free SDK-allocated strings
```

---

## Platform Integration

These examples wire up camera and gestures, which is everything the SDK does today.
Fetching the tiles at the configured URL and drawing them is still the app's job.

### Swift (iOS)

```swift
import TerraVista

class MapViewController: UIViewController {
    private var mapState: OpaquePointer?

    override func viewDidLoad() {
        super.viewDidLoad()
        let bounds = view.bounds
        let scale = Float(UIScreen.main.scale)
        mapState = tv_map_create(UInt32(bounds.width), UInt32(bounds.height), scale)
        tv_map_set_center(mapState, 51.5074, -0.1278)
        tv_map_set_zoom(mapState, 14.0)
        tv_map_set_tile_url(mapState, "https://tiles.tiletopia.dev/{z}/{x}/{y}.mvt")
    }

    @objc func handlePan(_ gesture: UIPanGestureRecognizer) {
        let translation = gesture.translation(in: view)
        tv_map_pan(mapState, Double(translation.x), Double(translation.y))
        gesture.setTranslation(.zero, in: view)
    }

    @objc func handlePinch(_ gesture: UIPinchGestureRecognizer) {
        let delta = log2(Double(gesture.scale))
        tv_map_zoom_by(mapState, delta)
        gesture.scale = 1.0
    }

    deinit {
        tv_map_destroy(mapState)
    }
}
```

### Kotlin (Android)

```kotlin
class MapView(context: Context) : View(context) {
    private var mapState: Long = 0

    init {
        System.loadLibrary("terravista_ffi")
        mapState = tvMapCreate(width.toUInt(), height.toUInt(), resources.displayMetrics.density)
        tvMapSetCenter(mapState, 51.5074, -0.1278)
        tvMapSetZoom(mapState, 14.0)
        tvMapSetTileUrl(mapState, "https://tiles.tiletopia.dev/{z}/{x}/{y}.mvt")
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        // Forward to gesture detector
        when (event.action) {
            MotionEvent.ACTION_MOVE -> {
                tvMapPan(mapState, event.x.toDouble(), event.y.toDouble())
            }
        }
        return true
    }

    fun destroy() {
        tvMapDestroy(mapState)
    }

    // JNI bindings
    private external fun tvMapCreate(w: UInt, h: UInt, dpr: Float): Long
    private external fun tvMapDestroy(state: Long)
    private external fun tvMapSetCenter(state: Long, lat: Double, lon: Double)
    private external fun tvMapSetZoom(state: Long, zoom: Double)
    private external fun tvMapSetTileUrl(state: Long, url: String)
    private external fun tvMapPan(state: Long, dx: Double, dy: Double)
}
```

---

## Roadmap

- [ ] **v0.2** — HTTP tile fetching (async runtime integration)
- [ ] **v0.2** — MVT (Mapbox Vector Tile) decoding
- [ ] **v0.3** — Metal rendering backend (iOS)
- [ ] **v0.3** — Vulkan rendering backend (Android)
- [ ] **v0.4** — Annotation layers (markers, polylines, polygons)
- [ ] **v0.4** — Clustering for point features
- [ ] **v0.5** — 3D terrain mesh from DEM tiles
- [ ] **v0.5** — Globe view (non-Mercator) for low zoom levels
- [ ] **v0.6** — Swift Package Manager distribution
- [ ] **v0.6** — Maven/Gradle distribution for Android
- [ ] **v1.0** — Stable C ABI with semantic versioning guarantees

---

## Related GeoLang Projects

| Project | Description |
|---------|-------------|
| [TileTopia](https://github.com/GeoLang/tiletopia) | 3D Tiles server |
| [ViewTopia](https://github.com/GeoLang/viewtopia) | Web map viewer |
| [Itinera](https://github.com/GeoLang/itinera) | Routing engine |
| [GeoKode](https://github.com/GeoLang/geokode) | Geocoding service |
| [Nubis](https://github.com/GeoLang/nubis) | Point cloud processing |
| [Terrano](https://github.com/GeoLang/terrano) | Raster algebra and terrain analysis |
| [Ptolemy](https://github.com/GeoLang/ptolemy) | Versioned geodatabase platform |
| [GeoDukt](https://github.com/GeoLang/geodukt) | Data pipeline/ETL |
| [GeoGit](https://github.com/GeoLang/geogit) | Versioned geodata |
| [Jung](https://github.com/GeoLang/jung) | Symbology and cartographic rendering |
| [Fluvius](https://github.com/GeoLang/fluvius) | Real-time streaming |
| [Panoptes](https://github.com/GeoLang/panoptes) | AI feature extraction from imagery |
| [Projicio](https://github.com/GeoLang/projicio) | CRS/projection library |
| [Topoi](https://github.com/GeoLang/topoi) | Computational geometry |
| [Fenestra](https://github.com/GeoLang/fenestra) | OGC services gateway |

---

## License

AGPL-3.0-or-later, see [LICENSE](LICENSE).

Copyright (C) 2026 Grok Image Compression Inc.
