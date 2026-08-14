// Comprehensive integration tests for terravista-core.

use terravista_core::camera::*;
use terravista_core::location::*;
use terravista_core::mvt::*;
use terravista_core::renderer::*;
use terravista_core::route::*;
use terravista_core::style::*;
use terravista_core::tile_cache::*;

// ═══════════════════════════════════════════════════════════════════════════
// Coordinate tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_coordinate_distance_same_point() {
    let c = Coordinate::new(51.5074, -0.1278);
    assert_eq!(c.distance_to(&c), 0.0);
}

#[test]
fn test_coordinate_distance_london_paris() {
    let london = Coordinate::new(51.5074, -0.1278);
    let paris = Coordinate::new(48.8566, 2.3522);
    let dist = london.distance_to(&paris);
    // Approximately 340km
    assert!((dist - 340_000.0).abs() < 5000.0);
}

#[test]
fn test_coordinate_bearing_north() {
    let a = Coordinate::new(51.0, 0.0);
    let b = Coordinate::new(52.0, 0.0); // due north
    let bearing = a.bearing_to(&b);
    assert!((bearing - 0.0).abs() < 1.0);
}

#[test]
fn test_coordinate_bearing_east() {
    let a = Coordinate::new(0.0, 0.0);
    let b = Coordinate::new(0.0, 1.0); // due east
    let bearing = a.bearing_to(&b);
    assert!((bearing - 90.0).abs() < 1.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Camera tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_camera_creation() {
    let cam = Camera::new(Coordinate::new(51.5, -0.1), 14.0);
    assert_eq!(cam.zoom, 14.0);
    assert_eq!(cam.bearing, 0.0);
    assert_eq!(cam.pitch, 0.0);
}

#[test]
fn test_camera_with_bearing() {
    let cam = Camera::new(Coordinate::new(0.0, 0.0), 10.0).with_bearing(45.0);
    assert_eq!(cam.bearing, 45.0);
}

#[test]
fn test_camera_with_pitch_clamped() {
    let cam = Camera::new(Coordinate::new(0.0, 0.0), 10.0).with_pitch(80.0);
    assert_eq!(cam.pitch, 60.0); // clamped to max 60°
}

#[test]
fn test_camera_tile_zoom() {
    let cam = Camera::new(Coordinate::new(0.0, 0.0), 14.4);
    assert_eq!(cam.tile_zoom(), 14);
    let cam2 = Camera::new(Coordinate::new(0.0, 0.0), 14.6);
    assert_eq!(cam2.tile_zoom(), 15);
}

#[test]
fn test_camera_tile_zoom_clamping() {
    let cam = Camera::new(Coordinate::new(0.0, 0.0), 25.0);
    assert_eq!(cam.tile_zoom(), 22); // max zoom level
}

#[test]
fn test_camera_pan() {
    let viewport = Viewport::new(1024, 768, 2.0);
    let mut cam = Camera::new(Coordinate::new(0.0, 0.0), 10.0);
    let original_lon = cam.center.longitude;
    cam.pan(100.0, 0.0, &viewport);
    // Panning right should decrease longitude (move west in screen coords)
    assert!(cam.center.longitude < original_lon);
}

#[test]
fn test_camera_zoom_by() {
    let mut cam = Camera::new(Coordinate::new(0.0, 0.0), 10.0);
    cam.zoom_by(2.0);
    assert_eq!(cam.zoom, 12.0);
    cam.zoom_by(-15.0); // clamp to 0
    assert_eq!(cam.zoom, 0.0);
}

#[test]
fn test_camera_visible_bounds() {
    let viewport = Viewport::new(800, 600, 1.0);
    let cam = Camera::new(Coordinate::new(51.5, -0.1), 10.0);
    let bounds = cam.visible_bounds(&viewport);
    assert!(bounds.min_lon < -0.1);
    assert!(bounds.max_lon > -0.1);
    assert!(bounds.min_lat < 51.5);
    assert!(bounds.max_lat > 51.5);
}

// ═══════════════════════════════════════════════════════════════════════════
// Viewport tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_viewport_logical_dimensions() {
    let vp = Viewport::new(2048, 1536, 2.0);
    assert_eq!(vp.logical_width(), 1024.0);
    assert_eq!(vp.logical_height(), 768.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// TileCoord tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_tile_coord_path() {
    let tc = TileCoord::new(14, 8192, 5450);
    assert_eq!(tc.path(), "14/8192/5450");
}

#[test]
fn test_tile_range_count() {
    let viewport = Viewport::new(512, 512, 1.0);
    let cam = Camera::new(Coordinate::new(0.0, 0.0), 2.0);
    let bounds = cam.visible_bounds(&viewport);
    let range = bounds.tile_range(2);
    assert!(range.count() > 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Tile cache tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_tile_cache_miss() {
    let mut cache = TileCache::new(CacheConfig::default());
    let coord = TileCoord::new(14, 100, 200);
    assert!(cache.get(&coord).is_none());
}

#[test]
fn test_tile_cache_insert_and_get() {
    let mut cache = TileCache::new(CacheConfig::default());
    let coord = TileCoord::new(10, 512, 340);
    let tile = TileData {
        meta: TileMeta {
            coord,
            size_bytes: 1024,
            fetched_at: 1000,
            etag: None,
            content_type: "application/vnd.mapbox-vector-tile".into(),
        },
        bytes: vec![0u8; 1024],
    };
    cache.insert(tile);
    assert!(cache.get(&coord).is_some());
}

// ═══════════════════════════════════════════════════════════════════════════
// Style tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_color_value_constant() {
    let color = ColorValue::Constant([1.0, 0.0, 0.0, 1.0]);
    let result = color.evaluate(10.0);
    assert_eq!(result, [1.0, 0.0, 0.0, 1.0]);
}

#[test]
fn test_color_value_stops_interpolation() {
    let color = ColorValue::Stops(vec![
        (0.0, [0.0, 0.0, 0.0, 1.0]),
        (20.0, [1.0, 1.0, 1.0, 1.0]),
    ]);
    let result = color.evaluate(10.0);
    // Should be interpolated to ~0.5
    assert!((result[0] - 0.5).abs() < 0.1);
}

#[test]
fn test_map_style_serialization() {
    let style = MapStyle {
        name: "Test Style".into(),
        sources: vec![Source {
            id: "osm".into(),
            source_type: SourceType::Vector,
            url: None,
            tile_url: Some("https://tiles.example.com/{z}/{x}/{y}.mvt".into()),
            min_zoom: Some(0),
            max_zoom: Some(14),
        }],
        layers: vec![StyleLayer {
            id: "water".into(),
            source: "osm".into(),
            source_layer: Some("water".into()),
            layer_type: LayerType::Fill,
            paint: Paint {
                fill_color: Some(ColorValue::Constant([0.0, 0.0, 1.0, 1.0])),
                fill_opacity: Some(0.8),
                ..Default::default()
            },
            min_zoom: None,
            max_zoom: None,
            filter: None,
        }],
    };

    let json = serde_json::to_string(&style).unwrap();
    let back: MapStyle = serde_json::from_str(&json).unwrap();
    assert_eq!(back.name, "Test Style");
    assert_eq!(back.layers.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════════
// Vector tile tests
// ═══════════════════════════════════════════════════════════════════════════

/// Three layers over one tile, encoded by the mapbox-vector-tile Python
/// reference implementation, so the decoder is checked against bytes it did
/// not produce.
const SAMPLE_TILE: &[u8] = include_bytes!("fixtures/sample.mvt");

#[test]
fn test_decode_reference_encoder_tile() {
    let tile = decode_tile(SAMPLE_TILE).unwrap();
    let names: Vec<&str> = tile.layers.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["places", "roads", "water"]);

    let places = tile.layer("places").unwrap();
    assert_eq!(places.extent, 4096);
    assert_eq!(
        places.features[0].geometry,
        TileGeometry::Points(vec![[25.0, 17.0]])
    );
    assert_eq!(
        places.features[0].attributes.get("name"),
        Some(&AttributeValue::String("London".into()))
    );
    assert_eq!(
        places.features[0].attributes.get("population"),
        Some(&AttributeValue::Integer(8_982_000))
    );

    assert_eq!(
        tile.layer("roads").unwrap().features[0].geometry,
        TileGeometry::Lines(vec![vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0]]])
    );
    assert_eq!(
        tile.layer("water").unwrap().features[0]
            .attributes
            .get("depth"),
        Some(&AttributeValue::Number(3.5))
    );
}

/// The encoder winds the outer ring one way and the hole the other, which is
/// the only thing separating a hole from a second polygon.
#[test]
fn test_decode_reference_polygon_hole() {
    let tile = decode_tile(SAMPLE_TILE).unwrap();
    let TileGeometry::Polygons(polygons) = &tile.layer("water").unwrap().features[0].geometry
    else {
        panic!("expected polygons");
    };
    assert_eq!(polygons.len(), 1);
    assert_eq!(polygons[0].holes.len(), 1);
    assert_eq!(polygons[0].exterior.first(), polygons[0].exterior.last());
}

#[test]
fn test_reference_tile_places_onto_the_screen() {
    let tile = decode_tile(SAMPLE_TILE).unwrap();
    let placement = TilePlacement {
        coord: TileCoord::new(14, 8192, 5450),
        screen_x: 0.0,
        screen_y: 0.0,
        size: 4096.0,
    };
    let commands = vector_tile_commands(&tile, &placement, &VectorStyle::default());
    assert_eq!(commands.len(), 3);

    let RenderCommand::DrawVectorLayer {
        layer_name,
        features,
    } = &commands[0]
    else {
        panic!("expected a layer command");
    };
    // a placement the size of the extent draws tile units one for one
    assert_eq!(layer_name, "places");
    assert!(matches!(
        features[0].geometry,
        RenderGeometry::Point { x, y, .. } if x == 25.0 && y == 17.0
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// Route/Navigator tests
// ═══════════════════════════════════════════════════════════════════════════

fn sample_route() -> Route {
    Route {
        geometry: vec![
            Coordinate::new(51.500, -0.100),
            Coordinate::new(51.501, -0.099),
            Coordinate::new(51.502, -0.098),
            Coordinate::new(51.503, -0.097),
        ],
        distance_m: 400.0,
        duration_s: 300.0,
        steps: vec![
            RouteStep {
                instruction: "Head north".into(),
                maneuver: Maneuver::Depart,
                distance_m: 200.0,
                duration_s: 150.0,
                start_index: 0,
                end_index: 1,
            },
            RouteStep {
                instruction: "Arrive at destination".into(),
                maneuver: Maneuver::Arrive,
                distance_m: 200.0,
                duration_s: 150.0,
                start_index: 2,
                end_index: 3,
            },
        ],
    }
}

#[test]
fn test_navigator_creation() {
    let mut nav = Navigator::new(sample_route());
    let loc = Coordinate::new(51.500, -0.100);
    let update = nav.update(&loc);
    assert_eq!(update.status, NavStatus::OnRoute);
}

#[test]
fn test_navigator_update_on_route() {
    let mut nav = Navigator::new(sample_route());
    // Location on the route
    let loc = Coordinate::new(51.500, -0.100);
    let update = nav.update(&loc);
    assert_eq!(update.status, NavStatus::OnRoute);
}

#[test]
fn test_navigator_update_off_route() {
    let mut nav = Navigator::new(sample_route());
    // Location far from the route
    let loc = Coordinate::new(52.0, 1.0);
    let update = nav.update(&loc);
    assert_eq!(update.status, NavStatus::OffRoute);
}
