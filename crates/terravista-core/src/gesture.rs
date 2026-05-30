//! Touch gesture recognition for map interaction.
//!
//! Translates raw touch events into map operations (pan, pinch-zoom, rotate, tilt).

use crate::camera::{Camera, Viewport};

/// A touch point from the platform.
#[derive(Debug, Clone, Copy)]
pub struct TouchPoint {
    pub id: u64,
    pub x: f64,
    pub y: f64,
}

/// Raw touch events from the platform layer.
#[derive(Debug, Clone)]
pub enum TouchEvent {
    Begin(Vec<TouchPoint>),
    Move(Vec<TouchPoint>),
    End(Vec<TouchPoint>),
    Cancel,
}

/// Gesture detection state machine.
pub struct GestureRecognizer {
    state: GestureState,
    prev_touches: Vec<TouchPoint>,
    initial_distance: Option<f64>,
    initial_angle: Option<f64>,
    initial_zoom: f64,
    initial_bearing: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureState {
    Idle,
    Pan,
    PinchZoom,
    #[allow(dead_code)]
    Rotate,
}

/// Result of processing a gesture — tells the caller what camera update to apply.
#[derive(Debug, Clone)]
pub enum GestureResult {
    None,
    Pan {
        dx: f64,
        dy: f64,
    },
    Zoom {
        delta: f64,
        anchor_x: f64,
        anchor_y: f64,
    },
    Rotate {
        delta_degrees: f64,
    },
    Pitch {
        delta_degrees: f64,
    },
}

impl GestureRecognizer {
    pub fn new() -> Self {
        Self {
            state: GestureState::Idle,
            prev_touches: Vec::new(),
            initial_distance: None,
            initial_angle: None,
            initial_zoom: 0.0,
            initial_bearing: 0.0,
        }
    }

    /// Process a touch event and return the resulting gesture action.
    pub fn process(&mut self, event: &TouchEvent, camera: &Camera) -> GestureResult {
        match event {
            TouchEvent::Begin(touches) => {
                self.prev_touches = touches.clone();
                if touches.len() >= 2 {
                    self.state = GestureState::PinchZoom;
                    self.initial_distance = Some(touch_distance(&touches[0], &touches[1]));
                    self.initial_angle = Some(touch_angle(&touches[0], &touches[1]));
                    self.initial_zoom = camera.zoom;
                    self.initial_bearing = camera.bearing;
                } else {
                    self.state = GestureState::Pan;
                }
                GestureResult::None
            }
            TouchEvent::Move(touches) => {
                let result = match self.state {
                    GestureState::Pan if !touches.is_empty() && !self.prev_touches.is_empty() => {
                        let dx = touches[0].x - self.prev_touches[0].x;
                        let dy = touches[0].y - self.prev_touches[0].y;
                        GestureResult::Pan { dx, dy }
                    }
                    GestureState::PinchZoom if touches.len() >= 2 => {
                        let dist = touch_distance(&touches[0], &touches[1]);
                        let angle = touch_angle(&touches[0], &touches[1]);

                        let mut result = GestureResult::None;

                        if let Some(initial_dist) = self.initial_distance {
                            let scale = dist / initial_dist;
                            let zoom_delta = scale.log2();
                            let cx = (touches[0].x + touches[1].x) / 2.0;
                            let cy = (touches[0].y + touches[1].y) / 2.0;
                            result = GestureResult::Zoom {
                                delta: zoom_delta - (camera.zoom - self.initial_zoom),
                                anchor_x: cx,
                                anchor_y: cy,
                            };
                        }

                        if let Some(initial_ang) = self.initial_angle {
                            let rotation = angle - initial_ang;
                            if rotation.abs() > 5.0 {
                                result = GestureResult::Rotate {
                                    delta_degrees: rotation
                                        - (camera.bearing - self.initial_bearing),
                                };
                            }
                        }

                        result
                    }
                    _ => GestureResult::None,
                };
                self.prev_touches = touches.clone();
                result
            }
            TouchEvent::End(_) | TouchEvent::Cancel => {
                self.state = GestureState::Idle;
                self.prev_touches.clear();
                self.initial_distance = None;
                self.initial_angle = None;
                GestureResult::None
            }
        }
    }

    /// Apply a gesture result to a camera.
    pub fn apply(result: &GestureResult, camera: &mut Camera, viewport: &Viewport) {
        match result {
            GestureResult::Pan { dx, dy } => camera.pan(*dx, *dy, viewport),
            GestureResult::Zoom {
                delta,
                anchor_x,
                anchor_y,
            } => camera.zoom_to(camera.zoom + delta, *anchor_x, *anchor_y),
            GestureResult::Rotate { delta_degrees } => {
                camera.bearing = (camera.bearing + delta_degrees) % 360.0;
            }
            GestureResult::Pitch { delta_degrees } => {
                camera.pitch = (camera.pitch + delta_degrees).clamp(0.0, 60.0);
            }
            GestureResult::None => {}
        }
    }
}

impl Default for GestureRecognizer {
    fn default() -> Self {
        Self::new()
    }
}

fn touch_distance(a: &TouchPoint, b: &TouchPoint) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    (dx * dx + dy * dy).sqrt()
}

fn touch_angle(a: &TouchPoint, b: &TouchPoint) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    dy.atan2(dx).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Coordinate;

    #[test]
    fn test_pan_gesture() {
        let mut recognizer = GestureRecognizer::new();
        let camera = Camera::new(Coordinate::new(0.0, 0.0), 10.0);

        let begin = TouchEvent::Begin(vec![TouchPoint {
            id: 0,
            x: 100.0,
            y: 100.0,
        }]);
        recognizer.process(&begin, &camera);

        let moved = TouchEvent::Move(vec![TouchPoint {
            id: 0,
            x: 110.0,
            y: 105.0,
        }]);
        let result = recognizer.process(&moved, &camera);

        match result {
            GestureResult::Pan { dx, dy } => {
                assert!((dx - 10.0).abs() < 0.01);
                assert!((dy - 5.0).abs() < 0.01);
            }
            _ => panic!("expected Pan gesture"),
        }
    }

    #[test]
    fn test_pinch_zoom() {
        let mut recognizer = GestureRecognizer::new();
        let camera = Camera::new(Coordinate::new(0.0, 0.0), 10.0);

        let begin = TouchEvent::Begin(vec![
            TouchPoint {
                id: 0,
                x: 100.0,
                y: 200.0,
            },
            TouchPoint {
                id: 1,
                x: 200.0,
                y: 200.0,
            },
        ]);
        recognizer.process(&begin, &camera);

        // Spread fingers apart (zoom in)
        let moved = TouchEvent::Move(vec![
            TouchPoint {
                id: 0,
                x: 50.0,
                y: 200.0,
            },
            TouchPoint {
                id: 1,
                x: 250.0,
                y: 200.0,
            },
        ]);
        let result = recognizer.process(&moved, &camera);

        match result {
            GestureResult::Zoom { delta, .. } => {
                assert!(delta > 0.0); // zooming in
            }
            _ => panic!("expected Zoom gesture"),
        }
    }
}
