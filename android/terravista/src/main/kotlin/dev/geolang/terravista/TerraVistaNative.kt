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

    @JvmStatic external fun version(): String?
}
