# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - 2026-08-14

- Vector features carry their layer: the frame keeps a table of the layer names
  it drew, each feature indexes into it, and the FFI reads it back with
  `tv_map_vector_layer_count` and `tv_map_vector_layer_name`. Colour was the
  only per-layer signal a host had before.
- Per-layer styling: `tv_map_set_layer_style` sets a layer's fill colour,
  stroke colour and stroke width by name, over the built-in look. C FFI grown
  from 50 to 53 symbols.
- Android: `MapView.setLayerStyle(...)` and `MapView.visibleVectorLayers`, and
  the sample app draws a vector layer over the raster basemap.

## [0.4.0] - 2026-08-13

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
