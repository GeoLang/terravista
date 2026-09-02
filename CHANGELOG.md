# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-09-02

- 2026-08-21: `docs/index.html` drops the iOS/Swift SDK, Mapbox GL style
  parse, and auto-generated JNI claims. Android fetches and draws. The
  core does not. There is no iOS binding.
- 2026-08-15: docs test count is 130.
- README status is v0.4, not v0.1: the core still does not fetch or draw; the
  Android library does both, including MVT.
- Vector features carry their layer: the frame keeps a table of the layer names
  it drew, each feature indexes into it, and the FFI reads it back with
  `tv_map_vector_layer_count` and `tv_map_vector_layer_name`. Colour was the
  only per-layer signal a host had before.
- Per-layer styling: `tv_map_set_layer_style` sets a layer's fill colour,
  stroke colour and stroke width by name, over the built-in look. C FFI grown
  from 50 to 53 symbols.
- Android: `MapView.setLayerStyle(...)` and `MapView.visibleVectorLayers`, and
  the sample app draws a vector layer over the raster basemap.
- Offline tiles on Android. Every fetched tile is written to a disk cache under
  the app's cache directory, keyed by source and coordinate, read before the
  network and evicted least-recently-read against `diskCacheSizeBytes`, 512 MB
  by default. Tiles never expire.
- Pinned regions: `MapView.downloadRegion(...)` with progress and cancellation,
  `regions()`, `deleteRegion(name)` and `estimateRegion(...)`. Region tiles live
  under the app's files directory, are read before the ambient cache and never
  evict. A region is capped at 10,000 tiles, because public tile servers forbid
  bulk downloading.
- `OfflineRegion` enumerates its own tiles: `tiles()` and `tile_count()` now
  share one range calculation, taken from the camera's, so a region clamps at
  the Mercator limit and crosses the antimeridian the short way round instead
  of underflowing its own arithmetic. `TileRange::count` counts in `u64`, which
  a whole world of tiles overflows past zoom 15.
- C FFI grown from 53 to 59 symbols: region tile count, size estimate, plan and
  read-back, and the camera's visible bounds, which is what a host hands the
  planner to save what the user is looking at.
- Tile packages define themselves by an `OfflineRegion`: `PackageDefinition` is
  a region plus a format, and the module's own bounding box, tile-range maths
  and size estimate are gone in favour of the region's, which was the third
  copy of that calculation. The TVPK bytes are unchanged. Bounds read back from
  a package's metadata now clamp at the Mercator limit and cross the
  antimeridian the short way round like any other region, where before an
  inverted box was rejected and silently replaced by the whole world.
- CI builds the Android SDK: a job on JDK 21 assembles the library's release
  AAR and the sample's debug APK and runs the sample's unit tests, so a Kotlin
  break fails the build instead of surfacing at the next JitPack release.
- Sample app draws street-level vector tiles. It reads OpenFreeMap's TileJSON at
  https://tiles.openfreemap.org/planet the first time the vector layer is asked
  for, and takes the current tile url and the tileset's max zoom from it, because
  OpenFreeMap rotates that url and a hardcoded one goes stale. The camera caps
  two zooms below the tileset, so a dense screen never asks for a tile past the
  end, and the readout carries the tileset's attribution. If the fetch fails the
  layer falls back to MapLibre's demo tileset, and the log says which one drew.
- Sample app has a small real UI: the basemap button opens a picker listing the
  six sources with the current one checked, saving a region asks first with the
  tile count and size from `estimateRegion`, and the map carries floating zoom
  buttons and a compass that turns with the bearing and faces the map north
  when tapped. Stock widgets throughout, no new dependencies.

- MVT decoding in the core: layers, features, points, lines, polygons and
  attributes, with ring winding deciding holes so multipolygons keep their
  parts. The decoder is `jung-mvt`, which reads the protobuf itself and pulls
  in nothing beyond `thiserror`.
- Vector tiles render: `renderer::vector_tile_commands` turns a decoded tile
  and its placement into `DrawVectorLayer` commands in screen coordinates,
  with a fixed look per layer name. No style spec, no labels, no fonts.
- C FFI grown from 41 to 50 symbols: a vector tile source with its own URL
  template and cache, and a per-frame geometry readback (features, ring
  lengths, coordinates).
- Android: `MapView.vectorTileUrlTemplate` (and the `tvVectorTileUrlTemplate`
  XML attribute) fetches, decodes and draws vector tiles over the raster ones.

## [0.3.0] - 2026-07-29

- Turn-by-turn navigation in the Android SDK: `MapView.startNavigation(Route)`,
  per-fix progress (`NavProgress` with status, next-step and remaining
  distances, current instruction).
- User location: blue dot with accuracy circle and heading wedge, plus
  `TrackingMode` (follow, follow-with-heading, follow-with-course) so the
  camera can track fixes.
- Core: `Camera::project` (coordinate to screen pixels, agrees with tile
  placement, wraps at the antimeridian) and `metres_per_pixel`.
- C FFI grown from 28 to 41 symbols: projection, user location, tracking
  mode, navigation route/update/progress, distance and bearing helpers.

## [0.2.0] - 2026-07-29

- Camera math rewritten in Web Mercator world-pixel space: pan, visible bounds,
  and tile placement now agree at every latitude, and rotation is rendered.
- Pinch zoom anchors under the fingers; tile zoom is dpr-biased for sharp HiDPI.
- C FFI grown from 18 to 28 symbols: gestures, tile range and placements,
  tile URLs, cache I/O, bearing/pitch getters.
- Fixed reachable panics and wrong results in navigation (point-to-segment
  off-route), offline bbox queries (envelope intersection), color parsing, and
  tile packages.
- Android: Kotlin `MapView` library (`android/`, published via JitPack as
  `com.github.GeoLang:terravista`) with raster tile fetching and drawing built
  in, prebuilt 16 KB-aligned natives, a sample app, and a plain-Java example
  (`examples/android-testapp`).

## [0.1.0] — 2025-07-14

### Added

- **Camera model** — Continuous zoom (0–22), bearing, pitch, Web Mercator projection, viewport-aware tile range calculation.
- **Gesture recognition** — Multi-touch state machine (pan, pinch-zoom, rotate, tilt) with camera delta output.
- **Tile cache** — LRU eviction with configurable size limits (256 MB / 50K tiles), per-region offline pre-fetch, URL template system.
- **Offline vector store** — On-device feature CRUD with GeoJSON geometry, sync status tracking (Synced/PendingCreate/PendingUpdate/PendingDelete/Conflict), bbox queries, GeoJSON export.
- **Style engine** — Mapbox GL-compatible style definitions with zoom-level interpolated properties, Fill/Line/Symbol/Circle/Raster layer types.
- **Turn-by-turn navigation** — On-device route tracking, step-by-step instructions, off-route detection (50m threshold), arrival notification.
- **Location service** — GPS coordinate model, haversine distance/bearing, tracking modes (None/Follow/FollowWithHeading/FollowWithCourse), abstract `LocationProvider` trait.
- **Render pipeline** — Frame-based command buffer (Clear/DrawRasterTile/DrawVectorLayer/DrawLocationMarker/DrawRoute), visible tile placement calculation, HiDPI awareness.
- **C FFI bindings** — 15 exported functions via `terravista-ffi` crate (cdylib + staticlib), `tv_` prefixed flat C API for Swift/Kotlin consumption.
- **Documentation** — Comprehensive README with architecture, API reference, platform integration examples. GitHub Pages landing site.

[0.1.0]: https://github.com/GeoLang/terravista/releases/tag/v0.1.0
