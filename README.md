# TerraVista

Mobile map SDK for the TileTopia ecosystem — offline-first tile caching, GPU-accelerated vector rendering, gesture-driven navigation, and turn-by-turn routing for iOS and Android.

## Architecture

```
┌──────────────────────────────────────────────────┐
│  Platform Layer (Swift / Kotlin)                 │
├──────────────────────────────────────────────────┤
│  terravista-ffi (C ABI / staticlib + cdylib)     │
├──────────────────────────────────────────────────┤
│  terravista-core                                 │
│  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐ │
│  │ Camera │ │ Tiles  │ │Offline │ │ Location │ │
│  │& Input │ │ Cache  │ │ Store  │ │ Service  │ │
│  └────────┘ └────────┘ └────────┘ └──────────┘ │
│  ┌────────┐ ┌────────┐ ┌────────┐              │
│  │Renderer│ │ Style  │ │ Route  │              │
│  │Pipeline│ │ Engine │ │ Engine │              │
│  └────────┘ └────────┘ └────────┘              │
└──────────────────────────────────────────────────┘
```

## Features

- **Offline Tile Cache**: LRU disk cache with configurable limits and offline region pre-fetch
- **Vector Tile Rendering**: MVT decode + GPU render via platform Metal/Vulkan
- **Gesture Recognition**: Pan, pinch-zoom, rotate, tilt — full multi-touch
- **Camera Model**: Continuous zoom (0-22), bearing, pitch, Web Mercator projection
- **Turn-by-Turn Navigation**: Route display, step tracking, off-route detection
- **Offline Vector Editing**: Local feature store with sync-when-online
- **Style Engine**: Mapbox GL-compatible zoom-interpolated styles
- **C FFI**: Flat C API for Swift (iOS) and Kotlin/JNI (Android) consumption

## Building

```bash
# Library (for development/testing)
cargo build

# iOS static library (aarch64)
cargo build --target aarch64-apple-ios -p terravista-ffi --release

# Android shared library (aarch64)
cargo build --target aarch64-linux-android -p terravista-ffi --release
```

## Usage (Swift)

```swift
import TerraVista

let map = tv_map_create(screenWidth, screenHeight, UIScreen.main.scale)
tv_map_set_center(map, 51.5074, -0.1278)  // London
tv_map_set_zoom(map, 14.0)
tv_map_set_tile_url(map, "https://tiles.tiletopia.dev/{z}/{x}/{y}.mvt")

// On pan gesture
tv_map_pan(map, dx, dy)

// Cleanup
tv_map_destroy(map)
```

## License

AGPL-3.0-or-later
