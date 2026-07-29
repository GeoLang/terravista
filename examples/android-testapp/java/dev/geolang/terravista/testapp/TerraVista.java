package dev.geolang.terravista.testapp;

/** Static binding to the terravista C ABI. All map math lives behind these calls. */
public final class TerraVista {
    static {
        System.loadLibrary("terravista_ffi");
        System.loadLibrary("terravista_jni");
    }

    private TerraVista() {}

    // touch phases, must match TV_TOUCH_* in terravista-ffi
    public static final int TOUCH_BEGIN = 0;
    public static final int TOUCH_MOVE = 1;
    public static final int TOUCH_END = 2;
    public static final int TOUCH_CANCEL = 3;

    // gesture kinds, must match TV_GESTURE_* in terravista-ffi
    public static final int GESTURE_NONE = 0;
    public static final int GESTURE_PAN = 1;
    public static final int GESTURE_ZOOM = 2;
    public static final int GESTURE_ROTATE = 3;
    public static final int GESTURE_PITCH = 4;

    public static native long create(int width, int height, float dpr);
    public static native void destroy(long handle);

    public static native void setCenter(long handle, double lat, double lon);
    public static native void setZoom(long handle, double zoom);
    public static native void setViewport(long handle, int width, int height, float dpr);
    public static native double getZoom(long handle);
    public static native double getCenterLat(long handle);
    public static native double getCenterLon(long handle);
    public static native double getBearing(long handle);
    public static native double getPitch(long handle);

    public static native int touch(long handle, int phase, double[] xs, double[] ys, long[] ids);

    /** out = {zoom, xMin, xMax, yMin, yMax} */
    public static native boolean tileRange(long handle, int[] out);

    /** Recomputes the visible set. Call once per frame before visibleTileAt. */
    public static native int visibleTileCount(long handle);

    /** zxy = {z, x, y}, xys = {screenX, screenY, size} in device pixels. */
    public static native boolean visibleTileAt(long handle, int index, int[] zxy, float[] xys);

    public static native void setTileUrl(long handle, String template);
    public static native String tileUrl(long handle, int z, int x, int y);
    public static native boolean cachePut(long handle, int z, int x, int y, byte[] bytes, String contentType);
    public static native boolean cacheHas(long handle, int z, int x, int y);
    public static native byte[] cacheGet(long handle, int z, int x, int y);
    public static native int cacheTileCount(long handle);
    public static native long cacheSizeBytes(long handle);

    public static native String version();
}
