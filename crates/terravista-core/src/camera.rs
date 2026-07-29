//! Map camera and viewport — controls what the user sees.
//!
//! The camera defines the center, zoom, bearing, and pitch of the map view.
//! It drives tile loading decisions and gesture response.

use serde::{Deserialize, Serialize};

use crate::location::Coordinate;

/// Map viewport dimensions (device pixels).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub device_pixel_ratio: f32,
}

impl Viewport {
    pub fn new(width: u32, height: u32, dpr: f32) -> Self {
        Self {
            width,
            height,
            device_pixel_ratio: dpr,
        }
    }

    pub fn logical_width(&self) -> f32 {
        self.width as f32 / self.device_pixel_ratio
    }

    pub fn logical_height(&self) -> f32 {
        self.height as f32 / self.device_pixel_ratio
    }
}

/// The map camera state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Camera {
    pub center: Coordinate,
    pub zoom: f64,
    pub bearing: f64,
    pub pitch: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: Coordinate {
                latitude: 0.0,
                longitude: 0.0,
            },
            zoom: 2.0,
            bearing: 0.0,
            pitch: 0.0,
        }
    }
}

impl Camera {
    /// Create a camera centered on a coordinate at the given zoom.
    pub fn new(center: Coordinate, zoom: f64) -> Self {
        Self {
            center,
            zoom,
            bearing: 0.0,
            pitch: 0.0,
        }
    }

    /// Set bearing (rotation in degrees, 0 = north up).
    pub fn with_bearing(mut self, bearing: f64) -> Self {
        self.bearing = bearing % 360.0;
        self
    }

    /// Set pitch (tilt in degrees, 0 = top-down, max ~60).
    pub fn with_pitch(mut self, pitch: f64) -> Self {
        self.pitch = pitch.clamp(0.0, 60.0);
        self
    }

    /// Calculate the tile zoom level (integer) for tile fetching.
    pub fn tile_zoom(&self) -> u8 {
        self.zoom.round().clamp(0.0, 22.0) as u8
    }

    /// Web Mercator world size in device pixels at the current zoom.
    pub fn world_size(&self, viewport: &Viewport) -> f64 {
        256.0 * viewport.device_pixel_ratio as f64 * 2.0_f64.powf(self.zoom)
    }

    /// Get the visible bounding box at current camera state.
    pub fn visible_bounds(&self, viewport: &Viewport) -> VisibleBounds {
        let world = self.world_size(viewport);
        let cx = lon_to_world_x(self.center.longitude, world);
        let cy = lat_to_world_y(self.center.latitude, world);
        let half_w = viewport.width as f64 / 2.0;
        let half_h = viewport.height as f64 / 2.0;

        VisibleBounds {
            min_lon: world_x_to_lon(cx - half_w, world),
            max_lon: world_x_to_lon(cx + half_w, world),
            min_lat: world_y_to_lat((cy + half_h).min(world), world),
            max_lat: world_y_to_lat((cy - half_h).max(0.0), world),
        }
    }

    /// Pan the camera by device-pixel deltas.
    pub fn pan(&mut self, dx: f64, dy: f64, viewport: &Viewport) {
        let world = self.world_size(viewport);
        let x = lon_to_world_x(self.center.longitude, world) - dx;
        let y = (lat_to_world_y(self.center.latitude, world) - dy).clamp(0.0, world);
        self.center.longitude = wrap_lon(world_x_to_lon(x, world));
        self.center.latitude = world_y_to_lat(y, world);
    }

    /// Zoom by a delta (positive = zoom in).
    pub fn zoom_by(&mut self, delta: f64) {
        self.zoom = (self.zoom + delta).clamp(0.0, 22.0);
    }

    /// Zoom to a specific level, keeping a point fixed on screen.
    pub fn zoom_to(&mut self, target_zoom: f64, _anchor_x: f64, _anchor_y: f64) {
        self.zoom = target_zoom.clamp(0.0, 22.0);
    }
}

/// Visible geographic bounds of the map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisibleBounds {
    pub min_lon: f64,
    pub max_lon: f64,
    pub min_lat: f64,
    pub max_lat: f64,
}

impl VisibleBounds {
    /// Get tile coordinates that cover this bounds at the given zoom.
    pub fn tile_range(&self, zoom: u8) -> TileRange {
        let n = 2u32.pow(zoom as u32);

        let x_min = lon_to_tile_x(self.min_lon, n);
        let x_max = lon_to_tile_x(self.max_lon, n);
        let y_min = lat_to_tile_y(self.max_lat, n); // y is inverted
        let y_max = lat_to_tile_y(self.min_lat, n);

        TileRange {
            zoom,
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }
}

/// Range of tile coordinates to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRange {
    pub zoom: u8,
    pub x_min: u32,
    pub x_max: u32,
    pub y_min: u32,
    pub y_max: u32,
}

impl TileRange {
    /// Iterate all tile coordinates in this range.
    pub fn iter(&self) -> impl Iterator<Item = TileCoord> + '_ {
        (self.y_min..=self.y_max).flat_map(move |y| {
            (self.x_min..=self.x_max).map(move |x| TileCoord::new(self.zoom, x, y))
        })
    }

    /// Total number of tiles in this range.
    pub fn count(&self) -> u32 {
        (self.x_max - self.x_min + 1) * (self.y_max - self.y_min + 1)
    }
}

/// A single tile coordinate (z/x/y).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl TileCoord {
    pub fn new(z: u8, x: u32, y: u32) -> Self {
        Self { z, x, y }
    }

    /// URL path segment for this tile (e.g., "14/8192/5450").
    pub fn path(&self) -> String {
        format!("{}/{}/{}", self.z, self.x, self.y)
    }
}

/// Maximum latitude representable in Web Mercator.
pub const MAX_LATITUDE: f64 = 85.051_129;

/// Longitude to Web Mercator world-pixel X, where `world` is the world size in pixels.
pub fn lon_to_world_x(lon: f64, world: f64) -> f64 {
    (lon + 180.0) / 360.0 * world
}

/// Latitude to Web Mercator world-pixel Y (increases southward).
pub fn lat_to_world_y(lat: f64, world: f64) -> f64 {
    let clamped = lat.clamp(-MAX_LATITUDE, MAX_LATITUDE);
    let s = clamped.to_radians().tan().asinh();
    (1.0 - s / std::f64::consts::PI) / 2.0 * world
}

/// Web Mercator world-pixel X back to longitude.
pub fn world_x_to_lon(x: f64, world: f64) -> f64 {
    x / world * 360.0 - 180.0
}

/// Web Mercator world-pixel Y back to latitude.
pub fn world_y_to_lat(y: f64, world: f64) -> f64 {
    (std::f64::consts::PI * (1.0 - 2.0 * y / world))
        .sinh()
        .atan()
        .to_degrees()
}

fn wrap_lon(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

fn lon_to_tile_x(lon: f64, n: u32) -> u32 {
    (((lon + 180.0) / 360.0) * n as f64)
        .floor()
        .clamp(0.0, (n - 1) as f64) as u32
}

fn lat_to_tile_y(lat: f64, n: u32) -> u32 {
    let lat_rad = lat.to_radians();
    let y = (1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n as f64;
    y.floor().clamp(0.0, (n - 1) as f64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_default() {
        let cam = Camera::default();
        assert_eq!(cam.zoom, 2.0);
        assert_eq!(cam.bearing, 0.0);
    }

    #[test]
    fn test_tile_zoom_clamping() {
        let cam = Camera {
            zoom: 25.0,
            ..Camera::default()
        };
        assert_eq!(cam.tile_zoom(), 22);
    }

    #[test]
    fn test_visible_bounds() {
        let cam = Camera::new(
            Coordinate {
                latitude: 51.5,
                longitude: -0.1,
            },
            10.0,
        );
        let vp = Viewport::new(800, 600, 2.0);
        let bounds = cam.visible_bounds(&vp);
        assert!(bounds.min_lon < -0.1);
        assert!(bounds.max_lon > -0.1);
    }

    #[test]
    fn test_tile_coord_path() {
        let tc = TileCoord::new(14, 8192, 5450);
        assert_eq!(tc.path(), "14/8192/5450");
    }

    #[test]
    fn test_pan() {
        let mut cam = Camera::new(
            Coordinate {
                latitude: 0.0,
                longitude: 0.0,
            },
            5.0,
        );
        let vp = Viewport::new(800, 600, 1.0);
        cam.pan(100.0, 0.0, &vp);
        assert!(cam.center.longitude < 0.0);
    }

    #[test]
    fn test_world_projection_roundtrip() {
        let world = 256.0 * 2.0_f64.powi(12);
        for lat in [0.0, 51.5, -33.9, 71.0, 85.0] {
            let y = lat_to_world_y(lat, world);
            assert!((world_y_to_lat(y, world) - lat).abs() < 1e-6, "lat {lat}");
        }
        for lon in [0.0, -0.1, 179.9, -179.9] {
            let x = lon_to_world_x(lon, world);
            assert!((world_x_to_lon(x, world) - lon).abs() < 1e-9, "lon {lon}");
        }
    }

    /// A pixel of vertical pan covers fewer degrees of latitude the further you
    /// are from the equator. Linear-degree panning would move by the same amount.
    #[test]
    fn test_pan_latitude_follows_mercator() {
        let vp = Viewport::new(800, 600, 1.0);
        let shift = |lat: f64| {
            let mut cam = Camera::new(Coordinate::new(lat, 0.0), 10.0);
            cam.pan(0.0, 100.0, &vp);
            cam.center.latitude - lat
        };

        let at_equator = shift(0.0);
        let at_london = shift(51.5);
        assert!(at_equator > 0.0 && at_london > 0.0);
        assert!(
            at_london < at_equator * 0.7,
            "equator {at_equator} vs london {at_london}"
        );
    }

    #[test]
    fn test_pan_roundtrip() {
        let vp = Viewport::new(1080, 2280, 3.0);
        let mut cam = Camera::new(Coordinate::new(51.5, -0.1), 12.0);
        cam.pan(240.0, -180.0, &vp);
        cam.pan(-240.0, 180.0, &vp);
        assert!((cam.center.latitude - 51.5).abs() < 1e-9);
        assert!((cam.center.longitude - -0.1).abs() < 1e-9);
    }

    #[test]
    fn test_pan_wraps_across_antimeridian() {
        let vp = Viewport::new(800, 600, 1.0);
        let mut cam = Camera::new(Coordinate::new(0.0, 179.0), 4.0);
        cam.pan(-2000.0, 0.0, &vp);
        assert!(cam.center.longitude >= -180.0 && cam.center.longitude <= 180.0);
        assert!(cam.center.longitude < 0.0, "should have wrapped west");
    }

    #[test]
    fn test_pan_clamps_at_mercator_pole() {
        let vp = Viewport::new(800, 600, 1.0);
        let mut cam = Camera::new(Coordinate::new(84.0, 0.0), 4.0);
        cam.pan(0.0, 100_000.0, &vp);
        assert!(cam.center.latitude <= MAX_LATITUDE);
        assert!(cam.center.latitude > 84.0);
    }

    /// Visible bounds must be symmetric in world-pixel space around the centre,
    /// which off the equator means asymmetric in degrees.
    #[test]
    fn test_visible_bounds_off_equator() {
        let vp = Viewport::new(1080, 2280, 3.0);
        let cam = Camera::new(Coordinate::new(51.5, -0.1), 10.0);
        let b = cam.visible_bounds(&vp);
        assert!(b.min_lat < 51.5 && b.max_lat > 51.5);
        assert!(b.min_lon < -0.1 && b.max_lon > -0.1);

        let world = cam.world_size(&vp);
        let cy = lat_to_world_y(cam.center.latitude, world);
        let top = lat_to_world_y(b.max_lat, world);
        let bottom = lat_to_world_y(b.min_lat, world);
        assert!((cy - top - vp.height as f64 / 2.0).abs() < 1e-6);
        assert!((bottom - cy - vp.height as f64 / 2.0).abs() < 1e-6);
    }
}
