//! Audio Settings
//!
//! Play and spatial settings for audio playback

// ============ Play Settings ============

#[derive(Clone, Debug)]
pub struct PlaySettings {
    pub volume: f32,
    pub looping: bool,
    pub is_music: bool,
}

impl Default for PlaySettings {
    fn default() -> Self {
        Self::sfx()
    }
}

impl PlaySettings {
    pub fn music() -> Self {
        Self {
            volume: 1.0,
            looping: true,
            is_music: true,
        }
    }

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

// ============ Spatial Settings ============

/// Settings for 3D spatial audio playback
#[derive(Clone, Debug)]
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
    pub fn at_vec3(pos: glam::Vec3) -> Self {
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
