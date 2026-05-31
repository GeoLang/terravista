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

use terravista_core::camera::{Camera, Viewport};
use terravista_core::gesture::GestureRecognizer;
use terravista_core::location::Coordinate;
use terravista_core::tile_cache::{CacheConfig, TileCache};

/// Opaque map state handle.
#[allow(dead_code)]
pub struct TvMapState {
    camera: Camera,
    viewport: Viewport,
    gesture: GestureRecognizer,
    tile_cache: TileCache,
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
        let config = CacheConfig {
            url_template: url_str.to_string(),
            ..Default::default()
        };
        s.tile_cache = TileCache::new(config);
    }
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
