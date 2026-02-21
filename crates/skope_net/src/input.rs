//! Input payload and server-side input buffer.
//!
//! Defines the `InputPayload` sent from clients to the server each tick,
//! and the `InputBuffer` resource that stores recent inputs per peer.

use serde::{Serialize, Deserialize};
use skope_ecs::prelude::*;
use std::collections::{HashMap, VecDeque};

/// Client input captured each tick and sent to the server.
/// Also used as a Component on player entities for server-side input application,
/// and as a Resource on the client for the current frame's input.
#[derive(Component, Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct InputPayload {
    /// Normalized movement direction (XY plane, Z-up coordinate system)
    pub move_direction: [f32; 3],
    /// Facing direction in radians (yaw)
    pub facing_yaw: f32,
    /// Whether the player is running (Shift held)
    pub is_running: bool,
    /// Jump requested this tick (Space pressed)
    pub jump: bool,
    /// Primary action (attack/use)
    pub primary_action: bool,
}

/// Server-side input buffer — stores the most recent N ticks of input per peer.
#[derive(Resource)]
pub struct InputBuffer {
    /// peer_id → ring buffer of (tick, payload)
    pub buffers: HashMap<u32, VecDeque<(u64, InputPayload)>>,
    /// Maximum number of inputs to retain per peer
    pub max_buffer_size: usize,
}

impl Default for InputBuffer {
    fn default() -> Self {
        Self {
            buffers: HashMap::new(),
            max_buffer_size: 32,
        }
    }
}

impl InputBuffer {
    /// Push an input for a peer. Trims oldest entries if over capacity.
    pub fn push(&mut self, peer_id: u32, tick: u64, payload: InputPayload) {
        let buf = self.buffers.entry(peer_id).or_default();
        buf.push_back((tick, payload));
        while buf.len() > self.max_buffer_size {
            buf.pop_front();
        }
    }

    /// Take the latest input for a peer (removes it from the buffer).
    pub fn take_latest(&mut self, peer_id: u32) -> Option<InputPayload> {
        self.buffers.get_mut(&peer_id)?.pop_back().map(|(_, p)| p)
    }

    /// Peek the latest input for a peer without removing it.
    pub fn peek_latest(&self, peer_id: u32) -> Option<&InputPayload> {
        self.buffers.get(&peer_id)?.back().map(|(_, p)| p)
    }

    /// Remove all inputs for a disconnected peer.
    pub fn remove_peer(&mut self, peer_id: u32) {
        self.buffers.remove(&peer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_payload_roundtrip() {
        let payload = InputPayload {
            move_direction: [0.7071, 0.7071, 0.0],
            facing_yaw: 1.57,
            is_running: true,
            jump: false,
            primary_action: true,
        };
        let bytes = bincode::serialize(&payload).unwrap();
        let decoded: InputPayload = bincode::deserialize(&bytes).unwrap();
        assert!((decoded.move_direction[0] - 0.7071).abs() < 0.001);
        assert_eq!(decoded.is_running, true);
        assert_eq!(decoded.primary_action, true);
    }

    #[test]
    fn test_input_buffer_push_and_take() {
        let mut buf = InputBuffer::default();
        buf.push(1, 10, InputPayload { is_running: true, ..Default::default() });
        buf.push(1, 11, InputPayload { jump: true, ..Default::default() });

        let latest = buf.take_latest(1).unwrap();
        assert!(latest.jump);

        let prev = buf.take_latest(1).unwrap();
        assert!(prev.is_running);

        assert!(buf.take_latest(1).is_none());
    }

    #[test]
    fn test_input_buffer_max_size() {
        let mut buf = InputBuffer { max_buffer_size: 3, ..Default::default() };
        for i in 0..5 {
            buf.push(1, i, InputPayload::default());
        }
        assert_eq!(buf.buffers[&1].len(), 3);
        assert_eq!(buf.buffers[&1].front().unwrap().0, 2); // oldest kept is tick 2
    }
}
