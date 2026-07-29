#!/usr/bin/env bash
# Builds the test APK without gradle: cargo-ndk -> clang -> aapt2 -> javac -> d8 -> zipalign -> apksigner.
set -euo pipefail

ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/sdk}"
ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$HOME/android-ndk-r27c}"
API=35
BUILD_TOOLS="$ANDROID_HOME/build-tools/35.0.0"
PLATFORM="$ANDROID_HOME/platforms/android-$API/android.jar"
NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
OUT="$HERE/build"
ABI=arm64-v8a
PKG=dev.geolang.terravista.testapp

rm -rf "$OUT/classes" "$OUT/dex" "$OUT/base.apk" "$OUT/app-unsigned.apk" "$OUT/app.apk"
mkdir -p "$OUT/lib/$ABI" "$OUT/classes" "$OUT/dex"

echo "==> terravista-ffi ($ABI)"
(cd "$REPO" && ANDROID_NDK_HOME="$ANDROID_NDK_HOME" \
    cargo ndk -t "$ABI" -o "$OUT/lib" build --release -p terravista-ffi)

echo "==> JNI glue"
"$NDK_BIN/aarch64-linux-android24-clang" -shared -fPIC -O2 \
    -o "$OUT/lib/$ABI/libterravista_jni.so" \
    "$HERE/jni/terravista_jni.c" \
    -L"$OUT/lib/$ABI" -lterravista_ffi -llog

echo "==> resources"
"$BUILD_TOOLS/aapt2" link \
    -o "$OUT/base.apk" \
    -I "$PLATFORM" \
    --manifest "$HERE/AndroidManifest.xml" \
    --min-sdk-version 24 --target-sdk-version "$API"

echo "==> javac"
# d8 from build-tools 35 rejects class files newer than 17
javac --release 17 -nowarn -classpath "$PLATFORM" -d "$OUT/classes" \
    "$HERE"/java/dev/geolang/terravista/testapp/*.java

echo "==> d8"
"$BUILD_TOOLS/d8" --release --min-api 24 --lib "$PLATFORM" \
    --output "$OUT/dex" \
    $(find "$OUT/classes" -name '*.class')

echo "==> package"
cp "$OUT/base.apk" "$OUT/app-unsigned.apk"
(cd "$OUT/dex" && zip -q "$OUT/app-unsigned.apk" classes.dex)
(cd "$OUT" && zip -q "$OUT/app-unsigned.apk" "lib/$ABI/libterravista_ffi.so" "lib/$ABI/libterravista_jni.so")

echo "==> sign"
KEYSTORE="$OUT/debug.keystore"
if [ ! -f "$KEYSTORE" ]; then
    keytool -genkeypair -keystore "$KEYSTORE" -alias androiddebugkey \
        -storepass android -keypass android -keyalg RSA -keysize 2048 \
        -validity 10000 -dname "CN=Android Debug,O=Android,C=US" >/dev/null 2>&1
fi

"$BUILD_TOOLS/zipalign" -f -p 4 "$OUT/app-unsigned.apk" "$OUT/app.apk"
"$BUILD_TOOLS/apksigner" sign --ks "$KEYSTORE" --ks-pass pass:android \
    --key-pass pass:android --min-sdk-version 24 "$OUT/app.apk"

echo "built $OUT/app.apk"
echo "install: adb install -r $OUT/app.apk"
echo "launch:  adb shell am start -n $PKG/.MainActivity"
