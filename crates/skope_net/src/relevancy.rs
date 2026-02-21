//! Spatial grid for interest management / relevancy filtering.
//!
//! Divides the world into 2D grid cells and tracks which entities are
//! relevant to each peer based on distance from the peer's viewpoint.

use glam::Vec3;
use skope_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

/// Configuration for relevancy filtering.
#[derive(Debug, Clone)]
pub struct RelevancyConfig {
    /// Size of each grid cell (world units).
    pub cell_size: f32,
    /// Maximum distance for an entity to be considered relevant.
    pub cull_distance: f32,
    /// Hysteresis frames: keep an entity relevant for N extra frames
    /// after it leaves the cull distance (prevents pop-in/pop-out).
    pub hysteresis_frames: u32,
}

impl Default for RelevancyConfig {
    fn default() -> Self {
        Self {
            cell_size: 50.0,
            cull_distance: 200.0,
            hysteresis_frames: 10,
        }
    }
}

/// Per-peer relevancy state.
#[derive(Debug, Default)]
pub struct PeerRelevancy {
    /// Set of net_ids currently considered relevant.
    pub relevant: HashSet<u64>,
    /// Hysteresis counters: net_id → remaining frames before removal.
    pub leaving: HashMap<u64, u32>,
}

/// 2D spatial grid for entity position lookups.
#[derive(Resource)]
pub struct SpatialGrid {
    pub config: RelevancyConfig,
    /// Grid cell → list of (net_id, position).
    cells: HashMap<(i32, i32), Vec<(u64, Vec3)>>,
    /// Per-peer relevancy state.
    pub peer_relevancy: HashMap<u32, PeerRelevancy>,
    /// Peer viewpoint positions (updated each tick).
    pub peer_positions: HashMap<u32, Vec3>,
    /// Flat net_id → position map for O(1) lookup (populated during rebuild).
    pub entity_positions: HashMap<u64, Vec3>,
}

impl Default for SpatialGrid {
    fn default() -> Self {
        Self {
            config: RelevancyConfig::default(),
            cells: HashMap::new(),
            peer_relevancy: HashMap::new(),
            peer_positions: HashMap::new(),
            entity_positions: HashMap::new(),
        }
    }
}

impl SpatialGrid {
    /// Convert a world position to a grid cell coordinate.
    fn cell_key(&self, pos: Vec3) -> (i32, i32) {
        let x = (pos.x / self.config.cell_size).floor() as i32;
        let y = (pos.y / self.config.cell_size).floor() as i32;
        (x, y)
    }

    /// Rebuild the spatial grid from entity positions.
    /// Call once per tick before querying relevancy.
    pub fn rebuild(&mut self, entities: &[(u64, Vec3)]) {
        self.cells.clear();
        self.entity_positions.clear();
        for &(net_id, pos) in entities {
            let key = self.cell_key(pos);
            self.cells.entry(key).or_default().push((net_id, pos));
            self.entity_positions.insert(net_id, pos);
        }
    }

    /// Query entities relevant to a peer based on their position.
    /// Returns the set of net_ids within cull_distance of the peer.
    pub fn query_relevant(&mut self, peer_id: u32, peer_pos: Vec3) -> HashSet<u64> {
        let cull_dist_sq = self.config.cull_distance * self.config.cull_distance;
        let cell_radius = (self.config.cull_distance / self.config.cell_size).ceil() as i32;
        let center = self.cell_key(peer_pos);

        let mut in_range: HashSet<u64> = HashSet::new();

        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                let key = (center.0 + dx, center.1 + dy);
                if let Some(entities) = self.cells.get(&key) {
                    for &(net_id, pos) in entities {
                        let dist_sq = (pos - peer_pos).length_squared();
                        if dist_sq <= cull_dist_sq {
                            in_range.insert(net_id);
                        }
                    }
                }
            }
        }

        // Apply hysteresis
        let relevancy = self.peer_relevancy.entry(peer_id).or_default();

        // Entities that were relevant but are now out of range → hysteresis
        let previously_relevant: Vec<u64> = relevancy.relevant.iter().copied().collect();
        for net_id in &previously_relevant {
            if !in_range.contains(net_id) {
                // Start or continue hysteresis
                let counter = relevancy.leaving.entry(*net_id).or_insert(self.config.hysteresis_frames);
                if *counter > 0 {
                    *counter -= 1;
                    in_range.insert(*net_id); // keep relevant during hysteresis
                } else {
                    relevancy.leaving.remove(net_id);
                }
            } else {
                // Back in range: cancel hysteresis
                relevancy.leaving.remove(net_id);
            }
        }

        relevancy.relevant = in_range.clone();
        self.peer_positions.insert(peer_id, peer_pos);

        in_range
    }

    /// Remove all state for a disconnected peer.
    pub fn remove_peer(&mut self, peer_id: u32) {
        self.peer_relevancy.remove(&peer_id);
        self.peer_positions.remove(&peer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_grid_range() {
        let mut grid = SpatialGrid {
            config: RelevancyConfig {
                cell_size: 50.0,
                cull_distance: 100.0,
                hysteresis_frames: 0,
            },
            ..Default::default()
        };

        // Place entities at various distances
        grid.rebuild(&[
            (1, Vec3::new(10.0, 0.0, 0.0)),   // 10 units away → relevant
            (2, Vec3::new(90.0, 0.0, 0.0)),   // 90 units → relevant
            (3, Vec3::new(150.0, 0.0, 0.0)),  // 150 units → culled
            (4, Vec3::new(0.0, 200.0, 0.0)),  // 200 units → culled
        ]);

        let relevant = grid.query_relevant(1, Vec3::ZERO);
        assert!(relevant.contains(&1));
        assert!(relevant.contains(&2));
        assert!(!relevant.contains(&3));
        assert!(!relevant.contains(&4));
    }

    #[test]
    fn test_hysteresis() {
        let mut grid = SpatialGrid {
            config: RelevancyConfig {
                cell_size: 50.0,
                cull_distance: 100.0,
                hysteresis_frames: 3,
            },
            ..Default::default()
        };

        // Entity 1 in range
        grid.rebuild(&[(1, Vec3::new(50.0, 0.0, 0.0))]);
        let relevant = grid.query_relevant(1, Vec3::ZERO);
        assert!(relevant.contains(&1));

        // Entity 1 moves out of range — should still be relevant (hysteresis)
        grid.rebuild(&[(1, Vec3::new(150.0, 0.0, 0.0))]);
        let relevant = grid.query_relevant(1, Vec3::ZERO);
        assert!(relevant.contains(&1)); // frame 1 of hysteresis

        let relevant = grid.query_relevant(1, Vec3::ZERO);
        assert!(relevant.contains(&1)); // frame 2

        let relevant = grid.query_relevant(1, Vec3::ZERO);
        assert!(relevant.contains(&1)); // frame 3

        // Frame 4: hysteresis expired
        let relevant = grid.query_relevant(1, Vec3::ZERO);
        assert!(!relevant.contains(&1));
    }
}
