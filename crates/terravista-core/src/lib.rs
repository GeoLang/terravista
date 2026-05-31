//! # terravista-core
//!
//! Mobile map SDK core — provides offline-first tile caching, vector rendering,
//! gesture-driven viewport control, and GPS integration for iOS and Android apps.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │  Platform Layer (Swift / Kotlin via FFI)         │
//! ├──────────────────────────────────────────────────┤
//! │  terravista-ffi  (C ABI / UniFFI bindings)      │
//! ├──────────────────────────────────────────────────┤
//! │  terravista-core                                 │
//! │  ┌────────┐ ┌────────┐ ┌────────┐ ┌──────────┐ │
//! │  │ Camera │ │ Tiles  │ │Offline │ │ Location │ │
//! │  │& Input │ │ Cache  │ │ Store  │ │ Service  │ │
//! │  └────────┘ └────────┘ └────────┘ └──────────┘ │
//! │  ┌────────┐ ┌────────┐ ┌────────┐              │
//! │  │Renderer│ │ Style  │ │ Route  │              │
//! │  │Pipeline│ │ Engine │ │ Engine │              │
//! │  └────────┘ └────────┘ └────────┘              │
//! └──────────────────────────────────────────────────┘
//! ```

pub mod camera;
pub mod error;
pub mod gesture;
pub mod location;
pub mod offline;
pub mod renderer;
pub mod route;
pub mod style;
pub mod tile_cache;
pub mod tile_package;

pub use camera::{Camera, Viewport};
pub use error::Error;
pub use location::{Coordinate, Location};
pub use tile_cache::TileCache;
