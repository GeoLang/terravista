//! Error types for the mobile map SDK.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("tile fetch failed: {0}")]
    TileFetch(String),

    #[error("cache I/O error: {0}")]
    CacheIo(String),

    #[error("invalid coordinate: lat={lat}, lon={lon}")]
    InvalidCoordinate { lat: f64, lon: f64 },

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("offline database error: {0}")]
    OfflineDb(String),

    #[error("style parse error: {0}")]
    StyleParse(String),

    #[error("renderer error: {0}")]
    Renderer(String),

    #[error("route calculation failed: {0}")]
    RouteError(String),

    #[error("GPS unavailable: {0}")]
    GpsUnavailable(String),
}
