// JNI glue over the terravista flat C ABI. The crate ships no cbindgen header,
// so the prototypes are declared here and must track crates/terravista-ffi.

#include <jni.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct TvMapState TvMapState;

typedef struct {
    uint8_t zoom;
    uint32_t x_min;
    uint32_t x_max;
    uint32_t y_min;
    uint32_t y_max;
} TvTileRange;

typedef struct {
    uint8_t z;
    uint32_t x;
    uint32_t y;
    float screen_x;
    float screen_y;
    float size;
} TvTilePlacement;

extern TvMapState *tv_map_create(uint32_t width, uint32_t height, float dpr);
extern void tv_map_destroy(TvMapState *state);
extern void tv_map_set_center(TvMapState *state, double lat, double lon);
extern void tv_map_set_zoom(TvMapState *state, double zoom);
extern void tv_map_set_viewport(TvMapState *state, uint32_t w, uint32_t h, float dpr);
extern double tv_map_get_zoom(const TvMapState *state);
extern double tv_map_get_center_lat(const TvMapState *state);
extern double tv_map_get_center_lon(const TvMapState *state);
extern double tv_map_get_bearing(const TvMapState *state);
extern double tv_map_get_pitch(const TvMapState *state);
extern void tv_map_set_tile_url(TvMapState *state, const char *url);
extern bool tv_map_tile_range(const TvMapState *state, TvTileRange *out);
extern uint32_t tv_map_visible_tile_count(TvMapState *state);
extern bool tv_map_visible_tile_at(const TvMapState *state, uint32_t index, TvTilePlacement *out);
extern int32_t tv_map_touch(TvMapState *state, int32_t phase, const double *xs, const double *ys,
                            const uint64_t *ids, size_t count);
extern char *tv_map_tile_url(const TvMapState *state, uint8_t z, uint32_t x, uint32_t y);
extern bool tv_cache_put(TvMapState *state, uint8_t z, uint32_t x, uint32_t y, const uint8_t *bytes,
                         size_t len, const char *content_type);
extern bool tv_cache_has(const TvMapState *state, uint8_t z, uint32_t x, uint32_t y);
extern size_t tv_cache_get(TvMapState *state, uint8_t z, uint32_t x, uint32_t y, uint8_t *out,
                           size_t cap);
extern uint32_t tv_cache_tile_count(const TvMapState *state);
extern uint64_t tv_cache_size_bytes(const TvMapState *state);
extern char *tv_version(void);
extern void tv_string_free(char *ptr);

#define STATE(h) ((TvMapState *)(intptr_t)(h))

static jstring take_string(JNIEnv *env, char *owned) {
    if (!owned) {
        return NULL;
    }
    jstring out = (*env)->NewStringUTF(env, owned);
    tv_string_free(owned);
    return out;
}

#define FN(name) Java_dev_geolang_terravista_testapp_TerraVista_##name

JNIEXPORT jlong JNICALL FN(create)(JNIEnv *env, jclass c, jint w, jint h, jfloat dpr) {
    (void)env;
    (void)c;
    return (jlong)(intptr_t)tv_map_create((uint32_t)w, (uint32_t)h, dpr);
}

JNIEXPORT void JNICALL FN(destroy)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    tv_map_destroy(STATE(h));
}

JNIEXPORT void JNICALL FN(setCenter)(JNIEnv *env, jclass c, jlong h, jdouble lat, jdouble lon) {
    (void)env;
    (void)c;
    tv_map_set_center(STATE(h), lat, lon);
}

JNIEXPORT void JNICALL FN(setZoom)(JNIEnv *env, jclass c, jlong h, jdouble z) {
    (void)env;
    (void)c;
    tv_map_set_zoom(STATE(h), z);
}

JNIEXPORT void JNICALL FN(setViewport)(JNIEnv *env, jclass c, jlong h, jint w, jint ht,
                                       jfloat dpr) {
    (void)env;
    (void)c;
    tv_map_set_viewport(STATE(h), (uint32_t)w, (uint32_t)ht, dpr);
}

JNIEXPORT jdouble JNICALL FN(getZoom)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return tv_map_get_zoom(STATE(h));
}

JNIEXPORT jdouble JNICALL FN(getCenterLat)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return tv_map_get_center_lat(STATE(h));
}

JNIEXPORT jdouble JNICALL FN(getCenterLon)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return tv_map_get_center_lon(STATE(h));
}

JNIEXPORT jdouble JNICALL FN(getBearing)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return tv_map_get_bearing(STATE(h));
}

JNIEXPORT jdouble JNICALL FN(getPitch)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return tv_map_get_pitch(STATE(h));
}

JNIEXPORT void JNICALL FN(setTileUrl)(JNIEnv *env, jclass c, jlong h, jstring url) {
    (void)c;
    const char *s = (*env)->GetStringUTFChars(env, url, NULL);
    tv_map_set_tile_url(STATE(h), s);
    (*env)->ReleaseStringUTFChars(env, url, s);
}

// out = {zoom, x_min, x_max, y_min, y_max}
JNIEXPORT jboolean JNICALL FN(tileRange)(JNIEnv *env, jclass c, jlong h, jintArray out) {
    (void)c;
    TvTileRange r;
    if (!tv_map_tile_range(STATE(h), &r) || (*env)->GetArrayLength(env, out) < 5) {
        return JNI_FALSE;
    }
    jint vals[5] = {r.zoom, (jint)r.x_min, (jint)r.x_max, (jint)r.y_min, (jint)r.y_max};
    (*env)->SetIntArrayRegion(env, out, 0, 5, vals);
    return JNI_TRUE;
}

JNIEXPORT jint JNICALL FN(visibleTileCount)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return (jint)tv_map_visible_tile_count(STATE(h));
}

// zxy = {z, x, y}; xys = {screen_x, screen_y, size}
JNIEXPORT jboolean JNICALL FN(visibleTileAt)(JNIEnv *env, jclass c, jlong h, jint index,
                                             jintArray zxy, jfloatArray xys) {
    (void)c;
    TvTilePlacement p;
    if (!tv_map_visible_tile_at(STATE(h), (uint32_t)index, &p)) {
        return JNI_FALSE;
    }
    if ((*env)->GetArrayLength(env, zxy) < 3 || (*env)->GetArrayLength(env, xys) < 3) {
        return JNI_FALSE;
    }
    jint coords[3] = {p.z, (jint)p.x, (jint)p.y};
    jfloat screen[3] = {p.screen_x, p.screen_y, p.size};
    (*env)->SetIntArrayRegion(env, zxy, 0, 3, coords);
    (*env)->SetFloatArrayRegion(env, xys, 0, 3, screen);
    return JNI_TRUE;
}

JNIEXPORT jint JNICALL FN(touch)(JNIEnv *env, jclass c, jlong h, jint phase, jdoubleArray xs,
                                 jdoubleArray ys, jlongArray ids) {
    (void)c;
    jsize n = (xs == NULL) ? 0 : (*env)->GetArrayLength(env, xs);
    if (n == 0) {
        return tv_map_touch(STATE(h), phase, NULL, NULL, NULL, 0);
    }

    jdouble *px = (*env)->GetDoubleArrayElements(env, xs, NULL);
    jdouble *py = (*env)->GetDoubleArrayElements(env, ys, NULL);
    jlong *pid = (*env)->GetLongArrayElements(env, ids, NULL);

    // jlong is signed 64-bit, tv wants uint64_t; same width, reinterpret
    int32_t result = tv_map_touch(STATE(h), phase, (const double *)px, (const double *)py,
                                  (const uint64_t *)pid, (size_t)n);

    (*env)->ReleaseDoubleArrayElements(env, xs, px, JNI_ABORT);
    (*env)->ReleaseDoubleArrayElements(env, ys, py, JNI_ABORT);
    (*env)->ReleaseLongArrayElements(env, ids, pid, JNI_ABORT);
    return result;
}

JNIEXPORT jstring JNICALL FN(tileUrl)(JNIEnv *env, jclass c, jlong h, jint z, jint x, jint y) {
    (void)c;
    return take_string(env, tv_map_tile_url(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y));
}

JNIEXPORT jboolean JNICALL FN(cachePut)(JNIEnv *env, jclass c, jlong h, jint z, jint x, jint y,
                                        jbyteArray bytes, jstring contentType) {
    (void)c;
    jsize len = (*env)->GetArrayLength(env, bytes);
    jbyte *buf = (*env)->GetByteArrayElements(env, bytes, NULL);
    const char *ct = (contentType == NULL) ? NULL
                                           : (*env)->GetStringUTFChars(env, contentType, NULL);

    bool ok = tv_cache_put(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y, (const uint8_t *)buf,
                           (size_t)len, ct);

    if (ct) {
        (*env)->ReleaseStringUTFChars(env, contentType, ct);
    }
    (*env)->ReleaseByteArrayElements(env, bytes, buf, JNI_ABORT);
    return ok ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT jboolean JNICALL FN(cacheHas)(JNIEnv *env, jclass c, jlong h, jint z, jint x, jint y) {
    (void)env;
    (void)c;
    return tv_cache_has(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y) ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT jbyteArray JNICALL FN(cacheGet)(JNIEnv *env, jclass c, jlong h, jint z, jint x, jint y) {
    (void)c;
    // probe the length first, then pull the bytes into a right-sized array
    size_t len = tv_cache_get(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y, NULL, 0);
    if (len == 0) {
        return NULL;
    }
    jbyteArray out = (*env)->NewByteArray(env, (jsize)len);
    if (!out) {
        return NULL;
    }
    jbyte *buf = (*env)->GetByteArrayElements(env, out, NULL);
    tv_cache_get(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y, (uint8_t *)buf, len);
    (*env)->ReleaseByteArrayElements(env, out, buf, 0);
    return out;
}

JNIEXPORT jint JNICALL FN(cacheTileCount)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return (jint)tv_cache_tile_count(STATE(h));
}

JNIEXPORT jlong JNICALL FN(cacheSizeBytes)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return (jlong)tv_cache_size_bytes(STATE(h));
}

JNIEXPORT jstring JNICALL FN(version)(JNIEnv *env, jclass c) {
    (void)c;
    return take_string(env, tv_version());
}
