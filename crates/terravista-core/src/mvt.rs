//! Mapbox Vector Tile decoding, from [`jung_mvt`].
//!
//! Coordinates stay in the tile's own units, 0 to `extent` with y increasing
//! southward, so placing a tile on screen is a scale and a translate.

pub use jung_mvt::{
    AttributeValue, TileGeometry, TilePolygon, VectorFeature, VectorLayer, VectorTile,
};

use crate::error::Error;

/// Decode a vector tile from raw protobuf bytes.
pub fn decode_tile(data: &[u8]) -> Result<VectorTile, Error> {
    jung_mvt::decode_tile(data).map_err(|error| Error::VectorTile(error.to_string()))
}
