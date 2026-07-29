//! Offline tile package generation.
//!
//! Creates self-contained tile packages (MBTiles-format SQLite databases)
//! for offline map usage on mobile devices.
//!
//! Supports:
//! - Defining tile regions by bounding box + zoom levels
//! - Packaging tiles into a portable SQLite-compatible format
//! - Estimating package size before download
//! - Reading tiles back from packages

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::location::Coordinate;

/// Bounding box defining a tile region.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

impl BoundingBox {
    pub fn new(min_lat: f64, min_lon: f64, max_lat: f64, max_lon: f64) -> Result<Self, Error> {
        if min_lat >= max_lat || min_lon >= max_lon {
            return Err(Error::InvalidInput(
                "min must be less than max for bounding box".into(),
            ));
        }
        if min_lat < -90.0 || max_lat > 90.0 || min_lon < -180.0 || max_lon > 180.0 {
            return Err(Error::InvalidInput("coordinates out of valid range".into()));
        }
        Ok(Self {
            min_lat,
            min_lon,
            max_lat,
            max_lon,
        })
    }

    /// Check if a coordinate is within this bounding box.
    pub fn contains(&self, coord: &Coordinate) -> bool {
        coord.latitude >= self.min_lat
            && coord.latitude <= self.max_lat
            && coord.longitude >= self.min_lon
            && coord.longitude <= self.max_lon
    }
}

/// Definition of a tile package to be created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDefinition {
    /// Human-readable name.
    pub name: String,
    /// Region to include.
    pub bounds: BoundingBox,
    /// Minimum zoom level (inclusive).
    pub min_zoom: u8,
    /// Maximum zoom level (inclusive).
    pub max_zoom: u8,
    /// Tile format (e.g., "png", "pbf", "webp").
    pub format: TileFormat,
}

/// Tile format in the package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileFormat {
    Png,
    Jpeg,
    Webp,
    Pbf,
}

impl TileFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Pbf => "pbf",
        }
    }

    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Webp => "image/webp",
            Self::Pbf => "application/x-protobuf",
        }
    }
}

/// A tile coordinate (z/x/y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

/// Estimate of a tile package size.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageEstimate {
    /// Total number of tiles.
    pub tile_count: u64,
    /// Estimated size in bytes (assuming average tile size).
    pub estimated_bytes: u64,
    /// Tiles per zoom level.
    pub tiles_per_zoom: Vec<(u8, u64)>,
}

/// An in-memory tile package (MBTiles-compatible structure).
#[derive(Debug, Clone)]
pub struct TilePackage {
    pub definition: PackageDefinition,
    /// Stored tiles: (z, x, y) → tile data.
    tiles: std::collections::HashMap<TileCoord, Vec<u8>>,
    /// Metadata entries.
    pub metadata: std::collections::HashMap<String, String>,
}

impl TilePackage {
    /// Create a new empty tile package from a definition.
    pub fn new(definition: PackageDefinition) -> Self {
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("name".into(), definition.name.clone());
        metadata.insert("format".into(), definition.format.extension().into());
        metadata.insert(
            "bounds".into(),
            format!(
                "{},{},{},{}",
                definition.bounds.min_lon,
                definition.bounds.min_lat,
                definition.bounds.max_lon,
                definition.bounds.max_lat
            ),
        );
        metadata.insert("minzoom".into(), definition.min_zoom.to_string());
        metadata.insert("maxzoom".into(), definition.max_zoom.to_string());
        metadata.insert("type".into(), "baselayer".into());

        Self {
            definition,
            tiles: std::collections::HashMap::new(),
            metadata,
        }
    }

    /// Insert a tile into the package.
    pub fn insert_tile(&mut self, coord: TileCoord, data: Vec<u8>) {
        self.tiles.insert(coord, data);
    }

    /// Get a tile from the package.
    pub fn get_tile(&self, coord: &TileCoord) -> Option<&[u8]> {
        self.tiles.get(coord).map(|v| v.as_slice())
    }

    /// Get the number of tiles in the package.
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }

    /// Get total size in bytes of all stored tiles.
    pub fn total_bytes(&self) -> usize {
        self.tiles.values().map(|v| v.len()).sum()
    }

    /// List all tile coordinates in the package.
    pub fn tile_coords(&self) -> Vec<TileCoord> {
        self.tiles.keys().copied().collect()
    }

    /// Serialize the package to MBTiles-compatible binary format.
    /// Returns a simple binary representation (header + tiles).
    pub fn to_bytes(&self) -> Vec<u8> {
        let metadata_json = serde_json::to_vec(&self.metadata).unwrap_or_default();
        let mut buf = Vec::new();

        // Magic bytes
        buf.extend_from_slice(b"TVPK");
        // Version
        buf.push(1);
        // Metadata length (4 bytes LE)
        buf.extend_from_slice(&(metadata_json.len() as u32).to_le_bytes());
        // Metadata
        buf.extend_from_slice(&metadata_json);
        // Tile count (4 bytes LE)
        buf.extend_from_slice(&(self.tiles.len() as u32).to_le_bytes());
        // Tiles: z(1) + x(4) + y(4) + data_len(4) + data
        for (coord, data) in &self.tiles {
            buf.push(coord.z);
            buf.extend_from_slice(&coord.x.to_le_bytes());
            buf.extend_from_slice(&coord.y.to_le_bytes());
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
        }

        buf
    }

    /// Deserialize a package from bytes.
    pub fn from_bytes(data: &[u8]) -> Result<Self, Error> {
        if data.len() < 14 || &data[0..4] != b"TVPK" {
            return Err(Error::InvalidInput("invalid tile package format".into()));
        }

        if data[4] != 1 {
            return Err(Error::InvalidInput(format!(
                "unsupported tile package version {}",
                data[4]
            )));
        }
        let meta_len = u32::from_le_bytes([data[5], data[6], data[7], data[8]]) as usize;

        if data.len() < 9 + meta_len + 4 {
            return Err(Error::InvalidInput("truncated package".into()));
        }

        let metadata: std::collections::HashMap<String, String> =
            serde_json::from_slice(&data[9..9 + meta_len])
                .map_err(|e| Error::InvalidInput(format!("invalid metadata: {e}")))?;

        let tile_count_offset = 9 + meta_len;
        let tile_count = u32::from_le_bytes([
            data[tile_count_offset],
            data[tile_count_offset + 1],
            data[tile_count_offset + 2],
            data[tile_count_offset + 3],
        ]) as usize;

        let mut tiles = std::collections::HashMap::new();
        let mut offset = tile_count_offset + 4;

        for _ in 0..tile_count {
            if offset + 13 > data.len() {
                return Err(Error::InvalidInput("truncated tile data".into()));
            }
            let z = data[offset];
            let x = u32::from_le_bytes([
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
            ]);
            let y = u32::from_le_bytes([
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
                data[offset + 8],
            ]);
            let data_len = u32::from_le_bytes([
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
                data[offset + 12],
            ]) as usize;
            offset += 13;

            if offset + data_len > data.len() {
                return Err(Error::InvalidInput("truncated tile data".into()));
            }

            tiles.insert(
                TileCoord { z, x, y },
                data[offset..offset + data_len].to_vec(),
            );
            offset += data_len;
        }

        // Reconstruct definition from metadata
        let name = metadata.get("name").cloned().unwrap_or_default();
        let format = match metadata.get("format").map(|s| s.as_str()) {
            Some("png") => TileFormat::Png,
            Some("jpg") | Some("jpeg") => TileFormat::Jpeg,
            Some("webp") => TileFormat::Webp,
            Some("pbf") => TileFormat::Pbf,
            _ => TileFormat::Png,
        };
        let min_zoom: u8 = metadata
            .get("minzoom")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let max_zoom: u8 = metadata
            .get("maxzoom")
            .and_then(|s| s.parse().ok())
            .unwrap_or(18);

        let whole_world = BoundingBox {
            min_lat: -90.0,
            min_lon: -180.0,
            max_lat: 90.0,
            max_lon: 180.0,
        };

        // metadata comes off disk unvalidated, so route it through `new` rather
        // than building a box `new` would have rejected
        let bounds = metadata
            .get("bounds")
            .map(|s| {
                s.split(',')
                    .filter_map(|p| p.trim().parse::<f64>().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|parts| parts.len() == 4)
            .and_then(|p| BoundingBox::new(p[1], p[0], p[3], p[2]).ok())
            .unwrap_or(whole_world);

        let definition = PackageDefinition {
            name,
            bounds,
            min_zoom,
            max_zoom,
            format,
        };

        Ok(Self {
            definition,
            tiles,
            metadata,
        })
    }
}

/// Deepest zoom the tile grid maths can represent. Beyond this `2^z` overflows
/// a `u32`, and package metadata is not trusted to stay in range.
pub const MAX_PACKAGE_ZOOM: u8 = 22;

/// Tile bounds covering `bounds` at zoom `z`, ordered low to high so an
/// inverted bounding box cannot underflow the callers' subtractions.
fn tile_bounds(bounds: &BoundingBox, z: u8) -> (u32, u32, u32, u32) {
    let n = 2u32.pow(z as u32);
    let (xa, xb) = (
        lon_to_tile_x(bounds.min_lon, n),
        lon_to_tile_x(bounds.max_lon, n),
    );
    // y is inverted, north edge gives the smaller row
    let (ya, yb) = (
        lat_to_tile_y(bounds.max_lat, n),
        lat_to_tile_y(bounds.min_lat, n),
    );
    (xa.min(xb), xa.max(xb), ya.min(yb), ya.max(yb))
}

/// Clamp a requested zoom span into the representable range. An inverted span
/// yields an empty range.
fn zoom_range(min_zoom: u8, max_zoom: u8) -> std::ops::RangeInclusive<u8> {
    min_zoom.min(MAX_PACKAGE_ZOOM)..=max_zoom.min(MAX_PACKAGE_ZOOM)
}

/// Calculate which tiles are needed for a given region and zoom range.
pub fn tiles_for_region(bounds: &BoundingBox, min_zoom: u8, max_zoom: u8) -> Vec<TileCoord> {
    let mut tiles = Vec::new();

    for z in zoom_range(min_zoom, max_zoom) {
        let (x_min, x_max, y_min, y_max) = tile_bounds(bounds, z);

        for x in x_min..=x_max {
            for y in y_min..=y_max {
                tiles.push(TileCoord { z, x, y });
            }
        }
    }

    tiles
}

/// Estimate the size of a tile package before downloading.
pub fn estimate_package(
    bounds: &BoundingBox,
    min_zoom: u8,
    max_zoom: u8,
    avg_tile_bytes: u64,
) -> PackageEstimate {
    let mut tile_count = 0u64;
    let mut tiles_per_zoom = Vec::new();

    for z in zoom_range(min_zoom, max_zoom) {
        let (x_min, x_max, y_min, y_max) = tile_bounds(bounds, z);

        let count = (x_max - x_min + 1) as u64 * (y_max - y_min + 1) as u64;
        tiles_per_zoom.push((z, count));
        tile_count += count;
    }

    PackageEstimate {
        tile_count,
        estimated_bytes: tile_count.saturating_mul(avg_tile_bytes),
        tiles_per_zoom,
    }
}

/// Convert longitude to tile X coordinate.
fn lon_to_tile_x(lon: f64, n: u32) -> u32 {
    let x = ((lon + 180.0) / 360.0 * n as f64).floor() as i32;
    x.clamp(0, n as i32 - 1) as u32
}

/// Convert latitude to tile Y coordinate.
fn lat_to_tile_y(lat: f64, n: u32) -> u32 {
    let lat_rad = lat.to_radians();
    let y = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n as f64).floor() as i32;
    y.clamp(0, n as i32 - 1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_box_valid() {
        let bb = BoundingBox::new(40.0, -74.0, 41.0, -73.0);
        assert!(bb.is_ok());
    }

    #[test]
    fn test_bounding_box_invalid() {
        // min > max
        assert!(BoundingBox::new(41.0, -74.0, 40.0, -73.0).is_err());
        // out of range
        assert!(BoundingBox::new(-91.0, 0.0, 0.0, 1.0).is_err());
    }

    #[test]
    fn test_bounding_box_contains() {
        let bb = BoundingBox::new(40.0, -74.0, 41.0, -73.0).unwrap();
        let inside = Coordinate {
            latitude: 40.5,
            longitude: -73.5,
        };
        let outside = Coordinate {
            latitude: 42.0,
            longitude: -73.5,
        };
        assert!(bb.contains(&inside));
        assert!(!bb.contains(&outside));
    }

    #[test]
    fn test_tiles_for_region() {
        let bb = BoundingBox::new(40.7, -74.0, 40.8, -73.9).unwrap();
        let tiles = tiles_for_region(&bb, 10, 10);
        assert!(!tiles.is_empty());
        // All tiles should be at zoom 10
        assert!(tiles.iter().all(|t| t.z == 10));
    }

    #[test]
    fn test_estimate_package() {
        let bb = BoundingBox::new(40.7, -74.0, 40.8, -73.9).unwrap();
        let est = estimate_package(&bb, 0, 5, 50_000);
        assert!(est.tile_count > 0);
        assert_eq!(est.estimated_bytes, est.tile_count * 50_000);
        assert!(!est.tiles_per_zoom.is_empty());
    }

    #[test]
    fn test_tile_package_insert_and_get() {
        let bb = BoundingBox::new(40.0, -74.0, 41.0, -73.0).unwrap();
        let def = PackageDefinition {
            name: "NYC".into(),
            bounds: bb,
            min_zoom: 10,
            max_zoom: 14,
            format: TileFormat::Png,
        };
        let mut pkg = TilePackage::new(def);

        let coord = TileCoord {
            z: 10,
            x: 301,
            y: 383,
        };
        let data = vec![0x89, 0x50, 0x4E, 0x47]; // fake PNG header

        pkg.insert_tile(coord, data.clone());
        assert_eq!(pkg.tile_count(), 1);
        assert_eq!(pkg.get_tile(&coord), Some(data.as_slice()));
    }

    #[test]
    fn test_tile_package_serialize_roundtrip() {
        let bb = BoundingBox::new(40.0, -74.0, 41.0, -73.0).unwrap();
        let def = PackageDefinition {
            name: "Test".into(),
            bounds: bb,
            min_zoom: 5,
            max_zoom: 10,
            format: TileFormat::Pbf,
        };
        let mut pkg = TilePackage::new(def);

        pkg.insert_tile(TileCoord { z: 5, x: 9, y: 12 }, vec![1, 2, 3, 4]);
        pkg.insert_tile(TileCoord { z: 6, x: 18, y: 24 }, vec![5, 6, 7]);

        let bytes = pkg.to_bytes();
        let restored = TilePackage::from_bytes(&bytes).unwrap();

        assert_eq!(restored.tile_count(), 2);
        assert_eq!(
            restored.get_tile(&TileCoord { z: 5, x: 9, y: 12 }),
            Some([1u8, 2, 3, 4].as_slice())
        );
        assert_eq!(
            restored.get_tile(&TileCoord { z: 6, x: 18, y: 24 }),
            Some([5u8, 6, 7].as_slice())
        );
        assert_eq!(restored.definition.name, "Test");
        assert_eq!(restored.definition.format, TileFormat::Pbf);
    }

    #[test]
    fn test_tile_format_info() {
        assert_eq!(TileFormat::Png.extension(), "png");
        assert_eq!(TileFormat::Pbf.mime_type(), "application/x-protobuf");
        assert_eq!(TileFormat::Webp.extension(), "webp");
    }

    #[test]
    fn test_invalid_package_bytes() {
        assert!(TilePackage::from_bytes(&[0, 1, 2]).is_err());
        assert!(TilePackage::from_bytes(b"XXXX1234567890").is_err());
    }

    #[test]
    fn test_package_total_bytes() {
        let bb = BoundingBox::new(0.0, 0.0, 1.0, 1.0).unwrap();
        let def = PackageDefinition {
            name: "size_test".into(),
            bounds: bb,
            min_zoom: 0,
            max_zoom: 2,
            format: TileFormat::Png,
        };
        let mut pkg = TilePackage::new(def);
        pkg.insert_tile(TileCoord { z: 0, x: 0, y: 0 }, vec![0; 100]);
        pkg.insert_tile(TileCoord { z: 1, x: 0, y: 0 }, vec![0; 200]);
        assert_eq!(pkg.total_bytes(), 300);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    fn inverted() -> BoundingBox {
        // what `new` would reject, but `from_bytes` and struct literals allow
        BoundingBox {
            min_lat: 41.0,
            min_lon: -73.0,
            max_lat: 40.0,
            max_lon: -74.0,
        }
    }

    /// An inverted box used to underflow the tile-count subtraction.
    #[test]
    fn test_inverted_bounds_does_not_panic() {
        let estimate = estimate_package(&inverted(), 10, 12, 20_000);
        assert!(estimate.tile_count > 0);
        // the two entry points must agree rather than one returning nothing
        assert_eq!(
            estimate.tile_count as usize,
            tiles_for_region(&inverted(), 10, 12).len()
        );
    }

    /// Zoom levels past the representable range must clamp, not overflow.
    #[test]
    fn test_extreme_zoom_does_not_panic() {
        // a tiny box, so clamping to z22 stays cheap to enumerate
        let bounds = BoundingBox::new(40.0, -74.0, 40.001, -73.999).unwrap();
        let estimate = estimate_package(&bounds, 32, 40, 1);
        assert!(
            estimate
                .tiles_per_zoom
                .iter()
                .all(|(z, _)| *z <= MAX_PACKAGE_ZOOM)
        );
        assert!(
            tiles_for_region(&bounds, 32, 40)
                .iter()
                .all(|t| t.z <= MAX_PACKAGE_ZOOM)
        );
    }

    /// An inverted zoom span is empty rather than panicking.
    #[test]
    fn test_inverted_zoom_span_is_empty() {
        let bounds = BoundingBox::new(40.0, -74.0, 41.0, -73.0).unwrap();
        assert!(tiles_for_region(&bounds, 12, 10).is_empty());
    }

    /// A package claiming an unsupported version must be rejected, not misparsed.
    #[test]
    fn test_unsupported_version_rejected() {
        let package = TilePackage::new(PackageDefinition {
            name: "test".to_string(),
            bounds: BoundingBox::new(40.0, -74.0, 41.0, -73.0).unwrap(),
            min_zoom: 10,
            max_zoom: 12,
            format: TileFormat::Png,
        });
        let mut bytes = package.to_bytes();
        bytes[4] = 2;
        assert!(TilePackage::from_bytes(&bytes).is_err());
    }

    /// Bounds read back from metadata go through validation.
    #[test]
    fn test_roundtrip_bounds_are_valid() {
        let package = TilePackage::new(PackageDefinition {
            name: "test".to_string(),
            bounds: BoundingBox::new(40.0, -74.0, 41.0, -73.0).unwrap(),
            min_zoom: 10,
            max_zoom: 12,
            format: TileFormat::Png,
        });
        let restored = TilePackage::from_bytes(&package.to_bytes()).unwrap();
        let b = &restored.definition.bounds;
        assert!(b.min_lat < b.max_lat && b.min_lon < b.max_lon);
    }
}
