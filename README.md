# TerraVista

**Mobile map SDK for the TileTopia ecosystem** — offline-first tile caching, GPU-accelerated vector rendering, gesture-driven navigation, and turn-by-turn routing for iOS and Android.

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg)](https://www.rust-lang.org/)

> Part of the [TileTopia](https://github.com/TileTopia-HQ) geospatial platform.

---

## Overview

TerraVista is a cross-platform mobile mapping engine written in Rust, designed to be consumed from Swift (iOS) and Kotlin (Android) via a flat C FFI. It provides everything needed to build a native mobile map experience without depending on proprietary SDKs like Mapbox or Google Maps.

### Why TerraVista?

| Feature | TerraVista | Mapbox SDK | Google Maps SDK |
|---------|-----------|------------|-----------------|
| Open source | ✅ AGPL-3.0 | Partial | ❌ |
| Offline-first | ✅ Built-in | Add-on | Limited |
| Custom tile sources | ✅ Any XYZ/MVT | Mapbox only | Google only |
| Binary size | ~2 MB | ~30 MB | ~20 MB |
| No API key required | ✅ | ❌ | ❌ |
| Self-hosted tiles | ✅ | ❌ | ❌ |
| Turn-by-turn nav | ✅ On-device | Cloud-dependent | Cloud-dependent |

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

- LRU eviction with configurable max size (default 256 MB) and tile count (50,000)
- Per-region pre-fetch for offline areas
- URL template system (`{z}/{x}/{y}` substitution)
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
- Platform GPU backend integration point (Metal on iOS, Vulkan on Android)

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

# Run tests (25 unit tests)
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

## Related TileTopia Projects

| Project | Description |
|---------|-------------|
| [TileTopia](https://github.com/TileTopia-HQ/tiletopia) | Vector tile server |
| [ViewTopia](https://github.com/TileTopia-HQ/viewtopia) | Web map viewer |
| [Itinera](https://github.com/TileTopia-HQ/itinera) | Routing engine |
| [GeoKode](https://github.com/TileTopia-HQ/geokode) | Geocoding service |
| [Nubis](https://github.com/TileTopia-HQ/nubis) | Cloud infrastructure |
| [Terrano](https://github.com/TileTopia-HQ/terrano) | Terrain/elevation service |
| [Ptolemy](https://github.com/TileTopia-HQ/ptolemy) | Cartographic styling |
| [GeoDukt](https://github.com/TileTopia-HQ/geodukt) | Data pipeline/ETL |
| [GeoGit](https://github.com/TileTopia-HQ/geogit) | Versioned geodata |
| [Jung](https://github.com/TileTopia-HQ/jung) | GPU rendering engine |
| [Fluvius](https://github.com/TileTopia-HQ/fluvius) | Real-time streaming |
| [Panoptes](https://github.com/TileTopia-HQ/panoptes) | Monitoring/observability |
| [Projicio](https://github.com/TileTopia-HQ/projicio) | CRS/projection library |
| [Topoi](https://github.com/TileTopia-HQ/topoi) | Topology engine |
| [Fenestra](https://github.com/TileTopia-HQ/fenestra) | WMS/WMTS server |

---

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE) or later.

Copyright © 2024 TileTopia
