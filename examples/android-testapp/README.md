# TerraVista Android test app

Minimal app proving the SDK drives a real map on real hardware. All map math comes from
terravista over the C ABI: touch events go into the SDK gesture recognizer, the visible tile
set and screen placements come out of it, and tile bytes are stored in and read back from the
SDK tile cache. The app only does HTTP, PNG decode, and `Canvas.drawBitmap`.

Drag to pan, pinch to zoom and twist to rotate. A single tap rotates 45 degrees,
which exists because `adb shell input` cannot inject a two-finger twist.

One activity, one view, one JNI class:

| File | Role |
|------|------|
| `java/.../MainActivity.java` | creates the view |
| `java/.../MapView.java` | touch in, tiles out, readout, HTTP fetch |
| `java/.../TerraVista.java` | native method declarations |
| `jni/terravista_jni.c` | JNI glue over the flat C ABI |

## Build and install

```bash
./build.sh
adb install -r build/app.apk
adb shell am start -n dev.geolang.terravista.testapp/.MainActivity
```

`build.sh` runs cargo-ndk, the NDK clang, aapt2, javac, d8, zipalign, and apksigner. It
generates `build/debug.keystore` on first run.

Environment, override by exporting before `build.sh`:

- `ANDROID_HOME` defaults to `~/Android/sdk`, needs `platforms/android-35` and `build-tools/35.0.0`
- `ANDROID_NDK_HOME` defaults to `~/android-ndk-r27c`
- `cargo-ndk` and the `aarch64-linux-android` rust target must be installed

## Things that bit us

- **javac must target 17.** `d8` from build-tools 35 rejects newer class files. `build.sh`
  passes `--release 17`.
- **arm64 only.** Add ABIs by extending `cargo ndk -t` and the clang triple in `build.sh`.
- **The SDK is not thread-safe.** `MapView` serializes every FFI call on one lock, because
  tile fetches land on a background thread.
- **No cbindgen header.** `jni/terravista_jni.c` declares the externs by hand, so it must
  track `crates/terravista-ffi/src/lib.rs`.
- **API 35 draws edge to edge.** The view spans behind the status bar, so the readout is
  offset by the top window inset. Without that it renders underneath the system bars.
- OSM tiles need a real `User-Agent` or you get HTTP 403.
- **The SDK biases tile zoom by screen density**, so on a 2.625x screen camera zoom 12
  fetches z13 tiles. The app therefore caps camera zoom at `19 - log2(dpr)`, since OSM
  serves nothing past z19.
- Rotation is applied by the app: the SDK returns north-up placements and the view
  rotates the canvas by `-bearing`. The readout is drawn after `restore()` so it stays
  upright.
- While a tile loads the view blits the matching crop of an already-decoded parent tile,
  so panning shows blurry map rather than blank white.

## Verify on device

```bash
adb shell am force-stop dev.geolang.terravista.testapp
adb logcat -c
adb shell am start -W -n dev.geolang.terravista.testapp/.MainActivity
adb exec-out screencap -p > before.png
adb shell input swipe 800 1600 200 1000 800
adb exec-out screencap -p > after.png
adb logcat -d -s TerraVistaTest:*
```

The `touch-end` log line carries the settled camera state, the frame lines are sampled every
30 frames and can land mid-gesture.
