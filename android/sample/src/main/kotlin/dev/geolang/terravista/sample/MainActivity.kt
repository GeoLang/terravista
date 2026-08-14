package dev.geolang.terravista.sample

import android.app.Activity
import android.graphics.Color
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.widget.Button
import android.widget.TextView
import dev.geolang.terravista.CameraPosition
import dev.geolang.terravista.MapView
import dev.geolang.terravista.OnCameraChangeListener
import dev.geolang.terravista.Route
import dev.geolang.terravista.RoutePoint
import dev.geolang.terravista.RouteStep
import dev.geolang.terravista.TrackingMode
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
    private var basemap = 0

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

        // everything shown here comes back from the SDK through the public API
        map.onCameraChangeListener = OnCameraChangeListener { camera ->
            if (!walking) showCamera(camera)
        }

        vectorButton.setOnClickListener { if (vector) hideVectorLayer() else showVectorLayer() }

        basemapButton.setOnClickListener {
            basemap = (basemap + 1) % BASEMAPS.size
            map.tileUrlTemplate = BASEMAPS[basemap].second
            Log.i(TAG, "basemap -> ${BASEMAPS[basemap].first}")
            // nudge the readout, switching source does not move the camera
            map.setCenter(map.cameraPosition.latitude, map.cameraPosition.longitude)
        }

        navigateButton.setOnClickListener { if (walking) stopWalk() else startWalk() }

        // seed the readout before the first gesture
        map.setCenter(LONDON_LATITUDE, LONDON_LONGITUDE)
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
