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

    /// Calculate the tile zoom level (integer) for tile fetching, ignoring
    /// screen density. Prefer [`Camera::tile_zoom_for`] when you have a viewport.
    pub fn tile_zoom(&self) -> u8 {
        self.zoom.round().clamp(0.0, 22.0) as u8
    }

    /// Integer tile zoom biased by device pixel ratio, so one 256 px tile lands
    /// on 256 device pixels instead of being upscaled and blurred.
    pub fn tile_zoom_for(&self, viewport: &Viewport) -> u8 {
        let bias = (viewport.device_pixel_ratio.max(0.01) as f64).log2();
        (self.zoom + bias).round().clamp(0.0, 22.0) as u8
    }

    /// Web Mercator world size in device pixels at the current zoom.
    pub fn world_size(&self, viewport: &Viewport) -> f64 {
        256.0 * viewport.device_pixel_ratio as f64 * 2.0_f64.powf(self.zoom)
    }

    /// Get the visible bounding box at current camera state.
    ///
    /// When the camera is rotated this is the bounding box of the rotated
    /// viewport, so it covers every tile the screen can show.
    pub fn visible_bounds(&self, viewport: &Viewport) -> VisibleBounds {
        let world = self.world_size(viewport);
        let cx = lon_to_world_x(self.center.longitude, world);
        let cy = lat_to_world_y(self.center.latitude, world);
        let half_w = viewport.width as f64 / 2.0;
        let half_h = viewport.height as f64 / 2.0;

        let (sin_b, cos_b) = self.bearing.to_radians().sin_cos();
        let ext_x = half_w * cos_b.abs() + half_h * sin_b.abs();
        let ext_y = half_w * sin_b.abs() + half_h * cos_b.abs();

        VisibleBounds {
            min_lon: world_x_to_lon(cx - ext_x, world),
            max_lon: world_x_to_lon(cx + ext_x, world),
            min_lat: world_y_to_lat((cy + ext_y).min(world), world),
            max_lat: world_y_to_lat((cy - ext_y).max(0.0), world),
        }
    }

    /// Screen position of a coordinate, in device pixels from the viewport's
    /// top-left corner.
    ///
    /// North-up, like [`crate::renderer::visible_tiles`]: the host draws map
    /// content unrotated and spins the canvas by `-bearing`.
    pub fn project(&self, coord: &Coordinate, viewport: &Viewport) -> (f64, f64) {
        let world = self.world_size(viewport);
        let mut dx =
            lon_to_world_x(coord.longitude, world) - lon_to_world_x(self.center.longitude, world);
        // take the short way round, so a point just past the antimeridian does
        // not land a whole world away
        if dx > world / 2.0 {
            dx -= world;
        } else if dx < -world / 2.0 {
            dx += world;
        }
        let dy =
            lat_to_world_y(coord.latitude, world) - lat_to_world_y(self.center.latitude, world);
        (
            viewport.width as f64 / 2.0 + dx,
            viewport.height as f64 / 2.0 + dy,
        )
    }

    /// Ground metres covered by one device pixel, at the camera's latitude.
    pub fn metres_per_pixel(&self, viewport: &Viewport) -> f64 {
        const EQUATOR_M: f64 = 40_075_016.686;
        let lat = self.center.latitude.clamp(-MAX_LATITUDE, MAX_LATITUDE);
        EQUATOR_M * lat.to_radians().cos() / self.world_size(viewport)
    }

    /// Rotate a screen-space delta into north-up map space.
    ///
    /// The host draws the map rotated by `-bearing`, so screen deltas have to
    /// come back the other way before they touch the centre.
    fn screen_to_map(&self, dx: f64, dy: f64) -> (f64, f64) {
        if self.bearing == 0.0 {
            return (dx, dy);
        }
        let (sin_b, cos_b) = self.bearing.to_radians().sin_cos();
        (dx * cos_b - dy * sin_b, dx * sin_b + dy * cos_b)
    }

    /// Pan the camera by device-pixel deltas in screen space.
    pub fn pan(&mut self, dx: f64, dy: f64, viewport: &Viewport) {
        let (dx, dy) = self.screen_to_map(dx, dy);
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

    /// Zoom to a specific level, keeping the point under `anchor` fixed on screen.
    ///
    /// The anchor is in device pixels from the viewport's top-left corner.
    pub fn zoom_to(&mut self, target_zoom: f64, anchor_x: f64, anchor_y: f64, viewport: &Viewport) {
        let target = target_zoom.clamp(0.0, 22.0);
        if target == self.zoom {
            return;
        }

        let (off_x, off_y) = self.screen_to_map(
            anchor_x - viewport.width as f64 / 2.0,
            anchor_y - viewport.height as f64 / 2.0,
        );

        // the coordinate sitting under the anchor before the zoom
        let before = self.world_size(viewport);
        let anchor_lon = world_x_to_lon(
            lon_to_world_x(self.center.longitude, before) + off_x,
            before,
        );
        let anchor_lat = world_y_to_lat(
            (lat_to_world_y(self.center.latitude, before) + off_y).clamp(0.0, before),
            before,
        );

        self.zoom = target;

        // put it back under the anchor at the new scale
        let after = self.world_size(viewport);
        let x = lon_to_world_x(anchor_lon, after) - off_x;
        let y = (lat_to_world_y(anchor_lat, after) - off_y).clamp(0.0, after);
        self.center.longitude = wrap_lon(world_x_to_lon(x, after));
        self.center.latitude = world_y_to_lat(y, after);
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
    pub fn iter(self) -> impl Iterator<Item = TileCoord> {
        (self.y_min..=self.y_max).flat_map(move |y| {
            (self.x_min..=self.x_max).map(move |x| TileCoord::new(self.zoom, x, y))
        })
    }

    /// Total number of tiles in this range.
    ///
    /// A whole world of tiles overflows a `u32` past zoom 15, so this counts in
    /// `u64`.
    pub fn count(self) -> u64 {
        let width = u64::from(self.x_max.saturating_sub(self.x_min)) + 1;
        let height = u64::from(self.y_max.saturating_sub(self.y_min)) + 1;
        width * height
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

    /// Zooming about an anchor must leave the coordinate under that anchor put.
    #[test]
    fn test_zoom_to_holds_anchor() {
        let vp = Viewport::new(1080, 2280, 3.0);
        let anchor_x = 250.0;
        let anchor_y = 1800.0;

        let mut cam = Camera::new(Coordinate::new(51.5, -0.1), 12.0);
        let geo_at = |c: &Camera| {
            let world = c.world_size(&vp);
            let x = lon_to_world_x(c.center.longitude, world) + anchor_x - vp.width as f64 / 2.0;
            let y = lat_to_world_y(c.center.latitude, world) + anchor_y - vp.height as f64 / 2.0;
            (world_y_to_lat(y, world), world_x_to_lon(x, world))
        };

        let (lat0, lon0) = geo_at(&cam);
        cam.zoom_to(14.0, anchor_x, anchor_y, &vp);
        let (lat1, lon1) = geo_at(&cam);

        assert!((cam.zoom - 14.0).abs() < 1e-12);
        assert!((lat1 - lat0).abs() < 1e-9, "lat drifted {lat0} -> {lat1}");
        assert!((lon1 - lon0).abs() < 1e-9, "lon drifted {lon0} -> {lon1}");
    }

    /// Zooming about the exact centre must not move the centre at all.
    #[test]
    fn test_zoom_to_centre_keeps_centre() {
        let vp = Viewport::new(1080, 2280, 3.0);
        let mut cam = Camera::new(Coordinate::new(51.5, -0.1), 12.0);
        cam.zoom_to(15.0, vp.width as f64 / 2.0, vp.height as f64 / 2.0, &vp);
        assert!((cam.center.latitude - 51.5).abs() < 1e-9);
        assert!((cam.center.longitude - -0.1).abs() < 1e-9);
    }

    /// One tile should land on 256 device pixels, not be upscaled by the density.
    #[test]
    fn test_tile_zoom_is_density_biased() {
        let cam = Camera::new(Coordinate::new(51.5, -0.1), 12.0);
        assert_eq!(cam.tile_zoom_for(&Viewport::new(1080, 2280, 1.0)), 12);
        assert_eq!(cam.tile_zoom_for(&Viewport::new(1080, 2280, 2.0)), 13);
        assert_eq!(cam.tile_zoom_for(&Viewport::new(1080, 2280, 4.0)), 14);
        // still clamped at the top of the range
        let deep = Camera::new(Coordinate::new(51.5, -0.1), 22.0);
        assert_eq!(deep.tile_zoom_for(&Viewport::new(1080, 2280, 3.0)), 22);
    }

    /// A rotated viewport covers ground outside its unrotated box.
    #[test]
    fn test_visible_bounds_grows_when_rotated() {
        let vp = Viewport::new(1080, 2280, 3.0);
        let north_up = Camera::new(Coordinate::new(51.5, -0.1), 12.0);
        let turned = Camera::new(Coordinate::new(51.5, -0.1), 12.0).with_bearing(45.0);

        let a = north_up.visible_bounds(&vp);
        let b = turned.visible_bounds(&vp);
        assert!(
            b.max_lon - b.min_lon > a.max_lon - a.min_lon,
            "rotated bounds must be wider"
        );
        // 90 degrees swaps the extents rather than growing them
        let quarter = Camera::new(Coordinate::new(51.5, -0.1), 12.0).with_bearing(90.0);
        let q = quarter.visible_bounds(&vp);
        let world = quarter.world_size(&vp);
        let q_px = lon_to_world_x(q.max_lon, world) - lon_to_world_x(q.min_lon, world);
        assert!((q_px - vp.height as f64).abs() < 1e-6);
    }

    /// Panning a rotated map follows the finger, not the meridian.
    #[test]
    fn test_pan_follows_screen_when_rotated() {
        let vp = Viewport::new(800, 600, 1.0);
        let mut cam = Camera::new(Coordinate::new(0.0, 0.0), 10.0).with_bearing(90.0);
        cam.pan(100.0, 0.0, &vp);
        // at 90 degrees a horizontal drag moves the camera in latitude
        assert!(cam.center.latitude.abs() > 1e-6, "expected latitude change");
        assert!(cam.center.longitude.abs() < 1e-9, "longitude should hold");
    }

    /// The camera centre projects to the middle of the viewport.
    #[test]
    fn test_project_centre() {
        let vp = Viewport::new(1080, 2280, 3.0);
        let cam = Camera::new(Coordinate::new(51.5, -0.1), 14.0);
        let (x, y) = cam.project(&cam.center, &vp);
        assert!((x - 540.0).abs() < 1e-9);
        assert!((y - 1140.0).abs() < 1e-9);
    }

    /// A projected point must land where the tile drawing it lands.
    #[test]
    fn test_project_agrees_with_tile_placement() {
        use crate::renderer::visible_tiles;

        let vp = Viewport::new(1080, 2280, 3.0);
        let cam = Camera::new(Coordinate::new(51.5, -0.1), 12.0);
        let tiles = visible_tiles(&cam, &vp);
        let zoom = cam.tile_zoom_for(&vp);
        let n = 2u32.pow(u32::from(zoom));

        for t in &tiles {
            // north-west corner of the tile, as a coordinate
            let world = 256.0 * n as f64;
            let lon = world_x_to_lon(t.coord.x as f64 * 256.0, world);
            let lat = world_y_to_lat(t.coord.y as f64 * 256.0, world);
            let (x, y) = cam.project(&Coordinate::new(lat, lon), &vp);
            assert!(
                (x - t.screen_x as f64).abs() < 0.01,
                "x {x} vs {}",
                t.screen_x
            );
            assert!(
                (y - t.screen_y as f64).abs() < 0.01,
                "y {y} vs {}",
                t.screen_y
            );
        }
    }

    /// Moving east projects right, moving north projects up.
    #[test]
    fn test_project_directions() {
        let vp = Viewport::new(800, 600, 1.0);
        let cam = Camera::new(Coordinate::new(51.5, -0.1), 12.0);
        let (east_x, _) = cam.project(&Coordinate::new(51.5, -0.09), &vp);
        let (_, north_y) = cam.project(&Coordinate::new(51.51, -0.1), &vp);
        assert!(east_x > 400.0);
        assert!(north_y < 300.0);
    }

    /// A point just across the antimeridian is a few pixels away, not a world away.
    #[test]
    fn test_project_wraps_across_antimeridian() {
        let vp = Viewport::new(800, 600, 1.0);
        let cam = Camera::new(Coordinate::new(0.0, 179.99), 10.0);
        let (x, _) = cam.project(&Coordinate::new(0.0, -179.99), &vp);
        assert!(x > 400.0 && x < 500.0, "x {x}");
    }

    #[test]
    fn test_metres_per_pixel() {
        // one 256 px tile spans the equator at zoom 0
        let vp = Viewport::new(800, 600, 1.0);
        let equator = Camera::new(Coordinate::new(0.0, 0.0), 0.0);
        assert!((equator.metres_per_pixel(&vp) - 156_543.03).abs() < 0.1);

        // halves with each zoom level
        let deeper = Camera::new(Coordinate::new(0.0, 0.0), 1.0);
        assert!((deeper.metres_per_pixel(&vp) * 2.0 - equator.metres_per_pixel(&vp)).abs() < 1e-6);

        // and shrinks with the cosine of latitude
        let london = Camera::new(Coordinate::new(51.5, 0.0), 0.0);
        let ratio = london.metres_per_pixel(&vp) / equator.metres_per_pixel(&vp);
        assert!((ratio - 51.5_f64.to_radians().cos()).abs() < 1e-9);

        // a denser screen packs more pixels into the same ground
        let dense = Viewport::new(800, 600, 2.0);
        assert!(
            (equator.metres_per_pixel(&dense) * 2.0 - equator.metres_per_pixel(&vp)).abs() < 1e-6
        );
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
