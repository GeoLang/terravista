//! Offline tile cache — disk-backed LRU tile storage.
//!
//! Caches raster/vector tiles on device for offline use and fast access.
//! Supports configurable size limits and per-region pre-fetching.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::camera::{TileCoord, TileRange, VisibleBounds};

/// Metadata about a cached tile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileMeta {
    pub coord: TileCoord,
    pub size_bytes: u64,
    pub fetched_at: u64,
    pub etag: Option<String>,
    pub content_type: String,
}

/// Tile data (raster PNG/WebP or vector MVT/PBF).
#[derive(Debug, Clone)]
pub struct TileData {
    pub meta: TileMeta,
    pub bytes: Vec<u8>,
}

/// Configuration for the tile cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum cache size in bytes (default: 256 MB).
    pub max_size_bytes: u64,
    /// Maximum number of tiles (default: 50_000).
    pub max_tiles: u32,
    /// Tile source URL template (e.g., "`https://tiles.example.com/`{z}/{x}/{y}.mvt").
    pub url_template: String,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 256 * 1024 * 1024,
            max_tiles: 50_000,
            url_template: String::new(),
        }
    }
}

/// In-memory tile cache (platform layer persists to disk via callbacks).
pub struct TileCache {
    config: CacheConfig,
    tiles: HashMap<TileCoord, TileData>,
    access_order: Vec<TileCoord>,
    total_size: u64,
}

impl TileCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            tiles: HashMap::new(),
            access_order: Vec::new(),
            total_size: 0,
        }
    }

    /// Get a tile from cache (returns None if not cached).
    pub fn get(&mut self, coord: &TileCoord) -> Option<&TileData> {
        if self.tiles.contains_key(coord) {
            // Move to front of access order (LRU)
            self.access_order.retain(|c| c != coord);
            self.access_order.push(*coord);
            self.tiles.get(coord)
        } else {
            None
        }
    }

    /// Insert a tile into the cache, evicting LRU tiles if over limits.
    pub fn insert(&mut self, tile: TileData) {
        let coord = tile.meta.coord;
        let size = tile.meta.size_bytes;

        // Remove existing entry if present
        if let Some(existing) = self.tiles.remove(&coord) {
            self.total_size -= existing.meta.size_bytes;
            self.access_order.retain(|c| c != &coord);
        }

        // Evict until we're under limits
        while self.total_size + size > self.config.max_size_bytes
            || self.tiles.len() as u32 >= self.config.max_tiles
        {
            if !self.evict_lru() {
                break;
            }
        }

        self.total_size += size;
        self.access_order.push(coord);
        self.tiles.insert(coord, tile);
    }

    /// Check if a tile is cached.
    pub fn contains(&self, coord: &TileCoord) -> bool {
        self.tiles.contains_key(coord)
    }

    /// Number of cached tiles.
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// Total bytes used by cached tiles.
    pub fn size_bytes(&self) -> u64 {
        self.total_size
    }

    /// Clear all cached tiles.
    pub fn clear(&mut self) {
        self.tiles.clear();
        self.access_order.clear();
        self.total_size = 0;
    }

    /// Point the cache at a new tile source. Cached tiles are keyed by
    /// coordinate only, so a different source must drop them or stale
    /// imagery would serve for the new source.
    pub fn set_url_template(&mut self, template: String) {
        if self.config.url_template != template {
            self.clear();
        }
        self.config.url_template = template;
    }

    /// Build a tile URL from the template.
    pub fn tile_url(&self, coord: &TileCoord) -> String {
        self.config
            .url_template
            .replace("{z}", &coord.z.to_string())
            .replace("{x}", &coord.x.to_string())
            .replace("{y}", &coord.y.to_string())
    }

    /// Get tiles needed for a region (for offline pre-fetch).
    pub fn missing_tiles(&self, coords: &[TileCoord]) -> Vec<TileCoord> {
        coords
            .iter()
            .filter(|c| !self.tiles.contains_key(c))
            .copied()
            .collect()
    }

    fn evict_lru(&mut self) -> bool {
        if let Some(coord) = self.access_order.first().copied() {
            self.access_order.remove(0);
            if let Some(tile) = self.tiles.remove(&coord) {
                self.total_size -= tile.meta.size_bytes;
                return true;
            }
        }
        false
    }
}

/// Offline region for pre-downloading tiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineRegion {
    pub name: String,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub min_lat: f64,
    pub max_lat: f64,
    pub min_lon: f64,
    pub max_lon: f64,
}

/// Deepest zoom a region may hold, the same ceiling the camera zooms to.
pub const MAX_REGION_ZOOM: u8 = 22;

/// What one tile weighs, averaged over raster and vector sources, for the
/// estimate a host shows before downloading.
pub const AVERAGE_TILE_BYTES: u64 = 20_000;

impl OfflineRegion {
    /// The tile ranges covering this region at one zoom.
    ///
    /// Two of them when the region crosses the antimeridian, where the eastern
    /// edge sits at a smaller longitude than the western one, so the columns
    /// run west edge to date line and date line to east edge. Latitudes past
    /// the Mercator limit clamp to the top and bottom rows.
    pub fn tile_ranges(&self, zoom: u8) -> Vec<TileRange> {
        let min_lat = self.min_lat.min(self.max_lat);
        let max_lat = self.min_lat.max(self.max_lat);
        let spans: &[(f64, f64)] = if self.min_lon <= self.max_lon {
            &[(self.min_lon, self.max_lon)]
        } else {
            &[(self.min_lon, 180.0), (-180.0, self.max_lon)]
        };

        spans
            .iter()
            .map(|(min_lon, max_lon)| {
                VisibleBounds {
                    min_lon: *min_lon,
                    max_lon: *max_lon,
                    min_lat,
                    max_lat,
                }
                .tile_range(zoom)
            })
            .collect()
    }

    /// Every tile coordinate in this region, lowest zoom first.
    pub fn tiles(&self) -> impl Iterator<Item = TileCoord> + '_ {
        self.zooms()
            .flat_map(move |zoom| self.tile_ranges(zoom).into_iter().flat_map(TileRange::iter))
    }

    /// How many tiles [`tiles`](Self::tiles) yields.
    pub fn tile_count(&self) -> u64 {
        self.zooms()
            .map(|zoom| {
                self.tile_ranges(zoom)
                    .into_iter()
                    .map(TileRange::count)
                    .sum::<u64>()
            })
            .sum()
    }

    /// Rough download size, for showing before a download starts.
    pub fn estimated_size_bytes(&self) -> u64 {
        self.tile_count().saturating_mul(AVERAGE_TILE_BYTES)
    }

    /// Empty when the zoom span is inverted, so such a region holds no tiles.
    fn zooms(&self) -> std::ops::RangeInclusive<u8> {
        self.min_zoom.min(MAX_REGION_ZOOM)..=self.max_zoom.min(MAX_REGION_ZOOM)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tile(z: u8, x: u32, y: u32, size: u64) -> TileData {
        TileData {
            meta: TileMeta {
                coord: TileCoord::new(z, x, y),
                size_bytes: size,
                fetched_at: 0,
                etag: None,
                content_type: "image/png".to_string(),
            },
            bytes: vec![0u8; size as usize],
        }
    }

    #[test]
    fn test_insert_and_get() {
        let mut cache = TileCache::new(CacheConfig::default());
        cache.insert(make_tile(10, 512, 340, 1024));
        assert!(cache.contains(&TileCoord::new(10, 512, 340)));
        assert!(!cache.contains(&TileCoord::new(10, 513, 340)));
    }

    #[test]
    fn test_lru_eviction() {
        let config = CacheConfig {
            max_size_bytes: 3000,
            max_tiles: 100,
            url_template: String::new(),
        };
        let mut cache = TileCache::new(config);

        cache.insert(make_tile(10, 0, 0, 1000));
        cache.insert(make_tile(10, 1, 0, 1000));
        cache.insert(make_tile(10, 2, 0, 1000));

        // This should evict the first tile
        cache.insert(make_tile(10, 3, 0, 1000));
        assert!(!cache.contains(&TileCoord::new(10, 0, 0)));
        assert!(cache.contains(&TileCoord::new(10, 3, 0)));
    }

    #[test]
    fn test_tile_url() {
        let config = CacheConfig {
            url_template: "https://tiles.example.com/{z}/{x}/{y}.mvt".to_string(),
            ..Default::default()
        };
        let cache = TileCache::new(config);
        let url = cache.tile_url(&TileCoord::new(14, 8192, 5450));
        assert_eq!(url, "https://tiles.example.com/14/8192/5450.mvt");
    }

    fn region(
        min_lat: f64,
        max_lat: f64,
        min_lon: f64,
        max_lon: f64,
        zooms: (u8, u8),
    ) -> OfflineRegion {
        OfflineRegion {
            name: "test".to_string(),
            min_zoom: zooms.0,
            max_zoom: zooms.1,
            min_lat,
            max_lat,
            min_lon,
            max_lon,
        }
    }

    #[test]
    fn test_offline_region_count() {
        let region = region(51.3, 51.7, -0.5, 0.3, (10, 14));
        let count = region.tile_count();
        assert!(count > 0);
        assert!(count < 100_000);
    }

    /// The count is a promise about what the enumerator yields, so the two must
    /// agree, and every tile must be a real coordinate at its zoom.
    #[test]
    fn test_region_tiles_match_the_count() {
        let region = region(51.3, 51.7, -0.5, 0.3, (8, 12));
        let tiles: Vec<TileCoord> = region.tiles().collect();
        assert_eq!(tiles.len() as u64, region.tile_count());
        assert!(!tiles.is_empty());

        for tile in &tiles {
            let n = 2u32.pow(u32::from(tile.z));
            assert!(
                tile.x < n && tile.y < n,
                "{tile:?} is off the world at z{}",
                tile.z
            );
        }
        // lowest zoom first, so a download draws a coarse layer before a fine one
        assert_eq!(tiles.first().unwrap().z, 8);
        assert_eq!(tiles.last().unwrap().z, 12);
        assert_eq!(
            tiles.len(),
            tiles.iter().collect::<std::collections::HashSet<_>>().len()
        );
    }

    /// A region crossing the date line has its eastern edge west of its western
    /// one. It covers two columns of tiles, not the whole world in between.
    #[test]
    fn test_region_crosses_the_antimeridian() {
        let crossing = region(-18.0, -16.0, 179.0, -179.0, (6, 6));
        let tiles: Vec<TileCoord> = crossing.tiles().collect();
        assert_eq!(tiles.len() as u64, crossing.tile_count());

        let n = 2u32.pow(6);
        assert!(
            tiles.iter().any(|t| t.x == n - 1),
            "the west side of the line"
        );
        assert!(tiles.iter().any(|t| t.x == 0), "the east side of the line");
        // the same span the long way round is most of the world
        let wrapped = region(-18.0, -16.0, -179.0, 179.0, (6, 6));
        assert!(wrapped.tile_count() > crossing.tile_count() * 10);
    }

    /// Past the Mercator limit there is no tile row to ask for, so the poles
    /// clamp instead of running off the world or overflowing the count.
    #[test]
    fn test_region_clamps_at_the_poles() {
        let polar = region(-90.0, 90.0, -180.0, 180.0, (3, 3));
        let n = 2u64.pow(3);
        assert_eq!(polar.tile_count(), n * n);
        assert_eq!(polar.tiles().count() as u64, n * n);
    }

    /// An inverted zoom span is a region with nothing in it, not a panic.
    #[test]
    fn test_region_with_no_zooms_is_empty() {
        let backwards = region(51.3, 51.7, -0.5, 0.3, (14, 10));
        assert_eq!(backwards.tile_count(), 0);
        assert_eq!(backwards.tiles().count(), 0);
        assert_eq!(backwards.estimated_size_bytes(), 0);
    }

    /// Inverted latitudes are the same box read the other way up.
    #[test]
    fn test_region_normalises_inverted_latitudes() {
        let upright = region(51.3, 51.7, -0.5, 0.3, (12, 12));
        let flipped = region(51.7, 51.3, -0.5, 0.3, (12, 12));
        assert_eq!(upright.tile_count(), flipped.tile_count());
        assert!(upright.tile_count() > 0);
    }

    /// The estimate is the count times a fixed tile weight, at any size.
    #[test]
    fn test_region_estimate_scales_with_the_count() {
        for region in [
            region(51.3, 51.7, -0.5, 0.3, (10, 14)),
            region(-85.0, 85.0, -180.0, 180.0, (22, 22)),
        ] {
            assert_eq!(
                region.estimated_size_bytes(),
                region.tile_count() * AVERAGE_TILE_BYTES
            );
        }
    }
}
