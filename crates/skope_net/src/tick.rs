//! Tick synchronization — RTT measurement, tick offset estimation, fixed timestep.
//!
//! Provides `TickSync` resource for client-side tick synchronization with the server.

use skope_ecs::prelude::*;

/// Client-side tick synchronization state.
#[derive(Resource)]
pub struct TickSync {
    /// Estimated round-trip time in seconds.
    pub rtt_seconds: f32,
    /// Tick offset: local_tick + offset ≈ server_tick.
    pub tick_offset: i64,
    /// Duration of one tick in seconds (1.0 / tick_rate).
    pub tick_duration: f32,
    /// Accumulator for fixed timestep simulation.
    pub accumulator: f32,
    /// Whether we have successfully synced with the server.
    pub synced: bool,
    /// Number of Ping/Pong samples collected.
    pub sample_count: u32,
}

impl Default for TickSync {
    fn default() -> Self {
        Self {
            rtt_seconds: 0.0,
            tick_offset: 0,
            tick_duration: 1.0 / 30.0,
            accumulator: 0.0,
            synced: false,
            sample_count: 0,
        }
    }
}

impl TickSync {
    /// Create with a specific tick rate.
    pub fn with_tick_rate(tick_rate: u32) -> Self {
        Self {
            tick_duration: 1.0 / tick_rate as f32,
            ..Default::default()
        }
    }

    /// Process a Pong response to update RTT and tick offset.
    ///
    /// - `local_tick_at_send`: the local tick when we sent the Ping
    /// - `remote_tick`: the server's tick when it processed the Ping
    /// - `current_local_tick`: current local tick
    pub fn process_pong(&mut self, local_tick_at_send: u64, remote_tick: u64, current_local_tick: u64) {
        let rtt_ticks = current_local_tick.saturating_sub(local_tick_at_send) as f32;
        let rtt = rtt_ticks * self.tick_duration;

        self.sample_count += 1;
        // Exponential moving average for smooth RTT
        if self.sample_count <= 1 {
            self.rtt_seconds = rtt;
        } else {
            self.rtt_seconds = self.rtt_seconds * 0.8 + rtt * 0.2;
        }

        // Estimate tick offset: account for half RTT
        let half_rtt_ticks = (rtt_ticks / 2.0) as i64;
        let estimated_offset = remote_tick as i64 - local_tick_at_send as i64 - half_rtt_ticks;

        if !self.synced {
            self.tick_offset = estimated_offset;
            self.synced = true;
        } else {
            // Gradual convergence
            let diff = estimated_offset - self.tick_offset;
            if diff.abs() > 10 {
                // Large jump → snap
                self.tick_offset = estimated_offset;
            } else {
                // Smooth adjustment
                self.tick_offset += diff.signum();
            }
        }
    }

    /// Convert a local tick to an estimated server tick.
    pub fn to_server_tick(&self, local_tick: u64) -> u64 {
        (local_tick as i64 + self.tick_offset).max(0) as u64
    }

    /// Accumulate frame delta time and return the number of fixed ticks to simulate.
    pub fn accumulate(&mut self, dt: f32) -> u32 {
        self.accumulator += dt;
        let max_ticks = 10u32;
        let mut ticks = 0u32;
        while self.accumulator >= self.tick_duration && ticks < max_ticks {
            self.accumulator -= self.tick_duration;
            ticks += 1;
        }
        // Prevent spiral of death: discard excess accumulated time
        if ticks == max_ticks && self.accumulator > self.tick_duration {
            self.accumulator = self.tick_duration * 0.5;
        }
        ticks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_sync_rtt() {
        let mut sync = TickSync::with_tick_rate(30);

        // Simulate: sent at local tick 100, server responded at tick 102,
        // we received at local tick 103 (RTT = 3 ticks)
        sync.process_pong(100, 102, 103);

        // RTT should be ~3 ticks * (1/30) = 0.1 seconds
        assert!((sync.rtt_seconds - 0.1).abs() < 0.01);
        assert!(sync.synced);
    }

    #[test]
    fn test_fixed_timestep() {
        let mut sync = TickSync::with_tick_rate(30);

        // dt = 1/60 (60fps) → need ~0.5 ticks, accumulator builds up
        let ticks1 = sync.accumulate(1.0 / 60.0);
        assert_eq!(ticks1, 0); // not enough yet

        let ticks2 = sync.accumulate(1.0 / 60.0);
        assert_eq!(ticks2, 1); // now we have enough for 1 tick

        // Large dt → multiple ticks (0.1s at 30Hz ≈ 3 ticks, but accumulator
        // has remainder from previous calls, so 2 or 3 depending on float precision)
        let ticks3 = sync.accumulate(0.1); // ~3 ticks at 30Hz
        assert!(ticks3 >= 2 && ticks3 <= 3, "expected 2 or 3, got {}", ticks3);
    }

    #[test]
    fn test_fixed_timestep_cap() {
        let mut sync = TickSync::with_tick_rate(30);

        // Huge dt → capped at 10 ticks
        let ticks = sync.accumulate(1.0); // 30 ticks worth
        assert_eq!(ticks, 10);
    }

    #[test]
    fn test_tick_conversion() {
        let mut sync = TickSync::default();
        sync.tick_offset = 5;
        sync.synced = true;

        assert_eq!(sync.to_server_tick(100), 105);
        assert_eq!(sync.to_server_tick(0), 5);
    }
}
