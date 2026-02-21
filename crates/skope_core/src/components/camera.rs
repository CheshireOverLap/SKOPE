//! Camera Components for SKOPE Engine
//!
//! Camera, CameraController, SpringArm, CameraShake
//! Ported from UE5 SpringArmComponent + CameraShakePattern

use skope_ecs::prelude::*;
use glam::{Vec3, Quat};
use serde::{Serialize, Deserialize};

/// Camera component
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    #[serde(default = "crate::default_true")]
    pub is_active: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov: 45.0_f32.to_radians(),
            near: 0.1,
            far: 100.0,
            is_active: true,
        }
    }
}

/// FPS-style camera controller
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct CameraController {
    pub yaw: f32,
    pub pitch: f32,
    pub move_speed: f32,
    pub sensitivity: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.3,
            move_speed: 5.0,
            sensitivity: 0.003,
        }
    }
}

// ============ Spring Arm (UE5 SpringArmComponent) ============

/// UE5-style Spring Arm component.
///
/// Attaches to a target entity (typically the player) and positions the camera
/// at `target_arm_length` behind, with collision sweep to prevent clipping
/// and optional position/rotation lag for smooth following.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct SpringArm {
    // --- Core geometry ---
    /// Natural arm length (distance from pivot to camera), in world units
    #[serde(default = "default_arm_length")]
    pub target_arm_length: f32,
    /// Additive offset at the arm END (camera position), in local space
    #[serde(default, with = "crate::vec3_serde")]
    pub socket_offset: Vec3,
    /// Additive offset at the arm ORIGIN (pivot point), in world space.
    /// Default: (0, 0, 2) — camera pivots from player's head height.
    #[serde(default = "default_target_offset", with = "crate::vec3_serde")]
    pub target_offset: Vec3,

    // --- Collision probe ---
    /// Sphere radius for collision sweep
    #[serde(default = "default_probe_size")]
    pub probe_size: f32,
    /// Whether to perform collision testing
    #[serde(default = "crate::default_true")]
    pub do_collision_test: bool,

    // --- Position lag ---
    /// Enable smooth position following
    #[serde(default)]
    pub enable_camera_lag: bool,
    /// Position interpolation speed (0=frozen, higher=snappier)
    #[serde(default = "default_lag_speed")]
    pub camera_lag_speed: f32,
    /// Maximum lag distance from target (0=unlimited)
    #[serde(default)]
    pub camera_lag_max_distance: f32,

    // --- Rotation lag ---
    /// Enable smooth rotation following
    #[serde(default)]
    pub enable_camera_rotation_lag: bool,
    /// Rotation interpolation speed
    #[serde(default = "default_lag_speed")]
    pub camera_rotation_lag_speed: f32,

    // --- Substepping ---
    /// Use fixed substeps for lag (prevents jerky motion at low FPS)
    #[serde(default = "crate::default_true")]
    pub use_lag_substepping: bool,
    /// Max time step for substepping (seconds)
    #[serde(default = "default_lag_max_timestep")]
    pub lag_max_time_step: f32,

    // --- Runtime state (not serialized) ---
    /// Previous frame desired location (for position lag)
    #[serde(skip)]
    pub previous_desired_loc: Vec3,
    /// Previous frame arm origin
    #[serde(skip)]
    pub previous_arm_origin: Vec3,
    /// Previous frame desired rotation (for rotation lag)
    #[serde(skip)]
    pub previous_desired_rot: Quat,
    /// Whether the arm was retracted by collision this frame
    #[serde(skip)]
    pub is_camera_fixed: bool,
    /// Camera position without collision adjustment
    #[serde(skip)]
    pub unfixed_camera_position: Vec3,
    /// Whether this is the first frame (skip lag on first update)
    #[serde(skip, default = "crate::default_true")]
    pub first_update: bool,
}

fn default_arm_length() -> f32 { 5.0 }
fn default_probe_size() -> f32 { 0.2 }
fn default_lag_speed() -> f32 { 10.0 }
fn default_lag_max_timestep() -> f32 { 1.0 / 60.0 }
fn default_target_offset() -> Vec3 { Vec3::new(0.0, 0.0, 2.0) }

impl Default for SpringArm {
    fn default() -> Self {
        Self {
            target_arm_length: 5.0,
            socket_offset: Vec3::ZERO,
            target_offset: Vec3::new(0.0, 0.0, 2.0), // height offset
            probe_size: 0.2,
            do_collision_test: true,
            enable_camera_lag: false,
            camera_lag_speed: 10.0,
            camera_lag_max_distance: 0.0,
            enable_camera_rotation_lag: false,
            camera_rotation_lag_speed: 10.0,
            use_lag_substepping: true,
            lag_max_time_step: 1.0 / 60.0,
            previous_desired_loc: Vec3::ZERO,
            previous_arm_origin: Vec3::ZERO,
            previous_desired_rot: Quat::IDENTITY,
            is_camera_fixed: false,
            unfixed_camera_position: Vec3::ZERO,
            first_update: true,
        }
    }
}

// ============ Camera Shake (UE5 CameraShakePattern) ============

/// Type of camera shake pattern
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CameraShakePatternType {
    /// Sine-wave based oscillation (UE5 WaveOscillatorCameraShakePattern)
    WaveOscillator,
    /// Perlin noise based shake (UE5 PerlinNoiseCameraShakePattern)
    PerlinNoise,
}

impl Default for CameraShakePatternType {
    fn default() -> Self { Self::WaveOscillator }
}

/// Per-axis oscillator configuration (used by both Wave and Perlin)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ShakeOscillator {
    /// Amplitude of oscillation
    pub amplitude: f32,
    /// Frequency in Hz
    pub frequency: f32,
}

impl Default for ShakeOscillator {
    fn default() -> Self {
        Self { amplitude: 1.0, frequency: 1.0 }
    }
}

impl ShakeOscillator {
    pub fn new(amplitude: f32, frequency: f32) -> Self {
        Self { amplitude, frequency }
    }

    pub fn off() -> Self {
        Self { amplitude: 0.0, frequency: 1.0 }
    }
}

/// Camera shake definition (blueprint-like data).
///
/// Describes the shake pattern. Actual playback state is in `CameraShakeInstance`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraShakeDef {
    /// Pattern type
    pub pattern: CameraShakePatternType,
    /// Duration in seconds (<=0 means infinite, must be stopped manually)
    pub duration: f32,
    /// Blend-in time (seconds)
    pub blend_in: f32,
    /// Blend-out time (seconds)
    pub blend_out: f32,

    // --- Location axes ---
    pub loc_x: ShakeOscillator,
    pub loc_y: ShakeOscillator,
    pub loc_z: ShakeOscillator,
    /// Multiplier for all location amplitudes
    pub location_amplitude_multiplier: f32,
    /// Multiplier for all location frequencies
    pub location_frequency_multiplier: f32,

    // --- Rotation axes ---
    pub rot_pitch: ShakeOscillator,
    pub rot_yaw: ShakeOscillator,
    pub rot_roll: ShakeOscillator,
    /// Multiplier for all rotation amplitudes (0 = rotation disabled)
    pub rotation_amplitude_multiplier: f32,
    /// Multiplier for all rotation frequencies
    pub rotation_frequency_multiplier: f32,

    // --- FOV ---
    pub fov: ShakeOscillator,
}

impl Default for CameraShakeDef {
    fn default() -> Self {
        Self {
            pattern: CameraShakePatternType::WaveOscillator,
            duration: 1.0,
            blend_in: 0.2,
            blend_out: 0.2,
            loc_x: ShakeOscillator::default(),
            loc_y: ShakeOscillator::default(),
            loc_z: ShakeOscillator::default(),
            location_amplitude_multiplier: 1.0,
            location_frequency_multiplier: 1.0,
            rot_pitch: ShakeOscillator::default(),
            rot_yaw: ShakeOscillator::default(),
            rot_roll: ShakeOscillator::default(),
            rotation_amplitude_multiplier: 0.0, // UE5 default: rotation OFF
            rotation_frequency_multiplier: 1.0,
            fov: ShakeOscillator::off(),
        }
    }
}

/// Runtime state for an active camera shake instance.
#[derive(Debug, Clone)]
pub struct CameraShakeInstance {
    /// The shake definition
    pub def: CameraShakeDef,
    /// Overall scale applied to the shake output
    pub shake_scale: f32,
    /// Elapsed time since shake started
    pub elapsed: f32,
    /// Current blend-in progress (seconds)
    pub blend_in_elapsed: f32,
    /// Current blend-out progress (seconds)
    pub blend_out_elapsed: f32,
    /// Whether currently blending in
    pub is_blending_in: bool,
    /// Whether currently blending out
    pub is_blending_out: bool,
    /// Whether the shake is still active
    pub is_playing: bool,

    // --- Per-axis phase offsets (WaveOscillator: radians, PerlinNoise: time seed) ---
    pub loc_offset: Vec3,
    pub rot_offset: Vec3,
    pub fov_offset: f32,
}

impl CameraShakeInstance {
    /// Create a new shake instance from a definition
    pub fn new(def: CameraShakeDef, scale: f32) -> Self {
        let is_wave = def.pattern == CameraShakePatternType::WaveOscillator;
        let random_phase = || -> f32 {
            if is_wave {
                // Random initial phase [0, 2π)
                rand_f32() * std::f32::consts::TAU
            } else {
                // Random seed offset [0, 255) for Perlin
                (rand_f32() * 255.0).floor()
            }
        };

        Self {
            shake_scale: scale,
            elapsed: 0.0,
            blend_in_elapsed: 0.0,
            blend_out_elapsed: 0.0,
            is_blending_in: def.blend_in > 0.0,
            is_blending_out: false,
            is_playing: true,
            loc_offset: Vec3::new(random_phase(), random_phase(), random_phase()),
            rot_offset: Vec3::new(random_phase(), random_phase(), random_phase()),
            fov_offset: random_phase(),
            def,
        }
    }

    /// Update timing and return the current blend weight (0..1)
    pub fn update_timing(&mut self, dt: f32) -> f32 {
        if !self.is_playing {
            return 0.0;
        }

        self.elapsed += dt;
        if self.is_blending_in {
            self.blend_in_elapsed += dt;
        }
        if self.is_blending_out {
            self.blend_out_elapsed += dt;
        }

        // Check if finite duration has expired
        if self.def.duration > 0.0 && self.elapsed >= self.def.duration {
            self.is_playing = false;
            return 0.0;
        }

        // Trigger blend-out near end of finite duration.
        // UE5: blend-in and blend-out can overlap — both remain active and
        // their weights multiply. Do NOT force blend-in to stop here.
        if self.def.duration > 0.0 && self.def.blend_out > 0.0 && !self.is_blending_out {
            let remaining = self.def.duration - self.elapsed;
            if remaining < self.def.blend_out {
                self.is_blending_out = true;
                self.blend_out_elapsed = self.def.blend_out - remaining;
            }
        }

        // Compute blend weight
        let mut weight = 1.0;

        if self.is_blending_in {
            if self.blend_in_elapsed < self.def.blend_in {
                weight *= self.blend_in_elapsed / self.def.blend_in;
            } else {
                self.is_blending_in = false;
            }
        }

        if self.is_blending_out {
            if self.blend_out_elapsed < self.def.blend_out {
                weight *= 1.0 - self.blend_out_elapsed / self.def.blend_out;
            } else {
                self.is_playing = false;
                return 0.0;
            }
        }

        weight
    }

    /// Compute the shake result for this frame.
    /// Returns (location_offset, rotation_offset_degrees, fov_offset).
    pub fn compute(&mut self, dt: f32) -> (Vec3, Vec3, f32) {
        let blend_weight = self.update_timing(dt);
        if blend_weight <= 0.0 {
            return (Vec3::ZERO, Vec3::ZERO, 0.0);
        }

        let total_scale = (self.shake_scale * blend_weight).max(0.0);

        match self.def.pattern {
            CameraShakePatternType::WaveOscillator => {
                self.compute_wave(dt, total_scale)
            }
            CameraShakePatternType::PerlinNoise => {
                self.compute_perlin(dt, total_scale)
            }
        }
    }

    fn compute_wave(&mut self, dt: f32, scale: f32) -> (Vec3, Vec3, f32) {
        let loc_amp = self.def.location_amplitude_multiplier;
        let loc_freq = self.def.location_frequency_multiplier;
        let rot_amp = self.def.rotation_amplitude_multiplier;
        let rot_freq = self.def.rotation_frequency_multiplier;

        // Advance phase (radians) and compute sin
        let loc = Vec3::new(
            wave_update(&self.def.loc_x, dt, loc_amp, loc_freq, &mut self.loc_offset.x),
            wave_update(&self.def.loc_y, dt, loc_amp, loc_freq, &mut self.loc_offset.y),
            wave_update(&self.def.loc_z, dt, loc_amp, loc_freq, &mut self.loc_offset.z),
        );

        let rot = Vec3::new(
            wave_update(&self.def.rot_pitch, dt, rot_amp, rot_freq, &mut self.rot_offset.x),
            wave_update(&self.def.rot_yaw, dt, rot_amp, rot_freq, &mut self.rot_offset.y),
            wave_update(&self.def.rot_roll, dt, rot_amp, rot_freq, &mut self.rot_offset.z),
        );

        let fov = wave_update(&self.def.fov, dt, 1.0, 1.0, &mut self.fov_offset);

        (loc * scale, rot * scale, fov * scale)
    }

    fn compute_perlin(&mut self, dt: f32, scale: f32) -> (Vec3, Vec3, f32) {
        let loc_amp = self.def.location_amplitude_multiplier;
        let loc_freq = self.def.location_frequency_multiplier;
        let rot_amp = self.def.rotation_amplitude_multiplier;
        let rot_freq = self.def.rotation_frequency_multiplier;

        let loc = Vec3::new(
            perlin_update(&self.def.loc_x, dt, loc_amp, loc_freq, &mut self.loc_offset.x),
            perlin_update(&self.def.loc_y, dt, loc_amp, loc_freq, &mut self.loc_offset.y),
            perlin_update(&self.def.loc_z, dt, loc_amp, loc_freq, &mut self.loc_offset.z),
        );

        let rot = Vec3::new(
            perlin_update(&self.def.rot_pitch, dt, rot_amp, rot_freq, &mut self.rot_offset.x),
            perlin_update(&self.def.rot_yaw, dt, rot_amp, rot_freq, &mut self.rot_offset.y),
            perlin_update(&self.def.rot_roll, dt, rot_amp, rot_freq, &mut self.rot_offset.z),
        );

        let fov = perlin_update(&self.def.fov, dt, 1.0, 1.0, &mut self.fov_offset);

        (loc * scale, rot * scale, fov * scale)
    }
}

/// Wave oscillator update: advance phase (radians), return amplitude * sin(phase)
fn wave_update(osc: &ShakeOscillator, dt: f32, amp_mul: f32, freq_mul: f32, offset: &mut f32) -> f32 {
    let total_amp = osc.amplitude * amp_mul;
    if total_amp.abs() < f32::EPSILON { return 0.0; }
    *offset += dt * osc.frequency * freq_mul * std::f32::consts::TAU;
    total_amp * offset.sin()
}

/// Perlin noise update: advance time coordinate (no TAU factor), return amplitude * noise(t)
fn perlin_update(osc: &ShakeOscillator, dt: f32, amp_mul: f32, freq_mul: f32, offset: &mut f32) -> f32 {
    let total_amp = osc.amplitude * amp_mul;
    if total_amp.abs() < f32::EPSILON { return 0.0; }
    *offset += dt * osc.frequency * freq_mul;
    total_amp * perlin_noise_1d(*offset)
}

/// Simple 1D Perlin-like noise using sine harmonics (deterministic, no external dependency).
/// Maps input t to range [-1, 1].
fn perlin_noise_1d(t: f32) -> f32 {
    // Approximate Perlin noise with layered sines (good enough for camera shake)
    let v = (t * 1.0).sin() * 0.5
          + (t * 2.3 + 1.7).sin() * 0.25
          + (t * 4.7 + 3.1).sin() * 0.125
          + (t * 8.1 + 5.3).sin() * 0.0625;
    (v / 0.9375).clamp(-1.0, 1.0) // normalize to [-1, 1]
}

/// Simple pseudo-random f32 in [0, 1) using a global atomic counter
fn rand_f32() -> f32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(12345);
    let s = SEED.fetch_add(1, Ordering::Relaxed);
    // Simple hash (splitmix32-like)
    let mut x = s.wrapping_mul(0x9E3779B9).wrapping_add(0x6A09E667);
    x ^= x >> 16;
    x = x.wrapping_mul(0x45D9F3B);
    x ^= x >> 16;
    (x & 0x00FFFFFF) as f32 / 16777216.0
}

// ============ View Target Blending ============

/// Blend function for view target transitions (UE5 EViewTargetBlendFunction)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ViewBlendFunction {
    /// Linear interpolation
    Linear,
    /// Cubic ease-in-out (3t² - 2t³)
    Cubic,
    /// Ease in: t^exp
    EaseIn,
    /// Ease out: t^(1/exp)
    EaseOut,
    /// Ease in-out with exponent
    EaseInOut,
}

impl Default for ViewBlendFunction {
    fn default() -> Self { Self::Cubic }
}

/// Parameters for blending between view targets (UE5 FViewTargetTransitionParams)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ViewBlendParams {
    /// Blend duration in seconds
    pub blend_time: f32,
    /// Blend curve function
    pub blend_function: ViewBlendFunction,
    /// Exponent for EaseIn/EaseOut/EaseInOut
    pub blend_exp: f32,
}

impl Default for ViewBlendParams {
    fn default() -> Self {
        Self {
            blend_time: 0.5,
            blend_function: ViewBlendFunction::Cubic,
            blend_exp: 2.0,
        }
    }
}

impl ViewBlendParams {
    /// Evaluate the blend curve at time_pct [0..1] → [0..1]
    pub fn evaluate(&self, time_pct: f32) -> f32 {
        let t = time_pct.clamp(0.0, 1.0);
        match self.blend_function {
            ViewBlendFunction::Linear => t,
            ViewBlendFunction::Cubic => {
                // Hermite cubic: 3t² - 2t³ (smooth step)
                t * t * (3.0 - 2.0 * t)
            }
            ViewBlendFunction::EaseIn => {
                t.powf(self.blend_exp)
            }
            ViewBlendFunction::EaseOut => {
                t.powf(1.0 / self.blend_exp.max(0.001))
            }
            ViewBlendFunction::EaseInOut => {
                if t < 0.5 {
                    0.5 * (2.0 * t).powf(self.blend_exp)
                } else {
                    1.0 - 0.5 * (2.0 * (1.0 - t)).powf(self.blend_exp)
                }
            }
        }
    }
}
