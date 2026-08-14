package dev.geolang.terravista.sample

import android.app.Activity
import android.app.AlertDialog
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.View
import android.widget.Button
import android.widget.PopupMenu
import android.widget.TextView
import dev.geolang.terravista.CameraPosition
import dev.geolang.terravista.MAX_REGION_TILES
import dev.geolang.terravista.MapView
import dev.geolang.terravista.OfflineRegion
import dev.geolang.terravista.OnCameraChangeListener
import dev.geolang.terravista.RegionDownload
import dev.geolang.terravista.RegionDownloadListener
import dev.geolang.terravista.Route
import dev.geolang.terravista.RoutePoint
import dev.geolang.terravista.RouteStep
import dev.geolang.terravista.TrackingMode
import dev.geolang.terravista.VisibleBounds
import dev.geolang.terravista.bearingBetween

class MainActivity : Activity() {

    private companion object {
        const val TAG = "TerraVistaSample"
        const val FIX_INTERVAL_MS = 400L
        const val FIXES_PER_LEG = 20

        val BASEMAPS = listOf(
            "OSM" to "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            "Satellite" to "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
            "Topo" to "https://tile.opentopomap.org/{z}/{x}/{y}.png",
            "Voyager" to "https://basemaps.cartocdn.com/rastertiles/voyager/{z}/{x}/{y}.png",
            "Light" to "https://basemaps.cartocdn.com/light_all/{z}/{x}/{y}.png",
            "Dark" to "https://basemaps.cartocdn.com/dark_all/{z}/{x}/{y}.png",
        )

        // MapLibre's keyless demo tileset: country polygons and graticule lines,
        // served to zoom 6 and no further
        const val VECTOR_TILES = "https://demotiles.maplibre.org/tiles/{z}/{x}/{y}.pbf"
        const val VECTOR_ZOOM = 3.0
        // the SDK asks for tiles above the camera zoom on a dense screen, and
        // anything past the tileset's zoom 6 is a 404
        const val VECTOR_MAX_ZOOM = 4.0
        const val LAYER_POLL_MS = 500L

        const val LONDON_LATITUDE = 51.5074
        const val LONDON_LONGITUDE = -0.1278
        const val LONDON_ZOOM = 12.0
        const val RASTER_MAX_ZOOM = 17.0

        /** Menu group for the basemaps, so the current one shows as checked. */
        const val BASEMAP_GROUP = 1

        /** The one region this app saves: whatever was on screen at the time. */
        const val REGION_NAME = "current view"
        /** Deep enough to be worth having offline, shallow enough to be polite. */
        const val REGION_EXTRA_ZOOMS = 2

        // down whitehall and over westminster bridge
        val DEMO_ROUTE = Route(
            points = listOf(
                RoutePoint(51.5074, -0.1278),
                RoutePoint(51.5035, -0.1258),
                RoutePoint(51.5008, -0.1246),
                RoutePoint(51.5010, -0.1220),
            ),
            steps = listOf(
                RouteStep("Head south on Whitehall", 0, 1),
                RouteStep("Continue to Westminster Bridge", 1, 2),
                RouteStep("Cross the bridge", 2, 3),
            ),
        )
    }

    private lateinit var map: MapView
    private lateinit var readout: TextView
    private lateinit var downloadButton: Button
    private lateinit var compass: TextView
    private var basemap = 0
    private var download: RegionDownload? = null

    private val handler = Handler(Looper.getMainLooper())
    private var walking = false
    private var fixIndex = 0
    private var vector = false

    /** The demo route densified into evenly spaced simulated GPS fixes. */
    private val fixes: List<RoutePoint> = buildList {
        DEMO_ROUTE.points.zipWithNext { a, b ->
            for (s in 0 until FIXES_PER_LEG) {
                val t = s.toDouble() / FIXES_PER_LEG
                add(
                    RoutePoint(
                        a.latitude + (b.latitude - a.latitude) * t,
                        a.longitude + (b.longitude - a.longitude) * t,
                    ),
                )
            }
        }
        add(DEMO_ROUTE.points.last())
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        map = findViewById(R.id.map)
        readout = findViewById(R.id.readout)
        val basemapButton = findViewById<Button>(R.id.basemap)
        val navigateButton = findViewById<Button>(R.id.navigate)
        val vectorButton = findViewById<Button>(R.id.vector)
        downloadButton = findViewById(R.id.download)
        compass = findViewById(R.id.compass)

        // everything shown here comes back from the SDK through the public API
        map.onCameraChangeListener = OnCameraChangeListener { camera ->
            compass.rotation = -camera.bearing.toFloat()
            if (!walking) showCamera(camera)
        }

        findViewById<TextView>(R.id.zoomIn).setOnClickListener { zoomBy(1.0) }
        findViewById<TextView>(R.id.zoomOut).setOnClickListener { zoomBy(-1.0) }
        compass.setOnClickListener {
            map.bearing = 0.0
            Log.i(TAG, "compass reset to north")
        }

        vectorButton.setOnClickListener { if (vector) hideVectorLayer() else showVectorLayer() }

        downloadButton.setOnClickListener {
            when {
                download != null -> cancelDownload()
                savedRegion() != null -> deleteRegion()
                else -> confirmDownload()
            }
        }
        showRegionState()

        basemapButton.setOnClickListener { showBasemapMenu(it) }

        navigateButton.setOnClickListener { if (walking) stopWalk() else startWalk() }

        // seed the readout before the first gesture
        map.setCenter(LONDON_LATITUDE, LONDON_LONGITUDE)
    }

    // ── Map controls ─────────────────────────────────────────────────────────

    /** The sources by name, the current one checked. */
    private fun showBasemapMenu(anchor: View) {
        val menu = PopupMenu(this, anchor)
        BASEMAPS.forEachIndexed { index, (name, _) ->
            menu.menu.add(BASEMAP_GROUP, index, index, name)
        }
        menu.menu.setGroupCheckable(BASEMAP_GROUP, true, true)
        menu.menu.findItem(basemap)?.isChecked = true

        menu.setOnMenuItemClickListener { item ->
            basemap = item.itemId
            map.tileUrlTemplate = BASEMAPS[basemap].second
            Log.i(TAG, "basemap -> ${BASEMAPS[basemap].first}")
            // nudge the readout, switching source does not move the camera
            map.setCenter(map.cameraPosition.latitude, map.cameraPosition.longitude)
            true
        }
        menu.show()
    }

    private fun zoomBy(steps: Double) {
        map.zoom += steps
        Log.i(TAG, "zoom -> %.2f".format(map.zoom))
    }

    private fun showCamera(camera: CameraPosition) {
        val line = "%s  zoom %.2f  bearing %.1f  %.5f, %.5f".format(
            BASEMAPS[basemap].first,
            camera.zoom,
            camera.bearing,
            camera.latitude,
            camera.longitude,
        )
        readout.text = if (vector) "$line\n${vectorLayerLine()}" else line
        Log.i(TAG, line)
    }

    /**
     * Draw the demo vector tileset over the basemap, in colours of this app's
     * choosing.
     *
     * Its layers are named for its own data, not the OpenMapTiles names the SDK
     * has a built-in look for, so without [MapView.setLayerStyle] the whole
     * tileset would draw in one fallback colour.
     */
    private fun showVectorLayer() {
        vector = true
        map.vectorTileUrlTemplate = VECTOR_TILES
        map.setLayerStyle(
            layerName = "countries",
            fillColor = Color.argb(110, 76, 175, 80),
            strokeColor = Color.argb(255, 27, 94, 32),
            strokeWidth = 2f,
        )
        map.setLayerStyle(
            layerName = "geolines",
            fillColor = Color.TRANSPARENT,
            strokeColor = Color.argb(140, 33, 33, 33),
            strokeWidth = 1f,
        )
        map.maxZoom = VECTOR_MAX_ZOOM
        map.zoom = VECTOR_ZOOM
        handler.post(::pollVectorLayers)
        Log.i(TAG, "vector layer on")
    }

    private fun hideVectorLayer() {
        vector = false
        map.vectorTileUrlTemplate = null
        map.maxZoom = RASTER_MAX_ZOOM
        map.zoom = LONDON_ZOOM
        map.setCenter(LONDON_LATITUDE, LONDON_LONGITUDE)
        Log.i(TAG, "vector layer off")
    }

    /** The layer names only exist once a frame has drawn, so wait for tiles. */
    private fun pollVectorLayers() {
        if (!vector) return
        showCamera(map.cameraPosition)
        if (map.visibleVectorLayers.isEmpty()) handler.postDelayed(::pollVectorLayers, LAYER_POLL_MS)
    }

    private fun vectorLayerLine(): String {
        val layers = map.visibleVectorLayers
        return if (layers.isEmpty()) "vector layers: waiting" else "vector layers: ${layers.joinToString(", ")}"
    }

    // ── Offline region ───────────────────────────────────────────────────────

    /**
     * Offer what saving the current view would cost, and only fetch if the
     * offer is taken.
     */
    private fun confirmDownload() {
        val bounds = map.visibleBounds ?: return
        val minZoom = map.cameraPosition.zoom.toInt()
        val maxZoom = minZoom + REGION_EXTRA_ZOOMS

        val estimate = map.estimateRegion(
            bounds.minLatitude, bounds.minLongitude,
            bounds.maxLatitude, bounds.maxLongitude,
            minZoom, maxZoom,
        )
        Log.i(TAG, "region estimate z$minZoom-$maxZoom: $estimate")

        if (estimate.tileCount > MAX_REGION_TILES) {
            AlertDialog.Builder(this)
                .setTitle(R.string.region_too_big)
                .setMessage(
                    "${estimate.tileCount} tiles, over the $MAX_REGION_TILES limit. " +
                        "Zoom in and try again.",
                )
                .setPositiveButton(R.string.ok, null)
                .show()
            return
        }

        AlertDialog.Builder(this)
            .setTitle(R.string.download_title)
            .setMessage(
                "zoom $minZoom to $maxZoom\n" +
                    "${estimate.tileCount} tiles, about ${megabytes(estimate.estimatedBytes)}",
            )
            .setPositiveButton(R.string.download_confirm) { _, _ ->
                startDownload(bounds, minZoom, maxZoom, estimate.tileCount)
            }
            .setNegativeButton(R.string.download_dismiss, null)
            .show()
    }

    private fun startDownload(
        bounds: VisibleBounds,
        minZoom: Int,
        maxZoom: Int,
        estimatedTiles: Long,
    ) {
        download = map.downloadRegion(
            REGION_NAME,
            bounds.minLatitude, bounds.minLongitude,
            bounds.maxLatitude, bounds.maxLongitude,
            minZoom, maxZoom,
            object : RegionDownloadListener {
                override fun onProgress(completed: Int, failed: Int, total: Int) {
                    readout.text = "region z$minZoom-$maxZoom: $completed/$total tiles, $failed failed"
                }

                override fun onFinished(region: OfflineRegion?) {
                    download = null
                    Log.i(TAG, "region finished: $region")
                    showRegionState()
                }
            },
        )
        showRegionState()
        Log.i(TAG, "region z$minZoom-$maxZoom, $estimatedTiles tiles started")
    }

    private fun cancelDownload() {
        download?.cancel()
        readout.text = "region cancelled"
        Log.i(TAG, "region cancelled")
    }

    private fun deleteRegion() {
        map.deleteRegion(REGION_NAME)
        readout.text = "region deleted"
        Log.i(TAG, "region deleted")
        showRegionState()
    }

    private fun savedRegion(): OfflineRegion? = map.regions().firstOrNull { it.name == REGION_NAME }

    /** The button says what tapping it will do, and the readout what is held. */
    private fun showRegionState() {
        val saved = savedRegion()
        downloadButton.text = when {
            download != null -> getString(R.string.region_cancel)
            saved != null -> getString(R.string.region_delete)
            else -> getString(R.string.region_save)
        }
        if (download == null && saved != null) {
            readout.text = "region: ${saved.tileCount} tiles, ${megabytes(saved.sizeBytes)}, " +
                "cache ${megabytes(map.diskCacheBytes)}"
        }
    }

    private fun megabytes(bytes: Long): String = "%.1f MB".format(bytes / 1024.0 / 1024.0)

    /** Walks simulated fixes along [DEMO_ROUTE], driving the dot and navigation. */
    private fun startWalk() {
        if (!map.startNavigation(DEMO_ROUTE)) return
        // the walk needs street zoom, which the demo tileset does not reach
        if (vector) hideVectorLayer()
        walking = true
        fixIndex = 0
        map.zoom = 16.0
        map.trackingMode = TrackingMode.FOLLOW_WITH_COURSE
        handler.post(::step)
        Log.i(TAG, "navigation started")
    }

    private fun stopWalk() {
        walking = false
        handler.removeCallbacksAndMessages(null)
        map.stopNavigation()
        map.trackingMode = TrackingMode.NONE
        map.bearing = 0.0
        Log.i(TAG, "navigation stopped")
    }

    private fun step() {
        if (!walking) return
        val fix = fixes[fixIndex]
        val next = fixes.getOrNull(fixIndex + 1)
        val course = if (next == null) {
            Double.NaN
        } else {
            bearingBetween(fix.latitude, fix.longitude, next.latitude, next.longitude)
        }
        map.setUserLocation(fix.latitude, fix.longitude, accuracyMetres = 12.0, bearingDegrees = course)

        val progress = map.updateNavigation(fix.latitude, fix.longitude)
        if (progress != null) {
            val line = "%s  next %.0f m  left %.0f m  %s".format(
                progress.instruction,
                progress.distanceToNextStepMetres,
                progress.distanceRemainingMetres,
                progress.status,
            )
            readout.text = line
            Log.i(TAG, line)
        }

        fixIndex++
        if (fixIndex >= fixes.size) {
            stopWalk()
        } else {
            handler.postDelayed(::step, FIX_INTERVAL_MS)
        }
    }

    override fun onDestroy() {
        handler.removeCallbacksAndMessages(null)
        map.destroy()
        super.onDestroy()
    }
}
