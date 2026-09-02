//! Renderer pipeline — abstraction over GPU rendering backend.
//!
//! The platform layer executes the draw commands, on Android through Canvas or Vulkan.
//! This module defines the interface between the tile/vector data and the rendering output.

use std::collections::HashMap;

use crate::camera::{Camera, TileCoord, Viewport, lat_to_world_y, lon_to_world_x};
use crate::mvt::{TileGeometry, TilePolygon, VectorTile};

/// Render command that the platform layer executes.
#[derive(Debug, Clone)]
pub enum RenderCommand {
    /// Clear the framebuffer to a color.
    Clear { r: f32, g: f32, b: f32, a: f32 },
    /// Draw a raster tile at a screen-space quad.
    DrawRasterTile {
        coord: TileCoord,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    /// Draw a vector layer from decoded MVT data.
    DrawVectorLayer {
        layer_name: String,
        features: Vec<RenderFeature>,
    },
    /// Draw a user-location indicator.
    DrawLocationMarker {
        x: f32,
        y: f32,
        accuracy_radius: f32,
    },
    /// Draw a route polyline.
    DrawRoute {
        points: Vec<[f32; 2]>,
        color: [f32; 4],
        width: f32,
    },
}

/// A decoded vector feature ready to render.
#[derive(Debug, Clone)]
pub struct RenderFeature {
    pub geometry: RenderGeometry,
    pub fill_color: Option<[f32; 4]>,
    pub stroke_color: Option<[f32; 4]>,
    pub stroke_width: f32,
}

/// Geometry in screen coordinates.
#[derive(Debug, Clone)]
pub enum RenderGeometry {
    Point {
        x: f32,
        y: f32,
        radius: f32,
    },
    Line {
        points: Vec<[f32; 2]>,
    },
    Polygon {
        exterior: Vec<[f32; 2]>,
        holes: Vec<Vec<[f32; 2]>>,
    },
}

/// Frame builder — produces render commands for one frame.
pub struct FrameBuilder {
    commands: Vec<RenderCommand>,
}

impl FrameBuilder {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Start a new frame with a background clear.
    pub fn begin(&mut self, bg_color: [f32; 4]) {
        self.commands.clear();
        self.commands.push(RenderCommand::Clear {
            r: bg_color[0],
            g: bg_color[1],
            b: bg_color[2],
            a: bg_color[3],
        });
    }

    /// Add a raster tile draw command.
    pub fn draw_raster_tile(&mut self, coord: TileCoord, x: f32, y: f32, width: f32, height: f32) {
        self.commands.push(RenderCommand::DrawRasterTile {
            coord,
            x,
            y,
            width,
            height,
        });
    }

    /// Add a vector layer draw command.
    pub fn draw_vector_layer(&mut self, layer_name: String, features: Vec<RenderFeature>) {
        self.commands.push(RenderCommand::DrawVectorLayer {
            layer_name,
            features,
        });
    }

    /// Draw user location indicator.
    pub fn draw_location(&mut self, x: f32, y: f32, accuracy_radius: f32) {
        self.commands.push(RenderCommand::DrawLocationMarker {
            x,
            y,
            accuracy_radius,
        });
    }

    /// Draw a route overlay.
    pub fn draw_route(&mut self, points: Vec<[f32; 2]>, color: [f32; 4], width: f32) {
        self.commands.push(RenderCommand::DrawRoute {
            points,
            color,
            width,
        });
    }

    /// Finish the frame and return render commands.
    pub fn finish(self) -> Vec<RenderCommand> {
        self.commands
    }
}

impl Default for FrameBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate which tiles are visible and their screen positions.
///
/// Positions and sizes are device pixels, matching `Viewport::width`/`height`.
pub fn visible_tiles(camera: &Camera, viewport: &Viewport) -> Vec<TilePlacement> {
    let bounds = camera.visible_bounds(viewport);
    let zoom = camera.tile_zoom_for(viewport);
    let range = bounds.tile_range(zoom);

    let world = camera.world_size(viewport);
    // one tile's footprint on screen, so fractional zoom scales the tiles
    let tile_px = world / 2.0_f64.powi(i32::from(zoom));
    let origin_x = viewport.width as f64 / 2.0 - lon_to_world_x(camera.center.longitude, world);
    let origin_y = viewport.height as f64 / 2.0 - lat_to_world_y(camera.center.latitude, world);

    range
        .iter()
        .map(|coord| TilePlacement {
            coord,
            screen_x: (origin_x + coord.x as f64 * tile_px) as f32,
            screen_y: (origin_y + coord.y as f64 * tile_px) as f32,
            size: tile_px as f32,
        })
        .collect()
}

/// How one vector layer draws. There is no style spec behind this: a layer gets
/// one look, picked by name.
#[derive(Debug, Clone, Copy)]
pub struct LayerStyle {
    pub fill_color: Option<[f32; 4]>,
    pub stroke_color: Option<[f32; 4]>,
    /// Device pixels.
    pub stroke_width: f32,
    /// Device pixels.
    pub point_radius: f32,
}

/// Colors for the layer names common to OpenMapTiles-style sources, and a look
/// for everything else.
#[derive(Debug, Clone)]
pub struct VectorStyle {
    pub default: LayerStyle,
    pub layers: HashMap<String, LayerStyle>,
}

impl VectorStyle {
    pub fn for_layer(&self, name: &str) -> &LayerStyle {
        self.layers.get(name).unwrap_or(&self.default)
    }
}

const INK: [f32; 4] = [0.29, 0.29, 0.33, 1.0];
const WATER: [f32; 4] = [0.627, 0.784, 0.941, 1.0];
const GREEN: [f32; 4] = [0.784, 0.902, 0.627, 1.0];
const BUILDING: [f32; 4] = [0.851, 0.816, 0.788, 1.0];
const BUILDING_EDGE: [f32; 4] = [0.753, 0.722, 0.690, 1.0];
const ROAD: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BOUNDARY: [f32; 4] = [0.62, 0.61, 0.67, 1.0];

impl Default for VectorStyle {
    fn default() -> Self {
        let default = LayerStyle {
            fill_color: Some(INK),
            stroke_color: Some(INK),
            stroke_width: 1.5,
            point_radius: 3.0,
        };
        let fill = |color| LayerStyle {
            fill_color: Some(color),
            stroke_color: None,
            ..default
        };
        let line = |color, stroke_width| LayerStyle {
            fill_color: None,
            stroke_color: Some(color),
            stroke_width,
            ..default
        };

        let layers = [
            ("water", fill(WATER)),
            ("waterway", line(WATER, 2.0)),
            ("landcover", fill(GREEN)),
            ("landuse", fill(GREEN)),
            ("park", fill(GREEN)),
            (
                "building",
                LayerStyle {
                    fill_color: Some(BUILDING),
                    stroke_color: Some(BUILDING_EDGE),
                    stroke_width: 1.0,
                    ..default
                },
            ),
            ("transportation", line(ROAD, 3.0)),
            ("boundary", line(BOUNDARY, 1.0)),
        ];

        Self {
            default,
            layers: layers
                .into_iter()
                .map(|(name, style)| (name.to_string(), style))
                .collect(),
        }
    }
}

/// Turn a decoded tile into one draw command per layer, in screen coordinates.
///
/// Layers with nothing to draw are left out, so a frame holds no empty commands.
pub fn vector_tile_commands(
    tile: &VectorTile,
    placement: &TilePlacement,
    style: &VectorStyle,
) -> Vec<RenderCommand> {
    tile.layers
        .iter()
        .filter_map(|layer| {
            if layer.extent == 0 {
                return None;
            }
            let look = style.for_layer(&layer.name);
            let scale = placement.size / layer.extent as f32;
            let to_screen = |point: [f32; 2]| {
                [
                    placement.screen_x + point[0] * scale,
                    placement.screen_y + point[1] * scale,
                ]
            };

            let features: Vec<RenderFeature> = layer
                .features
                .iter()
                .flat_map(|feature| render_features(&feature.geometry, look, &to_screen))
                .collect();

            (!features.is_empty()).then(|| RenderCommand::DrawVectorLayer {
                layer_name: layer.name.clone(),
                features,
            })
        })
        .collect()
}

/// One `RenderFeature` per part, because a render feature holds a single
/// geometry.
fn render_features(
    geometry: &TileGeometry,
    look: &LayerStyle,
    to_screen: &impl Fn([f32; 2]) -> [f32; 2],
) -> Vec<RenderFeature> {
    let ring = |points: &[[f32; 2]]| -> Vec<[f32; 2]> {
        points.iter().map(|point| to_screen(*point)).collect()
    };

    match geometry {
        TileGeometry::Points(points) => points
            .iter()
            .map(|point| {
                let [x, y] = to_screen(*point);
                RenderFeature {
                    geometry: RenderGeometry::Point {
                        x,
                        y,
                        radius: look.point_radius,
                    },
                    fill_color: look.fill_color,
                    stroke_color: look.stroke_color,
                    stroke_width: look.stroke_width,
                }
            })
            .collect(),
        TileGeometry::Lines(lines) => lines
            .iter()
            .map(|line| RenderFeature {
                geometry: RenderGeometry::Line { points: ring(line) },
                fill_color: None,
                stroke_color: look.stroke_color.or(look.fill_color),
                stroke_width: look.stroke_width,
            })
            .collect(),
        TileGeometry::Polygons(polygons) => polygons
            .iter()
            .map(|TilePolygon { exterior, holes }| RenderFeature {
                geometry: RenderGeometry::Polygon {
                    exterior: ring(exterior),
                    holes: holes.iter().map(|hole| ring(hole)).collect(),
                },
                fill_color: look.fill_color,
                stroke_color: look.stroke_color,
                stroke_width: look.stroke_width,
            })
            .collect(),
    }
}

/// Where to draw a tile on screen, in device pixels.
#[derive(Debug, Clone)]
pub struct TilePlacement {
    pub coord: TileCoord,
    /// Left edge, device pixels.
    pub screen_x: f32,
    /// Top edge, device pixels.
    pub screen_y: f32,
    /// Edge length, device pixels. Tiles are square.
    pub size: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Coordinate;
    use crate::mvt::{VectorFeature, VectorLayer};

    #[test]
    fn test_frame_builder() {
        let mut fb = FrameBuilder::new();
        fb.begin([1.0, 1.0, 1.0, 1.0]);
        fb.draw_raster_tile(TileCoord::new(10, 512, 340), 0.0, 0.0, 256.0, 256.0);
        let commands = fb.finish();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_visible_tiles() {
        let camera = Camera::new(Coordinate::new(51.5, -0.1), 10.0);
        let viewport = Viewport::new(800, 600, 2.0);
        let tiles = visible_tiles(&camera, &viewport);
        assert!(!tiles.is_empty());
    }

    /// Neighbouring tiles must sit exactly one tile apart on both axes, in the
    /// same units as `size`. Latitude 0 hides unit and projection errors, so
    /// check well off the equator too.
    #[test]
    fn test_tile_spacing_is_square_at_every_latitude() {
        for lat in [0.0, 51.5, -33.9, 71.0] {
            let camera = Camera::new(Coordinate::new(lat, -0.1), 10.0);
            let viewport = Viewport::new(1080, 2280, 3.0);
            let tiles = visible_tiles(&camera, &viewport);
            assert!(tiles.len() > 1, "lat {lat}");
            let size = tiles[0].size;

            for t in &tiles {
                if let Some(right) = tiles
                    .iter()
                    .find(|o| o.coord.x == t.coord.x + 1 && o.coord.y == t.coord.y)
                {
                    let dx = right.screen_x - t.screen_x;
                    assert!(
                        (dx - size).abs() < 0.01,
                        "lat {lat}: dx {dx} vs size {size}"
                    );
                }
                if let Some(below) = tiles
                    .iter()
                    .find(|o| o.coord.y == t.coord.y + 1 && o.coord.x == t.coord.x)
                {
                    let dy = below.screen_y - t.screen_y;
                    assert!(
                        (dy - size).abs() < 0.01,
                        "lat {lat}: dy {dy} vs size {size}"
                    );
                }
            }
        }
    }

    /// The tile under the viewport centre must be the tile containing the camera centre.
    #[test]
    fn test_placement_agrees_with_camera_centre() {
        let camera = Camera::new(Coordinate::new(51.5, -0.1), 10.0);
        let viewport = Viewport::new(1080, 2280, 3.0);
        let tiles = visible_tiles(&camera, &viewport);

        let cx = viewport.width as f32 / 2.0;
        let cy = viewport.height as f32 / 2.0;
        let hit = tiles
            .iter()
            .find(|t| {
                cx >= t.screen_x
                    && cx < t.screen_x + t.size
                    && cy >= t.screen_y
                    && cy < t.screen_y + t.size
            })
            .expect("a tile must cover the viewport centre");

        let expected = camera
            .visible_bounds(&viewport)
            .tile_range(camera.tile_zoom_for(&viewport));
        assert!(hit.coord.x >= expected.x_min && hit.coord.x <= expected.x_max);
        assert!(hit.coord.y >= expected.y_min && hit.coord.y <= expected.y_max);
    }

    fn placement(size: f32) -> TilePlacement {
        TilePlacement {
            coord: TileCoord::new(14, 8192, 5450),
            screen_x: 100.0,
            screen_y: 200.0,
            size,
        }
    }

    fn tile(name: &str, extent: u32, geometry: TileGeometry) -> VectorTile {
        VectorTile {
            layers: vec![VectorLayer {
                name: name.to_string(),
                extent,
                features: vec![VectorFeature {
                    geometry,
                    attributes: HashMap::new(),
                }],
            }],
        }
    }

    /// Tile units scale by the placement, so a feature at the tile's centre
    /// lands at the centre of where the tile draws.
    #[test]
    fn test_vector_features_land_inside_the_tile() {
        let tile = tile(
            "places",
            4096,
            TileGeometry::Points(vec![[0.0, 0.0], [2048.0, 2048.0], [4096.0, 4096.0]]),
        );
        let commands = vector_tile_commands(&tile, &placement(256.0), &VectorStyle::default());

        let [
            RenderCommand::DrawVectorLayer {
                layer_name,
                features,
            },
        ] = commands.as_slice()
        else {
            panic!("expected one layer command");
        };
        assert_eq!(layer_name, "places");
        assert_eq!(features.len(), 3);

        let positions: Vec<[f32; 2]> = features
            .iter()
            .map(|f| match f.geometry {
                RenderGeometry::Point { x, y, .. } => [x, y],
                _ => panic!("expected points"),
            })
            .collect();
        assert_eq!(positions[0], [100.0, 200.0]);
        assert_eq!(positions[1], [228.0, 328.0]);
        assert_eq!(positions[2], [356.0, 456.0]);
    }

    /// The extent is the tile's own unit count, so a 512-extent tile has to
    /// place the same feature at the same pixel as a 4096-extent one.
    #[test]
    fn test_extent_sets_the_scale() {
        let coarse = tile("places", 512, TileGeometry::Points(vec![[256.0, 0.0]]));
        let fine = tile("places", 4096, TileGeometry::Points(vec![[2048.0, 0.0]]));
        let style = VectorStyle::default();

        for tile in [coarse, fine] {
            let commands = vector_tile_commands(&tile, &placement(256.0), &style);
            let RenderCommand::DrawVectorLayer { features, .. } = &commands[0] else {
                panic!("expected a layer command");
            };
            let RenderGeometry::Point { x, .. } = features[0].geometry else {
                panic!("expected a point");
            };
            assert_eq!(x, 228.0);
        }
    }

    #[test]
    fn test_polygon_holes_are_placed_with_the_exterior() {
        let tile = tile(
            "water",
            4096,
            TileGeometry::Polygons(vec![TilePolygon {
                exterior: vec![[0.0, 0.0], [4096.0, 0.0], [4096.0, 4096.0], [0.0, 0.0]],
                holes: vec![vec![[1024.0, 1024.0], [2048.0, 1024.0], [1024.0, 1024.0]]],
            }]),
        );
        let commands = vector_tile_commands(&tile, &placement(256.0), &VectorStyle::default());

        let RenderCommand::DrawVectorLayer { features, .. } = &commands[0] else {
            panic!("expected a layer command");
        };
        let RenderGeometry::Polygon { exterior, holes } = &features[0].geometry else {
            panic!("expected a polygon");
        };
        assert_eq!(exterior[1], [356.0, 200.0]);
        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0][0], [164.0, 264.0]);
        assert_eq!(features[0].fill_color, Some(WATER));
    }

    /// A line is stroked, never filled, whatever the layer's look says.
    #[test]
    fn test_lines_are_stroked_only() {
        let tile = tile(
            "water",
            4096,
            TileGeometry::Lines(vec![vec![[0.0, 0.0], [4096.0, 4096.0]]]),
        );
        let commands = vector_tile_commands(&tile, &placement(256.0), &VectorStyle::default());

        let RenderCommand::DrawVectorLayer { features, .. } = &commands[0] else {
            panic!("expected a layer command");
        };
        assert!(features[0].fill_color.is_none());
        assert_eq!(features[0].stroke_color, Some(WATER));
    }

    /// An unnamed layer still draws, and a layer with no features draws nothing.
    #[test]
    fn test_unknown_layer_falls_back_and_empty_layers_are_dropped() {
        let style = VectorStyle::default();
        let unknown = tile("mystery", 4096, TileGeometry::Points(vec![[0.0, 0.0]]));
        let commands = vector_tile_commands(&unknown, &placement(256.0), &style);
        let RenderCommand::DrawVectorLayer { features, .. } = &commands[0] else {
            panic!("expected a layer command");
        };
        assert_eq!(features[0].fill_color, style.default.fill_color);

        let empty = VectorTile {
            layers: vec![VectorLayer {
                name: "water".into(),
                extent: 4096,
                features: Vec::new(),
            }],
        };
        assert!(vector_tile_commands(&empty, &placement(256.0), &style).is_empty());
    }

    /// A zero extent would divide by zero, and no source sends one.
    #[test]
    fn test_zero_extent_layer_is_dropped() {
        let tile = tile("water", 0, TileGeometry::Points(vec![[0.0, 0.0]]));
        assert!(vector_tile_commands(&tile, &placement(256.0), &VectorStyle::default()).is_empty());
    }

    /// Tiles at a fractional zoom scale away from 256, so the map zooms smoothly.
    #[test]
    fn test_fractional_zoom_scales_tiles() {
        let viewport = Viewport::new(1080, 2280, 1.0);
        let whole = visible_tiles(&Camera::new(Coordinate::new(51.5, -0.1), 10.0), &viewport);
        let frac = visible_tiles(&Camera::new(Coordinate::new(51.5, -0.1), 10.3), &viewport);
        assert!((whole[0].size - 256.0).abs() < 0.01);
        assert!(frac[0].size > whole[0].size);
    }
}
