package dev.geolang.terravista

/** Great-circle distance between two coordinates, in metres. */
fun distanceBetween(lat1: Double, lon1: Double, lat2: Double, lon2: Double): Double =
    TerraVistaNative.distanceBetween(lat1, lon1, lat2, lon2)

/** Initial bearing from the first coordinate to the second, in degrees from north. */
fun bearingBetween(lat1: Double, lon1: Double, lat2: Double, lon2: Double): Double =
    TerraVistaNative.bearingBetween(lat1, lon1, lat2, lon2)

/** How the camera follows the user location fed to [MapView.setUserLocation]. */
enum class TrackingMode(internal val code: Int) {
    /** The camera stays where it is. */
    NONE(TerraVistaNative.TRACKING_NONE),

    /** The camera centres on each location fix. */
    FOLLOW(TerraVistaNative.TRACKING_FOLLOW),

    /** Centre on each fix and rotate the map to the compass heading fed in as the bearing. */
    FOLLOW_WITH_HEADING(TerraVistaNative.TRACKING_FOLLOW_WITH_HEADING),

    /** Centre on each fix and rotate the map to the direction of travel fed in as the bearing. */
    FOLLOW_WITH_COURSE(TerraVistaNative.TRACKING_FOLLOW_WITH_COURSE),
    ;

    internal companion object {
        fun fromCode(code: Int): TrackingMode = entries.first { it.code == code }
    }
}

/** One vertex of a route's geometry. */
data class RoutePoint(val latitude: Double, val longitude: Double)

/** One turn instruction, covering the geometry from [startIndex] to [endIndex]. */
data class RouteStep(val instruction: String, val startIndex: Int, val endIndex: Int)

/**
 * A route to follow. The SDK follows routes, it does not compute them; get one
 * from a router such as Itinera and hand it to [MapView.startNavigation].
 */
data class Route(val points: List<RoutePoint>, val steps: List<RouteStep>)

enum class NavStatus {
    ON_ROUTE,
    OFF_ROUTE,
    ARRIVED,
    ;

    internal companion object {
        fun fromCode(code: Int): NavStatus = when (code) {
            TerraVistaNative.NAV_OFF_ROUTE -> OFF_ROUTE
            TerraVistaNative.NAV_ARRIVED -> ARRIVED
            else -> ON_ROUTE
        }
    }
}

/** Progress along the current route, from [MapView.updateNavigation]. */
data class NavProgress(
    val status: NavStatus,
    val stepIndex: Int,
    val stepCount: Int,
    val distanceToNextStepMetres: Double,
    val distanceRemainingMetres: Double,
    val instruction: String,
)
