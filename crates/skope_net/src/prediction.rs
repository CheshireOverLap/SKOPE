//! Client-side prediction buffer.
//!
//! Records predicted states per tick so the client can reconcile
//! when authoritative server state arrives.

use glam::{Vec3, Quat};
use skope_ecs::prelude::*;
use std::collections::VecDeque;

use crate::input::InputPayload;

/// A single predicted state snapshot for one tick.
#[derive(Debug, Clone)]
pub struct PredictedState {
    pub tick: u64,
    pub translation: Vec3,
    pub rotation: Quat,
    pub velocity: Vec3,
    pub input: InputPayload,
}

/// Client-side prediction buffer.
/// Stores recent predicted states so they can be compared against
/// authoritative server state for reconciliation.
#[derive(Resource)]
pub struct PredictionBuffer {
    /// Ring buffer of recent predicted states (newest at back).
    pub states: VecDeque<PredictedState>,
    /// Maximum states to keep (default: 128, ~4 sec at 30Hz).
    pub max_states: usize,
    /// The last server tick we received and acknowledged.
    pub last_server_tick: u64,
    /// Error threshold for snap correction (units). Above this → teleport.
    pub snap_threshold: f32,
    /// Blend rate for gradual correction (units/sec).
    pub blend_rate: f32,
    /// Current correction offset being blended out.
    pub correction_offset: Vec3,
}

impl Default for PredictionBuffer {
    fn default() -> Self {
        Self {
            states: VecDeque::new(),
            max_states: 128,
            last_server_tick: 0,
            snap_threshold: 2.0,
            blend_rate: 10.0,
            correction_offset: Vec3::ZERO,
        }
    }
}

impl PredictionBuffer {
    /// Record a new predicted state.
    pub fn push(&mut self, state: PredictedState) {
        self.states.push_back(state);
        while self.states.len() > self.max_states {
            self.states.pop_front();
        }
    }

    /// Acknowledge a server tick — removes all states at or before that tick.
    pub fn acknowledge(&mut self, server_tick: u64) {
        self.last_server_tick = server_tick;
        while self.states.front().map(|s| s.tick <= server_tick).unwrap_or(false) {
            self.states.pop_front();
        }
    }

    /// Get the predicted state for a specific tick (if still buffered).
    pub fn get_state(&self, tick: u64) -> Option<&PredictedState> {
        self.states.iter().find(|s| s.tick == tick)
    }

    /// Calculate reconciliation error between server state and our prediction.
    /// Returns the error vector (server_pos - predicted_pos).
    pub fn reconciliation_error(&self, server_tick: u64, server_pos: Vec3) -> Option<Vec3> {
        self.get_state(server_tick).map(|predicted| server_pos - predicted.translation)
    }

    /// Apply correction based on error magnitude.
    /// - error > snap_threshold → immediate snap (teleport). Returns `Some(error)` so caller can apply.
    /// - error <= snap_threshold → blend over time. Returns `None`.
    pub fn apply_correction(&mut self, error: Vec3) -> Option<Vec3> {
        if error.length() > self.snap_threshold {
            // Snap: return the full error for immediate teleport
            self.correction_offset = Vec3::ZERO;
            Some(error)
        } else {
            // Blend: accumulate correction offset
            self.correction_offset += error;
            None
        }
    }

    /// Blend out the correction offset over time. Returns the display offset to apply.
    pub fn blend_correction(&mut self, dt: f32) -> Vec3 {
        if self.correction_offset.length() < 0.001 {
            self.correction_offset = Vec3::ZERO;
            return Vec3::ZERO;
        }

        let blend_amount = (self.blend_rate * dt).min(1.0);
        let offset = self.correction_offset * blend_amount;
        self.correction_offset -= offset;
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prediction_buffer_push_ack() {
        let mut buf = PredictionBuffer::default();

        for i in 1..=10 {
            buf.push(PredictedState {
                tick: i,
                translation: Vec3::new(i as f32, 0.0, 0.0),
                rotation: Quat::IDENTITY,
                velocity: Vec3::ZERO,
                input: InputPayload::default(),
            });
        }
        assert_eq!(buf.states.len(), 10);

        // ACK tick 5 → ticks 1-5 removed
        buf.acknowledge(5);
        assert_eq!(buf.states.len(), 5);
        assert_eq!(buf.states.front().unwrap().tick, 6);
    }

    #[test]
    fn test_reconcile_small_error() {
        let mut buf = PredictionBuffer::default();
        buf.push(PredictedState {
            tick: 10,
            translation: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            velocity: Vec3::ZERO,
            input: InputPayload::default(),
        });

        let error = buf.reconciliation_error(10, Vec3::new(5.1, 0.0, 0.0)).unwrap();
        assert!(error.length() < buf.snap_threshold);

        let snap = buf.apply_correction(error);
        assert!(snap.is_none()); // small error → blend, not snap
        assert!(buf.correction_offset.length() > 0.0);

        // Blend should reduce correction over time
        let offset = buf.blend_correction(0.1);
        assert!(offset.length() > 0.0);
    }

    #[test]
    fn test_reconcile_large_error() {
        let mut buf = PredictionBuffer::default();
        buf.push(PredictedState {
            tick: 10,
            translation: Vec3::new(0.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            velocity: Vec3::ZERO,
            input: InputPayload::default(),
        });

        let error = buf.reconciliation_error(10, Vec3::new(5.0, 0.0, 0.0)).unwrap();
        assert!(error.length() > buf.snap_threshold);

        let snap = buf.apply_correction(error);
        // Snap correction: returns the error for immediate teleport
        assert!(snap.is_some());
        assert!((snap.unwrap() - error).length() < 0.001);
        assert_eq!(buf.correction_offset, Vec3::ZERO);
    }
}
