//! Offline tile cache — disk-backed LRU tile storage.
//!
//! Caches raster/vector tiles on device for offline use and fast access.
//! Supports configurable size limits and per-region pre-fetching.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::camera::TileCoord;

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
    /// Tile source URL template (e.g., "https://tiles.example.com/{z}/{x}/{y}.mvt").
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

impl OfflineRegion {
    /// Estimate the number of tiles in this region.
    pub fn tile_count(&self) -> u64 {
        let mut total = 0u64;
        for z in self.min_zoom..=self.max_zoom {
            let n = 2u64.pow(z as u32);
            let x_min = ((self.min_lon + 180.0) / 360.0 * n as f64).floor() as u64;
            let x_max = ((self.max_lon + 180.0) / 360.0 * n as f64).floor() as u64;
            let y_min = ((1.0 - self.max_lat.to_radians().tan().asinh() / std::f64::consts::PI)
                / 2.0
                * n as f64)
                .floor() as u64;
            let y_max = ((1.0 - self.min_lat.to_radians().tan().asinh() / std::f64::consts::PI)
                / 2.0
                * n as f64)
                .floor() as u64;
            total += (x_max - x_min + 1) * (y_max - y_min + 1);
        }
        total
    }

    /// Estimate total download size (assumes ~20KB per tile average).
    pub fn estimated_size_bytes(&self) -> u64 {
        self.tile_count() * 20_000
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

    #[test]
    fn test_offline_region_count() {
        let region = OfflineRegion {
            name: "London".to_string(),
            min_zoom: 10,
            max_zoom: 14,
            min_lat: 51.3,
            max_lat: 51.7,
            min_lon: -0.5,
            max_lon: 0.3,
        };
        let count = region.tile_count();
        assert!(count > 0);
        assert!(count < 100_000);
    }
}
