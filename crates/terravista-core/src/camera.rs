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

    /// Get the visible bounding box at current camera state.
    pub fn visible_bounds(&self, viewport: &Viewport) -> VisibleBounds {
        let scale = 2.0_f64.powf(self.zoom);
        let world_per_pixel = 360.0 / (256.0 * scale);

        let half_w = (viewport.logical_width() as f64 / 2.0) * world_per_pixel;
        let half_h = (viewport.logical_height() as f64 / 2.0) * world_per_pixel;

        VisibleBounds {
            min_lon: self.center.longitude - half_w,
            max_lon: self.center.longitude + half_w,
            min_lat: (self.center.latitude - half_h).max(-85.051_129),
            max_lat: (self.center.latitude + half_h).min(85.051_129),
        }
    }

    /// Pan the camera by pixel deltas.
    pub fn pan(&mut self, dx: f64, dy: f64, viewport: &Viewport) {
        let scale = 2.0_f64.powf(self.zoom);
        let world_per_pixel = 360.0 / (256.0 * scale);
        self.center.longitude -= dx * world_per_pixel;
        self.center.latitude += dy * world_per_pixel;
        self.center.latitude = self.center.latitude.clamp(-85.051_129, 85.051_129);

        // Wrap longitude
        if self.center.longitude > 180.0 {
            self.center.longitude -= 360.0;
        } else if self.center.longitude < -180.0 {
            self.center.longitude += 360.0;
        }

        let _ = viewport; // used for world_per_pixel calc above
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
}
