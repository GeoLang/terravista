#!/usr/bin/env bash
# Regenerate the prebuilt .so files under terravista/src/main/jniLibs.
#
# They are committed because JitPack has no Rust toolchain, so the AAR has to
# ship natives that are already built. Re-run this after any change to
# crates/terravista-ffi or to the JNI glue, then commit the result.
set -euo pipefail

ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$HOME/android-ndk-r27c}"
NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
API=24

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
JNI_LIBS="$HERE/../terravista/src/main/jniLibs"
JNI_SRC="$HERE/../terravista/src/main/jni/terravista_jni.c"

# abi:clang-triple
TARGETS=(
    "arm64-v8a:aarch64-linux-android$API-clang"
    "x86_64:x86_64-linux-android$API-clang"
)

# Android 15 and later require 16 KB page alignment, and devices with 16 KB
# pages refuse to load anything else. Neither rustc nor a bare clang link does
# this by default.
ALIGN_FLAG="-Wl,-z,max-page-size=16384"

echo "==> rust core (cargo-ndk)"
(cd "$REPO" && ANDROID_NDK_HOME="$ANDROID_NDK_HOME" \
    RUSTFLAGS="${RUSTFLAGS:-} -C link-arg=$ALIGN_FLAG" \
    cargo ndk -t arm64-v8a -t x86_64 -o "$JNI_LIBS" build --release -p terravista-ffi)

for entry in "${TARGETS[@]}"; do
    abi="${entry%%:*}"
    cc="${entry##*:}"
    echo "==> jni glue ($abi)"
    "$NDK_BIN/$cc" -shared -fPIC -O2 "$ALIGN_FLAG" \
        -o "$JNI_LIBS/$abi/libterravista_jni.so" \
        "$JNI_SRC" \
        -L"$JNI_LIBS/$abi" -lterravista_ffi -llog
done

echo
echo "prebuilt natives:"
fail=0
find "$JNI_LIBS" -name '*.so' | sort | while read -r so; do
    # every LOAD segment must be at least 16 KB aligned
    bad=$("$NDK_BIN/llvm-readelf" -l "$so" |
        awk '$1 == "LOAD" { if (strtonum($NF) < 16384) print $NF }')
    if [ -n "$bad" ]; then
        printf '  %-46s %6s  NOT 16K ALIGNED\n' "${so#"$JNI_LIBS/"}" "$(du -h "$so" | cut -f1)"
        fail=1
    else
        printf '  %-46s %6s  16K aligned\n' "${so#"$JNI_LIBS/"}" "$(du -h "$so" | cut -f1)"
    fi
done
exit $fail
