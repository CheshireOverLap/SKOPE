//! Audio Components
//!
//! ECS components for audio sources and listeners

use bevy_ecs::prelude::*;

// ============ Audio Components ============

/// Audio source component - attach to entities that emit sound
#[derive(Component, Clone)]
pub struct AudioSource {
    /// Name of the registered sound
    pub sound_name: String,
    /// Volume multiplier (0.0 - 1.0)
    pub volume: f32,
    /// Loop the sound
    pub looping: bool,
    /// Enable 3D spatial audio
    pub spatial: bool,
    /// Play immediately when spawned
    pub play_on_start: bool,
    /// Currently playing sound ID (None if not playing)
    pub playing_id: Option<u64>,
    /// Maximum audible distance (for spatial audio)
    pub max_distance: f32,
    /// Rolloff factor (for spatial audio, 1.0 = realistic)
    pub rolloff_factor: f32,
}

impl AudioSource {
    pub fn new(sound_name: &str) -> Self {
        Self {
            sound_name: sound_name.to_string(),
            volume: 1.0,
            looping: false,
            spatial: true,
            play_on_start: false,
            playing_id: None,
            max_distance: 50.0,
            rolloff_factor: 1.0,
        }
    }

    /// Create a 2D (non-spatial) sound source
    pub fn new_2d(sound_name: &str) -> Self {
        Self {
            spatial: false,
            ..Self::new(sound_name)
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume;
        self
    }

    pub fn with_loop(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn with_max_distance(mut self, distance: f32) -> Self {
        self.max_distance = distance;
        self
    }

    pub fn with_rolloff(mut self, factor: f32) -> Self {
        self.rolloff_factor = factor;
        self
    }

    pub fn play_on_start(mut self) -> Self {
        self.play_on_start = true;
        self
    }

    /// Check if this source is currently playing
    pub fn is_playing(&self) -> bool {
        self.playing_id.is_some()
    }
}

/// Audio listener component - attach to camera/player
/// Only one active listener should exist in the world
#[derive(Component, Clone)]
pub struct AudioListener {
    /// Whether this listener is active
    pub enabled: bool,
    /// Cached position (updated by audio system)
    pub position: [f32; 3],
    /// Cached forward direction (updated by audio system)
    pub forward: [f32; 3],
    /// Cached up direction (updated by audio system)
    pub up: [f32; 3],
}

impl Default for AudioListener {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioListener {
    pub fn new() -> Self {
        Self {
            enabled: true,
            position: [0.0, 0.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
        }
    }

    /// Update listener state from transform
    pub fn update_from_transform(&mut self, position: glam::Vec3, rotation: glam::Quat) {
        self.position = [position.x, position.y, position.z];

        // Calculate forward and up vectors from quaternion (Z-up coordinate system)
        let forward = rotation * glam::Vec3::NEG_Y;  // Z-up: forward is -Y
        let up = rotation * glam::Vec3::Z;           // Z-up: up is +Z

        self.forward = [forward.x, forward.y, forward.z];
        self.up = [up.x, up.y, up.z];
    }
}
