//! Network interpolation for remote (SimulatedProxy) entities.
//!
//! Stores two snapshots (from/to) and interpolates between them
//! for smooth visual representation of remote entities.

use glam::{Vec3, Quat};
use skope_ecs::prelude::*;

/// Interpolation state for a remote entity.
/// Attached to SimulatedProxy entities that receive periodic state updates.
#[derive(Component, Debug, Clone)]
pub struct NetInterpolation {
    pub from_translation: Vec3,
    pub to_translation: Vec3,
    pub from_rotation: Quat,
    pub to_rotation: Quat,
    /// Interpolation parameter [0.0, 1.0].
    pub t: f32,
    /// Duration in ticks between the two snapshots.
    pub duration_ticks: f32,
}

impl Default for NetInterpolation {
    fn default() -> Self {
        Self {
            from_translation: Vec3::ZERO,
            to_translation: Vec3::ZERO,
            from_rotation: Quat::IDENTITY,
            to_rotation: Quat::IDENTITY,
            t: 1.0, // fully arrived
            duration_ticks: 1.0,
        }
    }
}

impl NetInterpolation {
    /// Update interpolation targets when a new server snapshot arrives.
    pub fn update_targets(&mut self, new_translation: Vec3, new_rotation: Quat, ticks_between: f32) {
        self.from_translation = self.current_translation();
        self.from_rotation = self.current_rotation();
        self.to_translation = new_translation;
        self.to_rotation = new_rotation;
        self.t = 0.0;
        self.duration_ticks = ticks_between.max(1.0);
    }

    /// Advance interpolation by dt (in seconds) at the given tick rate.
    pub fn advance(&mut self, dt: f32, tick_rate: f32) {
        if self.t >= 1.0 {
            return;
        }
        let ticks_per_second = tick_rate;
        let duration_seconds = self.duration_ticks / ticks_per_second;
        if duration_seconds > 0.0 {
            self.t += dt / duration_seconds;
            self.t = self.t.min(1.0);
        } else {
            self.t = 1.0;
        }
    }

    /// Get current interpolated translation.
    pub fn current_translation(&self) -> Vec3 {
        self.from_translation.lerp(self.to_translation, self.t)
    }

    /// Get current interpolated rotation (spherical lerp).
    pub fn current_rotation(&self) -> Quat {
        self.from_rotation.slerp(self.to_rotation, self.t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolation_lerp() {
        let mut interp = NetInterpolation {
            from_translation: Vec3::ZERO,
            to_translation: Vec3::new(10.0, 0.0, 0.0),
            from_rotation: Quat::IDENTITY,
            to_rotation: Quat::IDENTITY,
            t: 0.0,
            duration_ticks: 1.0,
        };

        // At t=0 → from
        assert!((interp.current_translation() - Vec3::ZERO).length() < 0.001);

        // Set t=0.5
        interp.t = 0.5;
        let mid = interp.current_translation();
        assert!((mid.x - 5.0).abs() < 0.001);
        assert!((mid.y).abs() < 0.001);

        // At t=1 → to
        interp.t = 1.0;
        assert!((interp.current_translation() - Vec3::new(10.0, 0.0, 0.0)).length() < 0.001);
    }

    #[test]
    fn test_interpolation_advance() {
        let mut interp = NetInterpolation {
            from_translation: Vec3::ZERO,
            to_translation: Vec3::new(10.0, 0.0, 0.0),
            from_rotation: Quat::IDENTITY,
            to_rotation: Quat::IDENTITY,
            t: 0.0,
            duration_ticks: 3.0, // 3 ticks between snapshots
        };

        // At 30Hz, 3 ticks = 0.1 seconds
        // Advance by 0.05s → t should be 0.5
        interp.advance(0.05, 30.0);
        assert!((interp.t - 0.5).abs() < 0.01);

        // Another 0.05s → t should be 1.0
        interp.advance(0.05, 30.0);
        assert!((interp.t - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_update_targets() {
        let mut interp = NetInterpolation {
            from_translation: Vec3::ZERO,
            to_translation: Vec3::new(5.0, 0.0, 0.0),
            t: 0.5,
            ..Default::default()
        };

        // Current pos should be midpoint
        let current = interp.current_translation();
        assert!((current.x - 2.5).abs() < 0.01);

        // New target arrives
        interp.update_targets(Vec3::new(10.0, 0.0, 0.0), Quat::IDENTITY, 3.0);

        // from should be the old current position
        assert!((interp.from_translation.x - 2.5).abs() < 0.01);
        assert!((interp.to_translation.x - 10.0).abs() < 0.01);
        assert_eq!(interp.t, 0.0);
    }
}
