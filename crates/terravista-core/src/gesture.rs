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

/// Rotation dead zone, so a straight pinch does not also spin the map.
const ROTATE_THRESHOLD_DEG: f64 = 5.0;

/// Gesture detection state machine.
pub struct GestureRecognizer {
    state: GestureState,
    prev_touches: Vec<TouchPoint>,
    initial_distance: Option<f64>,
    initial_angle: Option<f64>,
    initial_zoom: f64,
    initial_bearing: f64,
    /// Latched once the rotation dead zone is cleared, so rotation does not
    /// snap on and off as the angle wobbles around the threshold.
    rotating: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureState {
    Idle,
    Pan,
    PinchZoom,
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
    /// A two-finger gesture, which zooms and rotates at the same time.
    Pinch {
        zoom_delta: f64,
        rotate_degrees: f64,
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
            rotating: false,
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
                    self.rotating = false;
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

                        // both deltas are measured from the start of the gesture,
                        // minus whatever the camera already absorbed
                        let zoom_delta = match self.initial_distance {
                            Some(d0) if d0 > 0.0 => {
                                (dist / d0).log2() - (camera.zoom - self.initial_zoom)
                            }
                            _ => 0.0,
                        };

                        let rotate_degrees = match self.initial_angle {
                            Some(a0) => {
                                let turned = shortest_angle(angle - a0);
                                if turned.abs() > ROTATE_THRESHOLD_DEG {
                                    self.rotating = true;
                                }
                                if self.rotating {
                                    shortest_angle(turned - (camera.bearing - self.initial_bearing))
                                } else {
                                    0.0
                                }
                            }
                            None => 0.0,
                        };

                        GestureResult::Pinch {
                            zoom_delta,
                            rotate_degrees,
                            anchor_x: (touches[0].x + touches[1].x) / 2.0,
                            anchor_y: (touches[0].y + touches[1].y) / 2.0,
                        }
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
                self.rotating = false;
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
            } => camera.zoom_to(camera.zoom + delta, *anchor_x, *anchor_y, viewport),
            GestureResult::Pinch {
                zoom_delta,
                rotate_degrees,
                anchor_x,
                anchor_y,
            } => {
                // rotate first, the anchor is a screen point under the new bearing
                if *rotate_degrees != 0.0 {
                    camera.bearing = (camera.bearing + rotate_degrees).rem_euclid(360.0);
                }
                if *zoom_delta != 0.0 {
                    camera.zoom_to(camera.zoom + zoom_delta, *anchor_x, *anchor_y, viewport);
                }
            }
            GestureResult::Rotate { delta_degrees } => {
                camera.bearing = (camera.bearing + delta_degrees).rem_euclid(360.0);
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

/// Normalise a degree difference to -180..180, so crossing the atan2 seam
/// reads as a small turn rather than a full circle.
fn shortest_angle(degrees: f64) -> f64 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
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
            GestureResult::Pinch { zoom_delta, .. } => {
                assert!(zoom_delta > 0.0); // zooming in
            }
            _ => panic!("expected Pinch gesture"),
        }
    }

    fn two_fingers(a: (f64, f64), b: (f64, f64)) -> TouchEvent {
        TouchEvent::Move(vec![
            TouchPoint {
                id: 0,
                x: a.0,
                y: a.1,
            },
            TouchPoint {
                id: 1,
                x: b.0,
                y: b.1,
            },
        ])
    }

    fn begin_two(a: (f64, f64), b: (f64, f64)) -> TouchEvent {
        match two_fingers(a, b) {
            TouchEvent::Move(t) => TouchEvent::Begin(t),
            _ => unreachable!(),
        }
    }

    /// Rotating used to discard the zoom, so a twisting pinch stopped scaling.
    #[test]
    fn test_pinch_zooms_and_rotates_together() {
        let mut r = GestureRecognizer::new();
        let camera = Camera::new(Coordinate::new(0.0, 0.0), 10.0);
        r.process(&begin_two((100.0, 200.0), (200.0, 200.0)), &camera);

        // spread apart and twist well past the dead zone
        let result = r.process(&two_fingers((60.0, 140.0), (240.0, 260.0)), &camera);
        match result {
            GestureResult::Pinch {
                zoom_delta,
                rotate_degrees,
                ..
            } => {
                assert!(zoom_delta > 0.0, "expected zoom in, got {zoom_delta}");
                assert!(rotate_degrees.abs() > 5.0, "expected rotation");
            }
            other => panic!("expected Pinch, got {other:?}"),
        }
    }

    /// A straight pinch inside the dead zone must not rotate the map.
    #[test]
    fn test_small_twist_does_not_rotate() {
        let mut r = GestureRecognizer::new();
        let camera = Camera::new(Coordinate::new(0.0, 0.0), 10.0);
        r.process(&begin_two((100.0, 200.0), (200.0, 200.0)), &camera);

        let result = r.process(&two_fingers((98.0, 199.0), (202.0, 201.0)), &camera);
        match result {
            GestureResult::Pinch { rotate_degrees, .. } => {
                assert_eq!(rotate_degrees, 0.0);
            }
            other => panic!("expected Pinch, got {other:?}"),
        }
    }

    /// Two fingers landing on the same point must not divide by zero into NaN.
    #[test]
    fn test_zero_distance_pinch_is_finite() {
        let mut r = GestureRecognizer::new();
        let camera = Camera::new(Coordinate::new(0.0, 0.0), 10.0);
        r.process(&begin_two((100.0, 100.0), (100.0, 100.0)), &camera);

        let result = r.process(&two_fingers((100.0, 100.0), (150.0, 100.0)), &camera);
        match result {
            GestureResult::Pinch {
                zoom_delta,
                rotate_degrees,
                ..
            } => {
                assert!(zoom_delta.is_finite(), "zoom {zoom_delta} must be finite");
                assert!(rotate_degrees.is_finite());
            }
            other => panic!("expected Pinch, got {other:?}"),
        }
    }

    /// Crossing the atan2 seam at 180 degrees is a small turn, not a full circle.
    #[test]
    fn test_rotation_across_angle_seam() {
        assert!((shortest_angle(-359.0) - 1.0).abs() < 1e-9);
        assert!((shortest_angle(359.0) + 1.0).abs() < 1e-9);
        assert!((shortest_angle(10.0) - 10.0).abs() < 1e-9);
    }

    /// Bearing must stay in 0..360 rather than going negative.
    #[test]
    fn test_apply_rotate_wraps_bearing() {
        let mut camera = Camera::new(Coordinate::new(0.0, 0.0), 10.0);
        let viewport = Viewport::new(800, 600, 1.0);
        GestureRecognizer::apply(
            &GestureResult::Rotate {
                delta_degrees: -30.0,
            },
            &mut camera,
            &viewport,
        );
        assert!((camera.bearing - 330.0).abs() < 1e-9);
    }
}
