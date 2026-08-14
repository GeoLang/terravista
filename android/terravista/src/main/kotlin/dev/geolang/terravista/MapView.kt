package dev.geolang.terravista

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
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
    private val vectorInFlight: MutableSet<Long> = ConcurrentHashMap.newKeySet()
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
    private val locationOut = DoubleArray(4)
    private val pointXY = FloatArray(2)
    private val navCounts = IntArray(3)
    private val navDistances = DoubleArray(2)

    // one frame's vector geometry, grown to fit and reused
    private val vectorFeature = IntArray(7)
    private val vectorPaintValues = FloatArray(2)
    private var vectorCoords = FloatArray(0)
    private var vectorRings = IntArray(0)
    private val vectorPath = Path()
    private val fillPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { style = Paint.Style.FILL }
    private val strokePaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        style = Paint.Style.STROKE
        strokeJoin = Paint.Join.ROUND
        strokeCap = Paint.Cap.ROUND
    }

    private val accuracyPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.argb(38, 66, 133, 244)
    }
    private val dotRingPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.WHITE }
    private val dotPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        color = Color.rgb(66, 133, 244)
    }
    private val headingPath = Path()

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

    /**
     * XYZ template for a vector tile source, for example
     * `https://tiles.example.com/{z}/{x}/{y}.mvt`, or null for none.
     *
     * Vector tiles draw over the raster ones, so a map can carry both. Features
     * are drawn with a fixed look per layer name, which [setLayerStyle]
     * overrides; there is no style spec and no labels.
     */
    var vectorTileUrlTemplate: String? = null
        set(value) {
            field = value
            synchronized(sdk) {
                if (handle == 0L) return@synchronized
                TerraVistaNative.setVectorTileUrl(handle, value ?: "")
                TerraVistaNative.vectorCacheClear(handle)
            }
            vectorInFlight.clear()
            invalidate()
        }

    /**
     * Set how one vector layer draws, by the layer's name in the source.
     *
     * Colours are Android colour ints, and an alpha of zero means do not paint,
     * so a transparent fill leaves a polygon as an outline. [strokeWidth] is in
     * device pixels, like [Paint.setStrokeWidth]. A name the source does not
     * serve is kept anyway, ready for a source that serves it.
     *
     * Returns false when the native map has been destroyed.
     */
    fun setLayerStyle(
        layerName: String,
        fillColor: Int,
        strokeColor: Int,
        strokeWidth: Float = 1.5f,
    ): Boolean {
        val ok = synchronized(sdk) {
            handle != 0L &&
                TerraVistaNative.setLayerStyle(handle, layerName, fillColor, strokeColor, strokeWidth)
        }
        if (ok) invalidate()
        return ok
    }

    /**
     * Names of the layers the last drawn vector frame held, in draw order.
     *
     * Empty when no vector source is set, or when nothing of it covers the
     * screen.
     */
    val visibleVectorLayers: List<String>
        get() = synchronized(sdk) {
            if (handle == 0L) {
                emptyList()
            } else {
                List(TerraVistaNative.vectorLayerCount(handle)) {
                    TerraVistaNative.vectorLayerName(handle, it) ?: ""
                }
            }
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

    // ── Location & navigation ────────────────────────────────────────────────

    /** How the camera follows location fixes. Switching snaps onto the last fix. */
    var trackingMode: TrackingMode
        get() = synchronized(sdk) {
            if (handle == 0L) {
                TrackingMode.NONE
            } else {
                TrackingMode.fromCode(TerraVistaNative.getTrackingMode(handle))
            }
        }
        set(value) {
            synchronized(sdk) {
                if (handle == 0L) return@synchronized
                TerraVistaNative.setTrackingMode(handle, value.code)
            }
            onCameraMoved()
        }

    /**
     * Feed a location fix, for the position dot and for camera tracking.
     *
     * The SDK never reads a platform location provider; the app owns the
     * permission and the provider and hands each fix in. Pass a negative
     * [accuracyMetres] or a NaN [bearingDegrees] when unknown.
     */
    fun setUserLocation(
        latitude: Double,
        longitude: Double,
        accuracyMetres: Double = -1.0,
        bearingDegrees: Double = Double.NaN,
    ) {
        val follows = synchronized(sdk) {
            if (handle == 0L) return
            TerraVistaNative.setUserLocation(handle, latitude, longitude, accuracyMetres, bearingDegrees)
            TerraVistaNative.getTrackingMode(handle) != TerraVistaNative.TRACKING_NONE
        }
        if (follows) onCameraMoved() else invalidate()
    }

    /**
     * Start following [route], replacing any current one.
     *
     * Returns false and keeps the previous route when [route] has fewer than
     * two points or no steps. Feed fixes to [updateNavigation] for progress.
     */
    fun startNavigation(route: Route): Boolean {
        val lats = DoubleArray(route.points.size)
        val lons = DoubleArray(route.points.size)
        route.points.forEachIndexed { i, p ->
            lats[i] = p.latitude
            lons[i] = p.longitude
        }
        val starts = IntArray(route.steps.size)
        val ends = IntArray(route.steps.size)
        val instructions = arrayOfNulls<String>(route.steps.size)
        route.steps.forEachIndexed { i, s ->
            starts[i] = s.startIndex
            ends[i] = s.endIndex
            instructions[i] = s.instruction
        }
        return synchronized(sdk) {
            handle != 0L && TerraVistaNative.navSetRoute(handle, lats, lons, starts, ends, instructions)
        }
    }

    /**
     * Advance navigation with a location fix.
     *
     * Independent of [setUserLocation]: feed each fix to both when a dot and
     * navigation are both wanted. Returns null when no route is set.
     */
    fun updateNavigation(latitude: Double, longitude: Double): NavProgress? =
        synchronized(sdk) {
            if (handle == 0L ||
                !TerraVistaNative.navUpdate(handle, latitude, longitude, navCounts, navDistances)
            ) {
                return null
            }
            readProgressLocked()
        }

    /** Progress from the last [updateNavigation], or null before the first fix. */
    val navigationProgress: NavProgress?
        get() = synchronized(sdk) {
            if (handle == 0L || !TerraVistaNative.navProgress(handle, navCounts, navDistances)) {
                return null
            }
            readProgressLocked()
        }

    /** Drop the current route and its progress. */
    fun stopNavigation() {
        synchronized(sdk) {
            if (handle == 0L) return
            TerraVistaNative.navClear(handle)
        }
    }

    /** Caller must hold [sdk] and have filled [navCounts] and [navDistances]. */
    private fun readProgressLocked(): NavProgress =
        NavProgress(
            status = NavStatus.fromCode(navCounts[0]),
            stepIndex = navCounts[1],
            stepCount = navCounts[2],
            distanceToNextStepMetres = navDistances[0],
            distanceRemainingMetres = navDistances[1],
            instruction = TerraVistaNative.navInstruction(handle) ?: "",
        )

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
        var vectorTemplate: String? = null

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
                vectorTemplate = a.getString(R.styleable.MapView_tvVectorTileUrlTemplate)
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
        // through the setters, so the core and the properties agree from the start
        tileUrlTemplate = template
        vectorTileUrlTemplate = vectorTemplate
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

            if (vectorTileUrlTemplate != null) requestVectorTile(z, x, y, key)
        }

        drawVectorFeatures(canvas)
        drawUserLocation(canvas)

        canvas.restore()
    }

    /**
     * Draw the decoded vector tiles the core placed for this frame.
     *
     * The core hands back flat arrays: one entry per feature, its geometry in
     * a shared coordinate pool, split into rings.
     */
    private fun drawVectorFeatures(canvas: Canvas) {
        if (vectorTileUrlTemplate == null) return

        val count = synchronized(sdk) {
            if (handle == 0L) 0 else TerraVistaNative.vectorFrame(handle)
        }
        if (count == 0) return

        synchronized(sdk) {
            if (handle == 0L) return
            val coords = TerraVistaNative.vectorCoords(handle, vectorCoords)
            if (coords > vectorCoords.size) {
                vectorCoords = FloatArray(coords)
                TerraVistaNative.vectorCoords(handle, vectorCoords)
            }
            val rings = TerraVistaNative.vectorRings(handle, vectorRings)
            if (rings > vectorRings.size) {
                vectorRings = IntArray(rings)
                TerraVistaNative.vectorRings(handle, vectorRings)
            }
        }

        for (i in 0 until count) {
            val ok = synchronized(sdk) {
                handle != 0L &&
                    TerraVistaNative.vectorFeatureAt(handle, i, vectorFeature, vectorPaintValues)
            }
            if (ok) drawVectorFeature(canvas)
        }
    }

    /** Draws whatever [vectorFeature] and [vectorPaintValues] currently hold. */
    private fun drawVectorFeature(canvas: Canvas) {
        val kind = vectorFeature[0]
        val ringOffset = vectorFeature[2]
        val ringCount = vectorFeature[3]
        val fill = vectorFeature[5]
        val stroke = vectorFeature[6]
        val strokeWidth = vectorPaintValues[0]
        var coord = vectorFeature[4]

        if (coord + 2 > vectorCoords.size) return
        fillPaint.color = fill
        strokePaint.color = stroke
        strokePaint.strokeWidth = strokeWidth

        if (kind == TerraVistaNative.VECTOR_POINT) {
            val radius = vectorPaintValues[1]
            val x = vectorCoords[coord]
            val y = vectorCoords[coord + 1]
            if (Color.alpha(fill) > 0) canvas.drawCircle(x, y, radius, fillPaint)
            if (Color.alpha(stroke) > 0 && strokeWidth > 0f) {
                canvas.drawCircle(x, y, radius, strokePaint)
            }
            return
        }

        vectorPath.rewind()
        vectorPath.fillType = Path.FillType.EVEN_ODD
        for (r in 0 until ringCount) {
            val points = vectorRings.getOrElse(ringOffset + r) { 0 }
            if (points == 0 || coord + points * 2 > vectorCoords.size) return
            vectorPath.moveTo(vectorCoords[coord], vectorCoords[coord + 1])
            for (p in 1 until points) {
                vectorPath.lineTo(vectorCoords[coord + p * 2], vectorCoords[coord + p * 2 + 1])
            }
            if (kind == TerraVistaNative.VECTOR_POLYGON) vectorPath.close()
            coord += points * 2
        }

        if (kind == TerraVistaNative.VECTOR_POLYGON && Color.alpha(fill) > 0) {
            canvas.drawPath(vectorPath, fillPaint)
        }
        if (Color.alpha(stroke) > 0 && strokeWidth > 0f) {
            canvas.drawPath(vectorPath, strokePaint)
        }
    }

    /** Draws in the same north-up frame as the tiles, so the canvas rotation applies. */
    private fun drawUserLocation(canvas: Canvas) {
        var metresPerPixel = 0.0
        val visible = synchronized(sdk) {
            if (handle == 0L || !TerraVistaNative.userLocation(handle, locationOut)) {
                return@synchronized false
            }
            if (!TerraVistaNative.project(handle, locationOut[0], locationOut[1], pointXY)) {
                return@synchronized false
            }
            metresPerPixel = TerraVistaNative.metresPerPixel(handle)
            true
        }
        if (!visible) return

        val x = pointXY[0]
        val y = pointXY[1]
        val dotRadius = 7f * density

        val accuracy = locationOut[2]
        if (accuracy > 0 && metresPerPixel > 0) {
            val radius = (accuracy / metresPerPixel).toFloat()
            if (radius > dotRadius) canvas.drawCircle(x, y, radius, accuracyPaint)
        }

        val bearing = locationOut[3]
        if (!bearing.isNaN()) {
            headingPath.rewind()
            headingPath.moveTo(x, y - 2.6f * dotRadius)
            headingPath.lineTo(x - 1.1f * dotRadius, y - 1.1f * dotRadius)
            headingPath.lineTo(x + 1.1f * dotRadius, y - 1.1f * dotRadius)
            headingPath.close()
            canvas.save()
            canvas.rotate(bearing.toFloat(), x, y)
            canvas.drawPath(headingPath, dotPaint)
            canvas.restore()
        }

        canvas.drawCircle(x, y, dotRadius + 2f * density, dotRingPaint)
        canvas.drawCircle(x, y, dotRadius, dotPaint)
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

    private fun requestVectorTile(z: Int, x: Int, y: Int, key: Long) {
        val held = synchronized(sdk) {
            handle == 0L || TerraVistaNative.vectorCacheHas(handle, z, x, y)
        }
        if (held || !vectorInFlight.add(key)) return

        val url = synchronized(sdk) {
            if (handle == 0L) null else TerraVistaNative.vectorTileUrl(handle, z, x, y)
        }
        if (url.isNullOrEmpty() || fetchers.isShutdown) {
            vectorInFlight.remove(key)
            return
        }

        fetchers.submit {
            try {
                val body = tiles.get(url)
                if (body != null) {
                    val ok = synchronized(sdk) {
                        handle != 0L && TerraVistaNative.vectorCachePut(handle, z, x, y, body)
                    }
                    if (ok) postInvalidate() else Log.w(TAG, "vector tile $z/$x/$y did not decode")
                }
            } catch (e: Exception) {
                Log.w(TAG, "vector tile $z/$x/$y failed: $e")
            } finally {
                vectorInFlight.remove(key)
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
