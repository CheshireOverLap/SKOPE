//! SKOPE Audio System
//!
//! Audio types and settings for cross-platform sound playback.
//!
//! # Features
//! - `backend`: Enable rodio-based audio playback

#![allow(dead_code)]

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Audio playback settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaySettings {
    /// Volume multiplier (0.0 - 1.0)
    pub volume: f32,
    /// Loop the sound
    pub looping: bool,
    /// Treat as music (uses music volume)
    pub is_music: bool,
}

impl Default for PlaySettings {
    fn default() -> Self {
        Self::sfx()
    }
}

impl PlaySettings {
    /// Settings for background music
    pub fn music() -> Self {
        Self {
            volume: 1.0,
            looping: true,
            is_music: true,
        }
    }

    /// Settings for sound effects
    pub fn sfx() -> Self {
        Self {
            volume: 1.0,
            looping: false,
            is_music: false,
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
}

/// Settings for 3D spatial audio playback
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpatialSettings {
    /// Volume multiplier (0.0 - 1.0)
    pub volume: f32,
    /// Loop the sound
    pub looping: bool,
    /// World position [x, y, z]
    pub position: [f32; 3],
    /// Maximum audible distance (sound fades to 0 at this distance)
    pub max_distance: f32,
    /// Rolloff factor (1.0 = realistic, higher = faster falloff)
    pub rolloff_factor: f32,
}

impl Default for SpatialSettings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            looping: false,
            position: [0.0, 0.0, 0.0],
            max_distance: 50.0,
            rolloff_factor: 1.0,
        }
    }
}

impl SpatialSettings {
    /// Create spatial settings at a position
    pub fn at(position: [f32; 3]) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    /// Create from glam Vec3
    pub fn at_vec3(pos: Vec3) -> Self {
        Self::at([pos.x, pos.y, pos.z])
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
}

/// Audio source configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioSourceConfig {
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
    /// Maximum audible distance (for spatial audio)
    pub max_distance: f32,
    /// Rolloff factor (for spatial audio, 1.0 = realistic)
    pub rolloff_factor: f32,
}

impl AudioSourceConfig {
    pub fn new(sound_name: &str) -> Self {
        Self {
            sound_name: sound_name.to_string(),
            volume: 1.0,
            looping: false,
            spatial: true,
            play_on_start: false,
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
}

/// Audio listener state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListenerState {
    /// Whether this listener is active
    pub enabled: bool,
    /// Cached position
    pub position: [f32; 3],
    /// Cached forward direction
    pub forward: [f32; 3],
    /// Cached up direction
    pub up: [f32; 3],
}

impl Default for ListenerState {
    fn default() -> Self {
        Self {
            enabled: true,
            position: [0.0, 0.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
        }
    }
}

impl ListenerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update listener state from transform
    pub fn update_from_transform(&mut self, position: Vec3, rotation: glam::Quat) {
        self.position = [position.x, position.y, position.z];

        // Calculate forward and up vectors from quaternion
        let forward = rotation * Vec3::NEG_Z;
        let up = rotation * Vec3::Y;

        self.forward = [forward.x, forward.y, forward.z];
        self.up = [up.x, up.y, up.z];
    }
}

/// Audio system configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioConfig {
    /// Master volume (0.0 - 1.0)
    pub master_volume: f32,
    /// Music volume (0.0 - 1.0)
    pub music_volume: f32,
    /// SFX volume (0.0 - 1.0)
    pub sfx_volume: f32,
    /// Audio enabled
    pub enabled: bool,
    /// Maximum simultaneous sounds
    pub max_sounds: usize,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 0.8,
            sfx_volume: 1.0,
            enabled: true,
            max_sounds: 64,
        }
    }
}

/// Audio error types
#[derive(Debug)]
pub enum AudioError {
    InitFailed(String),
    SoundNotFound(String),
    PlayFailed(String),
    DecodeFailed(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::InitFailed(e) => write!(f, "Audio init failed: {}", e),
            AudioError::SoundNotFound(name) => write!(f, "Sound not found: {}", name),
            AudioError::PlayFailed(e) => write!(f, "Play failed: {}", e),
            AudioError::DecodeFailed(e) => write!(f, "Decode failed: {}", e),
        }
    }
}

impl std::error::Error for AudioError {}

/// Audio channel for categorizing sounds
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AudioChannel {
    /// Master channel (affects all)
    Master,
    /// Background music
    Music,
    /// Sound effects
    SFX,
    /// Voice/dialogue
    Voice,
    /// Ambient sounds
    Ambient,
    /// UI sounds
    UI,
}

impl Default for AudioChannel {
    fn default() -> Self {
        AudioChannel::SFX
    }
}

/// Audio fade mode
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FadeMode {
    /// No fade
    None,
    /// Linear fade
    Linear { duration: f32 },
    /// Smooth (ease-in-out) fade
    Smooth { duration: f32 },
}

impl Default for FadeMode {
    fn default() -> Self {
        FadeMode::None
    }
}

/// Distance attenuation model
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AttenuationModel {
    /// No attenuation (constant volume)
    None,
    /// Inverse distance attenuation
    InverseDistance { reference_distance: f32 },
    /// Linear distance attenuation
    Linear { max_distance: f32 },
    /// Exponential distance attenuation
    Exponential { rolloff: f32 },
}

impl Default for AttenuationModel {
    fn default() -> Self {
        AttenuationModel::InverseDistance {
            reference_distance: 1.0,
        }
    }
}

impl AttenuationModel {
    /// Calculate attenuation factor based on distance
    pub fn calculate(&self, distance: f32, max_distance: f32) -> f32 {
        match self {
            AttenuationModel::None => 1.0,
            AttenuationModel::InverseDistance { reference_distance } => {
                if distance >= max_distance {
                    return 0.0;
                }
                if distance < *reference_distance {
                    return 1.0;
                }
                reference_distance / distance
            }
            AttenuationModel::Linear { max_distance: max_dist } => {
                let max_d = max_dist.min(max_distance);
                if distance >= max_d {
                    return 0.0;
                }
                1.0 - (distance / max_d)
            }
            AttenuationModel::Exponential { rolloff } => {
                if distance >= max_distance {
                    return 0.0;
                }
                (-(distance * rolloff)).exp()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_play_settings() {
        let settings = PlaySettings::music();
        assert!(settings.is_music);
        assert!(settings.looping);

        let settings = PlaySettings::sfx().with_volume(0.5);
        assert!(!settings.is_music);
        assert!(!settings.looping);
        assert_eq!(settings.volume, 0.5);
    }

    #[test]
    fn test_spatial_settings() {
        let settings = SpatialSettings::at([1.0, 2.0, 3.0])
            .with_volume(0.5)
            .with_max_distance(100.0)
            .with_rolloff(2.0);

        assert_eq!(settings.position, [1.0, 2.0, 3.0]);
        assert_eq!(settings.volume, 0.5);
        assert_eq!(settings.max_distance, 100.0);
        assert_eq!(settings.rolloff_factor, 2.0);
    }

    #[test]
    fn test_attenuation_models() {
        let inverse = AttenuationModel::InverseDistance {
            reference_distance: 1.0,
        };
        assert_eq!(inverse.calculate(0.5, 100.0), 1.0);
        assert!((inverse.calculate(2.0, 100.0) - 0.5).abs() < 0.001);

        let linear = AttenuationModel::Linear { max_distance: 10.0 };
        assert_eq!(linear.calculate(0.0, 100.0), 1.0);
        assert!((linear.calculate(5.0, 100.0) - 0.5).abs() < 0.001);
        assert_eq!(linear.calculate(10.0, 100.0), 0.0);
    }
}
