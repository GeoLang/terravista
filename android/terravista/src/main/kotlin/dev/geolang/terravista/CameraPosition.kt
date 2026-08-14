package dev.geolang.terravista

/** Where the map is looking. */
data class CameraPosition(
    val latitude: Double,
    val longitude: Double,
    val zoom: Double,
    val bearing: Double,
)

/** The geographic box the map is showing. */
data class VisibleBounds(
    val minLatitude: Double,
    val minLongitude: Double,
    val maxLatitude: Double,
    val maxLongitude: Double,
)

/** Notified whenever the camera moves, from gestures or from code. */
fun interface OnCameraChangeListener {
    fun onCameraChange(camera: CameraPosition)
}
