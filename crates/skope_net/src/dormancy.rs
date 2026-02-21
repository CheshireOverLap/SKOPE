//! Dormancy tracking for unchanged entities.
//!
//! Entities that haven't changed for N ticks are marked as dormant
//! and excluded from replication to save bandwidth.

use skope_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

/// Tracks which entities are dormant (unchanged for a threshold number of ticks).
#[derive(Resource)]
pub struct DormancyTracker {
    /// Set of net_ids currently considered dormant.
    pub dormant: HashSet<u64>,
    /// Net_id → number of consecutive ticks without changes.
    pub unchanged_ticks: HashMap<u64, u32>,
    /// Number of unchanged ticks before an entity becomes dormant.
    pub dormancy_threshold: u32,
}

impl Default for DormancyTracker {
    fn default() -> Self {
        Self {
            dormant: HashSet::new(),
            unchanged_ticks: HashMap::new(),
            dormancy_threshold: 60, // 2 seconds at 30Hz
        }
    }
}

impl DormancyTracker {
    /// Mark an entity as changed this tick. Resets its dormancy counter and wakes it up.
    pub fn mark_changed(&mut self, net_id: u64) {
        self.unchanged_ticks.remove(&net_id);
        self.dormant.remove(&net_id);
    }

    /// Advance the dormancy check for entities that were NOT changed this tick.
    /// Call once per tick with the set of changed net_ids.
    pub fn tick(&mut self, changed_ids: &HashSet<u64>, all_ids: &[u64]) {
        for &net_id in all_ids {
            if changed_ids.contains(&net_id) {
                self.mark_changed(net_id);
            } else {
                let counter = self.unchanged_ticks.entry(net_id).or_insert(0);
                *counter = (*counter + 1).min(self.dormancy_threshold);
                if *counter >= self.dormancy_threshold {
                    self.dormant.insert(net_id);
                }
            }
        }
    }

    /// Check if an entity is dormant.
    pub fn is_dormant(&self, net_id: u64) -> bool {
        self.dormant.contains(&net_id)
    }

    /// Wake an entity from dormancy (force it to be replicated next tick).
    pub fn wake(&mut self, net_id: u64) {
        self.dormant.remove(&net_id);
        self.unchanged_ticks.remove(&net_id);
    }

    /// Remove tracking for a despawned entity.
    pub fn remove(&mut self, net_id: u64) {
        self.dormant.remove(&net_id);
        self.unchanged_ticks.remove(&net_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dormancy_lifecycle() {
        let mut tracker = DormancyTracker {
            dormancy_threshold: 3,
            ..Default::default()
        };

        let all_ids = vec![1, 2];
        let changed: HashSet<u64> = HashSet::new();

        // Tick 1: both unchanged
        tracker.tick(&changed, &all_ids);
        assert!(!tracker.is_dormant(1));
        assert!(!tracker.is_dormant(2));

        // Tick 2
        tracker.tick(&changed, &all_ids);
        assert!(!tracker.is_dormant(1));

        // Tick 3: threshold reached → dormant
        tracker.tick(&changed, &all_ids);
        assert!(tracker.is_dormant(1));
        assert!(tracker.is_dormant(2));

        // Entity 1 changes → wakes up
        let mut changed = HashSet::new();
        changed.insert(1);
        tracker.tick(&changed, &all_ids);
        assert!(!tracker.is_dormant(1));
        assert!(tracker.is_dormant(2));

        // After 3 more ticks of no change → dormant again
        let no_change: HashSet<u64> = HashSet::new();
        tracker.tick(&no_change, &all_ids);
        tracker.tick(&no_change, &all_ids);
        tracker.tick(&no_change, &all_ids);
        assert!(tracker.is_dormant(1));
    }

    #[test]
    fn test_wake() {
        let mut tracker = DormancyTracker {
            dormancy_threshold: 1,
            ..Default::default()
        };

        tracker.tick(&HashSet::new(), &[1]);
        assert!(tracker.is_dormant(1));

        tracker.wake(1);
        assert!(!tracker.is_dormant(1));
    }
}
