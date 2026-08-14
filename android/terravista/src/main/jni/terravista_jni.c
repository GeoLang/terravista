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

typedef struct {
    float x;
    float y;
} TvScreenPoint;

typedef struct {
    double latitude;
    double longitude;
    double accuracy_m;
    double bearing_deg;
} TvUserLocation;

typedef struct {
    double latitude;
    double longitude;
} TvRoutePoint;

typedef struct {
    const char *instruction;
    uint32_t start_index;
    uint32_t end_index;
} TvRouteStep;

typedef struct {
    int32_t status;
    uint32_t step_index;
    uint32_t step_count;
    double distance_to_next_step_m;
    double distance_remaining_m;
    bool off_route;
} TvNavProgress;

typedef struct {
    int32_t kind;
    uint32_t ring_offset;
    uint32_t ring_count;
    uint32_t coord_offset;
    uint32_t fill_argb;
    uint32_t stroke_argb;
    float stroke_width;
    float point_radius;
} TvVectorFeature;

extern TvMapState *tv_map_create(uint32_t width, uint32_t height, float dpr);
extern void tv_map_destroy(TvMapState *state);
extern void tv_map_set_center(TvMapState *state, double lat, double lon);
extern void tv_map_set_zoom(TvMapState *state, double zoom);
extern void tv_map_set_viewport(TvMapState *state, uint32_t w, uint32_t h, float dpr);
extern double tv_map_get_zoom(const TvMapState *state);
extern double tv_map_get_center_lat(const TvMapState *state);
extern double tv_map_get_center_lon(const TvMapState *state);
extern void tv_map_set_bearing(TvMapState *state, double bearing);
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
extern void tv_cache_clear(TvMapState *state);
extern uint32_t tv_cache_tile_count(const TvMapState *state);
extern uint64_t tv_cache_size_bytes(const TvMapState *state);
extern char *tv_version(void);
extern void tv_string_free(char *ptr);
extern bool tv_map_project(const TvMapState *state, double lat, double lon, TvScreenPoint *out);
extern double tv_map_metres_per_pixel(const TvMapState *state);
extern bool tv_map_set_user_location(TvMapState *state, double lat, double lon, double accuracy_m,
                                     double bearing_deg);
extern bool tv_map_user_location(const TvMapState *state, TvUserLocation *out);
extern bool tv_map_set_tracking_mode(TvMapState *state, int32_t mode);
extern int32_t tv_map_get_tracking_mode(const TvMapState *state);
extern bool tv_nav_set_route(TvMapState *state, const TvRoutePoint *points, size_t point_count,
                             const TvRouteStep *steps, size_t step_count);
extern bool tv_nav_update(TvMapState *state, double lat, double lon, TvNavProgress *out);
extern bool tv_nav_progress(const TvMapState *state, TvNavProgress *out);
extern char *tv_nav_instruction(const TvMapState *state);
extern void tv_nav_clear(TvMapState *state);
extern double tv_distance_between(double lat1, double lon1, double lat2, double lon2);
extern double tv_bearing_between(double lat1, double lon1, double lat2, double lon2);
extern void tv_map_set_vector_tile_url(TvMapState *state, const char *url);
extern char *tv_map_vector_tile_url(const TvMapState *state, uint8_t z, uint32_t x, uint32_t y);
extern bool tv_vector_cache_put(TvMapState *state, uint8_t z, uint32_t x, uint32_t y,
                                const uint8_t *bytes, size_t len);
extern bool tv_vector_cache_has(const TvMapState *state, uint8_t z, uint32_t x, uint32_t y);
extern void tv_vector_cache_clear(TvMapState *state);
extern uint32_t tv_map_vector_frame(TvMapState *state);
extern bool tv_map_vector_feature_at(const TvMapState *state, uint32_t index,
                                     TvVectorFeature *out);
extern size_t tv_map_vector_coords(const TvMapState *state, float *out, size_t cap);
extern size_t tv_map_vector_rings(const TvMapState *state, uint32_t *out, size_t cap);

#define STATE(h) ((TvMapState *)(intptr_t)(h))

static jstring take_string(JNIEnv *env, char *owned) {
    if (!owned) {
        return NULL;
    }
    jstring out = (*env)->NewStringUTF(env, owned);
    tv_string_free(owned);
    return out;
}

#define FN(name) Java_dev_geolang_terravista_TerraVistaNative_##name

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

JNIEXPORT void JNICALL FN(setBearing)(JNIEnv *env, jclass c, jlong h, jdouble bearing) {
    (void)env;
    (void)c;
    tv_map_set_bearing(STATE(h), bearing);
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

JNIEXPORT void JNICALL FN(cacheClear)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    tv_cache_clear(STATE(h));
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

// xy = {screen_x, screen_y} in device pixels
JNIEXPORT jboolean JNICALL FN(project)(JNIEnv *env, jclass c, jlong h, jdouble lat, jdouble lon,
                                       jfloatArray xy) {
    (void)c;
    TvScreenPoint p;
    if ((*env)->GetArrayLength(env, xy) < 2 || !tv_map_project(STATE(h), lat, lon, &p)) {
        return JNI_FALSE;
    }
    jfloat vals[2] = {p.x, p.y};
    (*env)->SetFloatArrayRegion(env, xy, 0, 2, vals);
    return JNI_TRUE;
}

JNIEXPORT jdouble JNICALL FN(metresPerPixel)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return tv_map_metres_per_pixel(STATE(h));
}

JNIEXPORT jboolean JNICALL FN(setUserLocation)(JNIEnv *env, jclass c, jlong h, jdouble lat,
                                               jdouble lon, jdouble accuracy, jdouble bearing) {
    (void)env;
    (void)c;
    return tv_map_set_user_location(STATE(h), lat, lon, accuracy, bearing) ? JNI_TRUE : JNI_FALSE;
}

// out = {latitude, longitude, accuracy_m, bearing_deg}
JNIEXPORT jboolean JNICALL FN(userLocation)(JNIEnv *env, jclass c, jlong h, jdoubleArray out) {
    (void)c;
    TvUserLocation loc;
    if ((*env)->GetArrayLength(env, out) < 4 || !tv_map_user_location(STATE(h), &loc)) {
        return JNI_FALSE;
    }
    jdouble vals[4] = {loc.latitude, loc.longitude, loc.accuracy_m, loc.bearing_deg};
    (*env)->SetDoubleArrayRegion(env, out, 0, 4, vals);
    return JNI_TRUE;
}

JNIEXPORT jboolean JNICALL FN(setTrackingMode)(JNIEnv *env, jclass c, jlong h, jint mode) {
    (void)env;
    (void)c;
    return tv_map_set_tracking_mode(STATE(h), (int32_t)mode) ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT jint JNICALL FN(getTrackingMode)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return (jint)tv_map_get_tracking_mode(STATE(h));
}

JNIEXPORT jboolean JNICALL FN(navSetRoute)(JNIEnv *env, jclass c, jlong h, jdoubleArray lats,
                                           jdoubleArray lons, jintArray stepStart,
                                           jintArray stepEnd, jobjectArray instructions) {
    (void)c;
    jsize points = (*env)->GetArrayLength(env, lats);
    jsize steps = (*env)->GetArrayLength(env, stepStart);
    if (points < 2 || steps < 1 || (*env)->GetArrayLength(env, lons) != points ||
        (*env)->GetArrayLength(env, stepEnd) != steps ||
        (*env)->GetArrayLength(env, instructions) != steps) {
        return JNI_FALSE;
    }
    // one local ref per instruction string is held until the call returns
    if ((*env)->EnsureLocalCapacity(env, steps + 8) != 0) {
        return JNI_FALSE;
    }

    TvRoutePoint *geometry = calloc((size_t)points, sizeof(TvRoutePoint));
    TvRouteStep *route = calloc((size_t)steps, sizeof(TvRouteStep));
    jstring *held = calloc((size_t)steps, sizeof(jstring));
    if (!geometry || !route || !held) {
        free(geometry);
        free(route);
        free(held);
        return JNI_FALSE;
    }

    jdouble *plat = (*env)->GetDoubleArrayElements(env, lats, NULL);
    jdouble *plon = (*env)->GetDoubleArrayElements(env, lons, NULL);
    for (jsize i = 0; i < points; i++) {
        geometry[i].latitude = plat[i];
        geometry[i].longitude = plon[i];
    }
    (*env)->ReleaseDoubleArrayElements(env, lats, plat, JNI_ABORT);
    (*env)->ReleaseDoubleArrayElements(env, lons, plon, JNI_ABORT);

    jint *pstart = (*env)->GetIntArrayElements(env, stepStart, NULL);
    jint *pend = (*env)->GetIntArrayElements(env, stepEnd, NULL);
    for (jsize i = 0; i < steps; i++) {
        held[i] = (jstring)(*env)->GetObjectArrayElement(env, instructions, i);
        route[i].instruction =
            held[i] ? (*env)->GetStringUTFChars(env, held[i], NULL) : NULL;
        route[i].start_index = (uint32_t)pstart[i];
        route[i].end_index = (uint32_t)pend[i];
    }
    (*env)->ReleaseIntArrayElements(env, stepStart, pstart, JNI_ABORT);
    (*env)->ReleaseIntArrayElements(env, stepEnd, pend, JNI_ABORT);

    bool ok = tv_nav_set_route(STATE(h), geometry, (size_t)points, route, (size_t)steps);

    for (jsize i = 0; i < steps; i++) {
        if (!held[i]) {
            continue;
        }
        if (route[i].instruction) {
            (*env)->ReleaseStringUTFChars(env, held[i], route[i].instruction);
        }
        (*env)->DeleteLocalRef(env, held[i]);
    }
    free(geometry);
    free(route);
    free(held);
    return ok ? JNI_TRUE : JNI_FALSE;
}

// counts = {status, step_index, step_count}; distances = {to_next_step_m, remaining_m}
static jboolean write_progress(JNIEnv *env, const TvNavProgress *p, jintArray counts,
                               jdoubleArray distances) {
    if ((*env)->GetArrayLength(env, counts) < 3 || (*env)->GetArrayLength(env, distances) < 2) {
        return JNI_FALSE;
    }
    jint ints[3] = {p->status, (jint)p->step_index, (jint)p->step_count};
    jdouble doubles[2] = {p->distance_to_next_step_m, p->distance_remaining_m};
    (*env)->SetIntArrayRegion(env, counts, 0, 3, ints);
    (*env)->SetDoubleArrayRegion(env, distances, 0, 2, doubles);
    return JNI_TRUE;
}

JNIEXPORT jboolean JNICALL FN(navUpdate)(JNIEnv *env, jclass c, jlong h, jdouble lat, jdouble lon,
                                         jintArray counts, jdoubleArray distances) {
    (void)c;
    TvNavProgress p;
    if (!tv_nav_update(STATE(h), lat, lon, &p)) {
        return JNI_FALSE;
    }
    return write_progress(env, &p, counts, distances);
}

JNIEXPORT jboolean JNICALL FN(navProgress)(JNIEnv *env, jclass c, jlong h, jintArray counts,
                                           jdoubleArray distances) {
    (void)c;
    TvNavProgress p;
    if (!tv_nav_progress(STATE(h), &p)) {
        return JNI_FALSE;
    }
    return write_progress(env, &p, counts, distances);
}

JNIEXPORT jstring JNICALL FN(navInstruction)(JNIEnv *env, jclass c, jlong h) {
    (void)c;
    return take_string(env, tv_nav_instruction(STATE(h)));
}

JNIEXPORT void JNICALL FN(navClear)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    tv_nav_clear(STATE(h));
}

JNIEXPORT void JNICALL FN(setVectorTileUrl)(JNIEnv *env, jclass c, jlong h, jstring url) {
    (void)c;
    const char *s = (*env)->GetStringUTFChars(env, url, NULL);
    tv_map_set_vector_tile_url(STATE(h), s);
    (*env)->ReleaseStringUTFChars(env, url, s);
}

JNIEXPORT jstring JNICALL FN(vectorTileUrl)(JNIEnv *env, jclass c, jlong h, jint z, jint x,
                                            jint y) {
    (void)c;
    return take_string(env, tv_map_vector_tile_url(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y));
}

JNIEXPORT jboolean JNICALL FN(vectorCachePut)(JNIEnv *env, jclass c, jlong h, jint z, jint x,
                                              jint y, jbyteArray bytes) {
    (void)c;
    jsize len = (*env)->GetArrayLength(env, bytes);
    jbyte *buf = (*env)->GetByteArrayElements(env, bytes, NULL);
    bool ok = tv_vector_cache_put(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y,
                                  (const uint8_t *)buf, (size_t)len);
    (*env)->ReleaseByteArrayElements(env, bytes, buf, JNI_ABORT);
    return ok ? JNI_TRUE : JNI_FALSE;
}

JNIEXPORT jboolean JNICALL FN(vectorCacheHas)(JNIEnv *env, jclass c, jlong h, jint z, jint x,
                                              jint y) {
    (void)env;
    (void)c;
    return tv_vector_cache_has(STATE(h), (uint8_t)z, (uint32_t)x, (uint32_t)y) ? JNI_TRUE
                                                                              : JNI_FALSE;
}

JNIEXPORT void JNICALL FN(vectorCacheClear)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    tv_vector_cache_clear(STATE(h));
}

JNIEXPORT jint JNICALL FN(vectorFrame)(JNIEnv *env, jclass c, jlong h) {
    (void)env;
    (void)c;
    return (jint)tv_map_vector_frame(STATE(h));
}

// ints = {kind, ring_offset, ring_count, coord_offset, fill_argb, stroke_argb};
// floats = {stroke_width, point_radius}
JNIEXPORT jboolean JNICALL FN(vectorFeatureAt)(JNIEnv *env, jclass c, jlong h, jint index,
                                               jintArray ints, jfloatArray floats) {
    (void)c;
    TvVectorFeature f;
    if ((*env)->GetArrayLength(env, ints) < 6 || (*env)->GetArrayLength(env, floats) < 2) {
        return JNI_FALSE;
    }
    if (!tv_map_vector_feature_at(STATE(h), (uint32_t)index, &f)) {
        return JNI_FALSE;
    }
    jint counts[6] = {f.kind, (jint)f.ring_offset, (jint)f.ring_count, (jint)f.coord_offset,
                      (jint)f.fill_argb, (jint)f.stroke_argb};
    jfloat paint[2] = {f.stroke_width, f.point_radius};
    (*env)->SetIntArrayRegion(env, ints, 0, 6, counts);
    (*env)->SetFloatArrayRegion(env, floats, 0, 2, paint);
    return JNI_TRUE;
}

// Fills as much of out as fits and returns the frame's full length.
JNIEXPORT jint JNICALL FN(vectorCoords)(JNIEnv *env, jclass c, jlong h, jfloatArray out) {
    (void)c;
    jsize cap = (*env)->GetArrayLength(env, out);
    if (cap == 0) {
        return (jint)tv_map_vector_coords(STATE(h), NULL, 0);
    }
    jfloat *buf = (*env)->GetFloatArrayElements(env, out, NULL);
    size_t len = tv_map_vector_coords(STATE(h), buf, (size_t)cap);
    (*env)->ReleaseFloatArrayElements(env, out, buf, 0);
    return (jint)len;
}

JNIEXPORT jint JNICALL FN(vectorRings)(JNIEnv *env, jclass c, jlong h, jintArray out) {
    (void)c;
    jsize cap = (*env)->GetArrayLength(env, out);
    if (cap == 0) {
        return (jint)tv_map_vector_rings(STATE(h), NULL, 0);
    }
    // jint is signed 32-bit, tv writes uint32_t; same width, reinterpret
    jint *buf = (*env)->GetIntArrayElements(env, out, NULL);
    size_t len = tv_map_vector_rings(STATE(h), (uint32_t *)buf, (size_t)cap);
    (*env)->ReleaseIntArrayElements(env, out, buf, 0);
    return (jint)len;
}

JNIEXPORT jdouble JNICALL FN(distanceBetween)(JNIEnv *env, jclass c, jdouble lat1, jdouble lon1,
                                              jdouble lat2, jdouble lon2) {
    (void)env;
    (void)c;
    return tv_distance_between(lat1, lon1, lat2, lon2);
}

JNIEXPORT jdouble JNICALL FN(bearingBetween)(JNIEnv *env, jclass c, jdouble lat1, jdouble lon1,
                                             jdouble lat2, jdouble lon2) {
    (void)env;
    (void)c;
    return tv_bearing_between(lat1, lon1, lat2, lon2);
}
