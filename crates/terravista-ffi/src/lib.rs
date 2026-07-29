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
use terravista_core::location::Coordinate;
use terravista_core::renderer::{TilePlacement, visible_tiles};
use terravista_core::tile_cache::{CacheConfig, TileCache, TileData, TileMeta};

/// Opaque map state handle.
pub struct TvMapState {
    camera: Camera,
    viewport: Viewport,
    gesture: GestureRecognizer,
    tile_cache: TileCache,
    /// Filled by `tv_map_visible_tile_count`, read by `tv_map_visible_tile_at`.
    placements: Vec<TilePlacement>,
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

// ─── Utility ─────────────────────────────────────────────────────────────────

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
