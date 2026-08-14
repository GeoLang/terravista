//! Offline tile package generation.
//!
//! Creates self-contained tile packages (TVPK binary archives, with
//! MBTiles-style metadata keys) for offline map usage on mobile devices.
//!
//! Supports:
//! - Defining a package as an offline region plus a tile format
//! - Packaging tiles into a portable archive
//! - Reading tiles back from packages

use serde::{Deserialize, Serialize};

use crate::camera::TileCoord;
use crate::error::Error;
use crate::tile_cache::OfflineRegion;

/// Definition of a tile package to be created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDefinition {
    /// Area and zoom levels to include, and the name to record.
    pub region: OfflineRegion,
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

const FALLBACK_MIN_ZOOM: u8 = 0;
const FALLBACK_MAX_ZOOM: u8 = 18;

// lon,lat,lon,lat, the order the metadata bounds value uses
const WHOLE_WORLD_BOUNDS: [f64; 4] = [-180.0, -90.0, 180.0, 90.0];

/// An in-memory tile package.
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
        let region = &definition.region;
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("name".into(), region.name.clone());
        metadata.insert("format".into(), definition.format.extension().into());
        metadata.insert(
            "bounds".into(),
            format!(
                "{},{},{},{}",
                region.min_lon, region.min_lat, region.max_lon, region.max_lat
            ),
        );
        metadata.insert("minzoom".into(), region.min_zoom.to_string());
        metadata.insert("maxzoom".into(), region.max_zoom.to_string());
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

    /// Serialize the package to its binary format.
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
                TileCoord::new(z, x, y),
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
            .unwrap_or(FALLBACK_MIN_ZOOM);
        let max_zoom: u8 = metadata
            .get("maxzoom")
            .and_then(|s| s.parse().ok())
            .unwrap_or(FALLBACK_MAX_ZOOM);

        let [min_lon, min_lat, max_lon, max_lat] = parse_bounds(metadata.get("bounds"));

        let definition = PackageDefinition {
            region: OfflineRegion {
                name,
                min_zoom,
                max_zoom,
                min_lat,
                max_lat,
                min_lon,
                max_lon,
            },
            format,
        };

        Ok(Self {
            definition,
            tiles,
            metadata,
        })
    }
}

/// Read the metadata bounds value, which is lon,lat,lon,lat. Anything that is
/// not four numbers reads as the whole world.
fn parse_bounds(value: Option<&String>) -> [f64; 4] {
    let parsed: Vec<f64> = value
        .map(|s| {
            s.split(',')
                .filter_map(|part| part.trim().parse::<f64>().ok())
                .collect()
        })
        .unwrap_or_default();

    match parsed[..] {
        [min_lon, min_lat, max_lon, max_lat] => [min_lon, min_lat, max_lon, max_lat],
        _ => WHOLE_WORLD_BOUNDS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn definition(name: &str, zooms: (u8, u8), format: TileFormat) -> PackageDefinition {
        PackageDefinition {
            region: OfflineRegion {
                name: name.to_string(),
                min_zoom: zooms.0,
                max_zoom: zooms.1,
                min_lat: 40.0,
                max_lat: 41.0,
                min_lon: -74.0,
                max_lon: -73.0,
            },
            format,
        }
    }

    #[test]
    fn test_tile_package_insert_and_get() {
        let mut pkg = TilePackage::new(definition("NYC", (10, 14), TileFormat::Png));

        let coord = TileCoord::new(10, 301, 383);
        let data = vec![0x89, 0x50, 0x4E, 0x47]; // fake PNG header

        pkg.insert_tile(coord, data.clone());
        assert_eq!(pkg.tile_count(), 1);
        assert_eq!(pkg.get_tile(&coord), Some(data.as_slice()));
    }

    #[test]
    fn test_tile_package_serialize_roundtrip() {
        let mut pkg = TilePackage::new(definition("Test", (5, 10), TileFormat::Pbf));

        pkg.insert_tile(TileCoord::new(5, 9, 12), vec![1, 2, 3, 4]);
        pkg.insert_tile(TileCoord::new(6, 18, 24), vec![5, 6, 7]);

        let bytes = pkg.to_bytes();
        let restored = TilePackage::from_bytes(&bytes).unwrap();

        assert_eq!(restored.tile_count(), 2);
        assert_eq!(
            restored.get_tile(&TileCoord::new(5, 9, 12)),
            Some([1u8, 2, 3, 4].as_slice())
        );
        assert_eq!(
            restored.get_tile(&TileCoord::new(6, 18, 24)),
            Some([5u8, 6, 7].as_slice())
        );
        assert_eq!(restored.definition.region.name, "Test");
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
        let mut pkg = TilePackage::new(definition("size_test", (0, 2), TileFormat::Png));
        pkg.insert_tile(TileCoord::new(0, 0, 0), vec![0; 100]);
        pkg.insert_tile(TileCoord::new(1, 0, 0), vec![0; 200]);
        assert_eq!(pkg.total_bytes(), 300);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::tests::definition;
    use super::*;

    /// A package claiming an unsupported version must be rejected, not misparsed.
    #[test]
    fn test_unsupported_version_rejected() {
        let package = TilePackage::new(definition("test", (10, 12), TileFormat::Png));
        let mut bytes = package.to_bytes();
        bytes[4] = 2;
        assert!(TilePackage::from_bytes(&bytes).is_err());
    }

    /// The region is what a package is defined by, so it must survive the wire
    /// format intact and describe the same tiles on the way back.
    #[test]
    fn test_roundtrip_restores_the_region() {
        let package = TilePackage::new(definition("test", (10, 12), TileFormat::Png));
        let restored = TilePackage::from_bytes(&package.to_bytes()).unwrap();

        let region = &restored.definition.region;
        assert_eq!(region.min_zoom, 10);
        assert_eq!(region.max_zoom, 12);
        assert_eq!(region.min_lat, 40.0);
        assert_eq!(region.max_lat, 41.0);
        assert_eq!(region.min_lon, -74.0);
        assert_eq!(region.max_lon, -73.0);
        assert_eq!(region.tile_count(), package.definition.region.tile_count());
        assert!(region.tile_count() > 0);
    }

    /// Packages already written to disk have to keep opening, so the layout is
    /// pinned to a literal buffer rather than only to what `to_bytes` produces.
    #[test]
    fn test_reads_a_package_built_byte_by_byte() {
        let metadata = br#"{"name":"NYC","format":"png","bounds":"-74,40,-73,41","minzoom":"10","maxzoom":"12","type":"baselayer"}"#;
        let tile = [0x89u8, 0x50, 0x4E, 0x47];

        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"TVPK");
        bytes.push(1);
        bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
        bytes.extend_from_slice(metadata);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.push(10);
        bytes.extend_from_slice(&301u32.to_le_bytes());
        bytes.extend_from_slice(&383u32.to_le_bytes());
        bytes.extend_from_slice(&(tile.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&tile);

        let package = TilePackage::from_bytes(&bytes).unwrap();
        assert_eq!(
            package.get_tile(&TileCoord::new(10, 301, 383)),
            Some(tile.as_slice())
        );
        assert_eq!(package.to_bytes().len(), bytes.len());

        let region = &package.definition.region;
        assert_eq!(region.name, "NYC");
        assert_eq!((region.min_lon, region.min_lat), (-74.0, 40.0));
        assert_eq!((region.max_lon, region.max_lat), (-73.0, 41.0));
        assert_eq!((region.min_zoom, region.max_zoom), (10, 12));
        assert_eq!(package.definition.format, TileFormat::Png);
    }

    /// Metadata comes off disk unchecked, so bounds that are not four numbers
    /// have to land somewhere usable rather than on garbage coordinates.
    #[test]
    fn test_unusable_bounds_metadata_reads_as_the_whole_world() {
        for bounds in ["", "1,2", "40,-74,not-a-number,-73", "1,2,3,4,5"] {
            let mut package = TilePackage::new(definition("test", (0, 1), TileFormat::Png));
            package.metadata.insert("bounds".into(), bounds.to_string());

            let region = TilePackage::from_bytes(&package.to_bytes())
                .unwrap()
                .definition
                .region;
            assert_eq!(region.min_lat, -90.0);
            assert_eq!(region.max_lat, 90.0);
            assert_eq!(region.min_lon, -180.0);
            assert_eq!(region.max_lon, 180.0);
        }
    }
}
