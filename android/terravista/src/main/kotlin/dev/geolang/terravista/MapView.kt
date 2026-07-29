package dev.geolang.terravista

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Rect
import android.graphics.RectF
import android.util.AttributeSet
import android.util.Log
import android.view.MotionEvent
import android.view.View
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors

/**
 * A pannable, pinch-zoomable raster map.
 *
 * All map maths comes from the TerraVista Rust core: touches drive its gesture
 * recognizer, and the visible tiles and their screen placements come back from
 * it. This view fetches tiles over HTTP, decodes them, and draws them where the
 * core says.
 *
 * Call [destroy] when the hosting activity is finished, to free the native map.
 *
 * ```xml
 * <dev.geolang.terravista.MapView
 *     android:layout_width="match_parent"
 *     android:layout_height="match_parent"
 *     app:tvCenterLatitude="51.5074"
 *     app:tvCenterLongitude="-0.1278"
 *     app:tvZoom="12" />
 * ```
 */
class MapView @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0,
) : View(context, attrs, defStyleAttr) {

    private companion object {
        const val TAG = "TerraVista"
        const val MAX_DECODED_TILES = 128
        const val MAX_PARENT_LEVELS = 4
        const val FETCH_THREADS = 4
        const val DEFAULT_TILE_URL = "https://tile.openstreetmap.org/{z}/{x}/{y}.png"
    }

    /** The core is not thread safe, so every native call happens under this. */
    private val sdk = Any()
    private var handle: Long = 0L

    private val density: Float = resources.displayMetrics.density

    private val fetchers = Executors.newFixedThreadPool(FETCH_THREADS)
    private val inFlight: MutableSet<Long> = ConcurrentHashMap.newKeySet()
    private val tiles = TileFetcher()

    private val decoded = object : LinkedHashMap<Long, Bitmap>(16, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Long, Bitmap>?): Boolean =
            size > MAX_DECODED_TILES
    }

    private val tilePaint = Paint(Paint.FILTER_BITMAP_FLAG)

    // scratch, so drawing a frame allocates nothing per tile
    private val zxy = IntArray(3)
    private val xys = FloatArray(3)
    private val dst = RectF()
    private val src = Rect()

    // ── Public API ───────────────────────────────────────────────────────────

    /** Deepest the user may zoom in. */
    var maxZoom: Double = 18.0
        set(value) {
            field = value
            clampZoomIntoRange()
        }

    /** Furthest the user may zoom out. */
    var minZoom: Double = 0.0
        set(value) {
            field = value
            clampZoomIntoRange()
        }

    /**
     * XYZ tile template, for example `https://tile.example.com/{z}/{x}/{y}.png`.
     *
     * Changing this drops every cached tile, because the cache is keyed by tile
     * coordinate alone and would otherwise serve the previous basemap's images.
     */
    var tileUrlTemplate: String = DEFAULT_TILE_URL
        set(value) {
            field = value
            synchronized(sdk) {
                if (handle == 0L) return@synchronized
                TerraVistaNative.setTileUrl(handle, value)
                TerraVistaNative.cacheClear(handle)
            }
            inFlight.clear()
            decoded.clear()
            invalidate()
        }

    var zoom: Double
        get() = synchronized(sdk) { if (handle == 0L) 0.0 else TerraVistaNative.getZoom(handle) }
        set(value) {
            synchronized(sdk) {
                if (handle == 0L) return@synchronized
                TerraVistaNative.setZoom(handle, value.coerceIn(minZoom, maxZoom))
            }
            onCameraMoved()
        }

    var bearing: Double
        get() = synchronized(sdk) { if (handle == 0L) 0.0 else TerraVistaNative.getBearing(handle) }
        set(value) {
            synchronized(sdk) {
                if (handle == 0L) return@synchronized
                TerraVistaNative.setBearing(handle, value)
            }
            onCameraMoved()
        }

    /** Current camera, read back from the core. */
    val cameraPosition: CameraPosition
        get() = synchronized(sdk) { readCameraLocked() }

    var onCameraChangeListener: OnCameraChangeListener? = null

    /** Move the map so this coordinate sits at the centre of the view. */
    fun setCenter(latitude: Double, longitude: Double) {
        synchronized(sdk) {
            if (handle == 0L) return@synchronized
            TerraVistaNative.setCenter(handle, latitude, longitude)
        }
        onCameraMoved()
    }

    /** Free the native map. Idempotent, and the view draws nothing afterwards. */
    fun destroy() {
        fetchers.shutdownNow()
        synchronized(sdk) {
            if (handle != 0L) {
                TerraVistaNative.destroy(handle)
                handle = 0L
            }
        }
        decoded.clear()
    }

    // ── Setup ────────────────────────────────────────────────────────────────

    init {
        setBackgroundColor(Color.rgb(233, 229, 220))

        var latitude = 0.0
        var longitude = 0.0
        var initialZoom = 2.0
        var initialBearing = 0.0
        var template = DEFAULT_TILE_URL

        if (attrs != null) {
            val a = context.obtainStyledAttributes(attrs, R.styleable.MapView)
            try {
                latitude = a.getFloat(R.styleable.MapView_tvCenterLatitude, 0f).toDouble()
                longitude = a.getFloat(R.styleable.MapView_tvCenterLongitude, 0f).toDouble()
                initialZoom = a.getFloat(R.styleable.MapView_tvZoom, 2f).toDouble()
                initialBearing = a.getFloat(R.styleable.MapView_tvBearing, 0f).toDouble()
                minZoom = a.getFloat(R.styleable.MapView_tvMinZoom, 0f).toDouble()
                maxZoom = a.getFloat(R.styleable.MapView_tvMaxZoom, 18f).toDouble()
                a.getString(R.styleable.MapView_tvTileUrlTemplate)?.let { template = it }
            } finally {
                a.recycle()
            }
        }

        synchronized(sdk) {
            handle = TerraVistaNative.create(1, 1, density)
            TerraVistaNative.setCenter(handle, latitude, longitude)
            TerraVistaNative.setZoom(handle, initialZoom.coerceIn(minZoom, maxZoom))
            TerraVistaNative.setBearing(handle, initialBearing)
        }
        // through the setter, so the core and the property agree from the start
        tileUrlTemplate = template
    }

    override fun onSizeChanged(w: Int, h: Int, oldW: Int, oldH: Int) {
        super.onSizeChanged(w, h, oldW, oldH)
        synchronized(sdk) {
            if (handle == 0L) return@synchronized
            TerraVistaNative.setViewport(handle, w, h, density)
        }
    }

    // ── Input ────────────────────────────────────────────────────────────────

    override fun onTouchEvent(event: MotionEvent): Boolean {
        val phase = when (event.actionMasked) {
            MotionEvent.ACTION_DOWN, MotionEvent.ACTION_POINTER_DOWN -> TerraVistaNative.TOUCH_BEGIN
            MotionEvent.ACTION_MOVE -> TerraVistaNative.TOUCH_MOVE
            MotionEvent.ACTION_UP -> TerraVistaNative.TOUCH_END
            MotionEvent.ACTION_CANCEL -> TerraVistaNative.TOUCH_CANCEL
            MotionEvent.ACTION_POINTER_UP -> {
                // restart the gesture with the fingers still down, so lifting
                // one of two keeps panning with the other
                sendTouch(TerraVistaNative.TOUCH_BEGIN, event, event.actionIndex)
                return true
            }
            else -> return true
        }
        sendTouch(phase, event, -1)
        return true
    }

    private fun sendTouch(phase: Int, event: MotionEvent, skipIndex: Int) {
        val total = event.pointerCount
        val n = if (skipIndex >= 0) total - 1 else total
        val xs = DoubleArray(n)
        val ys = DoubleArray(n)
        val ids = LongArray(n)

        var out = 0
        for (i in 0 until total) {
            if (i == skipIndex) continue
            xs[out] = event.getX(i).toDouble()
            ys[out] = event.getY(i).toDouble()
            ids[out] = event.getPointerId(i).toLong()
            out++
        }

        synchronized(sdk) {
            if (handle == 0L) return
            TerraVistaNative.touch(handle, phase, xs, ys, ids)
            // a pinch can overshoot the allowed range
            val z = TerraVistaNative.getZoom(handle)
            val clamped = z.coerceIn(minZoom, maxZoom)
            if (clamped != z) TerraVistaNative.setZoom(handle, clamped)
        }
        onCameraMoved()
    }

    // ── Drawing ──────────────────────────────────────────────────────────────

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        val count: Int
        val currentBearing: Double
        synchronized(sdk) {
            if (handle == 0L) return
            count = TerraVistaNative.visibleTileCount(handle)
            currentBearing = TerraVistaNative.getBearing(handle)
        }

        // placements come back north-up, so spin the canvas to face the bearing
        canvas.save()
        if (currentBearing != 0.0) {
            canvas.rotate(-currentBearing.toFloat(), width / 2f, height / 2f)
        }

        for (i in 0 until count) {
            val ok = synchronized(sdk) {
                handle != 0L && TerraVistaNative.visibleTileAt(handle, i, zxy, xys)
            }
            if (!ok) continue

            val z = zxy[0]
            val x = zxy[1]
            val y = zxy[2]
            val key = tileKey(z, x, y)
            var bitmap = decoded[key]

            if (bitmap == null) {
                val bytes = synchronized(sdk) {
                    if (handle == 0L) null else TerraVistaNative.cacheGet(handle, z, x, y)
                }
                if (bytes != null) {
                    bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
                    if (bitmap != null) decoded[key] = bitmap
                } else {
                    requestTile(z, x, y, key)
                }
            }

            dst.set(xys[0], xys[1], xys[0] + xys[2], xys[1] + xys[2])
            if (bitmap != null) {
                canvas.drawBitmap(bitmap, null, dst, tilePaint)
            } else {
                drawParentTile(canvas, z, x, y)
            }
        }

        canvas.restore()
    }

    /**
     * Blit the matching crop of an already-decoded lower-zoom tile, so panning
     * and over-zooming show blurry map instead of blank space.
     */
    private fun drawParentTile(canvas: Canvas, z: Int, x: Int, y: Int) {
        for (up in 1..MAX_PARENT_LEVELS) {
            if (z - up < 0) return
            val parent = decoded[tileKey(z - up, x shr up, y shr up)] ?: continue

            val span = 1 shl up
            val cell = parent.width / span
            if (cell < 1) return
            val left = (x and (span - 1)) * cell
            val top = (y and (span - 1)) * cell
            src.set(left, top, left + cell, top + cell)
            canvas.drawBitmap(parent, src, dst, tilePaint)
            return
        }
    }

    // ── Tiles ────────────────────────────────────────────────────────────────

    private fun requestTile(z: Int, x: Int, y: Int, key: Long) {
        if (!inFlight.add(key)) return

        val url = synchronized(sdk) {
            if (handle == 0L) null else TerraVistaNative.tileUrl(handle, z, x, y)
        }
        if (url.isNullOrEmpty()) {
            inFlight.remove(key)
            return
        }

        if (fetchers.isShutdown) {
            inFlight.remove(key)
            return
        }

        fetchers.submit {
            try {
                val body = tiles.get(url)
                if (body != null) {
                    synchronized(sdk) {
                        if (handle != 0L) {
                            TerraVistaNative.cachePut(handle, z, x, y, body, "image/png")
                        }
                    }
                    postInvalidate()
                }
            } catch (e: Exception) {
                Log.w(TAG, "tile $z/$x/$y failed: $e")
            } finally {
                inFlight.remove(key)
            }
        }
    }

    private fun tileKey(z: Int, x: Int, y: Int): Long =
        (z.toLong() shl 58) or (x.toLong() shl 29) or y.toLong()

    // ── Camera plumbing ──────────────────────────────────────────────────────

    /** Caller must hold [sdk]. */
    private fun readCameraLocked(): CameraPosition =
        if (handle == 0L) {
            CameraPosition(0.0, 0.0, 0.0, 0.0)
        } else {
            CameraPosition(
                latitude = TerraVistaNative.getCenterLat(handle),
                longitude = TerraVistaNative.getCenterLon(handle),
                zoom = TerraVistaNative.getZoom(handle),
                bearing = TerraVistaNative.getBearing(handle),
            )
        }

    private fun onCameraMoved() {
        invalidate()
        val listener = onCameraChangeListener ?: return
        val camera = synchronized(sdk) { readCameraLocked() }
        listener.onCameraChange(camera)
    }

    /** Pull the current zoom back inside [minZoom]..[maxZoom] after a bound moves. */
    private fun clampZoomIntoRange() {
        val changed = synchronized(sdk) {
            if (handle == 0L) return@synchronized false
            val current = TerraVistaNative.getZoom(handle)
            val clamped = current.coerceIn(minZoom, maxZoom)
            if (clamped == current) return@synchronized false
            TerraVistaNative.setZoom(handle, clamped)
            true
        }
        if (changed) onCameraMoved()
    }
}
