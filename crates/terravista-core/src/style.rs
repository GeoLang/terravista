//! Style engine — runtime style evaluation for vector tiles.
//!
//! Parses and evaluates Mapbox GL-compatible styles to determine how features render.

use serde::{Deserialize, Serialize};

/// A complete map style document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapStyle {
    pub name: String,
    pub sources: Vec<Source>,
    pub layers: Vec<StyleLayer>,
}

/// Tile data source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub id: String,
    pub source_type: SourceType,
    pub url: Option<String>,
    pub tile_url: Option<String>,
    pub min_zoom: Option<u8>,
    pub max_zoom: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SourceType {
    Vector,
    Raster,
    GeoJson,
}

/// A style layer — determines how a data layer renders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleLayer {
    pub id: String,
    pub source: String,
    pub source_layer: Option<String>,
    pub layer_type: LayerType,
    pub paint: Paint,
    pub min_zoom: Option<f64>,
    pub max_zoom: Option<f64>,
    pub filter: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayerType {
    Fill,
    Line,
    Circle,
    Symbol,
    Background,
}

/// Paint properties for a layer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Paint {
    pub fill_color: Option<ColorValue>,
    pub fill_opacity: Option<f32>,
    pub line_color: Option<ColorValue>,
    pub line_width: Option<f32>,
    pub line_opacity: Option<f32>,
    pub circle_radius: Option<f32>,
    pub circle_color: Option<ColorValue>,
    pub background_color: Option<ColorValue>,
}

/// A color value (constant or zoom-interpolated).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ColorValue {
    Constant([f32; 4]),
    Stops(Vec<(f64, [f32; 4])>),
}

impl ColorValue {
    /// Evaluate color at a given zoom level.
    pub fn evaluate(&self, zoom: f64) -> [f32; 4] {
        match self {
            ColorValue::Constant(c) => *c,
            ColorValue::Stops(stops) => {
                if stops.is_empty() {
                    return [0.0, 0.0, 0.0, 1.0];
                }
                if zoom <= stops[0].0 {
                    return stops[0].1;
                }
                if zoom >= stops[stops.len() - 1].0 {
                    return stops[stops.len() - 1].1;
                }
                // Linear interpolation between stops
                for i in 0..stops.len() - 1 {
                    if zoom >= stops[i].0 && zoom <= stops[i + 1].0 {
                        let t = (zoom - stops[i].0) / (stops[i + 1].0 - stops[i].0);
                        let t = t as f32;
                        let a = stops[i].1;
                        let b = stops[i + 1].1;
                        return [
                            a[0] + (b[0] - a[0]) * t,
                            a[1] + (b[1] - a[1]) * t,
                            a[2] + (b[2] - a[2]) * t,
                            a[3] + (b[3] - a[3]) * t,
                        ];
                    }
                }
                stops[0].1
            }
        }
    }
}

/// Parse a hex color string to RGBA floats.
pub fn parse_color(hex: &str) -> Option<[f32; 4]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() >= 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()? as f32 / 255.0;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()? as f32 / 255.0;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()? as f32 / 255.0;
        let a = if hex.len() == 8 {
            u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0
        } else {
            1.0
        };
        Some([r, g, b, a])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_constant() {
        let c = ColorValue::Constant([1.0, 0.0, 0.0, 1.0]);
        assert_eq!(c.evaluate(10.0), [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_color_interpolation() {
        let c = ColorValue::Stops(vec![
            (5.0, [0.0, 0.0, 0.0, 1.0]),
            (15.0, [1.0, 1.0, 1.0, 1.0]),
        ]);
        let result = c.evaluate(10.0);
        assert!((result[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_parse_color() {
        let c = parse_color("#ff0000").unwrap();
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!((c[1]).abs() < 0.01);
    }
}
