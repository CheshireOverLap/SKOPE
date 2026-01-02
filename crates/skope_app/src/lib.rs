//! SKOPE Application Framework
//!
//! Configuration, input handling, and game loop types.

#![allow(dead_code)]

use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Window title
    pub title: String,
    /// Initial window width
    pub width: u32,
    /// Initial window height
    pub height: u32,
    /// Fullscreen mode
    pub fullscreen: bool,
    /// VSync enabled
    pub vsync: bool,
    /// Target FPS (0 = unlimited)
    pub target_fps: u32,
    /// Fixed timestep for physics (seconds)
    pub fixed_timestep: f32,
    /// Maximum delta time (prevents spiral of death)
    pub max_delta: f32,
    /// Enable debug overlays
    pub debug_mode: bool,
    /// Editor mode (vs game mode)
    pub editor_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "SKOPE Engine".to_string(),
            width: 1920,
            height: 1080,
            fullscreen: false,
            vsync: true,
            target_fps: 0,
            fixed_timestep: 1.0 / 60.0,
            max_delta: 0.25,
            debug_mode: true,
            editor_mode: true,
        }
    }
}

/// Graphics quality preset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QualityPreset {
    Low,
    #[default]
    Medium,
    High,
    Ultra,
    Custom,
}

/// Graphics settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicsSettings {
    pub preset: QualityPreset,
    /// Render scale (0.5 = half resolution)
    pub render_scale: f32,
    /// Shadow quality (0-4)
    pub shadow_quality: u32,
    /// Anti-aliasing samples (1, 2, 4, 8)
    pub msaa_samples: u32,
    /// Texture quality (0-3)
    pub texture_quality: u32,
    /// Enable bloom
    pub bloom_enabled: bool,
    /// Enable ambient occlusion
    pub ao_enabled: bool,
    /// Enable motion blur
    pub motion_blur_enabled: bool,
    /// Enable depth of field
    pub dof_enabled: bool,
    /// Max lights per cluster
    pub max_lights: u32,
}

impl Default for GraphicsSettings {
    fn default() -> Self {
        Self::medium()
    }
}

impl GraphicsSettings {
    pub fn low() -> Self {
        Self {
            preset: QualityPreset::Low,
            render_scale: 0.75,
            shadow_quality: 1,
            msaa_samples: 1,
            texture_quality: 1,
            bloom_enabled: false,
            ao_enabled: false,
            motion_blur_enabled: false,
            dof_enabled: false,
            max_lights: 256,
        }
    }

    pub fn medium() -> Self {
        Self {
            preset: QualityPreset::Medium,
            render_scale: 1.0,
            shadow_quality: 2,
            msaa_samples: 2,
            texture_quality: 2,
            bloom_enabled: true,
            ao_enabled: true,
            motion_blur_enabled: false,
            dof_enabled: false,
            max_lights: 512,
        }
    }

    pub fn high() -> Self {
        Self {
            preset: QualityPreset::High,
            render_scale: 1.0,
            shadow_quality: 3,
            msaa_samples: 4,
            texture_quality: 3,
            bloom_enabled: true,
            ao_enabled: true,
            motion_blur_enabled: true,
            dof_enabled: true,
            max_lights: 1024,
        }
    }

    pub fn ultra() -> Self {
        Self {
            preset: QualityPreset::Ultra,
            render_scale: 1.0,
            shadow_quality: 4,
            msaa_samples: 8,
            texture_quality: 3,
            bloom_enabled: true,
            ao_enabled: true,
            motion_blur_enabled: true,
            dof_enabled: true,
            max_lights: 2048,
        }
    }
}

/// Key code (simplified from winit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Escape, Tab, CapsLock, Shift, Control, Alt, Space,
    Enter, Backspace, Delete, Insert, Home, End, PageUp, PageDown,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4,
    Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadEnter,
    Grave, Minus, Equal, BracketLeft, BracketRight,
    Semicolon, Quote, Backslash, Comma, Period, Slash,
}

/// Mouse button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

/// Input state for current frame
#[derive(Debug, Clone, Default)]
pub struct InputState {
    /// Currently pressed keys
    pub keys_pressed: HashSet<KeyCode>,
    /// Keys that were just pressed this frame
    pub keys_just_pressed: HashSet<KeyCode>,
    /// Keys that were just released this frame
    pub keys_just_released: HashSet<KeyCode>,
    /// Mouse buttons pressed
    pub mouse_pressed: HashSet<MouseButton>,
    /// Mouse buttons just pressed
    pub mouse_just_pressed: HashSet<MouseButton>,
    /// Mouse buttons just released
    pub mouse_just_released: HashSet<MouseButton>,
    /// Mouse position in screen coordinates
    pub mouse_position: Vec2,
    /// Mouse delta since last frame
    pub mouse_delta: Vec2,
    /// Scroll wheel delta
    pub scroll_delta: Vec2,
    /// Whether mouse is captured (hidden and locked)
    pub mouse_captured: bool,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a key is currently held down
    pub fn key(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    /// Check if a key was just pressed this frame
    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.contains(&key)
    }

    /// Check if a key was just released this frame
    pub fn key_just_released(&self, key: KeyCode) -> bool {
        self.keys_just_released.contains(&key)
    }

    /// Check if a mouse button is currently held down
    pub fn mouse(&self, button: MouseButton) -> bool {
        self.mouse_pressed.contains(&button)
    }

    /// Check if a mouse button was just pressed
    pub fn mouse_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_just_pressed.contains(&button)
    }

    /// Check if a mouse button was just released
    pub fn mouse_just_released(&self, button: MouseButton) -> bool {
        self.mouse_just_released.contains(&button)
    }

    /// Clear frame-specific state (call at start of frame)
    pub fn begin_frame(&mut self) {
        self.keys_just_pressed.clear();
        self.keys_just_released.clear();
        self.mouse_just_pressed.clear();
        self.mouse_just_released.clear();
        self.mouse_delta = Vec2::ZERO;
        self.scroll_delta = Vec2::ZERO;
    }

    /// Handle key press event
    pub fn on_key_press(&mut self, key: KeyCode) {
        if !self.keys_pressed.contains(&key) {
            self.keys_just_pressed.insert(key);
        }
        self.keys_pressed.insert(key);
    }

    /// Handle key release event
    pub fn on_key_release(&mut self, key: KeyCode) {
        if self.keys_pressed.contains(&key) {
            self.keys_just_released.insert(key);
        }
        self.keys_pressed.remove(&key);
    }

    /// Handle mouse press event
    pub fn on_mouse_press(&mut self, button: MouseButton) {
        if !self.mouse_pressed.contains(&button) {
            self.mouse_just_pressed.insert(button);
        }
        self.mouse_pressed.insert(button);
    }

    /// Handle mouse release event
    pub fn on_mouse_release(&mut self, button: MouseButton) {
        if self.mouse_pressed.contains(&button) {
            self.mouse_just_released.insert(button);
        }
        self.mouse_pressed.remove(&button);
    }

    /// Handle mouse move event
    pub fn on_mouse_move(&mut self, position: Vec2, delta: Vec2) {
        self.mouse_position = position;
        self.mouse_delta += delta;
    }

    /// Handle scroll event
    pub fn on_scroll(&mut self, delta: Vec2) {
        self.scroll_delta += delta;
    }
}

/// Time information for current frame
#[derive(Debug, Clone)]
pub struct Time {
    /// Time since application start
    pub elapsed: Duration,
    /// Delta time since last frame (capped)
    pub delta: Duration,
    /// Raw delta time (uncapped)
    pub delta_raw: Duration,
    /// Fixed timestep delta (for physics)
    pub fixed_delta: Duration,
    /// Accumulated time for fixed updates
    pub accumulator: Duration,
    /// Current frame number
    pub frame: u64,
    /// Frames per second (smoothed)
    pub fps: f32,
    /// Time scale (1.0 = normal, 0.5 = half speed)
    pub time_scale: f32,
    // Internal
    last_instant: Instant,
    fps_samples: Vec<f32>,
}

impl Default for Time {
    fn default() -> Self {
        Self::new()
    }
}

impl Time {
    pub fn new() -> Self {
        Self {
            elapsed: Duration::ZERO,
            delta: Duration::from_secs_f32(1.0 / 60.0),
            delta_raw: Duration::from_secs_f32(1.0 / 60.0),
            fixed_delta: Duration::from_secs_f32(1.0 / 60.0),
            accumulator: Duration::ZERO,
            frame: 0,
            fps: 60.0,
            time_scale: 1.0,
            last_instant: Instant::now(),
            fps_samples: Vec::with_capacity(60),
        }
    }

    /// Update time at start of frame
    pub fn update(&mut self, max_delta: Duration) {
        let now = Instant::now();
        self.delta_raw = now - self.last_instant;
        self.last_instant = now;

        // Cap delta to prevent spiral of death
        self.delta = self.delta_raw.min(max_delta);
        self.delta = Duration::from_secs_f32(self.delta.as_secs_f32() * self.time_scale);

        self.elapsed += self.delta;
        self.accumulator += self.delta;
        self.frame += 1;

        // Update FPS (exponential moving average)
        let instant_fps = 1.0 / self.delta_raw.as_secs_f32().max(0.001);
        self.fps_samples.push(instant_fps);
        if self.fps_samples.len() > 60 {
            self.fps_samples.remove(0);
        }
        self.fps = self.fps_samples.iter().sum::<f32>() / self.fps_samples.len() as f32;
    }

    /// Check if fixed update should run, and consume accumulator
    pub fn should_fixed_update(&mut self) -> bool {
        if self.accumulator >= self.fixed_delta {
            self.accumulator -= self.fixed_delta;
            true
        } else {
            false
        }
    }

    /// Get delta as seconds (f32)
    pub fn delta_secs(&self) -> f32 {
        self.delta.as_secs_f32()
    }

    /// Get elapsed as seconds (f32)
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed.as_secs_f32()
    }

    /// Get fixed delta as seconds (f32)
    pub fn fixed_delta_secs(&self) -> f32 {
        self.fixed_delta.as_secs_f32()
    }
}

/// Application running mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppMode {
    /// Editor mode - scene editing, gizmos, panels
    #[default]
    Editor,
    /// Play mode - game running in editor
    Play,
    /// Pause mode - game paused in editor
    Pause,
    /// Standalone game mode - no editor UI
    Game,
}

/// Window state
#[derive(Debug, Clone)]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub focused: bool,
    pub minimized: bool,
    pub fullscreen: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
            focused: true,
            minimized: false,
            fullscreen: false,
        }
    }
}

impl WindowState {
    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height.max(1) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state() {
        let mut input = InputState::new();

        input.on_key_press(KeyCode::W);
        assert!(input.key(KeyCode::W));
        assert!(input.key_just_pressed(KeyCode::W));

        input.begin_frame();
        assert!(input.key(KeyCode::W));
        assert!(!input.key_just_pressed(KeyCode::W));

        input.on_key_release(KeyCode::W);
        assert!(!input.key(KeyCode::W));
        assert!(input.key_just_released(KeyCode::W));
    }

    #[test]
    fn test_time() {
        let mut time = Time::new();
        time.update(Duration::from_secs_f32(0.25));
        assert!(time.delta_secs() > 0.0);
        assert!(time.frame == 1);
    }

    #[test]
    fn test_graphics_presets() {
        let low = GraphicsSettings::low();
        let high = GraphicsSettings::high();

        assert!(low.msaa_samples < high.msaa_samples);
        assert!(low.max_lights < high.max_lights);
    }
}
