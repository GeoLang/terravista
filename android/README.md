# TerraVista for Android

A drop-in map view. Add the dependency, put `MapView` in a layout, and you get a
pannable, pinch-zoomable, rotatable map, raster tiles or vector. No Rust, no JNI,
no NDK.

<img src="docs/sample.png" width="320" alt="The sample app showing OpenStreetMap tiles over London" />

## Install

```groovy
// settings.gradle.kts
dependencyResolutionManagement {
    repositories {
        maven { url = uri("https://jitpack.io") }
    }
}

// app/build.gradle.kts
dependencies {
    implementation("com.github.GeoLang:terravista:0.2.0")
}
```

Ships `arm64-v8a` and `x86_64`. The `INTERNET` permission comes with the library.

## Use

```xml
<dev.geolang.terravista.MapView
    android:id="@+id/map"
    android:layout_width="match_parent"
    android:layout_height="match_parent"
    app:tvCenterLatitude="51.5074"
    app:tvCenterLongitude="-0.1278"
    app:tvZoom="12" />
```

```kotlin
class MainActivity : Activity() {
    private lateinit var map: MapView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        map = findViewById(R.id.map)
        map.onCameraChangeListener = OnCameraChangeListener { Log.i("map", "$it") }
    }

    override fun onDestroy() {
        map.destroy()
        super.onDestroy()
    }
}
```

`destroy()` frees the native map. Skip it and the map leaks until the process dies.

## API

| Member | Meaning |
|--------|---------|
| `setCenter(lat, lon)` | move the centre |
| `cameraPosition` | current centre, zoom and bearing |
| `zoom` | camera zoom, clamped to `minZoom`..`maxZoom` |
| `bearing` | rotation in degrees, 0 is north up |
| `minZoom` / `maxZoom` | zoom limits, default 0 and 18 |
| `tileUrlTemplate` | XYZ template, default OpenStreetMap |
| `vectorTileUrlTemplate` | XYZ template for MVT tiles, null for none |
| `setLayerStyle(name, fill, stroke, width)` | how one vector layer draws |
| `visibleVectorLayers` | layer names in the last drawn vector frame |
| `visibleBounds` | the box the map is showing |
| `diskCacheSizeBytes` / `diskCacheBytes` | ambient tile cache cap and current size |
| `estimateRegion(...)` | tiles and bytes a region would cost |
| `downloadRegion(name, ..., listener)` | save a region to disk |
| `regions()` / `deleteRegion(name)` | list and delete saved regions |
| `onCameraChangeListener` | fires on every camera move |
| `destroy()` | free the native map, idempotent |

XML attributes mirror these: `tvCenterLatitude`, `tvCenterLongitude`, `tvZoom`,
`tvBearing`, `tvMinZoom`, `tvMaxZoom`, `tvTileUrlTemplate`,
`tvVectorTileUrlTemplate`.

Changing `tileUrlTemplate` drops every cached tile, because the cache is keyed by
tile coordinate alone and would otherwise serve the old basemap's images.

## Vector tiles

Point `vectorTileUrlTemplate` at an MVT source and its tiles are fetched,
decoded and drawn over the raster ones:

```kotlin
map.vectorTileUrlTemplate = "https://tiles.example.com/{z}/{x}/{y}.mvt"
```

Features get a fixed look per layer name, matching the layer names
OpenMapTiles-style sources use (`water`, `landcover`, `building`,
`transportation`, `boundary`). There is no style spec, no filters and no labels,
so a source with other layer names still draws, in the fallback colour. Set the
property to null to turn the source off, which drops its tiles.

Override the look one layer at a time:

```kotlin
map.setLayerStyle(
    layerName = "water",
    fillColor = Color.argb(120, 0, 90, 200),
    strokeColor = Color.TRANSPARENT,
    strokeWidth = 2f,
)
```

Colours are Android colour ints, and a zero alpha means do not paint, so a
transparent fill leaves a polygon as an outline. The stroke width is in device
pixels. A layer name the source does not serve is kept anyway, ready for a
source that serves it. `visibleVectorLayers` lists the layer names the last
drawn frame held, which is how to find out what a source actually calls things.

## Offline

Every tile the map fetches is written to disk and read before the network, so a
map that has been looked at once draws again with no signal. The cache lives
under the app's `cacheDir`, keyed by tile source and coordinate, so switching
basemaps never serves the wrong imagery and several sources coexist. It evicts
least recently read against a 512 MB cap:

```kotlin
map.diskCacheSizeBytes = 128L * 1024 * 1024
```

Tiles never expire and are never revalidated, so a source that redraws its
imagery keeps serving the old tiles until they are evicted. The system may also
delete the whole cache when it needs the space, which is the point of putting it
in `cacheDir`: nothing there is worth keeping at the user's expense.

For tiles that have to survive that, save a region:

```kotlin
val bounds = map.visibleBounds ?: return
val zoom = map.cameraPosition.zoom.toInt()

val estimate = map.estimateRegion(
    bounds.minLatitude, bounds.minLongitude,
    bounds.maxLatitude, bounds.maxLongitude,
    zoom, zoom + 2,
)
if (estimate.tileCount > MAX_REGION_TILES) return

val download = map.downloadRegion(
    "home", bounds.minLatitude, bounds.minLongitude,
    bounds.maxLatitude, bounds.maxLongitude, zoom, zoom + 2,
    object : RegionDownloadListener {
        override fun onProgress(completed: Int, failed: Int, total: Int) = Unit
        override fun onFinished(region: OfflineRegion?) = Unit
    },
)
```

A region covers the current `tileUrlTemplate`, and `vectorTileUrlTemplate` too
when one is set. Its tiles are read before the ambient cache and never evict, so
`deleteRegion(name)` is the only way they go. `download.cancel()` stops a
download and keeps whatever it already wrote; the listener is called on the main
thread.

A region is capped at 10,000 tiles (`MAX_REGION_TILES`), and `downloadRegion`
throws `IllegalArgumentException` above that. Public tile servers, OpenStreetMap
included, [forbid bulk downloading](https://operations.osmfoundation.org/policies/tiles/),
so ask `estimateRegion` first and offer a smaller area rather than raising the
limit or looping over smaller boxes.

## Tile sources

Default is `https://tile.openstreetmap.org/{z}/{x}/{y}.png`. Respect the
[OSM tile policy](https://operations.osmfoundation.org/policies/tiles/) or point
`tileUrlTemplate` at your own server.

The core asks for tiles at `round(zoom + log2(density))`, so a 2.6x screen at camera
zoom 12 fetches z13 and one tile lands on 256 device pixels instead of being upscaled.
Zooming past what your source serves degrades to upscaled parent tiles rather than
blank space, so `maxZoom` is a quality knob, not a correctness one.

## Build from source

```bash
cd android
./gradlew :terravista:assembleRelease      # AAR
./gradlew :sample:installDebug             # sample on a connected device
./gradlew :terravista:publishToMavenLocal  # com.github.GeoLang:terravista:0.2.0
```

Needs JDK 17 or later. Gradle 8.9 does not run on JDK 25, so if that is your default
point it at an older one:

```bash
JAVA_HOME=~/.jdks/jdk-21.0.12+8 ./gradlew :terravista:assembleRelease
```

### Native libraries

`terravista/src/main/jniLibs/*/*.so` are **committed prebuilt**, because JitPack has
no Rust toolchain and could not otherwise produce them. Rebuild after any change to
`crates/terravista-ffi` or the JNI glue, then commit the result:

```bash
./tools/build-natives.sh
```

That builds the Rust core with cargo-ndk and the JNI glue with the NDK clang, for both
ABIs, and verifies every LOAD segment is 16 KB aligned. Android 15 and later require
that alignment, and devices with 16 KB pages refuse to load anything else. Neither
rustc nor a bare clang link does it by default, so the script passes
`-Wl,-z,max-page-size=16384`.

Needs `ANDROID_NDK_HOME` (default `~/android-ndk-r27c`), `cargo-ndk`, and the
`aarch64-linux-android` and `x86_64-linux-android` Rust targets.
