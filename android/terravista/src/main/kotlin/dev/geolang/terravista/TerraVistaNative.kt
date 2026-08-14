package dev.geolang.terravista

/**
 * Binding to the terravista C ABI.
 *
 * The native side holds no locks of its own, so every call must be made under
 * the caller's own lock. [MapView] owns that lock.
 */
internal object TerraVistaNative {
    init {
        System.loadLibrary("terravista_ffi")
        System.loadLibrary("terravista_jni")
    }

    // touch phases, must match TV_TOUCH_* in terravista-ffi
    const val TOUCH_BEGIN = 0
    const val TOUCH_MOVE = 1
    const val TOUCH_END = 2
    const val TOUCH_CANCEL = 3

    // tracking modes, must match TV_TRACKING_* in terravista-ffi
    const val TRACKING_NONE = 0
    const val TRACKING_FOLLOW = 1
    const val TRACKING_FOLLOW_WITH_HEADING = 2
    const val TRACKING_FOLLOW_WITH_COURSE = 3

    // navigation statuses, must match TV_NAV_* in terravista-ffi
    const val NAV_ON_ROUTE = 0
    const val NAV_OFF_ROUTE = 1
    const val NAV_ARRIVED = 2

    // vector geometry kinds, must match TV_VECTOR_* in terravista-ffi
    const val VECTOR_POINT = 0
    const val VECTOR_LINE = 1
    const val VECTOR_POLYGON = 2

    @JvmStatic external fun create(width: Int, height: Int, dpr: Float): Long
    @JvmStatic external fun destroy(handle: Long)

    @JvmStatic external fun setCenter(handle: Long, latitude: Double, longitude: Double)
    @JvmStatic external fun setZoom(handle: Long, zoom: Double)
    @JvmStatic external fun setBearing(handle: Long, bearing: Double)
    @JvmStatic external fun setViewport(handle: Long, width: Int, height: Int, dpr: Float)
    @JvmStatic external fun getZoom(handle: Long): Double
    @JvmStatic external fun getCenterLat(handle: Long): Double
    @JvmStatic external fun getCenterLon(handle: Long): Double
    @JvmStatic external fun getBearing(handle: Long): Double

    @JvmStatic external fun touch(
        handle: Long,
        phase: Int,
        xs: DoubleArray?,
        ys: DoubleArray?,
        ids: LongArray?,
    ): Int

    /** Recomputes the frame's tile set. Call before [visibleTileAt]. */
    @JvmStatic external fun visibleTileCount(handle: Long): Int

    /** `zxy` receives z, x, y; `xys` receives screenX, screenY, size in device pixels. */
    @JvmStatic external fun visibleTileAt(
        handle: Long,
        index: Int,
        zxy: IntArray,
        xys: FloatArray,
    ): Boolean

    @JvmStatic external fun setTileUrl(handle: Long, template: String)
    @JvmStatic external fun tileUrl(handle: Long, z: Int, x: Int, y: Int): String?
    @JvmStatic external fun cachePut(
        handle: Long,
        z: Int,
        x: Int,
        y: Int,
        bytes: ByteArray,
        contentType: String?,
    ): Boolean

    @JvmStatic external fun cacheGet(handle: Long, z: Int, x: Int, y: Int): ByteArray?
    @JvmStatic external fun cacheClear(handle: Long)

    @JvmStatic external fun setVectorTileUrl(handle: Long, template: String)
    @JvmStatic external fun vectorTileUrl(handle: Long, z: Int, x: Int, y: Int): String?

    /** Decodes on the way in, so false means the bytes were not a vector tile. */
    @JvmStatic external fun vectorCachePut(handle: Long, z: Int, x: Int, y: Int, bytes: ByteArray): Boolean

    @JvmStatic external fun vectorCacheHas(handle: Long, z: Int, x: Int, y: Int): Boolean
    @JvmStatic external fun vectorCacheClear(handle: Long)

    /** Recomputes the frame's vector geometry. Call before the readers below. */
    @JvmStatic external fun vectorFrame(handle: Long): Int

    /**
     * `ints` receives kind, layerIndex, ringOffset, ringCount, coordOffset,
     * fillArgb, strokeArgb; `floats` receives strokeWidth and pointRadius.
     */
    @JvmStatic external fun vectorFeatureAt(
        handle: Long,
        index: Int,
        ints: IntArray,
        floats: FloatArray,
    ): Boolean

    /** Layers in the frame [vectorFrame] built, indexed by a feature's layerIndex. */
    @JvmStatic external fun vectorLayerCount(handle: Long): Int
    @JvmStatic external fun vectorLayerName(handle: Long, index: Int): String?

    /** Colours are 0xAARRGGBB, alpha zero meaning do not paint. */
    @JvmStatic external fun setLayerStyle(
        handle: Long,
        layerName: String,
        fillArgb: Int,
        strokeArgb: Int,
        strokeWidth: Float,
    ): Boolean

    /** Fills as much of `out` as fits and returns the frame's full length. */
    @JvmStatic external fun vectorCoords(handle: Long, out: FloatArray): Int

    /** Point count per ring, in the order the features reference them. */
    @JvmStatic external fun vectorRings(handle: Long, out: IntArray): Int

    /** `xy` receives screenX, screenY in device pixels, north-up like tile placements. */
    @JvmStatic external fun project(
        handle: Long,
        latitude: Double,
        longitude: Double,
        xy: FloatArray,
    ): Boolean

    @JvmStatic external fun metresPerPixel(handle: Long): Double

    @JvmStatic external fun setUserLocation(
        handle: Long,
        latitude: Double,
        longitude: Double,
        accuracyMetres: Double,
        bearingDegrees: Double,
    ): Boolean

    /** `out` receives latitude, longitude, accuracyMetres, bearingDegrees. */
    @JvmStatic external fun userLocation(handle: Long, out: DoubleArray): Boolean

    @JvmStatic external fun setTrackingMode(handle: Long, mode: Int): Boolean
    @JvmStatic external fun getTrackingMode(handle: Long): Int

    @JvmStatic external fun navSetRoute(
        handle: Long,
        lats: DoubleArray,
        lons: DoubleArray,
        stepStart: IntArray,
        stepEnd: IntArray,
        instructions: Array<String?>,
    ): Boolean

    /** `counts` receives status, stepIndex, stepCount; `distances` receives metres to next step and remaining. */
    @JvmStatic external fun navUpdate(
        handle: Long,
        latitude: Double,
        longitude: Double,
        counts: IntArray,
        distances: DoubleArray,
    ): Boolean

    @JvmStatic external fun navProgress(handle: Long, counts: IntArray, distances: DoubleArray): Boolean
    @JvmStatic external fun navInstruction(handle: Long): String?
    @JvmStatic external fun navClear(handle: Long)

    @JvmStatic external fun distanceBetween(lat1: Double, lon1: Double, lat2: Double, lon2: Double): Double
    @JvmStatic external fun bearingBetween(lat1: Double, lon1: Double, lat2: Double, lon2: Double): Double

    @JvmStatic external fun version(): String?
}
