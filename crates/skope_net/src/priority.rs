//! Replication priority and bandwidth budget.
//!
//! Provides distance-based priority scoring and per-peer bandwidth limiting
//! to ensure the most important entities are replicated first.

use glam::Vec3;
use skope_ecs::prelude::*;
use std::collections::HashMap;

/// Per-peer entity replication priorities.
/// Priority accumulates over time — entities that haven't been sent recently
/// get higher priority, ensuring eventual consistency.
#[derive(Resource, Default)]
pub struct ReplicationPriority {
    /// peer_id → (net_id → accumulated_priority)
    pub peer_priorities: HashMap<u32, HashMap<u64, f32>>,
}

impl ReplicationPriority {
    /// Update priorities for a peer based on entity distances.
    /// Closer entities get higher priority increments.
    pub fn update(&mut self, peer_id: u32, entities: &[(u64, Vec3)], peer_pos: Vec3) {
        let priorities = self.peer_priorities.entry(peer_id).or_default();

        for &(net_id, pos) in entities {
            let distance = (pos - peer_pos).length().max(1.0);
            // Inverse distance priority: closer = higher
            let priority_increment = 1.0 / distance;
            *priorities.entry(net_id).or_insert(0.0) += priority_increment;
        }
    }

    /// Get entities sorted by priority (highest first) for a peer.
    pub fn sorted_entities(&self, peer_id: u32) -> Vec<(u64, f32)> {
        let priorities = match self.peer_priorities.get(&peer_id) {
            Some(p) => p,
            None => return Vec::new(),
        };

        let mut sorted: Vec<(u64, f32)> = priorities.iter().map(|(&k, &v)| (k, v)).collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted
    }

    /// Reset priority for entities that were successfully sent.
    pub fn mark_sent(&mut self, peer_id: u32, sent_ids: &[u64]) {
        if let Some(priorities) = self.peer_priorities.get_mut(&peer_id) {
            for id in sent_ids {
                priorities.remove(id);
            }
        }
    }

    /// Remove all state for a disconnected peer.
    pub fn remove_peer(&mut self, peer_id: u32) {
        self.peer_priorities.remove(&peer_id);
    }
}

/// Per-peer bandwidth budget resource.
#[derive(Resource)]
pub struct BandwidthBudget {
    /// Maximum bytes per peer per tick.
    pub max_bytes_per_peer_per_tick: usize,
}

impl Default for BandwidthBudget {
    fn default() -> Self {
        Self {
            max_bytes_per_peer_per_tick: 4096,
        }
    }
}

impl BandwidthBudget {
    /// Select entities to send within the budget.
    /// Takes a list of (net_id, serialized_size) sorted by priority (highest first).
    /// Returns the net_ids that fit within the budget.
    pub fn select_within_budget(&self, candidates: &[(u64, usize)]) -> Vec<u64> {
        let mut remaining = self.max_bytes_per_peer_per_tick;
        let mut selected = Vec::new();

        for &(net_id, size) in candidates {
            if size <= remaining {
                selected.push(net_id);
                remaining -= size;
            }
            if remaining == 0 {
                break;
            }
        }

        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_accumulation() {
        let mut prio = ReplicationPriority::default();

        let entities = vec![
            (1, Vec3::new(10.0, 0.0, 0.0)),  // close
            (2, Vec3::new(100.0, 0.0, 0.0)), // far
        ];

        // Update twice
        prio.update(1, &entities, Vec3::ZERO);
        prio.update(1, &entities, Vec3::ZERO);

        let sorted = prio.sorted_entities(1);
        assert_eq!(sorted[0].0, 1); // closer entity has higher priority
        assert!(sorted[0].1 > sorted[1].1);

        // Mark sent → resets
        prio.mark_sent(1, &[1]);
        let sorted = prio.sorted_entities(1);
        assert_eq!(sorted.len(), 1);
        assert_eq!(sorted[0].0, 2);
    }

    #[test]
    fn test_bandwidth_budget() {
        let budget = BandwidthBudget {
            max_bytes_per_peer_per_tick: 100,
        };

        // 10 entities, 50 bytes each → only 2 fit
        let candidates: Vec<(u64, usize)> = (0..10).map(|i| (i, 50)).collect();
        let selected = budget.select_within_budget(&candidates);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0], 0);
        assert_eq!(selected[1], 1);
    }
}
