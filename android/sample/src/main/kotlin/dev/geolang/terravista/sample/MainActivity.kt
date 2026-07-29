package dev.geolang.terravista.sample

import android.app.Activity
import android.os.Bundle
import android.util.Log
import android.widget.Button
import android.widget.TextView
import dev.geolang.terravista.MapView
import dev.geolang.terravista.OnCameraChangeListener

class MainActivity : Activity() {

    private companion object {
        const val TAG = "TerraVistaSample"

        val BASEMAPS = listOf(
            "OSM" to "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
            "Carto" to "https://basemaps.cartocdn.com/light_all/{z}/{x}/{y}.png",
        )
    }

    private lateinit var map: MapView
    private var basemap = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        map = findViewById(R.id.map)
        val readout = findViewById<TextView>(R.id.readout)
        val button = findViewById<Button>(R.id.basemap)

        // everything shown here comes back from the SDK through the public API
        map.onCameraChangeListener = OnCameraChangeListener { camera ->
            val line = "%s  zoom %.2f  bearing %.1f  %.5f, %.5f".format(
                BASEMAPS[basemap].first,
                camera.zoom,
                camera.bearing,
                camera.latitude,
                camera.longitude,
            )
            readout.text = line
            Log.i(TAG, line)
        }

        button.setOnClickListener {
            basemap = (basemap + 1) % BASEMAPS.size
            map.tileUrlTemplate = BASEMAPS[basemap].second
            Log.i(TAG, "basemap -> ${BASEMAPS[basemap].first}")
            // nudge the readout, switching source does not move the camera
            map.setCenter(map.cameraPosition.latitude, map.cameraPosition.longitude)
        }

        // seed the readout before the first gesture
        map.setCenter(51.5074, -0.1278)
    }

    override fun onDestroy() {
        map.destroy()
        super.onDestroy()
    }
}
