package dev.geolang.terravista

import android.util.Log
import java.io.File
import java.io.IOException
import java.security.MessageDigest

/** Directory name for one tile source, so two sources never share a tile. */
internal fun sourceKey(template: String): String {
    val digest = MessageDigest.getInstance("MD5").digest(template.toByteArray())
    return digest.take(8).joinToString("") { "%02x".format(it) }
}

internal fun tileName(z: Int, x: Int, y: Int): String = "${z}_${x}_$y"

private const val TAG = "TerraVista"

/** Bytes of one tile file, or null when it is not there or unreadable. */
internal fun readTile(directory: File, z: Int, x: Int, y: Int): ByteArray? {
    val file = File(directory, tileName(z, x, y))
    return try {
        if (file.isFile) file.readBytes() else null
    } catch (e: IOException) {
        // an eviction between the check and the read is the expected race
        Log.w(TAG, "tile read ${file.path} failed: $e")
        null
    }
}

/** Write through a temporary name, so a torn write never looks like a tile. */
internal fun writeTile(directory: File, z: Int, x: Int, y: Int, bytes: ByteArray): Boolean {
    val file = File(directory, tileName(z, x, y))
    val partial = File(directory, "${tileName(z, x, y)}.part")
    try {
        directory.mkdirs()
        partial.writeBytes(bytes)
        if (partial.renameTo(file)) return true
        partial.delete()
    } catch (e: IOException) {
        Log.w(TAG, "tile write ${file.path} failed: $e")
    }
    return false
}

/**
 * Every tile the map fetches, kept on disk and read before the network.
 *
 * Lives under the app's cache directory, so the system may delete it whenever
 * it needs the space: nothing here is worth keeping at the user's expense.
 * Tiles never expire and are never revalidated, so a source that redraws its
 * imagery serves the old one here until the entry is evicted.
 *
 * Evicts least recently read first, against [maxBytes].
 */
internal class TileStore(private val root: File, @Volatile var maxBytes: Long) {

    private companion object {
        /** Evicting to just under the cap would evict again on the next tile. */
        const val EVICT_TO = 0.9
    }

    /** Bytes on disk, counted on first use. -1 until then. */
    private var totalBytes = -1L
    private val lock = Any()

    fun read(source: String, z: Int, x: Int, y: Int): ByteArray? {
        val directory = File(root, source)
        // read time is the eviction order, so a tile in use stays
        File(directory, tileName(z, x, y)).setLastModified(System.currentTimeMillis())
        return readTile(directory, z, x, y)
    }

    fun write(source: String, z: Int, x: Int, y: Int, bytes: ByteArray) {
        val directory = File(root, source)
        // a refetch replaces a tile rather than adding one
        val replaced = File(directory, tileName(z, x, y)).length()
        if (!writeTile(directory, z, x, y, bytes)) return

        synchronized(lock) {
            if (totalBytes < 0) totalBytes = measure()
            totalBytes += bytes.size - replaced
            if (totalBytes > maxBytes) evictTo((maxBytes * EVICT_TO).toLong())
        }
    }

    fun sizeBytes(): Long = synchronized(lock) {
        if (totalBytes < 0) totalBytes = measure()
        totalBytes
    }

    /** Caller must hold [lock]. */
    private fun evictTo(target: Long) {
        val files = root.walkTopDown().filter { it.isFile }.sortedBy { it.lastModified() }
        for (file in files) {
            if (totalBytes <= target) return
            val size = file.length()
            if (file.delete()) totalBytes -= size
        }
    }

    private fun measure(): Long = root.walkTopDown().filter { it.isFile }.sumOf { it.length() }
}
