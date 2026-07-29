package dev.geolang.terravista

import android.util.Log
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL

/**
 * Plain HTTP tile fetch.
 *
 * The core builds the URL and stores the bytes, it never touches the network,
 * so this is the whole of the transport layer.
 */
internal class TileFetcher {

    private companion object {
        const val TAG = "TerraVista"
        const val CONNECT_TIMEOUT_MS = 10_000
        const val READ_TIMEOUT_MS = 15_000

        /** Tile servers reject the default Java agent, openstreetmap.org with a 403. */
        val USER_AGENT =
            "TerraVista/${BuildConfig.LIB_VERSION} (+https://github.com/GeoLang/terravista)"
    }

    fun get(url: String): ByteArray? {
        val connection = URL(url).openConnection() as HttpURLConnection
        try {
            connection.setRequestProperty("User-Agent", USER_AGENT)
            connection.connectTimeout = CONNECT_TIMEOUT_MS
            connection.readTimeout = READ_TIMEOUT_MS

            val code = connection.responseCode
            if (code != HttpURLConnection.HTTP_OK) {
                Log.w(TAG, "HTTP $code for $url")
                return null
            }

            val out = ByteArrayOutputStream()
            connection.inputStream.use { input ->
                val chunk = ByteArray(8192)
                while (true) {
                    val read = input.read(chunk)
                    if (read == -1) break
                    out.write(chunk, 0, read)
                }
            }
            return out.toByteArray()
        } finally {
            connection.disconnect()
        }
    }
}
