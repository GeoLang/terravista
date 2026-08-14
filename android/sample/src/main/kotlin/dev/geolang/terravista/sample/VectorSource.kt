package dev.geolang.terravista.sample

import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import org.json.JSONException
import org.json.JSONObject

/** A vector tileset: where its tiles are now, how deep they go, and who it credits. */
data class VectorSource(
    val name: String,
    val tileUrlTemplate: String,
    val tilesetMaxZoom: Int,
    val attribution: String,
)

private const val CONNECT_TIMEOUT_MS = 10_000
private const val READ_TIMEOUT_MS = 15_000
private const val DEEPEST_TILESET_ZOOM = 24
private const val UNNAMED_TILESET = "vector tiles"

/**
 * Read a tileset's current tile url from its TileJSON, null if the network or
 * the document does not give one.
 *
 * Blocks, so call it off the main thread.
 */
fun fetchVectorSource(tileJsonUrl: String): VectorSource? = try {
    val connection = URL(tileJsonUrl).openConnection() as HttpURLConnection
    try {
        connection.connectTimeout = CONNECT_TIMEOUT_MS
        connection.readTimeout = READ_TIMEOUT_MS
        if (connection.responseCode != HttpURLConnection.HTTP_OK) {
            null
        } else {
            parseTileJson(connection.inputStream.bufferedReader().use { it.readText() })
        }
    } finally {
        connection.disconnect()
    }
} catch (error: IOException) {
    null
}

/** Null for anything this sample cannot draw: no tile url, no usable max zoom. */
fun parseTileJson(body: String): VectorSource? {
    val document = try {
        JSONObject(body)
    } catch (error: JSONException) {
        return null
    }
    val template = document.optJSONArray("tiles")?.optString(0).orEmpty()
    val maxZoom = document.optInt("maxzoom", 0)
    if (template.isBlank() || maxZoom !in 1..DEEPEST_TILESET_ZOOM) return null
    return VectorSource(
        name = document.optString("name", UNNAMED_TILESET),
        tileUrlTemplate = template,
        tilesetMaxZoom = maxZoom,
        attribution = document.optString("attribution"),
    )
}
