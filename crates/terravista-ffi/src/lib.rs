//! # terravista-ffi
//!
//! C-compatible FFI bindings for the TerraVista mobile map SDK.
//!
//! Provides a flat C API that Swift (iOS) and Kotlin (Android) can call
//! through their respective FFI mechanisms.
//!
//! ## Memory Management
//!
//! - All opaque pointers returned by `tv_*_create` must be freed with the
//!   corresponding `tv_*_destroy` function.
//! - String pointers returned by the SDK are owned by the caller and must
//!   be freed with `tv_string_free`.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::{SystemTime, UNIX_EPOCH};

use terravista_core::camera::{Camera, TileCoord, Viewport};
use terravista_core::gesture::{GestureRecognizer, GestureResult, TouchEvent, TouchPoint};
use terravista_core::location::{Coordinate, TrackingMode};
use terravista_core::renderer::{TilePlacement, visible_tiles};
use terravista_core::route::{Maneuver, NavStatus, NavigationUpdate, Navigator, Route, RouteStep};
use terravista_core::tile_cache::{CacheConfig, TileCache, TileData, TileMeta};

/// Opaque map state handle.
pub struct TvMapState {
    camera: Camera,
    viewport: Viewport,
    gesture: GestureRecognizer,
    tile_cache: TileCache,
    /// Filled by `tv_map_visible_tile_count`, read by `tv_map_visible_tile_at`.
    placements: Vec<TilePlacement>,
    tracking: TrackingMode,
    user_location: Option<TvUserLocation>,
    navigator: Option<Navigator>,
    /// Last result of `tv_nav_update`, read back by `tv_nav_progress`.
    nav_last: Option<NavigationUpdate>,
}

// ─── Map State ───────────────────────────────────────────────────────────────

/// Create a new map state with default camera.
#[unsafe(no_mangle)]
pub extern "C" fn tv_map_create(
    width: u32,
    height: u32,
    device_pixel_ratio: f32,
) -> *mut TvMapState {
    let state = Box::new(TvMapState {
        camera: Camera::default(),
        viewport: Viewport::new(width, height, device_pixel_ratio),
        gesture: GestureRecognizer::new(),
        tile_cache: TileCache::new(CacheConfig::default()),
        placements: Vec::new(),
        tracking: TrackingMode::None,
        user_location: None,
        navigator: None,
        nav_last: None,
    });
    Box::into_raw(state)
}

/// Destroy a map state.
///
/// # Safety
/// `state` must be a valid pointer returned by `tv_map_create`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_destroy(state: *mut TvMapState) {
    if !state.is_null() {
        drop(unsafe { Box::from_raw(state) });
    }
}

/// Set the map center.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_center(state: *mut TvMapState, latitude: f64, longitude: f64) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.camera.center = Coordinate::new(latitude, longitude);
    }
}

/// Set the zoom level (0-22).
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_zoom(state: *mut TvMapState, zoom: f64) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.camera.zoom = zoom.clamp(0.0, 22.0);
    }
}

/// Get current zoom level.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_get_zoom(state: *const TvMapState) -> f64 {
    unsafe { state.as_ref() }.map_or(0.0, |s| s.camera.zoom)
}

/// Get current center latitude.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_get_center_lat(state: *const TvMapState) -> f64 {
    unsafe { state.as_ref() }.map_or(0.0, |s| s.camera.center.latitude)
}

/// Get current center longitude.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_get_center_lon(state: *const TvMapState) -> f64 {
    unsafe { state.as_ref() }.map_or(0.0, |s| s.camera.center.longitude)
}

/// Set bearing (rotation degrees, 0 = north up).
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_bearing(state: *mut TvMapState, bearing: f64) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.camera.bearing = bearing % 360.0;
    }
}

/// Set pitch (tilt degrees, 0-60).
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_pitch(state: *mut TvMapState, pitch: f64) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.camera.pitch = pitch.clamp(0.0, 60.0);
    }
}

/// Update viewport size (e.g., on device rotation).
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_viewport(
    state: *mut TvMapState,
    width: u32,
    height: u32,
    dpr: f32,
) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.viewport = Viewport::new(width, height, dpr);
    }
}

/// Get current bearing in degrees.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_get_bearing(state: *const TvMapState) -> f64 {
    unsafe { state.as_ref() }.map_or(0.0, |s| s.camera.bearing)
}

/// Get current pitch in degrees.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_get_pitch(state: *const TvMapState) -> f64 {
    unsafe { state.as_ref() }.map_or(0.0, |s| s.camera.pitch)
}

// ─── Visible Tiles ───────────────────────────────────────────────────────────

/// Range of tile coordinates covering the viewport.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvTileRange {
    pub zoom: u8,
    pub x_min: u32,
    pub x_max: u32,
    pub y_min: u32,
    pub y_max: u32,
}

/// Where to draw one tile, in device pixels.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvTilePlacement {
    pub z: u8,
    pub x: u32,
    pub y: u32,
    pub screen_x: f32,
    pub screen_y: f32,
    pub size: f32,
}

/// Write the tile range covering the current viewport into `out`.
///
/// Returns false if either pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_tile_range(
    state: *const TvMapState,
    out: *mut TvTileRange,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let range = s
        .camera
        .visible_bounds(&s.viewport)
        .tile_range(s.camera.tile_zoom_for(&s.viewport));
    *out = TvTileRange {
        zoom: range.zoom,
        x_min: range.x_min,
        x_max: range.x_max,
        y_min: range.y_min,
        y_max: range.y_max,
    };
    true
}

/// Recompute the visible tile set and return how many tiles it holds.
///
/// Call this once per frame before `tv_map_visible_tile_at`.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_visible_tile_count(state: *mut TvMapState) -> u32 {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    s.placements = visible_tiles(&s.camera, &s.viewport);
    s.placements.len() as u32
}

/// Read one placement from the set computed by `tv_map_visible_tile_count`.
///
/// Returns false if the index is out of range or a pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_visible_tile_at(
    state: *const TvMapState,
    index: u32,
    out: *mut TvTilePlacement,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let Some(p) = s.placements.get(index as usize) else {
        return false;
    };
    *out = TvTilePlacement {
        z: p.coord.z,
        x: p.coord.x,
        y: p.coord.y,
        screen_x: p.screen_x,
        screen_y: p.screen_y,
        size: p.size,
    };
    true
}

// ─── Gesture Handling ────────────────────────────────────────────────────────

/// Process a pan gesture (single-finger drag).
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_pan(state: *mut TvMapState, dx: f64, dy: f64) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.camera.pan(dx, dy, &s.viewport);
    }
}

/// Process a zoom gesture (pinch or scroll wheel).
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_zoom_by(state: *mut TvMapState, delta: f64) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.camera.zoom_by(delta);
    }
}

/// Touch phase for `tv_map_touch`.
pub const TV_TOUCH_BEGIN: i32 = 0;
pub const TV_TOUCH_MOVE: i32 = 1;
pub const TV_TOUCH_END: i32 = 2;
pub const TV_TOUCH_CANCEL: i32 = 3;

/// Gesture applied by `tv_map_touch`.
pub const TV_GESTURE_NONE: i32 = 0;
pub const TV_GESTURE_PAN: i32 = 1;
pub const TV_GESTURE_ZOOM: i32 = 2;
pub const TV_GESTURE_ROTATE: i32 = 3;
pub const TV_GESTURE_PITCH: i32 = 4;
/// A two-finger gesture, zoom and rotation applied together.
pub const TV_GESTURE_PINCH: i32 = 5;

/// Feed a touch event through the gesture recognizer and apply it to the camera.
///
/// `xs`, `ys` and `ids` are parallel arrays of `count` touch points, in device
/// pixels. Returns the `TV_GESTURE_*` kind that was applied.
///
/// # Safety
/// `state` must be valid. `xs`, `ys` and `ids` must each point to at least
/// `count` elements, or be null when `count` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_touch(
    state: *mut TvMapState,
    phase: i32,
    xs: *const f64,
    ys: *const f64,
    ids: *const u64,
    count: usize,
) -> i32 {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return TV_GESTURE_NONE;
    };

    let mut points = Vec::with_capacity(count);
    if count > 0 {
        if xs.is_null() || ys.is_null() || ids.is_null() {
            return TV_GESTURE_NONE;
        }
        let (xs, ys, ids) = unsafe {
            (
                std::slice::from_raw_parts(xs, count),
                std::slice::from_raw_parts(ys, count),
                std::slice::from_raw_parts(ids, count),
            )
        };
        for i in 0..count {
            points.push(TouchPoint {
                id: ids[i],
                x: xs[i],
                y: ys[i],
            });
        }
    }

    let event = match phase {
        TV_TOUCH_BEGIN => TouchEvent::Begin(points),
        TV_TOUCH_MOVE => TouchEvent::Move(points),
        TV_TOUCH_END => TouchEvent::End(points),
        TV_TOUCH_CANCEL => TouchEvent::Cancel,
        _ => return TV_GESTURE_NONE,
    };

    let result = s.gesture.process(&event, &s.camera);
    GestureRecognizer::apply(&result, &mut s.camera, &s.viewport);

    match result {
        GestureResult::None => TV_GESTURE_NONE,
        GestureResult::Pan { .. } => TV_GESTURE_PAN,
        GestureResult::Zoom { .. } => TV_GESTURE_ZOOM,
        GestureResult::Pinch { .. } => TV_GESTURE_PINCH,
        GestureResult::Rotate { .. } => TV_GESTURE_ROTATE,
        GestureResult::Pitch { .. } => TV_GESTURE_PITCH,
    }
}

// ─── Tile Cache ──────────────────────────────────────────────────────────────

/// Set the tile URL template (e.g., "`https://tiles.example.com/`{z}/{x}/{y}.mvt").
///
/// # Safety
/// `state` and `url` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_tile_url(state: *mut TvMapState, url: *const c_char) {
    if state.is_null() || url.is_null() {
        return;
    }
    let s = unsafe { &mut *state };
    if let Ok(url_str) = unsafe { CStr::from_ptr(url) }.to_str() {
        s.tile_cache.set_url_template(url_str.to_string());
    }
}

/// Build the tile URL for a coordinate from the configured template.
///
/// Returns a string the caller must free with `tv_string_free`, or null.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_tile_url(
    state: *const TvMapState,
    z: u8,
    x: u32,
    y: u32,
) -> *mut c_char {
    let Some(s) = (unsafe { state.as_ref() }) else {
        return std::ptr::null_mut();
    };
    match CString::new(s.tile_cache.tile_url(&TileCoord::new(z, x, y))) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Store tile bytes in the cache.
///
/// Returns false on a null pointer. `content_type` may be null.
///
/// # Safety
/// `state` must be valid, `bytes` must point to at least `len` bytes, and
/// `content_type` must be a valid C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_cache_put(
    state: *mut TvMapState,
    z: u8,
    x: u32,
    y: u32,
    bytes: *const u8,
    len: usize,
    content_type: *const c_char,
) -> bool {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return false;
    };
    if bytes.is_null() && len > 0 {
        return false;
    }
    let data = if len == 0 {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec()
    };
    let content_type = if content_type.is_null() {
        "application/octet-stream".to_string()
    } else {
        unsafe { CStr::from_ptr(content_type) }
            .to_string_lossy()
            .into_owned()
    };
    let fetched_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    s.tile_cache.insert(TileData {
        meta: TileMeta {
            coord: TileCoord::new(z, x, y),
            size_bytes: data.len() as u64,
            fetched_at,
            etag: None,
            content_type,
        },
        bytes: data,
    });
    true
}

/// Whether a tile is cached.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_cache_has(state: *const TvMapState, z: u8, x: u32, y: u32) -> bool {
    unsafe { state.as_ref() }.is_some_and(|s| s.tile_cache.contains(&TileCoord::new(z, x, y)))
}

/// Copy cached tile bytes into `out`, writing at most `cap` bytes.
///
/// Returns the tile's full length, which may exceed `cap`, or 0 if the tile is
/// not cached. Marks the tile as recently used.
///
/// # Safety
/// `state` must be valid and `out` must point to at least `cap` bytes, or be
/// null when `cap` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_cache_get(
    state: *mut TvMapState,
    z: u8,
    x: u32,
    y: u32,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    let Some(tile) = s.tile_cache.get(&TileCoord::new(z, x, y)) else {
        return 0;
    };
    let len = tile.bytes.len();
    let n = len.min(cap);
    if n > 0 && !out.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(tile.bytes.as_ptr(), out, n) };
    }
    len
}

/// Get the number of cached tiles.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_cache_tile_count(state: *const TvMapState) -> u32 {
    unsafe { state.as_ref() }.map_or(0, |s| s.tile_cache.len() as u32)
}

/// Get cache size in bytes.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_cache_size_bytes(state: *const TvMapState) -> u64 {
    unsafe { state.as_ref() }.map_or(0, |s| s.tile_cache.size_bytes())
}

/// Clear the tile cache.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_cache_clear(state: *mut TvMapState) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.tile_cache.clear();
    }
}

// ─── Projection ──────────────────────────────────────────────────────────────

/// A point on screen, in device pixels from the viewport's top-left corner.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvScreenPoint {
    pub x: f32,
    pub y: f32,
}

/// Project a coordinate to its screen position, north-up like the tile
/// placements from `tv_map_visible_tile_at`.
///
/// Returns false if a pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_project(
    state: *const TvMapState,
    latitude: f64,
    longitude: f64,
    out: *mut TvScreenPoint,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let (x, y) = s
        .camera
        .project(&Coordinate::new(latitude, longitude), &s.viewport);
    *out = TvScreenPoint {
        x: x as f32,
        y: y as f32,
    };
    true
}

/// Ground metres covered by one device pixel at the current camera.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_metres_per_pixel(state: *const TvMapState) -> f64 {
    unsafe { state.as_ref() }.map_or(0.0, |s| s.camera.metres_per_pixel(&s.viewport))
}

// ─── User Location ───────────────────────────────────────────────────────────

/// The last location handed to `tv_map_set_user_location`.
///
/// `accuracy_m` is a horizontal radius; it is negative when unknown.
/// `bearing_deg` is NaN when unknown.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvUserLocation {
    pub latitude: f64,
    pub longitude: f64,
    pub accuracy_m: f64,
    pub bearing_deg: f64,
}

/// Tracking mode for `tv_map_set_tracking_mode`.
pub const TV_TRACKING_NONE: i32 = 0;
pub const TV_TRACKING_FOLLOW: i32 = 1;
/// Follow and rotate the map to the compass heading fed in as the bearing.
pub const TV_TRACKING_FOLLOW_WITH_HEADING: i32 = 2;
/// Follow and rotate the map to the direction of travel fed in as the bearing.
pub const TV_TRACKING_FOLLOW_WITH_COURSE: i32 = 3;

fn tracking_from_code(code: i32) -> Option<TrackingMode> {
    match code {
        TV_TRACKING_NONE => Some(TrackingMode::None),
        TV_TRACKING_FOLLOW => Some(TrackingMode::Follow),
        TV_TRACKING_FOLLOW_WITH_HEADING => Some(TrackingMode::FollowWithHeading),
        TV_TRACKING_FOLLOW_WITH_COURSE => Some(TrackingMode::FollowWithCourse),
        _ => None,
    }
}

fn tracking_code(mode: TrackingMode) -> i32 {
    match mode {
        TrackingMode::None => TV_TRACKING_NONE,
        TrackingMode::Follow => TV_TRACKING_FOLLOW,
        TrackingMode::FollowWithHeading => TV_TRACKING_FOLLOW_WITH_HEADING,
        TrackingMode::FollowWithCourse => TV_TRACKING_FOLLOW_WITH_COURSE,
    }
}

/// Move the camera onto the stored user location, as the tracking mode asks.
fn apply_tracking(state: &mut TvMapState) {
    let Some(loc) = state.user_location else {
        return;
    };
    if state.tracking.follows() {
        state.camera.center = Coordinate::new(loc.latitude, loc.longitude);
    }
    if state.tracking.rotates() && loc.bearing_deg.is_finite() {
        state.camera.bearing = loc.bearing_deg % 360.0;
    }
}

/// Set the user's location, for drawing and for camera tracking.
///
/// Pass a negative `accuracy_m` or a NaN `bearing_deg` when unknown. The SDK
/// never reads a platform location provider; the host supplies every fix.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_user_location(
    state: *mut TvMapState,
    latitude: f64,
    longitude: f64,
    accuracy_m: f64,
    bearing_deg: f64,
) -> bool {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return false;
    };
    if !latitude.is_finite() || !longitude.is_finite() {
        return false;
    }
    s.user_location = Some(TvUserLocation {
        latitude,
        longitude,
        accuracy_m,
        bearing_deg,
    });
    apply_tracking(s);
    true
}

/// Read back the stored user location.
///
/// Returns false if none has been set or a pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_user_location(
    state: *const TvMapState,
    out: *mut TvUserLocation,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let Some(loc) = s.user_location else {
        return false;
    };
    *out = loc;
    true
}

/// Set the camera tracking mode to one of the `TV_TRACKING_*` values.
///
/// Snaps the camera onto the stored location straight away, so switching mode
/// does not wait for the next fix. Returns false for an unknown mode.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_tracking_mode(state: *mut TvMapState, mode: i32) -> bool {
    let (Some(s), Some(mode)) = (unsafe { state.as_mut() }, tracking_from_code(mode)) else {
        return false;
    };
    s.tracking = mode;
    apply_tracking(s);
    true
}

/// Get the current tracking mode as a `TV_TRACKING_*` value.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_get_tracking_mode(state: *const TvMapState) -> i32 {
    unsafe { state.as_ref() }.map_or(TV_TRACKING_NONE, |s| tracking_code(s.tracking))
}

// ─── Navigation ──────────────────────────────────────────────────────────────

/// One vertex of a route's geometry.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvRoutePoint {
    pub latitude: f64,
    pub longitude: f64,
}

/// One step of a route, covering the geometry from `start_index` to `end_index`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvRouteStep {
    /// Instruction text, borrowed for the duration of the call. May be null.
    pub instruction: *const c_char,
    pub start_index: u32,
    pub end_index: u32,
}

/// Navigation status in `TvNavProgress`.
pub const TV_NAV_ON_ROUTE: i32 = 0;
pub const TV_NAV_OFF_ROUTE: i32 = 1;
pub const TV_NAV_ARRIVED: i32 = 2;

/// Progress along the current route.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvNavProgress {
    /// One of the `TV_NAV_*` values.
    pub status: i32,
    pub step_index: u32,
    pub step_count: u32,
    pub distance_to_next_step_m: f64,
    pub distance_remaining_m: f64,
    pub off_route: bool,
}

fn nav_status_code(status: NavStatus) -> i32 {
    match status {
        NavStatus::OnRoute => TV_NAV_ON_ROUTE,
        NavStatus::OffRoute => TV_NAV_OFF_ROUTE,
        NavStatus::Arrived => TV_NAV_ARRIVED,
    }
}

fn nav_progress(update: &NavigationUpdate, step_count: usize) -> TvNavProgress {
    TvNavProgress {
        status: nav_status_code(update.status),
        step_index: update.current_step as u32,
        step_count: step_count as u32,
        distance_to_next_step_m: update.distance_to_next_step,
        distance_remaining_m: update.distance_remaining,
        off_route: update.status == NavStatus::OffRoute,
    }
}

/// Set the route to navigate, replacing any current one.
///
/// Routes come from a router (itinera server-side, say); this SDK follows one,
/// it does not compute one. Needs at least two points and one step. Step
/// indices are clamped into the geometry. Returns false on invalid input,
/// leaving the previous route in place.
///
/// # Safety
/// `points` must hold `point_count` elements and `steps` must hold `step_count`
/// elements. Each step's `instruction` must be a valid C string or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_nav_set_route(
    state: *mut TvMapState,
    points: *const TvRoutePoint,
    point_count: usize,
    steps: *const TvRouteStep,
    step_count: usize,
) -> bool {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return false;
    };
    if points.is_null() || steps.is_null() || point_count < 2 || step_count == 0 {
        return false;
    }

    let geometry: Vec<Coordinate> = unsafe { std::slice::from_raw_parts(points, point_count) }
        .iter()
        .map(|p| Coordinate::new(p.latitude, p.longitude))
        .collect();
    if geometry
        .iter()
        .any(|c| !c.latitude.is_finite() || !c.longitude.is_finite())
    {
        return false;
    }

    let last = geometry.len() - 1;
    let route_steps: Vec<RouteStep> = unsafe { std::slice::from_raw_parts(steps, step_count) }
        .iter()
        .map(|step| {
            let instruction = if step.instruction.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(step.instruction) }
                    .to_string_lossy()
                    .into_owned()
            };
            let start = (step.start_index as usize).min(last);
            RouteStep {
                instruction,
                // the C ABI carries instruction text only; nothing in the
                // progress maths reads the maneuver
                maneuver: Maneuver::Straight,
                distance_m: 0.0,
                duration_s: 0.0,
                start_index: start,
                end_index: (step.end_index as usize).clamp(start, last),
            }
        })
        .collect();

    let distance_m = geometry
        .windows(2)
        .map(|pair| pair[0].distance_to(&pair[1]))
        .sum();

    s.navigator = Some(Navigator::new(Route {
        geometry,
        distance_m,
        duration_s: 0.0,
        steps: route_steps,
    }));
    s.nav_last = None;
    true
}

/// Advance navigation with a new location and write the progress into `out`.
///
/// Returns false if no route is set or a pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_nav_update(
    state: *mut TvMapState,
    latitude: f64,
    longitude: f64,
    out: *mut TvNavProgress,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_mut() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let Some(nav) = s.navigator.as_mut() else {
        return false;
    };
    let update = nav.update(&Coordinate::new(latitude, longitude));
    *out = nav_progress(&update, nav.route().steps.len());
    s.nav_last = Some(update);
    true
}

/// Write the progress from the last `tv_nav_update` into `out`.
///
/// Returns false before the first update, or if a pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_nav_progress(
    state: *const TvMapState,
    out: *mut TvNavProgress,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let (Some(update), Some(nav)) = (s.nav_last.as_ref(), s.navigator.as_ref()) else {
        return false;
    };
    *out = nav_progress(update, nav.route().steps.len());
    true
}

/// Instruction text from the last `tv_nav_update`.
///
/// Returns a string the caller must free with `tv_string_free`, or null before
/// the first update.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_nav_instruction(state: *const TvMapState) -> *mut c_char {
    let Some(s) = (unsafe { state.as_ref() }) else {
        return std::ptr::null_mut();
    };
    let Some(update) = s.nav_last.as_ref() else {
        return std::ptr::null_mut();
    };
    match CString::new(update.instruction.as_str()) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Drop the current route and its progress.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_nav_clear(state: *mut TvMapState) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.navigator = None;
        s.nav_last = None;
    }
}

// ─── Utility ─────────────────────────────────────────────────────────────────

/// Great-circle distance between two coordinates, in metres.
#[unsafe(no_mangle)]
pub extern "C" fn tv_distance_between(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    Coordinate::new(lat1, lon1).distance_to(&Coordinate::new(lat2, lon2))
}

/// Initial bearing from the first coordinate to the second, in degrees from north.
#[unsafe(no_mangle)]
pub extern "C" fn tv_bearing_between(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    Coordinate::new(lat1, lon1).bearing_to(&Coordinate::new(lat2, lon2))
}

/// Free a string allocated by the SDK.
///
/// # Safety
/// `ptr` must be a valid CString pointer allocated by this SDK, or null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(unsafe { CString::from_raw(ptr) });
    }
}

/// Get SDK version string.
#[unsafe(no_mangle)]
pub extern "C" fn tv_version() -> *mut c_char {
    let version = CString::new(env!("CARGO_PKG_VERSION")).unwrap();
    version.into_raw()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A map centred on the start of [`ROUTE`], zoomed in enough to navigate.
    fn map() -> *mut TvMapState {
        let state = tv_map_create(1080, 2280, 3.0);
        unsafe {
            tv_map_set_center(state, ROUTE[0].0, ROUTE[0].1);
            tv_map_set_zoom(state, 16.0);
        }
        state
    }

    /// Four vertices heading north up a street, then east.
    const ROUTE: [(f64, f64); 4] = [
        (51.5000, -0.1000),
        (51.5010, -0.1000),
        (51.5020, -0.1000),
        (51.5020, -0.0980),
    ];

    fn points() -> Vec<TvRoutePoint> {
        ROUTE
            .iter()
            .map(|(lat, lon)| TvRoutePoint {
                latitude: *lat,
                longitude: *lon,
            })
            .collect()
    }

    /// Set a two-step route on `state`, keeping the instruction strings alive
    /// only for the call, which is all the ABI promises.
    fn set_route(state: *mut TvMapState) -> bool {
        let pts = points();
        let head = CString::new("Head north").unwrap();
        let turn = CString::new("Turn right").unwrap();
        let steps = [
            TvRouteStep {
                instruction: head.as_ptr(),
                start_index: 0,
                end_index: 2,
            },
            TvRouteStep {
                instruction: turn.as_ptr(),
                start_index: 2,
                end_index: 3,
            },
        ];
        unsafe { tv_nav_set_route(state, pts.as_ptr(), pts.len(), steps.as_ptr(), steps.len()) }
    }

    fn take_string(ptr: *mut c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        let s = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { tv_string_free(ptr) };
        Some(s)
    }

    fn progress(state: *mut TvMapState, lat: f64, lon: f64) -> TvNavProgress {
        let mut out = TvNavProgress {
            status: -1,
            step_index: 0,
            step_count: 0,
            distance_to_next_step_m: 0.0,
            distance_remaining_m: 0.0,
            off_route: false,
        };
        assert!(unsafe { tv_nav_update(state, lat, lon, &mut out) });
        out
    }

    #[test]
    fn test_project_centre_and_nulls() {
        let state = map();
        let mut p = TvScreenPoint { x: 0.0, y: 0.0 };
        assert!(unsafe { tv_map_project(state, ROUTE[0].0, ROUTE[0].1, &mut p) });
        assert!((p.x - 540.0).abs() < 0.01);
        assert!((p.y - 1140.0).abs() < 0.01);

        // north of the centre draws above it
        assert!(unsafe { tv_map_project(state, ROUTE[2].0, ROUTE[2].1, &mut p) });
        assert!(p.y < 1140.0);

        assert!(!unsafe { tv_map_project(std::ptr::null(), 0.0, 0.0, &mut p) });
        assert!(!unsafe { tv_map_project(state, 0.0, 0.0, std::ptr::null_mut()) });
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_metres_per_pixel_halves_per_zoom() {
        let state = map();
        let at16 = unsafe { tv_map_metres_per_pixel(state) };
        unsafe { tv_map_set_zoom(state, 17.0) };
        let at17 = unsafe { tv_map_metres_per_pixel(state) };
        assert!(at16 > 0.0);
        assert!((at17 * 2.0 - at16).abs() < 1e-9);
        assert_eq!(unsafe { tv_map_metres_per_pixel(std::ptr::null()) }, 0.0);
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_user_location_roundtrip() {
        let state = map();
        let mut out = TvUserLocation {
            latitude: 0.0,
            longitude: 0.0,
            accuracy_m: 0.0,
            bearing_deg: 0.0,
        };
        // nothing set yet
        assert!(!unsafe { tv_map_user_location(state, &mut out) });

        assert!(unsafe { tv_map_set_user_location(state, 51.5, -0.1, 12.5, 90.0) });
        assert!(unsafe { tv_map_user_location(state, &mut out) });
        assert_eq!(out.latitude, 51.5);
        assert_eq!(out.longitude, -0.1);
        assert_eq!(out.accuracy_m, 12.5);
        assert_eq!(out.bearing_deg, 90.0);

        // a fix with no accuracy or bearing is still a fix
        assert!(unsafe { tv_map_set_user_location(state, 51.6, -0.2, -1.0, f64::NAN) });
        assert!(unsafe { tv_map_user_location(state, &mut out) });
        assert!(out.accuracy_m < 0.0 && out.bearing_deg.is_nan());

        assert!(!unsafe { tv_map_set_user_location(state, f64::NAN, -0.1, -1.0, f64::NAN) });
        assert!(!unsafe { tv_map_set_user_location(std::ptr::null_mut(), 1.0, 1.0, -1.0, 0.0) });
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_tracking_moves_the_camera() {
        let state = map();
        assert_eq!(unsafe { tv_map_get_tracking_mode(state) }, TV_TRACKING_NONE);

        // untracked, a fix leaves the camera alone
        unsafe { tv_map_set_user_location(state, 51.6, -0.2, -1.0, 45.0) };
        assert_eq!(unsafe { tv_map_get_center_lat(state) }, ROUTE[0].0);

        // switching to follow snaps onto the fix already held
        assert!(unsafe { tv_map_set_tracking_mode(state, TV_TRACKING_FOLLOW) });
        assert_eq!(unsafe { tv_map_get_center_lat(state) }, 51.6);
        assert_eq!(unsafe { tv_map_get_bearing(state) }, 0.0);

        // and later fixes keep pulling it along
        unsafe { tv_map_set_user_location(state, 51.7, -0.3, -1.0, 45.0) };
        assert_eq!(unsafe { tv_map_get_center_lat(state) }, 51.7);

        // heading mode also rotates, course mode likewise
        for mode in [
            TV_TRACKING_FOLLOW_WITH_HEADING,
            TV_TRACKING_FOLLOW_WITH_COURSE,
        ] {
            unsafe { tv_map_set_bearing(state, 0.0) };
            assert!(unsafe { tv_map_set_tracking_mode(state, mode) });
            assert_eq!(unsafe { tv_map_get_tracking_mode(state) }, mode);
            assert_eq!(unsafe { tv_map_get_bearing(state) }, 45.0);
        }

        // an unknown bearing must not spin the map
        unsafe { tv_map_set_user_location(state, 51.8, -0.4, -1.0, f64::NAN) };
        assert_eq!(unsafe { tv_map_get_bearing(state) }, 45.0);

        assert!(!unsafe { tv_map_set_tracking_mode(state, 99) });
        assert_eq!(
            unsafe { tv_map_get_tracking_mode(state) },
            TV_TRACKING_FOLLOW_WITH_COURSE
        );
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_set_route_rejects_bad_input() {
        let state = map();
        let pts = points();
        let steps = [TvRouteStep {
            instruction: std::ptr::null(),
            start_index: 0,
            end_index: 3,
        }];

        // too few points, no steps, null arrays
        assert!(!unsafe { tv_nav_set_route(state, pts.as_ptr(), 1, steps.as_ptr(), 1) });
        assert!(!unsafe { tv_nav_set_route(state, pts.as_ptr(), pts.len(), steps.as_ptr(), 0) });
        assert!(!unsafe { tv_nav_set_route(state, std::ptr::null(), 2, steps.as_ptr(), 1) });
        assert!(!unsafe { tv_nav_set_route(state, pts.as_ptr(), pts.len(), std::ptr::null(), 1) });

        // a null instruction is allowed, and rejected input left no route behind
        let mut out = TvNavProgress {
            status: -1,
            step_index: 0,
            step_count: 0,
            distance_to_next_step_m: 0.0,
            distance_remaining_m: 0.0,
            off_route: false,
        };
        assert!(!unsafe { tv_nav_update(state, ROUTE[0].0, ROUTE[0].1, &mut out) });
        assert!(unsafe { tv_nav_set_route(state, pts.as_ptr(), pts.len(), steps.as_ptr(), 1) });
        assert!(unsafe { tv_nav_update(state, ROUTE[0].0, ROUTE[0].1, &mut out) });
        assert_eq!(
            take_string(unsafe { tv_nav_instruction(state) }).as_deref(),
            Some("")
        );
        unsafe { tv_map_destroy(state) };
    }

    /// Out-of-range step indices must clamp instead of panicking the update.
    #[test]
    fn test_set_route_clamps_step_indices() {
        let state = map();
        let pts = points();
        let steps = [TvRouteStep {
            instruction: std::ptr::null(),
            start_index: 900,
            end_index: 3,
        }];
        assert!(unsafe { tv_nav_set_route(state, pts.as_ptr(), pts.len(), steps.as_ptr(), 1) });
        let p = progress(state, ROUTE[0].0, ROUTE[0].1);
        assert_eq!(p.step_index, 0);
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_navigation_progresses_along_the_route() {
        let state = map();
        assert!(set_route(state));

        let start = progress(state, ROUTE[0].0, ROUTE[0].1);
        assert_eq!(start.status, TV_NAV_ON_ROUTE);
        assert_eq!(start.step_index, 0);
        assert_eq!(start.step_count, 2);
        assert!(!start.off_route);
        assert!(start.distance_remaining_m > 300.0);
        assert!(start.distance_to_next_step_m > 200.0);
        assert_eq!(
            take_string(unsafe { tv_nav_instruction(state) }).as_deref(),
            Some("Head north")
        );

        // halfway up the first step: closer to the turn, less left overall
        let mid = progress(state, ROUTE[1].0, ROUTE[1].1);
        assert_eq!(mid.step_index, 0);
        assert!(mid.distance_to_next_step_m < start.distance_to_next_step_m);
        assert!(mid.distance_remaining_m < start.distance_remaining_m);

        // past the turn the step advances and the instruction follows
        let turn = progress(state, ROUTE[2].0, ROUTE[2].1);
        assert_eq!(turn.step_index, 1);
        assert_eq!(
            take_string(unsafe { tv_nav_instruction(state) }).as_deref(),
            Some("Turn right")
        );

        let end = progress(state, ROUTE[3].0, ROUTE[3].1);
        assert_eq!(end.status, TV_NAV_ARRIVED);
        assert!(end.distance_remaining_m < 20.0);
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_navigation_reports_off_route() {
        let state = map();
        assert!(set_route(state));
        let p = progress(state, 51.5000, -0.2000);
        assert_eq!(p.status, TV_NAV_OFF_ROUTE);
        assert!(p.off_route);
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_progress_read_back_and_cleared() {
        let state = map();
        let mut out = TvNavProgress {
            status: -1,
            step_index: 0,
            step_count: 0,
            distance_to_next_step_m: 0.0,
            distance_remaining_m: 0.0,
            off_route: false,
        };
        assert!(set_route(state));

        // nothing to read before the first update
        assert!(!unsafe { tv_nav_progress(state, &mut out) });
        assert!(unsafe { tv_nav_instruction(state) }.is_null());

        let live = progress(state, ROUTE[1].0, ROUTE[1].1);
        assert!(unsafe { tv_nav_progress(state, &mut out) });
        assert_eq!(out.step_index, live.step_index);
        assert_eq!(out.distance_remaining_m, live.distance_remaining_m);

        unsafe { tv_nav_clear(state) };
        assert!(!unsafe { tv_nav_progress(state, &mut out) });
        assert!(!unsafe { tv_nav_update(state, ROUTE[1].0, ROUTE[1].1, &mut out) });
        assert!(unsafe { tv_nav_instruction(state) }.is_null());
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_distance_and_bearing() {
        // London to Paris, roughly 340 km on a south-east heading
        let d = tv_distance_between(51.5074, -0.1278, 48.8566, 2.3522);
        assert!(d > 330_000.0 && d < 350_000.0);
        let b = tv_bearing_between(51.5074, -0.1278, 48.8566, 2.3522);
        assert!(b > 90.0 && b < 180.0, "bearing {b}");

        assert_eq!(tv_distance_between(51.5, -0.1, 51.5, -0.1), 0.0);
        // due north
        assert!(tv_bearing_between(0.0, 0.0, 1.0, 0.0).abs() < 1e-9);
    }
}
