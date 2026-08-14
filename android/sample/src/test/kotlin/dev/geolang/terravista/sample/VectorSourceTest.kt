package dev.geolang.terravista.sample

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class VectorSourceTest {

    // trimmed from https://tiles.openfreemap.org/planet, vector_layers dropped
    private val openFreeMapTileJson = """
        {
          "tilejson": "3.0.0",
          "tiles": ["https://tiles.openfreemap.org/planet/20260802_080001_pt/{z}/{x}/{y}.pbf"],
          "attribution": "<a href=\"https://openfreemap.org\">OpenFreeMap</a> Data from OpenStreetMap",
          "bounds": [-180.0, -85.05113, 180.0, 85.05113],
          "maxzoom": 14,
          "minzoom": 0,
          "name": "OpenFreeMap"
        }
    """.trimIndent()

    @Test
    fun readsTheCurrentTileUrlAndDepth() {
        val source = parseTileJson(openFreeMapTileJson)

        assertEquals("OpenFreeMap", source?.name)
        assertEquals(
            "https://tiles.openfreemap.org/planet/20260802_080001_pt/{z}/{x}/{y}.pbf",
            source?.tileUrlTemplate,
        )
        assertEquals(14, source?.tilesetMaxZoom)
        assertEquals(
            "<a href=\"https://openfreemap.org\">OpenFreeMap</a> Data from OpenStreetMap",
            source?.attribution,
        )
    }

    @Test
    fun rejectsATileJsonWithNoTiles() {
        assertNull(parseTileJson("""{"tiles": [], "maxzoom": 14}"""))
        assertNull(parseTileJson("""{"maxzoom": 14}"""))
    }

    @Test
    fun rejectsATileJsonWithNoUsableMaxZoom() {
        assertNull(parseTileJson("""{"tiles": ["https://example.com/{z}/{x}/{y}.pbf"]}"""))
        assertNull(parseTileJson("""{"tiles": ["https://example.com/{z}/{x}/{y}.pbf"], "maxzoom": 99}"""))
    }

    @Test
    fun rejectsAnythingThatIsNotJson() {
        assertNull(parseTileJson("<html>404</html>"))
    }
}
