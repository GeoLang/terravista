//! Location and coordinate types.

use serde::{Deserialize, Serialize};

/// A geographic coordinate (WGS84).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coordinate {
    pub latitude: f64,
    pub longitude: f64,
}

impl Coordinate {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }

    /// Haversine distance to another coordinate (meters).
    pub fn distance_to(&self, other: &Coordinate) -> f64 {
        const R: f64 = 6_371_000.0; // Earth radius in meters
        let d_lat = (other.latitude - self.latitude).to_radians();
        let d_lon = (other.longitude - self.longitude).to_radians();
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();

        let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        R * c
    }

    /// Bearing to another coordinate (degrees from north).
    pub fn bearing_to(&self, other: &Coordinate) -> f64 {
        let lat1 = self.latitude.to_radians();
        let lat2 = other.latitude.to_radians();
        let d_lon = (other.longitude - self.longitude).to_radians();

        let y = d_lon.sin() * lat2.cos();
        let x = lat1.cos() * lat2.sin() - lat1.sin() * lat2.cos() * d_lon.cos();
        let bearing = y.atan2(x).to_degrees();
        (bearing + 360.0) % 360.0
    }
}

/// A full location fix from GPS.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub coordinate: Coordinate,
    pub altitude: Option<f64>,
    pub accuracy: Option<f64>,
    pub speed: Option<f64>,
    pub heading: Option<f64>,
    pub timestamp_ms: u64,
}

/// Location tracking mode.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TrackingMode {
    /// No tracking — user controls camera freely.
    #[default]
    None,
    /// Follow user location, don't rotate map.
    Follow,
    /// Follow user and rotate map to match heading.
    FollowWithHeading,
    /// Follow user and rotate to match course (driving mode).
    FollowWithCourse,
}

/// Location service abstraction (implemented by platform layer).
pub trait LocationProvider {
    fn last_location(&self) -> Option<Location>;
    fn start_updates(&mut self);
    fn stop_updates(&mut self);
    fn tracking_mode(&self) -> TrackingMode;
    fn set_tracking_mode(&mut self, mode: TrackingMode);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_london_paris() {
        let london = Coordinate::new(51.5074, -0.1278);
        let paris = Coordinate::new(48.8566, 2.3522);
        let dist = london.distance_to(&paris);
        // ~340 km
        assert!(dist > 330_000.0 && dist < 350_000.0);
    }

    #[test]
    fn test_bearing() {
        let a = Coordinate::new(0.0, 0.0);
        let b = Coordinate::new(1.0, 0.0);
        let bearing = a.bearing_to(&b);
        // Should be roughly north (0 degrees)
        assert!(!(1.0..=359.0).contains(&bearing));
    }
}
