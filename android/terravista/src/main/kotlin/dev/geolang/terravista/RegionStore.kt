package dev.geolang.terravista

import android.os.Handler
import android.os.Looper
import android.util.Log
import java.io.File
import java.io.IOException
import java.util.concurrent.Callable
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import org.json.JSONException
import org.json.JSONObject

/**
 * A region of the map held on disk until it is deleted.
 *
 * Its tiles never expire and are never evicted, unlike the ambient cache
 * behind [MapView.diskCacheSizeBytes].
 */
data class OfflineRegion(
    val name: String,
    val minLatitude: Double,
    val minLongitude: Double,
    val maxLatitude: Double,
    val maxLongitude: Double,
    val minZoom: Int,
    val maxZoom: Int,
    /** Tiles stored, counting a raster and a vector tile as two. */
    val tileCount: Int,
    val sizeBytes: Long,
)

/** What a region would cost, before downloading it. */
data class RegionEstimate(val tileCount: Long, val estimatedBytes: Long)

/**
 * Most tiles one region may hold.
 *
 * Public tile servers forbid bulk downloading, and this is about as much as a
 * well-behaved client should pull in one go. Ask [MapView.estimateRegion] and
 * offer the user a smaller area rather than working around it.
 */
const val MAX_REGION_TILES = 10_000L

/** Progress of a region download. Every call arrives on the main thread. */
interface RegionDownloadListener {
    fun onProgress(completed: Int, failed: Int, total: Int)

    /** [region] is null when the download was cancelled before it finished. */
    fun onFinished(region: OfflineRegion?)
}

/** A download in flight. Cancelling keeps whatever it has already written. */
class RegionDownload internal constructor() {
    internal val cancelled = AtomicBoolean(false)

    val isCancelled: Boolean
        get() = cancelled.get()

    fun cancel() {
        cancelled.set(true)
    }
}

/** One tile to download: which source it belongs to and where it comes from. */
internal class RegionTile(
    val source: String,
    val z: Int,
    val x: Int,
    val y: Int,
    val url: String,
)

/**
 * Regions pinned to disk, under the app's files directory so the system never
 * reclaims them.
 *
 * Laid out as `<region>/<source>/<z>_<x>_<y>`, where the region directory is
 * named for a hash of the region's name and holds the metadata that names it
 * properly.
 */
internal class RegionStore(private val root: File, private val fetcher: TileFetcher) {

    private companion object {
        const val TAG = "TerraVista"
        const val METADATA = "region.json"
        /** Enough to keep the link busy, few enough to stay a polite client. */
        const val DOWNLOAD_THREADS = 2
        /** Reporting every tile would post thousands of times for one region. */
        const val PROGRESS_EVERY = 16
    }

    private val main = Handler(Looper.getMainLooper())

    /** Region directories, reread after one is written or deleted. */
    @Volatile
    private var directories: Array<File>? = null

    fun read(source: String, z: Int, x: Int, y: Int): ByteArray? {
        for (region in regionDirectories()) {
            readTile(File(region, source), z, x, y)?.let { return it }
        }
        return null
    }

    fun regions(): List<OfflineRegion> = regionDirectories().mapNotNull { metadata(it) }

    fun delete(name: String): Boolean {
        val deleted = File(root, sourceKey(name)).deleteRecursively()
        directories = null
        return deleted
    }

    /**
     * Fetch every tile of [region] and store it. Blocks until done, so the
     * caller owns the thread this runs on.
     */
    fun download(
        region: OfflineRegion,
        tiles: List<RegionTile>,
        download: RegionDownload,
        listener: RegionDownloadListener?,
    ) {
        if (tiles.isEmpty()) {
            // nothing to save, so leave no region claiming to hold anything
            main.post { listener?.onFinished(null) }
            return
        }

        val directory = File(root, sourceKey(region.name))
        val completed = AtomicInteger(0)
        val failed = AtomicInteger(0)
        val total = tiles.size

        val pool = Executors.newFixedThreadPool(DOWNLOAD_THREADS)
        val work = tiles.map { tile ->
            Callable {
                if (download.isCancelled) return@Callable
                if (fetch(directory, tile)) {
                    val done = completed.incrementAndGet()
                    if (done % PROGRESS_EVERY == 0) {
                        report(listener, done, failed.get(), total)
                    }
                } else {
                    failed.incrementAndGet()
                }
            }
        }
        pool.invokeAll(work)
        pool.shutdown()

        val stored = completed.get()
        report(listener, stored, failed.get(), total)
        if (download.isCancelled) {
            main.post { listener?.onFinished(null) }
            directories = null
            return
        }

        val saved = region.copy(tileCount = stored, sizeBytes = directorySize(directory))
        write(directory, saved)
        directories = null
        main.post { listener?.onFinished(saved) }
    }

    private fun fetch(directory: File, tile: RegionTile): Boolean {
        val source = File(directory, tile.source)
        if (readTile(source, tile.z, tile.x, tile.y) != null) return true
        return try {
            val body = fetcher.get(tile.url) ?: return false
            writeTile(source, tile.z, tile.x, tile.y, body)
        } catch (e: IOException) {
            Log.w(TAG, "region tile ${tile.z}/${tile.x}/${tile.y} failed: $e")
            false
        }
    }

    private fun report(listener: RegionDownloadListener?, completed: Int, failed: Int, total: Int) {
        if (listener == null) return
        main.post { listener.onProgress(completed, failed, total) }
    }

    private fun regionDirectories(): Array<File> {
        directories?.let { return it }
        val found = root.listFiles { file: File -> file.isDirectory } ?: emptyArray()
        directories = found
        return found
    }

    private fun write(directory: File, region: OfflineRegion) {
        val json = JSONObject()
            .put("name", region.name)
            .put("minLatitude", region.minLatitude)
            .put("minLongitude", region.minLongitude)
            .put("maxLatitude", region.maxLatitude)
            .put("maxLongitude", region.maxLongitude)
            .put("minZoom", region.minZoom)
            .put("maxZoom", region.maxZoom)
            .put("tileCount", region.tileCount)
        try {
            directory.mkdirs()
            File(directory, METADATA).writeText(json.toString())
        } catch (e: IOException) {
            Log.w(TAG, "region ${region.name} metadata failed: $e")
        }
    }

    private fun metadata(directory: File): OfflineRegion? {
        val file = File(directory, METADATA)
        if (!file.isFile) return null
        return try {
            val json = JSONObject(file.readText())
            OfflineRegion(
                name = json.getString("name"),
                minLatitude = json.getDouble("minLatitude"),
                minLongitude = json.getDouble("minLongitude"),
                maxLatitude = json.getDouble("maxLatitude"),
                maxLongitude = json.getDouble("maxLongitude"),
                minZoom = json.getInt("minZoom"),
                maxZoom = json.getInt("maxZoom"),
                tileCount = json.getInt("tileCount"),
                sizeBytes = directorySize(directory),
            )
        } catch (e: JSONException) {
            Log.w(TAG, "region metadata at ${file.path} is unreadable: $e")
            null
        } catch (e: IOException) {
            Log.w(TAG, "region metadata at ${file.path} failed: $e")
            null
        }
    }
}

private fun directorySize(directory: File): Long =
    directory.walkTopDown().filter { it.isFile }.sumOf { it.length() }
