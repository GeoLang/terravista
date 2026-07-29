package dev.geolang.terravista

/** Where the map is looking. */
data class CameraPosition(
    val latitude: Double,
    val longitude: Double,
    val zoom: Double,
    val bearing: Double,
)

/** Notified whenever the camera moves, from gestures or from code. */
fun interface OnCameraChangeListener {
    fun onCameraChange(camera: CameraPosition)
}
