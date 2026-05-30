//! Offline data store — local vector/attribute storage for field work.
//!
//! Stores GeoJSON features locally, syncs with server when online.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::location::Coordinate;

/// A stored feature in the offline database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineFeature {
    pub id: String,
    pub layer: String,
    pub geometry: OfflineGeometry,
    pub properties: HashMap<String, serde_json::Value>,
    pub created_at: u64,
    pub modified_at: u64,
    pub sync_status: SyncStatus,
}

/// Geometry for offline storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OfflineGeometry {
    Point(Coordinate),
    LineString(Vec<Coordinate>),
    Polygon(Vec<Vec<Coordinate>>),
}

/// Sync status of a feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Synced with server.
    Synced,
    /// Created locally, not yet uploaded.
    PendingCreate,
    /// Modified locally, not yet uploaded.
    PendingUpdate,
    /// Deleted locally, not yet synced.
    PendingDelete,
    /// Sync conflict detected.
    Conflict,
}

/// In-memory offline feature store.
pub struct OfflineStore {
    features: HashMap<String, OfflineFeature>,
}

impl OfflineStore {
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
        }
    }

    /// Insert or update a feature.
    pub fn upsert(&mut self, feature: OfflineFeature) {
        self.features.insert(feature.id.clone(), feature);
    }

    /// Get a feature by ID.
    pub fn get(&self, id: &str) -> Option<&OfflineFeature> {
        self.features.get(id)
    }

    /// Mark a feature as deleted (soft delete for sync).
    pub fn mark_deleted(&mut self, id: &str) -> Result<(), Error> {
        if let Some(feature) = self.features.get_mut(id) {
            feature.sync_status = SyncStatus::PendingDelete;
            Ok(())
        } else {
            Err(Error::OfflineDb(format!("feature not found: {id}")))
        }
    }

    /// Get all features pending sync.
    pub fn pending_sync(&self) -> Vec<&OfflineFeature> {
        self.features
            .values()
            .filter(|f| !matches!(f.sync_status, SyncStatus::Synced))
            .collect()
    }

    /// Mark features as synced after successful upload.
    pub fn mark_synced(&mut self, ids: &[&str]) {
        for id in ids {
            if let Some(feature) = self.features.get_mut(*id) {
                if feature.sync_status == SyncStatus::PendingDelete {
                    self.features.remove(*id);
                } else {
                    feature.sync_status = SyncStatus::Synced;
                }
            }
        }
    }

    /// Query features within a bounding box.
    pub fn query_bbox(
        &self,
        min_lat: f64,
        max_lat: f64,
        min_lon: f64,
        max_lon: f64,
    ) -> Vec<&OfflineFeature> {
        self.features
            .values()
            .filter(|f| {
                if f.sync_status == SyncStatus::PendingDelete {
                    return false;
                }
                match &f.geometry {
                    OfflineGeometry::Point(c) => {
                        c.latitude >= min_lat
                            && c.latitude <= max_lat
                            && c.longitude >= min_lon
                            && c.longitude <= max_lon
                    }
                    OfflineGeometry::LineString(coords) => coords.iter().any(|c| {
                        c.latitude >= min_lat
                            && c.latitude <= max_lat
                            && c.longitude >= min_lon
                            && c.longitude <= max_lon
                    }),
                    OfflineGeometry::Polygon(rings) => rings.iter().flatten().any(|c| {
                        c.latitude >= min_lat
                            && c.latitude <= max_lat
                            && c.longitude >= min_lon
                            && c.longitude <= max_lon
                    }),
                }
            })
            .collect()
    }

    /// Number of features in the store.
    pub fn len(&self) -> usize {
        self.features.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }

    /// Export all features as GeoJSON.
    pub fn export_geojson(&self) -> serde_json::Value {
        let features: Vec<serde_json::Value> = self
            .features
            .values()
            .filter(|f| f.sync_status != SyncStatus::PendingDelete)
            .map(|f| {
                serde_json::json!({
                    "type": "Feature",
                    "id": f.id,
                    "geometry": geometry_to_geojson(&f.geometry),
                    "properties": f.properties,
                })
            })
            .collect();

        serde_json::json!({
            "type": "FeatureCollection",
            "features": features,
        })
    }
}

impl Default for OfflineStore {
    fn default() -> Self {
        Self::new()
    }
}

fn geometry_to_geojson(geom: &OfflineGeometry) -> serde_json::Value {
    match geom {
        OfflineGeometry::Point(c) => serde_json::json!({
            "type": "Point",
            "coordinates": [c.longitude, c.latitude]
        }),
        OfflineGeometry::LineString(coords) => serde_json::json!({
            "type": "LineString",
            "coordinates": coords.iter().map(|c| [c.longitude, c.latitude]).collect::<Vec<_>>()
        }),
        OfflineGeometry::Polygon(rings) => serde_json::json!({
            "type": "Polygon",
            "coordinates": rings.iter().map(|ring|
                ring.iter().map(|c| [c.longitude, c.latitude]).collect::<Vec<_>>()
            ).collect::<Vec<_>>()
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_feature(id: &str, lat: f64, lon: f64) -> OfflineFeature {
        OfflineFeature {
            id: id.to_string(),
            layer: "test".to_string(),
            geometry: OfflineGeometry::Point(Coordinate::new(lat, lon)),
            properties: HashMap::new(),
            created_at: 0,
            modified_at: 0,
            sync_status: SyncStatus::PendingCreate,
        }
    }

    #[test]
    fn test_upsert_and_get() {
        let mut store = OfflineStore::new();
        store.upsert(test_feature("1", 51.5, -0.1));
        assert!(store.get("1").is_some());
        assert!(store.get("2").is_none());
    }

    #[test]
    fn test_bbox_query() {
        let mut store = OfflineStore::new();
        store.upsert(test_feature("london", 51.5, -0.1));
        store.upsert(test_feature("paris", 48.8, 2.3));
        store.upsert(test_feature("tokyo", 35.6, 139.7));

        let results = store.query_bbox(48.0, 52.0, -1.0, 3.0);
        assert_eq!(results.len(), 2); // London and Paris
    }

    #[test]
    fn test_pending_sync() {
        let mut store = OfflineStore::new();
        store.upsert(test_feature("1", 51.5, -0.1));

        let pending = store.pending_sync();
        assert_eq!(pending.len(), 1);

        store.mark_synced(&["1"]);
        let pending = store.pending_sync();
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn test_export_geojson() {
        let mut store = OfflineStore::new();
        store.upsert(test_feature("1", 51.5, -0.1));
        let geojson = store.export_geojson();
        assert_eq!(geojson["type"], "FeatureCollection");
        assert_eq!(geojson["features"].as_array().unwrap().len(), 1);
    }
}
