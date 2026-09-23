# TerraVista

[![CI](https://github.com/GeoLang/terravista/actions/workflows/ci.yml/badge.svg)](https://github.com/GeoLang/terravista/actions)

**Mobile map SDK for Android**: a Rust core for camera math, gestures, tile caching, MVT decoding and turn-by-turn navigation, behind a flat C FFI and a Kotlin `MapView`.

[![License: AGPL-3.0](https://img.shields.io/badge/License-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_edition-orange.svg)](https://www.rust-lang.org/)

> Part of the [GeoLang](https://github.com/GeoLang) geospatial platform.

---

## Overview

The Rust core holds map state and does the map math. It has no HTTP client and no GPU code. The Kotlin library in [`android/`](android/README.md) fetches tiles over HTTP and draws raster and MVT tiles on Canvas. Android is the only platform: there is no iOS or macOS binding.

### Status

Version 0.4.0.

**What works today**

- Camera and viewport: Web Mercator projection, zoom, bearing, pitch, visible bounds, tile range
- Gesture recognition: pan, and a two-finger pinch that zooms and rotates together
- Tile cache: in-memory LRU keyed by tile coordinate, with XYZ URL template building
- MVT decoding: layers, features, geometry and attributes, straight to screen-space draw commands
- Offline regions: tile count, size estimate and tile list for a bounding box
- Turn-by-turn navigation over a pre-computed route
- User location with camera tracking modes
- Render command buffer: describes what to draw, in screen coordinates
- C FFI covering map lifecycle, camera, gestures, raster and vector caches, per-layer styling, offline regions, projection, user location and navigation
- Kotlin `MapView` on JitPack, with a disk tile cache and saved offline regions

The offline vector store and TVPK tile packages are Rust API only. No `tv_` function reaches them.

**What the host app must supply**

- **HTTP tile fetching.** The core builds tile URLs, it does not request them. The Android library does the requests.
- **Drawing.** The core emits draw commands and screen placements. The Android library draws them on Canvas.
- **Routing.** The navigator tracks progress along a route computed elsewhere, for example by [Itinera](https://github.com/GeoLang/itinera).
- **GPS.** The host feeds every location fix in. `LocationProvider` is a trait for the platform to implement.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                Platform Layer                                 │
│        Kotlin (Android), no iOS binding                      │
├──────────────────────────────────────────────────────────────┤
│              terravista-ffi (C ABI)                           │
│        cdylib (Android) + staticlib                          │
├──────────────────────────────────────────────────────────────┤
│                    terravista-core                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │  Camera  │  │  Tile    │  │ Offline  │  │  Location  │  │
│  │  Model   │  │  Cache   │  │  Store   │  │  Service   │  │
│  │          │  │  (LRU)   │  │  (Sync)  │  │  (GPS)     │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌────────────┐  │
│  │ Gesture  │  │ Renderer │  │   MVT    │  │   Route    │  │
│  │Recognizer│  │ Pipeline │  │  Decode  │  │  Engine    │  │
│  │          │  │          │  │          │  │  (Nav)     │  │
│  └──────────┘  └──────────┘  └──────────┘  └────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `terravista-core` | Rust map engine: camera, tiles, MVT decoding, navigation |
| `terravista-ffi` | C ABI over `terravista-core`, built as a cdylib and a staticlib |
| `android/` | Kotlin library and sample app, built with Gradle, see [`android/README.md`](android/README.md) |

---

## Features

### Camera and viewport

- Continuous zoom from 0 to 22
- Bearing (rotation) and pitch (tilt, clamped to 0 to 60 degrees)
- Web Mercator projection with tile coordinate calculation
- Visible bounds and tile range for the viewport
- Tile zoom is `round(zoom + log2(device pixel ratio))`, so one 256 px tile covers 256 device pixels

### Gesture recognition

- Multi-touch state machine: Idle, Pan, PinchZoom
- A two-finger gesture zooms and rotates at once, with a 5 degree dead zone before rotation starts
- Zoom is anchored, so the point between the fingers stays put
- Takes plain touch points in device pixels, from any platform

No gesture sets pitch, and nothing draws a pitched map: that needs a perspective
transform `TilePlacement` cannot express.

### Tile cache

- In-memory LRU with a size cap (default 256 MB) and a tile count cap (default 50,000)
- `missing_tiles` lists what a region still needs, so the host can pre-fetch it
- XYZ URL templates (`{z}/{x}/{y}` substitution). The host makes the request
- Per-tile content type, size and fetch time

### Vector tiles

- MVT spec v2 decoding by [`jung-mvt`](https://github.com/GeoLang/jung): layers, features, points, lines, polygons and attributes
- Ring winding decides holes, so a multipolygon keeps its parts
- Coordinates stay in tile units, so placing a tile is a scale and a translate
- A fixed default look per layer name, no style spec, no labels and no fonts
- Vector tiles cache and draw alongside raster ones, from their own URL template

### Offline vector store (Rust API only)

- In-memory feature CRUD with GeoJSON geometry
- Sync status per feature: `Synced`, `PendingCreate`, `PendingUpdate`, `PendingDelete`, `Conflict`
- Bounding-box queries
- `export_geojson` writes a FeatureCollection. Sending it to a server is the host's job

### Turn-by-turn navigation

- Tracks progress along a route computed elsewhere, with no network calls
- Step instructions, distance to the next step and total distance remaining
- Off-route when a fix is more than 50 m from the route. The threshold is fixed
- Arrived when on the last step and within 20 m of the end
- The core `Maneuver` enum has Depart, Turn, Slight and Sharp left and right, UTurn, Straight, Merge, RampLeft, RampRight, Roundabout and Arrive. The C ABI carries instruction text only

### Location

- Location fix model with altitude, accuracy, speed and heading
- Haversine distance and initial bearing
- Tracking modes: None, Follow, FollowWithHeading, FollowWithCourse
- `LocationProvider` trait for the platform to implement

### Render pipeline

- Frame command buffer: Clear, DrawRasterTile, DrawVectorLayer, DrawLocationMarker, DrawRoute
- Visible tiles with their screen placement, in device pixels
- Describes the frame, it does not draw it. The Android library draws on Canvas. No GPU backend is built

### Offline tile packages (Rust API only)

- TVPK, a custom binary archive with a magic-byte header and MBTiles-style metadata keys. It is not MBTiles or SQLite
- `PackageDefinition` pairs an `OfflineRegion` with a tile format (png, jpg, webp, pbf)
- Tile enumeration and size estimation come from `OfflineRegion`, the same type the `tv_region_*` calls use
- Held in memory and filled by the host. There is no downloader in the core

---

## Building

### Prerequisites

- Rust 1.85+ (2024 edition)
- For Android: Android NDK + `aarch64-linux-android` target
- For the Kotlin library and the sample app: JDK 17 or later, and see
  [`android/README.md`](android/README.md)

### Development

```bash
# Build all crates
cargo build

# Run tests
cargo test --all

# Lint, as CI runs it
cargo clippy --all-targets --all-features -- -D warnings

# Format
cargo fmt --all
```

### Android (Shared Library)

```bash
rustup target add aarch64-linux-android
# Requires ANDROID_NDK_HOME set and a cargo config for the linker
cargo build --target aarch64-linux-android -p terravista-ffi --release

# Output: target/aarch64-linux-android/release/libterravista_ffi.so
```

### All Android ABIs

```bash
cargo build --target aarch64-linux-android -p terravista-ffi --release
cargo build --target armv7-linux-androideabi -p terravista-ffi --release
cargo build --target x86_64-linux-android -p terravista-ffi --release
```

The published Kotlin library ships `arm64-v8a` and `x86_64` only, built by
`android/tools/build-natives.sh` and committed under
`android/terravista/src/main/jniLibs`.

---

## FFI API Reference

All functions use the `tv_` prefix. Free a map with `tv_map_destroy` and every
returned `char*` with `tv_string_free`. There is no generated C header: the
declarations below are hand-written, and the JNI glue declares its externs the
same way.

No `tv_` call fetches a tile or draws one. The host does both, from the URLs and
the placements the SDK gives it.

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
void tv_map_set_zoom(TvMapState* state, double zoom);       // clamped to 0 to 22
void tv_map_set_bearing(TvMapState* state, double bearing); // degrees, taken mod 360
void tv_map_set_pitch(TvMapState* state, double pitch);     // clamped to 0 to 60
void tv_map_set_viewport(TvMapState* state, uint32_t width, uint32_t height, float dpr);

double tv_map_get_zoom(const TvMapState* state);
double tv_map_get_center_lat(const TvMapState* state);
double tv_map_get_center_lon(const TvMapState* state);
double tv_map_get_bearing(const TvMapState* state);
double tv_map_get_pitch(const TvMapState* state);
```

A worked example that drives all of this from an app is in
[`examples/android-testapp`](examples/android-testapp).

### Gesture Input

```c
void tv_map_pan(TvMapState* state, double dx, double dy);
void tv_map_zoom_by(TvMapState* state, double delta);

// Feed raw touches through the recognizer. Returns the TV_GESTURE_* applied.
int32_t tv_map_touch(TvMapState* state, int32_t phase, const double* xs,
                     const double* ys, const uint64_t* ids, size_t count);
```

`phase` is one of `TV_TOUCH_BEGIN`, `_MOVE`, `_END`, `_CANCEL`. The return value is a
`TV_GESTURE_*` constant. The recognizer only produces `_NONE`, `_PAN` and `_PINCH`,
and `_ZOOM`, `_ROTATE` and `_PITCH` are defined but never returned.

### Visible Tiles

```c
typedef struct { uint8_t zoom; uint32_t x_min, x_max, y_min, y_max; } TvTileRange;
typedef struct {
    uint8_t z; uint32_t x, y;
    float screen_x, screen_y, size;  // device pixels
} TvTilePlacement;

bool tv_map_tile_range(const TvMapState* state, TvTileRange* out);

// Recompute the frame's tile set, then read placements back by index.
uint32_t tv_map_visible_tile_count(TvMapState* state);
bool tv_map_visible_tile_at(const TvMapState* state, uint32_t i, TvTilePlacement* out);
```

Placements are north-up. When `bearing` is non-zero the host rotates its canvas by
`-bearing` about the viewport centre, and the tile range already covers the corners
the rotation exposes.

### Tile Cache

```c
void tv_map_set_tile_url(TvMapState* state, const char* url_template);
char* tv_map_tile_url(const TvMapState* state, uint8_t z, uint32_t x, uint32_t y);

bool tv_cache_put(TvMapState* state, uint8_t z, uint32_t x, uint32_t y,
                  const uint8_t* bytes, size_t len, const char* content_type);
bool tv_cache_has(const TvMapState* state, uint8_t z, uint32_t x, uint32_t y);
// Returns the tile's full length, copying at most `cap` bytes. 0 when absent.
size_t tv_cache_get(TvMapState* state, uint8_t z, uint32_t x, uint32_t y,
                    uint8_t* out, size_t cap);

uint32_t tv_cache_tile_count(const TvMapState* state);
uint64_t tv_cache_size_bytes(const TvMapState* state);
void tv_cache_clear(TvMapState* state);
```

### Vector Tiles

```c
void tv_map_set_vector_tile_url(TvMapState* state, const char* url_template);
char* tv_map_vector_tile_url(const TvMapState* state, uint8_t z, uint32_t x, uint32_t y);

// Decodes on the way in, so false means the bytes were not a vector tile.
bool tv_vector_cache_put(TvMapState* state, uint8_t z, uint32_t x, uint32_t y,
                         const uint8_t* bytes, size_t len);
bool tv_vector_cache_has(const TvMapState* state, uint8_t z, uint32_t x, uint32_t y);
void tv_vector_cache_clear(TvMapState* state);

// How one layer draws, over the built-in look. Alpha 0 means do not paint, so a
// transparent fill leaves a polygon as an outline.
bool tv_map_set_layer_style(TvMapState* state, const char* layer_name,
                            uint32_t fill_argb, uint32_t stroke_argb, float stroke_width);

typedef struct {
    int32_t kind;            // TV_VECTOR_POINT, _LINE, _POLYGON
    uint32_t layer_index;    // into the frame's layer names
    uint32_t ring_offset, ring_count, coord_offset;
    uint32_t fill_argb, stroke_argb;  // 0xAARRGGBB, alpha 0 means do not paint
    float stroke_width, point_radius;
} TvVectorFeature;

// Recompute the frame's vector geometry, then read it back.
uint32_t tv_map_vector_frame(TvMapState* state);
bool tv_map_vector_feature_at(const TvMapState* state, uint32_t i, TvVectorFeature* out);
size_t tv_map_vector_coords(const TvMapState* state, float* out, size_t cap);
size_t tv_map_vector_rings(const TvMapState* state, uint32_t* out, size_t cap);

uint32_t tv_map_vector_layer_count(const TvMapState* state);
char* tv_map_vector_layer_name(const TvMapState* state, uint32_t index);  // caller must free
```

A feature's geometry is a run of rings. `tv_map_vector_rings` gives each ring's
point count and `tv_map_vector_coords` gives every point as an x and a y, in the
same device pixels as the tile placements. A point is one ring of one point, a
line is one ring, and a polygon's first ring is its exterior and the rest are
holes. Both readers fill as much as `cap` allows and return the full length.

Each feature names its source layer through `layer_index`, which indexes the
frame's layer table. Both the index and the table only hold until the next
`tv_map_vector_frame`.

Vector tiles are a second source: they cache and draw alongside the raster ones,
each with its own URL template.

### Offline Regions

```c
// What a region costs, without enumerating it.
uint64_t tv_region_tile_count(double min_lat, double min_lon, double max_lat, double max_lon,
                              uint8_t min_zoom, uint8_t max_zoom);
uint64_t tv_region_estimated_bytes(double min_lat, double min_lon, double max_lat, double max_lon,
                                   uint8_t min_zoom, uint8_t max_zoom);

typedef struct { uint8_t z; uint32_t x, y; } TvTileCoordinate;

// Enumerate the region, read it back, then drop it. Lowest zoom first.
uint32_t tv_region_plan(TvMapState* state, double min_lat, double min_lon, double max_lat,
                        double max_lon, uint8_t min_zoom, uint8_t max_zoom);
bool tv_region_tile_at(const TvMapState* state, uint32_t index, TvTileCoordinate* out);
void tv_region_clear(TvMapState* state);

// The box the camera covers, to plan a download of what the user is looking at.
typedef struct { double min_lat, min_lon, max_lat, max_lon; } TvBounds;
bool tv_map_visible_bounds(const TvMapState* state, TvBounds* out);
```

Latitudes past the Mercator limit clamp to the top and bottom tile rows, and a
region whose east edge is west of its west edge crosses the antimeridian and
covers the short way round. `tv_region_plan` returns 0 for a region covering
nothing and for one over `TV_REGION_MAX_TILES`, so ask for the count first.

Fetching and storing the tiles is the host's job, as it is for the tile cache.

### Projection

```c
typedef struct { float x, y; } TvScreenPoint;

// North-up screen position, in the same device pixels as a tile placement.
bool tv_map_project(const TvMapState* state, double latitude, double longitude,
                    TvScreenPoint* out);
double tv_map_metres_per_pixel(const TvMapState* state);
```

### User Location

```c
typedef struct {
    double latitude, longitude;
    double accuracy_m;   // horizontal radius, negative when unknown
    double bearing_deg;  // NaN when unknown
} TvUserLocation;

bool tv_map_set_user_location(TvMapState* state, double latitude, double longitude,
                              double accuracy_m, double bearing_deg);
bool tv_map_user_location(const TvMapState* state, TvUserLocation* out);

// TV_TRACKING_NONE, _FOLLOW, _FOLLOW_WITH_HEADING, _FOLLOW_WITH_COURSE.
bool tv_map_set_tracking_mode(TvMapState* state, int32_t mode);
int32_t tv_map_get_tracking_mode(const TvMapState* state);
```

Setting a location moves the camera as the tracking mode asks, so the host feeds
fixes in and reads the camera back.

### Navigation

```c
typedef struct { double latitude, longitude; } TvRoutePoint;
typedef struct {
    const char* instruction;  // borrowed for the call, may be null
    uint32_t start_index, end_index;
} TvRouteStep;

typedef struct {
    int32_t status;  // TV_NAV_ON_ROUTE, _OFF_ROUTE, _ARRIVED
    uint32_t step_index, step_count;
    double distance_to_next_step_m, distance_remaining_m;
    bool off_route;
} TvNavProgress;

bool tv_nav_set_route(TvMapState* state, const TvRoutePoint* points, size_t point_count,
                      const TvRouteStep* steps, size_t step_count);
bool tv_nav_update(TvMapState* state, double latitude, double longitude,
                   TvNavProgress* out);
bool tv_nav_progress(const TvMapState* state, TvNavProgress* out);
char* tv_nav_instruction(const TvMapState* state);  // caller must free
void tv_nav_clear(TvMapState* state);
```

The route comes from elsewhere. `tv_nav_update` takes a fix and returns where on
that route it falls.

### Utility

```c
double tv_distance_between(double lat1, double lon1, double lat2, double lon2);
double tv_bearing_between(double lat1, double lon1, double lat2, double lon2);

char* tv_version(void);        // Returns SDK version (caller must free)
void tv_string_free(char* ptr); // Free SDK-allocated strings
```

---

## Platform Integration

### Android library

Most Android apps do not touch the FFI. Add `com.github.GeoLang:terravista` from
JitPack and put `MapView` in a layout. The library fetches tiles over HTTP, draws
raster and vector tiles on Canvas, caches every fetched tile on disk, saves
offline regions that are never evicted, draws the user location, and runs
navigation. See [`android/README.md`](android/README.md) for the install snippet,
the `MapView` members and the offline behaviour.

### Calling the C ABI from JNI

Kotlin cannot call a `tv_` function directly, so a small C library sits between
them. The Android library's glue is `android/terravista/src/main/jni/terravista_jni.c`,
bound in Kotlin by `TerraVistaNative.kt`, and it loads two libraries:

```kotlin
System.loadLibrary("terravista_ffi")
System.loadLibrary("terravista_jni")
```

[`examples/android-testapp`](examples/android-testapp) does the same from Java,
built without Gradle.

---

## Roadmap

Unchecked items are not built.

- [x] **v0.2**: HTTP tile fetching, in the Android library
- [x] **v0.2**: Gradle distribution for Android, published through JitPack
- [x] **v0.3**: turn-by-turn navigation and user location in the Android library
- [x] **v0.4**: MVT (Mapbox Vector Tile) decoding
- [ ] Planned: a style spec the renderer reads. `terravista_core::style` holds style structs, but nothing reads them, no `tv_` function reaches them, and a Mapbox GL JSON style does not deserialize into them
- [ ] Planned: Vulkan rendering backend (Android)
- [ ] Planned: iOS binding, then a Metal rendering backend
- [ ] Planned: annotation layers (markers, polylines, polygons)
- [ ] Planned: clustering for point features
- [ ] Planned: 3D terrain mesh from DEM tiles
- [ ] Planned: globe view (non-Mercator) for low zoom levels
- [ ] Planned: Swift Package Manager distribution
- [ ] Planned: stable C ABI with semantic versioning guarantees

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
