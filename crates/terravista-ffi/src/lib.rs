//! # terravista-ffi
//!
//! C-compatible FFI bindings for the TerraVista mobile map SDK.
//!
//! Provides a flat C API that Kotlin on Android calls through JNI, and that any
//! other language with a C FFI can call.
//!
//! ## Memory Management
//!
//! - All opaque pointers returned by `tv_*_create` must be freed with the
//!   corresponding `tv_*_destroy` function.
//! - String pointers returned by the SDK are owned by the caller and must
//!   be freed with `tv_string_free`.

use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::time::{SystemTime, UNIX_EPOCH};

use terravista_core::camera::{Camera, TileCoord, Viewport};
use terravista_core::gesture::{GestureRecognizer, GestureResult, TouchEvent, TouchPoint};
use terravista_core::location::{Coordinate, TrackingMode};
use terravista_core::mvt::{VectorTile, decode_tile};
use terravista_core::renderer::{
    LayerStyle, RenderCommand, RenderFeature, RenderGeometry, TilePlacement, VectorStyle,
    vector_tile_commands, visible_tiles,
};
use terravista_core::route::{Maneuver, NavStatus, NavigationUpdate, Navigator, Route, RouteStep};
use terravista_core::tile_cache::{CacheConfig, OfflineRegion, TileCache, TileData, TileMeta};

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
    vector: VectorState,
    /// Filled by `tv_region_plan`, read by `tv_region_tile_at`.
    region: Vec<TileCoord>,
}

/// The vector tile source: raw tiles as fetched, their decoded form, and the
/// flattened frame the host reads back.
struct VectorState {
    cache: TileCache,
    style: VectorStyle,
    decoded: HashMap<TileCoord, VectorTile>,
    features: Vec<TvVectorFeature>,
    /// x and y interleaved.
    coords: Vec<f32>,
    /// Point count per ring.
    rings: Vec<u32>,
    /// Every layer the frame drew, indexed by `TvVectorFeature::layer_index`.
    layer_names: Vec<String>,
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
        vector: VectorState {
            cache: TileCache::new(CacheConfig::default()),
            style: VectorStyle::default(),
            decoded: HashMap::new(),
            features: Vec::new(),
            coords: Vec::new(),
            rings: Vec::new(),
            layer_names: Vec::new(),
        },
        region: Vec::new(),
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

/// The geographic box the viewport covers.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvBounds {
    pub min_lat: f64,
    pub min_lon: f64,
    pub max_lat: f64,
    pub max_lon: f64,
}

/// Write the bounds the current camera and viewport cover into `out`.
///
/// This is what to hand `tv_region_plan` to download what the user is looking
/// at. Returns false if either pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_visible_bounds(
    state: *const TvMapState,
    out: *mut TvBounds,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let bounds = s.camera.visible_bounds(&s.viewport);
    *out = TvBounds {
        min_lat: bounds.min_lat,
        min_lon: bounds.min_lon,
        max_lat: bounds.max_lat,
        max_lon: bounds.max_lon,
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

// ─── Vector Tiles ────────────────────────────────────────────────────────────

/// Geometry kind in `TvVectorFeature`.
pub const TV_VECTOR_POINT: i32 = 0;
pub const TV_VECTOR_LINE: i32 = 1;
pub const TV_VECTOR_POLYGON: i32 = 2;

/// One feature of the frame built by `tv_map_vector_frame`.
///
/// `ring_offset` indexes the ring lengths from `tv_map_vector_rings` and
/// `coord_offset` indexes the floats from `tv_map_vector_coords`, where each
/// ring holds its point count and each point is an x and a y. A point feature
/// is one ring of one point, a line is one ring, and a polygon's first ring is
/// its exterior and the rest are holes. Colors are `0xAARRGGBB`, and an alpha
/// of zero means do not paint.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvVectorFeature {
    /// One of the `TV_VECTOR_*` values.
    pub kind: i32,
    /// The layer this feature came from, read back with
    /// `tv_map_vector_layer_name`.
    pub layer_index: u32,
    pub ring_offset: u32,
    pub ring_count: u32,
    pub coord_offset: u32,
    pub fill_argb: u32,
    pub stroke_argb: u32,
    pub stroke_width: f32,
    pub point_radius: f32,
}

fn argb(color: Option<[f32; 4]>) -> u32 {
    let Some(color) = color else {
        return 0;
    };
    let byte = |channel: f32| (channel.clamp(0.0, 1.0) * 255.0).round() as u32;
    (byte(color[3]) << 24) | (byte(color[0]) << 16) | (byte(color[1]) << 8) | byte(color[2])
}

fn color_from_argb(value: u32) -> Option<[f32; 4]> {
    let channel = |shift: u32| ((value >> shift) & 0xFF) as f32 / 255.0;
    let alpha = channel(24);
    (alpha > 0.0).then(|| [channel(16), channel(8), channel(0), alpha])
}

/// Point the vector source at a new URL template.
///
/// Vector tiles are cached and drawn separately from the raster tiles set with
/// `tv_map_set_tile_url`, so a map can carry both. Changing the template drops
/// the vector tiles held for the old one.
///
/// # Safety
/// `state` and `url` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_vector_tile_url(state: *mut TvMapState, url: *const c_char) {
    if state.is_null() || url.is_null() {
        return;
    }
    let s = unsafe { &mut *state };
    if let Ok(url) = unsafe { CStr::from_ptr(url) }.to_str() {
        s.vector.cache.set_url_template(url.to_string());
        s.vector.decoded.clear();
    }
}

/// Build the vector tile URL for a coordinate from the configured template.
///
/// Returns a string the caller must free with `tv_string_free`, or null.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_vector_tile_url(
    state: *const TvMapState,
    z: u8,
    x: u32,
    y: u32,
) -> *mut c_char {
    let Some(s) = (unsafe { state.as_ref() }) else {
        return std::ptr::null_mut();
    };
    match CString::new(s.vector.cache.tile_url(&TileCoord::new(z, x, y))) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Set how one vector layer draws, by name.
///
/// Colors are `0xAARRGGBB`, and an alpha of zero means do not paint, so a
/// polygon layer with a zero fill draws as an outline. `stroke_width` is in
/// device pixels. The layer keeps whatever point radius it already had. A name
/// no source serves is stored anyway, ready for a source that does. Takes
/// effect on the next `tv_map_vector_frame`.
///
/// Returns false on a null pointer or a name that is not UTF-8.
///
/// # Safety
/// `state` and `layer_name` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_set_layer_style(
    state: *mut TvMapState,
    layer_name: *const c_char,
    fill_argb: u32,
    stroke_argb: u32,
    stroke_width: f32,
) -> bool {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return false;
    };
    if layer_name.is_null() {
        return false;
    }
    let Ok(name) = unsafe { CStr::from_ptr(layer_name) }.to_str() else {
        return false;
    };
    let point_radius = s.vector.style.for_layer(name).point_radius;
    s.vector.style.layers.insert(
        name.to_string(),
        LayerStyle {
            fill_color: color_from_argb(fill_argb),
            stroke_color: color_from_argb(stroke_argb),
            stroke_width,
            point_radius,
        },
    );
    true
}

/// Store a fetched vector tile, decoding it on the way in.
///
/// Returns false on a null pointer or bytes that are not a vector tile, so the
/// host can tell a bad response from a slow one.
///
/// # Safety
/// `state` must be valid and `bytes` must point to at least `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_vector_cache_put(
    state: *mut TvMapState,
    z: u8,
    x: u32,
    y: u32,
    bytes: *const u8,
    len: usize,
) -> bool {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return false;
    };
    if bytes.is_null() || len == 0 {
        return false;
    }
    let data = unsafe { std::slice::from_raw_parts(bytes, len) };
    let Ok(tile) = decode_tile(data) else {
        return false;
    };

    let coord = TileCoord::new(z, x, y);
    let fetched_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    s.vector.cache.insert(TileData {
        meta: TileMeta {
            coord,
            size_bytes: len as u64,
            fetched_at,
            etag: None,
            content_type: "application/vnd.mapbox-vector-tile".to_string(),
        },
        bytes: data.to_vec(),
    });
    s.vector.decoded.insert(coord, tile);
    true
}

/// Whether a vector tile is held for this coordinate.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_vector_cache_has(
    state: *const TvMapState,
    z: u8,
    x: u32,
    y: u32,
) -> bool {
    unsafe { state.as_ref() }.is_some_and(|s| s.vector.cache.contains(&TileCoord::new(z, x, y)))
}

/// Drop every vector tile.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_vector_cache_clear(state: *mut TvMapState) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.vector.cache.clear();
        s.vector.decoded.clear();
    }
}

/// Recompute the frame's vector geometry and return how many features it holds.
///
/// Call this once per frame, then read the features back with
/// `tv_map_vector_feature_at` and their geometry with `tv_map_vector_coords`
/// and `tv_map_vector_rings`. Screen positions match the raster placements from
/// `tv_map_visible_tile_at`, so both draw in the same north-up frame.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_vector_frame(state: *mut TvMapState) -> u32 {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    s.placements = visible_tiles(&s.camera, &s.viewport);
    s.vector.features.clear();
    s.vector.coords.clear();
    s.vector.rings.clear();
    s.vector.layer_names.clear();

    let mut visible = HashMap::with_capacity(s.placements.len());
    for placement in &s.placements {
        let coord = placement.coord;
        let tile = match s.vector.decoded.remove(&coord) {
            Some(tile) => tile,
            None => {
                let Some(cached) = s.vector.cache.get(&coord) else {
                    continue;
                };
                let Ok(tile) = decode_tile(&cached.bytes) else {
                    continue;
                };
                tile
            }
        };

        let commands = vector_tile_commands(&tile, placement, &s.vector.style);
        for command in commands {
            let RenderCommand::DrawVectorLayer {
                layer_name,
                features,
            } = command
            else {
                continue;
            };
            let layer_index = layer_index(&mut s.vector, &layer_name);
            for feature in &features {
                push_feature(&mut s.vector, feature, layer_index);
            }
        }
        visible.insert(coord, tile);
    }

    // holding only what the frame drew keeps decoded tiles bounded by the screen
    s.vector.decoded = visible;
    s.vector.features.len() as u32
}

/// Where `name` sits in the frame's layer table, appending it if this is the
/// first tile to carry it.
fn layer_index(vector: &mut VectorState, name: &str) -> u32 {
    if let Some(index) = vector.layer_names.iter().position(|held| held == name) {
        return index as u32;
    }
    vector.layer_names.push(name.to_string());
    (vector.layer_names.len() - 1) as u32
}

fn push_feature(vector: &mut VectorState, feature: &RenderFeature, layer_index: u32) {
    let ring_offset = vector.rings.len() as u32;
    let coord_offset = vector.coords.len() as u32;
    let mut push_ring = |points: &[[f32; 2]]| {
        vector.rings.push(points.len() as u32);
        for point in points {
            vector.coords.push(point[0]);
            vector.coords.push(point[1]);
        }
    };

    let (kind, ring_count) = match &feature.geometry {
        RenderGeometry::Point { x, y, .. } => {
            push_ring(&[[*x, *y]]);
            (TV_VECTOR_POINT, 1)
        }
        RenderGeometry::Line { points } => {
            push_ring(points);
            (TV_VECTOR_LINE, 1)
        }
        RenderGeometry::Polygon { exterior, holes } => {
            push_ring(exterior);
            for hole in holes {
                push_ring(hole);
            }
            (TV_VECTOR_POLYGON, 1 + holes.len() as u32)
        }
    };

    let radius = match feature.geometry {
        RenderGeometry::Point { radius, .. } => radius,
        _ => 0.0,
    };

    vector.features.push(TvVectorFeature {
        kind,
        layer_index,
        ring_offset,
        ring_count,
        coord_offset,
        fill_argb: argb(feature.fill_color),
        stroke_argb: argb(feature.stroke_color),
        stroke_width: feature.stroke_width,
        point_radius: radius,
    });
}

/// Read one feature from the set built by `tv_map_vector_frame`.
///
/// Returns false if the index is out of range or a pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_vector_feature_at(
    state: *const TvMapState,
    index: u32,
    out: *mut TvVectorFeature,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let Some(feature) = s.vector.features.get(index as usize) else {
        return false;
    };
    *out = *feature;
    true
}

/// How many layers the frame built by `tv_map_vector_frame` drew.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_vector_layer_count(state: *const TvMapState) -> u32 {
    unsafe { state.as_ref() }.map_or(0, |s| s.vector.layer_names.len() as u32)
}

/// Name of one of the frame's layers, indexed by a feature's `layer_index`.
///
/// Returns a string the caller must free with `tv_string_free`, or null if the
/// index is out of range. The indices only hold until the next
/// `tv_map_vector_frame`.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_vector_layer_name(
    state: *const TvMapState,
    index: u32,
) -> *mut c_char {
    let Some(s) = (unsafe { state.as_ref() }) else {
        return std::ptr::null_mut();
    };
    let Some(name) = s.vector.layer_names.get(index as usize) else {
        return std::ptr::null_mut();
    };
    match CString::new(name.as_str()) {
        Ok(c) => c.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// Copy the frame's coordinates into `out`, writing at most `cap` floats.
///
/// Returns the full float count, which may exceed `cap`.
///
/// # Safety
/// `state` must be valid and `out` must point to at least `cap` floats, or be
/// null when `cap` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_vector_coords(
    state: *const TvMapState,
    out: *mut f32,
    cap: usize,
) -> usize {
    let Some(s) = (unsafe { state.as_ref() }) else {
        return 0;
    };
    unsafe { copy_out(&s.vector.coords, out, cap) }
}

/// Copy the frame's ring lengths, in points, into `out`.
///
/// Returns the full ring count, which may exceed `cap`.
///
/// # Safety
/// `state` must be valid and `out` must point to at least `cap` elements, or be
/// null when `cap` is 0.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_map_vector_rings(
    state: *const TvMapState,
    out: *mut u32,
    cap: usize,
) -> usize {
    let Some(s) = (unsafe { state.as_ref() }) else {
        return 0;
    };
    unsafe { copy_out(&s.vector.rings, out, cap) }
}

/// # Safety
/// `out` must point to at least `cap` elements, or be null when `cap` is 0.
unsafe fn copy_out<T: Copy>(values: &[T], out: *mut T, cap: usize) -> usize {
    let n = values.len().min(cap);
    if n > 0 && !out.is_null() {
        unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), out, n) };
    }
    values.len()
}

// ─── Offline Regions ─────────────────────────────────────────────────────────

/// Most tiles `tv_region_plan` will enumerate.
///
/// The host decides what a reasonable region is; this only stops a bad bounding
/// box from asking for a list too big to hold in memory.
pub const TV_REGION_MAX_TILES: u64 = 100_000;

/// One tile of a planned region.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TvTileCoordinate {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

fn region(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    min_zoom: u8,
    max_zoom: u8,
) -> OfflineRegion {
    OfflineRegion {
        name: String::new(),
        min_zoom,
        max_zoom,
        min_lat,
        max_lat,
        min_lon,
        max_lon,
    }
}

/// How many tiles a region covers, without enumerating them.
///
/// Latitudes past the Mercator limit clamp, and a region whose east edge is
/// west of its west edge crosses the antimeridian and covers the short way
/// round. An inverted zoom span covers nothing.
#[unsafe(no_mangle)]
pub extern "C" fn tv_region_tile_count(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    min_zoom: u8,
    max_zoom: u8,
) -> u64 {
    region(min_lat, min_lon, max_lat, max_lon, min_zoom, max_zoom).tile_count()
}

/// Rough bytes a region would take on disk, at an average tile weight.
#[unsafe(no_mangle)]
pub extern "C" fn tv_region_estimated_bytes(
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    min_zoom: u8,
    max_zoom: u8,
) -> u64 {
    region(min_lat, min_lon, max_lat, max_lon, min_zoom, max_zoom).estimated_size_bytes()
}

/// Enumerate a region's tiles and return how many it holds.
///
/// Read them back with `tv_region_tile_at`, lowest zoom first, and drop them
/// with `tv_region_clear` once the download is done. Returns 0 for a region
/// covering nothing and for one over `TV_REGION_MAX_TILES`, which is why a host
/// asks `tv_region_tile_count` first.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_region_plan(
    state: *mut TvMapState,
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
    min_zoom: u8,
    max_zoom: u8,
) -> u32 {
    let Some(s) = (unsafe { state.as_mut() }) else {
        return 0;
    };
    s.region.clear();

    let region = region(min_lat, min_lon, max_lat, max_lon, min_zoom, max_zoom);
    if region.tile_count() > TV_REGION_MAX_TILES {
        return 0;
    }
    s.region.extend(region.tiles());
    s.region.len() as u32
}

/// Read one tile from the plan built by `tv_region_plan`.
///
/// Returns false if the index is out of range or a pointer is null.
///
/// # Safety
/// `state` and `out` must be valid pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_region_tile_at(
    state: *const TvMapState,
    index: u32,
    out: *mut TvTileCoordinate,
) -> bool {
    let (Some(s), Some(out)) = (unsafe { state.as_ref() }, unsafe { out.as_mut() }) else {
        return false;
    };
    let Some(coord) = s.region.get(index as usize) else {
        return false;
    };
    *out = TvTileCoordinate {
        z: coord.z,
        x: coord.x,
        y: coord.y,
    };
    true
}

/// Drop the planned region, freeing the list.
///
/// # Safety
/// `state` must be a valid pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tv_region_clear(state: *mut TvMapState) {
    if let Some(s) = unsafe { state.as_mut() } {
        s.region = Vec::new();
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

    /// Encoded by the mapbox-vector-tile Python reference implementation.
    const SAMPLE_TILE: &[u8] = include_bytes!("../../terravista-core/tests/fixtures/sample.mvt");

    /// The first tile the current camera shows, and where it draws.
    fn first_visible_tile(state: *mut TvMapState) -> TvTilePlacement {
        assert!(unsafe { tv_map_visible_tile_count(state) } > 0);
        let mut placement = TvTilePlacement {
            z: 0,
            x: 0,
            y: 0,
            screen_x: 0.0,
            screen_y: 0.0,
            size: 0.0,
        };
        assert!(unsafe { tv_map_visible_tile_at(state, 0, &mut placement) });
        placement
    }

    fn feature_at(state: *mut TvMapState, index: u32) -> Option<TvVectorFeature> {
        let mut feature = TvVectorFeature {
            kind: -1,
            layer_index: 0,
            ring_offset: 0,
            ring_count: 0,
            coord_offset: 0,
            fill_argb: 0,
            stroke_argb: 0,
            stroke_width: 0.0,
            point_radius: 0.0,
        };
        unsafe { tv_map_vector_feature_at(state, index, &mut feature) }.then_some(feature)
    }

    /// Copy the frame's coordinates out, sized by the probe the ABI promises.
    fn frame_coords(state: *mut TvMapState) -> Vec<f32> {
        let len = unsafe { tv_map_vector_coords(state, std::ptr::null_mut(), 0) };
        let mut out = vec![0.0; len];
        assert_eq!(
            unsafe { tv_map_vector_coords(state, out.as_mut_ptr(), len) },
            len
        );
        out
    }

    fn frame_rings(state: *mut TvMapState) -> Vec<u32> {
        let len = unsafe { tv_map_vector_rings(state, std::ptr::null_mut(), 0) };
        let mut out = vec![0; len];
        assert_eq!(
            unsafe { tv_map_vector_rings(state, out.as_mut_ptr(), len) },
            len
        );
        out
    }

    fn put_sample_tile(state: *mut TvMapState, placement: &TvTilePlacement) -> bool {
        unsafe {
            tv_vector_cache_put(
                state,
                placement.z,
                placement.x,
                placement.y,
                SAMPLE_TILE.as_ptr(),
                SAMPLE_TILE.len(),
            )
        }
    }

    /// The vector source is a second source, not a replacement: both templates
    /// answer, and each keeps its own tiles.
    #[test]
    fn test_vector_url_template_is_separate_from_the_raster_one() {
        let state = map();
        let raster = CString::new("https://raster.example.com/{z}/{x}/{y}.png").unwrap();
        let vector = CString::new("https://vector.example.com/{z}/{x}/{y}.mvt").unwrap();
        unsafe {
            tv_map_set_tile_url(state, raster.as_ptr());
            tv_map_set_vector_tile_url(state, vector.as_ptr());
        }

        assert_eq!(
            take_string(unsafe { tv_map_tile_url(state, 14, 8192, 5450) }).as_deref(),
            Some("https://raster.example.com/14/8192/5450.png")
        );
        assert_eq!(
            take_string(unsafe { tv_map_vector_tile_url(state, 14, 8192, 5450) }).as_deref(),
            Some("https://vector.example.com/14/8192/5450.mvt")
        );
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_vector_put_rejects_bytes_that_are_not_a_tile() {
        let state = map();
        let placement = first_visible_tile(state);
        let junk = [0xFFu8, 0xFF, 0xFF, 0xFF];
        assert!(!unsafe {
            tv_vector_cache_put(
                state,
                placement.z,
                placement.x,
                placement.y,
                junk.as_ptr(),
                junk.len(),
            )
        });
        assert!(!unsafe { tv_vector_cache_put(state, 0, 0, 0, std::ptr::null(), 0) });
        assert!(!unsafe { tv_vector_cache_has(state, placement.z, placement.x, placement.y) });
        assert_eq!(unsafe { tv_map_vector_frame(state) }, 0);
        unsafe { tv_map_destroy(state) };
    }

    /// A tile put into the cache draws inside the quad the raster placement
    /// gives for the same coordinate.
    #[test]
    fn test_vector_frame_places_features_inside_the_tile() {
        let state = map();
        let placement = first_visible_tile(state);
        assert!(put_sample_tile(state, &placement));
        assert!(unsafe { tv_vector_cache_has(state, placement.z, placement.x, placement.y) });

        let count = unsafe { tv_map_vector_frame(state) };
        assert_eq!(count, 3, "one point, one line, one polygon");

        let coords = frame_coords(state);
        let rings = frame_rings(state);
        assert!(!coords.is_empty());
        assert_eq!(
            rings.iter().sum::<u32>() as usize * 2,
            coords.len(),
            "every ring's points must be in the coordinate pool"
        );

        for x in coords.iter().step_by(2) {
            assert!(*x >= placement.screen_x && *x <= placement.screen_x + placement.size);
        }
        for y in coords.iter().skip(1).step_by(2) {
            assert!(*y >= placement.screen_y && *y <= placement.screen_y + placement.size);
        }

        let kinds: Vec<i32> = (0..count)
            .map(|i| feature_at(state, i).unwrap().kind)
            .collect();
        assert_eq!(kinds, [TV_VECTOR_POINT, TV_VECTOR_LINE, TV_VECTOR_POLYGON]);
        assert!(feature_at(state, count).is_none());

        let polygon = feature_at(state, 2).unwrap();
        assert_eq!(polygon.ring_count, 2, "an exterior and one hole");
        assert_eq!(polygon.fill_argb >> 24, 255);
        unsafe { tv_map_destroy(state) };
    }

    /// Every feature says which layer it came from, so a host can tell a road
    /// from a river without going by colour.
    #[test]
    fn test_features_name_their_layer() {
        let state = map();
        let placement = first_visible_tile(state);
        assert!(put_sample_tile(state, &placement));
        let count = unsafe { tv_map_vector_frame(state) };

        assert_eq!(unsafe { tv_map_vector_layer_count(state) }, 3);
        let names: Vec<String> = (0..count)
            .map(|i| {
                let index = feature_at(state, i).unwrap().layer_index;
                take_string(unsafe { tv_map_vector_layer_name(state, index) }).unwrap()
            })
            .collect();
        assert_eq!(names, ["places", "roads", "water"]);

        let past_end = unsafe { tv_map_vector_layer_count(state) };
        assert!(unsafe { tv_map_vector_layer_name(state, past_end) }.is_null());
        assert!(unsafe { tv_map_vector_layer_name(std::ptr::null(), 0) }.is_null());
        assert_eq!(unsafe { tv_map_vector_layer_count(std::ptr::null()) }, 0);

        // the table belongs to the frame, so a frame with nothing to draw empties it
        unsafe { tv_map_set_center(state, -33.9, 151.2) };
        assert_eq!(unsafe { tv_map_vector_frame(state) }, 0);
        assert_eq!(unsafe { tv_map_vector_layer_count(state) }, 0);
        unsafe { tv_map_destroy(state) };
    }

    /// A style set by name reaches the features of that layer and no other.
    #[test]
    fn test_layer_style_repaints_one_layer() {
        let state = map();
        let placement = first_visible_tile(state);
        assert!(put_sample_tile(state, &placement));
        unsafe { tv_map_vector_frame(state) };
        let road_before = feature_at(state, 1).unwrap();
        let water_before = feature_at(state, 2).unwrap();

        let roads = CString::new("roads").unwrap();
        assert!(unsafe { tv_map_set_layer_style(state, roads.as_ptr(), 0, 0xFF00FF00, 7.0) });
        unsafe { tv_map_vector_frame(state) };

        let road = feature_at(state, 1).unwrap();
        assert_ne!(road.stroke_argb, road_before.stroke_argb);
        assert_eq!(road.stroke_argb, 0xFF00FF00);
        assert_eq!(road.stroke_width, 7.0);
        assert_eq!(
            feature_at(state, 2).unwrap().fill_argb,
            water_before.fill_argb
        );

        assert!(!unsafe { tv_map_set_layer_style(state, std::ptr::null(), 0, 0, 1.0) });
        assert!(!unsafe {
            tv_map_set_layer_style(std::ptr::null_mut(), roads.as_ptr(), 0, 0, 1.0)
        });
        unsafe { tv_map_destroy(state) };
    }

    /// A point layer keeps the radius it had, which the setter has no argument
    /// for, and still takes the new colours.
    #[test]
    fn test_layer_style_keeps_the_point_radius() {
        let state = map();
        let placement = first_visible_tile(state);
        assert!(put_sample_tile(state, &placement));
        unsafe { tv_map_vector_frame(state) };
        let radius = feature_at(state, 0).unwrap().point_radius;
        assert!(radius > 0.0);

        let places = CString::new("places").unwrap();
        assert!(unsafe { tv_map_set_layer_style(state, places.as_ptr(), 0xFF0000FF, 0, 1.0) });
        unsafe { tv_map_vector_frame(state) };

        let point = feature_at(state, 0).unwrap();
        assert_eq!(point.point_radius, radius);
        assert_eq!(point.fill_argb, 0xFF0000FF);
        assert_eq!(point.stroke_argb, 0);
        unsafe { tv_map_destroy(state) };
    }

    /// Panning away from a tile drops its geometry from the frame, and panning
    /// back brings it in again without a refetch.
    #[test]
    fn test_vector_frame_follows_the_camera() {
        let state = map();
        let placement = first_visible_tile(state);
        assert!(put_sample_tile(state, &placement));
        assert!(unsafe { tv_map_vector_frame(state) } > 0);

        unsafe { tv_map_set_center(state, -33.9, 151.2) };
        assert_eq!(unsafe { tv_map_vector_frame(state) }, 0);
        assert!(frame_coords(state).is_empty());

        unsafe { tv_map_set_center(state, ROUTE[0].0, ROUTE[0].1) };
        assert!(unsafe { tv_map_vector_frame(state) } > 0);
        unsafe { tv_map_destroy(state) };
    }

    /// A short buffer is filled as far as it goes and still reports the full
    /// length, so the host can size its own.
    #[test]
    fn test_vector_coords_report_the_full_length() {
        let state = map();
        let placement = first_visible_tile(state);
        assert!(put_sample_tile(state, &placement));
        unsafe { tv_map_vector_frame(state) };

        let len = unsafe { tv_map_vector_coords(state, std::ptr::null_mut(), 0) };
        assert!(len > 2);
        let mut two = [0.0f32; 2];
        assert_eq!(
            unsafe { tv_map_vector_coords(state, two.as_mut_ptr(), 2) },
            len
        );
        assert_eq!(two, frame_coords(state)[..2]);
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_vector_calls_reject_nulls() {
        let state = std::ptr::null_mut();
        assert_eq!(unsafe { tv_map_vector_frame(state) }, 0);
        assert!(!unsafe { tv_vector_cache_has(std::ptr::null(), 0, 0, 0) });
        assert!(unsafe { tv_map_vector_tile_url(std::ptr::null(), 0, 0, 0) }.is_null());
        assert!(feature_at(state, 0).is_none());
        assert_eq!(
            unsafe { tv_map_vector_coords(std::ptr::null(), std::ptr::null_mut(), 0) },
            0
        );
        assert_eq!(
            unsafe { tv_map_vector_rings(std::ptr::null(), std::ptr::null_mut(), 0) },
            0
        );
        unsafe { tv_map_set_vector_tile_url(state, std::ptr::null()) };
        unsafe { tv_vector_cache_clear(state) };
    }

    /// Changing the source drops what the old one served.
    #[test]
    fn test_vector_cache_clears_with_the_template() {
        let state = map();
        let placement = first_visible_tile(state);
        assert!(put_sample_tile(state, &placement));

        let url = CString::new("https://vector.example.com/{z}/{x}/{y}.mvt").unwrap();
        unsafe { tv_map_set_vector_tile_url(state, url.as_ptr()) };
        assert!(!unsafe { tv_vector_cache_has(state, placement.z, placement.x, placement.y) });
        assert_eq!(unsafe { tv_map_vector_frame(state) }, 0);

        assert!(put_sample_tile(state, &placement));
        unsafe { tv_vector_cache_clear(state) };
        assert_eq!(unsafe { tv_map_vector_frame(state) }, 0);
        unsafe { tv_map_destroy(state) };
    }

    /// London, small enough to download and deep enough to span zooms.
    const REGION: (f64, f64, f64, f64) = (51.50, -0.13, 51.52, -0.10);

    fn region_tile_at(state: *mut TvMapState, index: u32) -> Option<TvTileCoordinate> {
        let mut tile = TvTileCoordinate { z: 0, x: 0, y: 0 };
        unsafe { tv_region_tile_at(state, index, &mut tile) }.then_some(tile)
    }

    /// The plan hands back exactly what the estimate promised, and every tile
    /// of it is readable.
    #[test]
    fn test_region_plan_matches_the_estimate() {
        let state = map();
        let (min_lat, min_lon, max_lat, max_lon) = REGION;
        let count = tv_region_tile_count(min_lat, min_lon, max_lat, max_lon, 12, 14);
        assert!(count > 0);
        assert_eq!(
            tv_region_estimated_bytes(min_lat, min_lon, max_lat, max_lon, 12, 14),
            count * 20_000
        );

        let planned = unsafe { tv_region_plan(state, min_lat, min_lon, max_lat, max_lon, 12, 14) };
        assert_eq!(u64::from(planned), count);

        let zooms: Vec<u8> = (0..planned)
            .map(|i| region_tile_at(state, i).unwrap().z)
            .collect();
        assert_eq!(*zooms.first().unwrap(), 12);
        assert_eq!(*zooms.last().unwrap(), 14);
        assert!(region_tile_at(state, planned).is_none());

        unsafe { tv_region_clear(state) };
        assert!(region_tile_at(state, 0).is_none());
        unsafe { tv_map_destroy(state) };
    }

    /// A region too big to hold plans nothing, so the host has to ask for the
    /// count and offer a smaller one.
    #[test]
    fn test_region_plan_refuses_the_world() {
        let state = map();
        assert!(tv_region_tile_count(-85.0, -180.0, 85.0, 180.0, 0, 14) > TV_REGION_MAX_TILES);
        assert_eq!(
            unsafe { tv_region_plan(state, -85.0, -180.0, 85.0, 180.0, 0, 14) },
            0
        );
        assert!(region_tile_at(state, 0).is_none());

        // and a fresh plan replaces whatever the last one left
        let (min_lat, min_lon, max_lat, max_lon) = REGION;
        assert!(unsafe { tv_region_plan(state, min_lat, min_lon, max_lat, max_lon, 12, 12) } > 0);
        assert_eq!(
            unsafe { tv_region_plan(state, min_lat, min_lon, max_lat, max_lon, 14, 12) },
            0,
            "an inverted zoom span covers nothing"
        );
        assert!(region_tile_at(state, 0).is_none());
        unsafe { tv_map_destroy(state) };
    }

    /// The bounds a host would hand to `tv_region_plan` must cover the tiles
    /// the same camera draws, or a download would save the wrong area.
    #[test]
    fn test_visible_bounds_cover_the_visible_tiles() {
        let state = map();
        let mut bounds = TvBounds {
            min_lat: 0.0,
            min_lon: 0.0,
            max_lat: 0.0,
            max_lon: 0.0,
        };
        assert!(unsafe { tv_map_visible_bounds(state, &mut bounds) });
        assert!(bounds.min_lat < ROUTE[0].0 && bounds.max_lat > ROUTE[0].0);
        assert!(bounds.min_lon < ROUTE[0].1 && bounds.max_lon > ROUTE[0].1);

        let drawn = unsafe { tv_map_visible_tile_count(state) };
        let zoom = first_visible_tile(state).z;
        let planned = unsafe {
            tv_region_plan(
                state,
                bounds.min_lat,
                bounds.min_lon,
                bounds.max_lat,
                bounds.max_lon,
                zoom,
                zoom,
            )
        };
        assert_eq!(planned, drawn);

        assert!(!unsafe { tv_map_visible_bounds(std::ptr::null(), &mut bounds) });
        assert!(!unsafe { tv_map_visible_bounds(state, std::ptr::null_mut()) });
        unsafe { tv_map_destroy(state) };
    }

    #[test]
    fn test_region_calls_reject_nulls() {
        assert_eq!(
            unsafe { tv_region_plan(std::ptr::null_mut(), 51.5, -0.1, 51.6, 0.0, 12, 12) },
            0
        );
        assert!(region_tile_at(std::ptr::null_mut(), 0).is_none());
        let state = map();
        assert!(!unsafe { tv_region_tile_at(state, 0, std::ptr::null_mut()) });
        unsafe { tv_region_clear(std::ptr::null_mut()) };
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
