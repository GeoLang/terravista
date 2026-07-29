//! Route engine — on-device turn-by-turn navigation.
//!
//! Provides route geometry display, step-by-step instructions,
//! and progress tracking along a route.

use serde::{Deserialize, Serialize};

use crate::location::Coordinate;

/// A navigation route with geometry and instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub geometry: Vec<Coordinate>,
    pub distance_m: f64,
    pub duration_s: f64,
    pub steps: Vec<RouteStep>,
}

/// A single step/maneuver in a route.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    pub instruction: String,
    pub maneuver: Maneuver,
    pub distance_m: f64,
    pub duration_s: f64,
    pub start_index: usize,
    pub end_index: usize,
}

/// Turn-by-turn maneuver type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Maneuver {
    Depart,
    TurnLeft,
    TurnRight,
    SlightLeft,
    SlightRight,
    SharpLeft,
    SharpRight,
    UTurn,
    Straight,
    Merge,
    RampLeft,
    RampRight,
    Roundabout,
    Arrive,
}

/// Navigation state — tracks progress along a route.
pub struct Navigator {
    route: Route,
    current_step: usize,
    closest_point_index: usize,
    distance_remaining: f64,
    off_route: bool,
}

impl Navigator {
    /// Create a navigator for a given route.
    pub fn new(route: Route) -> Self {
        let total_dist = route.distance_m;
        Self {
            route,
            current_step: 0,
            closest_point_index: 0,
            distance_remaining: total_dist,
            off_route: false,
        }
    }

    /// Update navigation state with new user location.
    pub fn update(&mut self, location: &Coordinate) -> NavigationUpdate {
        // nothing to follow, and the index maths below would underflow
        if self.route.steps.is_empty() || self.route.geometry.is_empty() {
            self.off_route = true;
            return NavigationUpdate {
                status: NavStatus::OffRoute,
                current_step: 0,
                distance_to_next_step: 0.0,
                distance_remaining: 0.0,
                instruction: "No route".to_string(),
            };
        }

        // Find closest point on route
        let (closest_idx, dist_to_route) = self.find_closest_point(location);
        self.closest_point_index = closest_idx;

        // Check if off-route (>50m from route)
        self.off_route = dist_to_route > 50.0;

        if self.off_route {
            return NavigationUpdate {
                status: NavStatus::OffRoute,
                current_step: self.current_step,
                distance_to_next_step: 0.0,
                distance_remaining: self.distance_remaining,
                instruction: "Recalculating...".to_string(),
            };
        }

        // Update current step based on position
        while self.current_step < self.route.steps.len() - 1 {
            let step = &self.route.steps[self.current_step];
            if closest_idx >= step.end_index {
                self.current_step += 1;
            } else {
                break;
            }
        }

        // Calculate distances
        let step = &self.route.steps[self.current_step];
        let dist_to_next = self.distance_along_route(closest_idx, step.end_index);
        self.distance_remaining =
            self.distance_along_route(closest_idx, self.route.geometry.len() - 1);

        let status = if self.current_step >= self.route.steps.len() - 1
            && dist_to_route < 20.0
            && self.distance_remaining < 20.0
        {
            NavStatus::Arrived
        } else {
            NavStatus::OnRoute
        };

        NavigationUpdate {
            status,
            current_step: self.current_step,
            distance_to_next_step: dist_to_next,
            distance_remaining: self.distance_remaining,
            instruction: step.instruction.clone(),
        }
    }

    /// Get current route.
    pub fn route(&self) -> &Route {
        &self.route
    }

    /// Whether user is off-route.
    pub fn is_off_route(&self) -> bool {
        self.off_route
    }

    /// Closest vertex index and the distance to the route *line*.
    ///
    /// Measuring to vertices instead of segments reports a driver as off-route
    /// whenever the route's vertices are further apart than the threshold.
    fn find_closest_point(&self, location: &Coordinate) -> (usize, f64) {
        let geometry = &self.route.geometry;
        match geometry.len() {
            0 => return (0, f64::MAX),
            1 => return (0, location.distance_to(&geometry[0])),
            _ => {}
        }

        let mut best_idx = 0;
        let mut best_dist = f64::MAX;

        for i in 0..geometry.len() - 1 {
            let (dist, along) = distance_to_segment(location, &geometry[i], &geometry[i + 1]);
            if dist < best_dist {
                best_dist = dist;
                // report the vertex already passed, so step advance stays monotonic
                best_idx = if along > 0.5 { i + 1 } else { i };
            }
        }

        (best_idx, best_dist)
    }

    fn distance_along_route(&self, from_idx: usize, to_idx: usize) -> f64 {
        if from_idx >= to_idx || to_idx >= self.route.geometry.len() {
            return 0.0;
        }
        let mut dist = 0.0;
        for i in from_idx..to_idx {
            dist += self.route.geometry[i].distance_to(&self.route.geometry[i + 1]);
        }
        dist
    }
}

/// Result of a navigation update.
#[derive(Debug, Clone)]
pub struct NavigationUpdate {
    pub status: NavStatus,
    pub current_step: usize,
    pub distance_to_next_step: f64,
    pub distance_remaining: f64,
    pub instruction: String,
}

/// Navigation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavStatus {
    OnRoute,
    OffRoute,
    Arrived,
}

/// Distance in metres from `p` to the segment `a`-`b`, plus how far along that
/// segment the closest point sits (0 at `a`, 1 at `b`).
///
/// Uses a local equirectangular projection, which is accurate over the short
/// spans between route vertices.
fn distance_to_segment(p: &Coordinate, a: &Coordinate, b: &Coordinate) -> (f64, f64) {
    const METRES_PER_DEGREE: f64 = 111_320.0;
    let lon_scale = p.latitude.to_radians().cos() * METRES_PER_DEGREE;

    let px = (p.longitude - a.longitude) * lon_scale;
    let py = (p.latitude - a.latitude) * METRES_PER_DEGREE;
    let bx = (b.longitude - a.longitude) * lon_scale;
    let by = (b.latitude - a.latitude) * METRES_PER_DEGREE;

    let len_sq = bx * bx + by * by;
    let along = if len_sq > 0.0 {
        ((px * bx + py * by) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let dx = px - along * bx;
    let dy = py - along * by;
    ((dx * dx + dy * dy).sqrt(), along)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple_route() -> Route {
        Route {
            geometry: vec![
                Coordinate::new(51.5, -0.1),
                Coordinate::new(51.501, -0.1),
                Coordinate::new(51.502, -0.1),
                Coordinate::new(51.503, -0.099),
                Coordinate::new(51.504, -0.098),
            ],
            distance_m: 500.0,
            duration_s: 60.0,
            steps: vec![
                RouteStep {
                    instruction: "Head north".to_string(),
                    maneuver: Maneuver::Depart,
                    distance_m: 300.0,
                    duration_s: 36.0,
                    start_index: 0,
                    end_index: 2,
                },
                RouteStep {
                    instruction: "Turn right".to_string(),
                    maneuver: Maneuver::TurnRight,
                    distance_m: 200.0,
                    duration_s: 24.0,
                    start_index: 2,
                    end_index: 4,
                },
            ],
        }
    }

    #[test]
    fn test_navigator_on_route() {
        let mut nav = Navigator::new(simple_route());
        // Within 50m of route point (51.501, -0.1)
        let update = nav.update(&Coordinate::new(51.501, -0.1001));
        assert_eq!(update.status, NavStatus::OnRoute);
        assert_eq!(update.current_step, 0);
    }

    #[test]
    fn test_navigator_off_route() {
        let mut nav = Navigator::new(simple_route());
        // Way off route
        let update = nav.update(&Coordinate::new(52.0, 1.0));
        assert_eq!(update.status, NavStatus::OffRoute);
    }

    #[test]
    fn test_navigator_arrived() {
        let mut nav = Navigator::new(simple_route());
        // At the end point
        let update = nav.update(&Coordinate::new(51.504, -0.098));
        assert_eq!(update.status, NavStatus::Arrived);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    fn straight_route() -> Route {
        // two vertices 2 km apart, as a router would emit for a straight road
        Route {
            geometry: vec![
                Coordinate::new(51.5000, -0.1),
                Coordinate::new(51.5180, -0.1),
            ],
            distance_m: 2001.0,
            duration_s: 120.0,
            steps: vec![RouteStep {
                instruction: "Head north".to_string(),
                maneuver: Maneuver::Depart,
                distance_m: 2001.0,
                duration_s: 120.0,
                start_index: 0,
                end_index: 1,
            }],
        }
    }

    /// Standing on the line between two distant vertices is on-route, even
    /// though both vertices are a kilometre away.
    #[test]
    fn test_midpoint_of_long_segment_is_on_route() {
        let mut nav = Navigator::new(straight_route());
        let update = nav.update(&Coordinate::new(51.5090, -0.1));
        assert_eq!(update.status, NavStatus::OnRoute);
        assert!(!nav.is_off_route());
    }

    /// Genuinely leaving the road still reports off-route.
    #[test]
    fn test_far_from_line_is_off_route() {
        let mut nav = Navigator::new(straight_route());
        let update = nav.update(&Coordinate::new(51.5090, -0.11));
        assert_eq!(update.status, NavStatus::OffRoute);
    }

    /// A route with no steps must not underflow the step index.
    #[test]
    fn test_empty_route_does_not_panic() {
        let mut nav = Navigator::new(Route {
            geometry: vec![Coordinate::new(51.5, -0.1)],
            distance_m: 0.0,
            duration_s: 0.0,
            steps: vec![],
        });
        let update = nav.update(&Coordinate::new(51.5, -0.1));
        assert_eq!(update.status, NavStatus::OffRoute);
    }
}
