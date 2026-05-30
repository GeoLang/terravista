//! Renderer pipeline — abstraction over GPU rendering backend.
//!
//! The mobile SDK renders via the platform's GPU (Metal on iOS, Vulkan on Android).
//! This module defines the interface between the tile/vector data and the rendering output.

use crate::camera::{Camera, TileCoord, Viewport};

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
pub fn visible_tiles(camera: &Camera, viewport: &Viewport) -> Vec<TilePlacement> {
    let bounds = camera.visible_bounds(viewport);
    let zoom = camera.tile_zoom();
    let range = bounds.tile_range(zoom);
    let tile_size = 256.0 * viewport.device_pixel_ratio;

    let n = 2u32.pow(zoom as u32) as f64;
    let scale = 2.0_f64.powf(camera.zoom);

    range
        .iter()
        .map(|coord| {
            // Calculate screen position of tile
            let tile_lon = coord.x as f64 / n * 360.0 - 180.0;
            let tile_lat_rad = (std::f64::consts::PI * (1.0 - 2.0 * coord.y as f64 / n))
                .sinh()
                .atan();
            let tile_lat = tile_lat_rad.to_degrees();

            let dx = tile_lon - camera.center.longitude;
            let dy = camera.center.latitude - tile_lat;

            let pixels_per_degree = 256.0 * scale / 360.0;
            let screen_x = (viewport.logical_width() as f64 / 2.0 + dx * pixels_per_degree) as f32;
            let screen_y = (viewport.logical_height() as f64 / 2.0 + dy * pixels_per_degree) as f32;

            TilePlacement {
                coord,
                screen_x,
                screen_y,
                size: tile_size,
            }
        })
        .collect()
}

/// Where to draw a tile on screen.
#[derive(Debug, Clone)]
pub struct TilePlacement {
    pub coord: TileCoord,
    pub screen_x: f32,
    pub screen_y: f32,
    pub size: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Coordinate;

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
}
