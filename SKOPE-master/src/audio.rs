// SKOPE Audio System
// Cross-platform audio engine with optional backend
// Supports 3D spatial audio with distance attenuation
#![allow(dead_code)]

mod backend;
mod components;
mod error;
pub mod pool;
mod settings;

pub use components::*;
pub use error::*;
pub use settings::*;

use backend::AudioBackend;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============ Audio System ============

/// Main audio manager resource
pub struct AudioSystem {
    backend: AudioBackend,
    sounds: HashMap<String, PathBuf>,
    playing: HashMap<u64, PlayingSound>,
    master_volume: f32,
    music_volume: f32,
    sfx_volume: f32,
    enabled: bool,
}

struct PlayingSound {
    sound_name: String,
    is_music: bool,
}

impl AudioSystem {
    /// Create new audio system
    pub fn new() -> Result<Self, AudioError> {
        let backend = AudioBackend::new()
            .map_err(AudioError::InitFailed)?;

        Ok(Self {
            backend,
            sounds: HashMap::new(),
            playing: HashMap::new(),
            master_volume: 1.0,
            music_volume: 0.8,
            sfx_volume: 1.0,
            enabled: true,
        })
    }

    /// Register a sound (path only, loaded on demand)
    pub fn register_sound(&mut self, name: &str, path: PathBuf) {
        self.sounds.insert(name.to_string(), path);
    }

    /// Load sounds from a directory
    pub fn load_sounds_from_dir(&mut self, dir: &Path) -> usize {
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    if ext == "wav" || ext == "ogg" || ext == "mp3" || ext == "flac" {
                        if let Some(stem) = path.file_stem() {
                            let name = stem.to_string_lossy().to_string();
                            self.register_sound(&name, path);
                            count += 1;
                        }
                    }
                }
            }
        }
        if count > 0 {
            log::info!("[Audio] Registered {} sounds from {:?}", count, dir);
        }
        count
    }

    /// Play a sound
    pub fn play(&mut self, name: &str) -> Result<u64, AudioError> {
        self.play_with_settings(name, PlaySettings::sfx())
    }

    /// Play a sound with settings
    pub fn play_with_settings(&mut self, name: &str, settings: PlaySettings) -> Result<u64, AudioError> {
        if !self.enabled {
            return Ok(0);
        }

        let path = self.sounds.get(name)
            .ok_or_else(|| AudioError::SoundNotFound(name.to_string()))?
            .clone();

        let volume = self.master_volume * if settings.is_music {
            self.music_volume
        } else {
            self.sfx_volume
        } * settings.volume;

        let id = self.backend.play(&path, volume, settings.looping)
            .map_err(AudioError::PlayFailed)?;

        if id > 0 {
            self.playing.insert(id, PlayingSound {
                sound_name: name.to_string(),
                is_music: settings.is_music,
            });
        }

        Ok(id)
    }

    /// Play music (looping by default)
    pub fn play_music(&mut self, name: &str) -> Result<u64, AudioError> {
        self.play_with_settings(name, PlaySettings::music())
    }

    /// Stop a sound
    pub fn stop(&mut self, id: u64) {
        self.backend.stop(id);
        self.playing.remove(&id);
    }

    /// Stop all sounds
    pub fn stop_all(&mut self) {
        self.backend.stop_all();
        self.playing.clear();
    }

    /// Stop all music
    pub fn stop_music(&mut self) {
        let music_ids: Vec<u64> = self.playing.iter()
            .filter(|(_, s)| s.is_music)
            .map(|(id, _)| *id)
            .collect();
        for id in music_ids {
            self.stop(id);
        }
    }

    /// Pause a sound
    pub fn pause(&mut self, id: u64) {
        self.backend.pause(id);
    }

    /// Resume a sound
    pub fn resume(&mut self, id: u64) {
        self.backend.resume(id);
    }

    /// Set master volume (0.0 - 1.0)
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    /// Set music volume (0.0 - 1.0)
    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
    }

    /// Set SFX volume (0.0 - 1.0)
    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
    }

    /// Enable/disable audio
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.stop_all();
        }
    }

    /// Check if a sound is registered
    pub fn has_sound(&self, name: &str) -> bool {
        self.sounds.contains_key(name)
    }

    /// Get list of registered sound names
    pub fn registered_sounds(&self) -> Vec<&str> {
        self.sounds.keys().map(|s| s.as_str()).collect()
    }

    /// Get number of currently playing sounds
    pub fn playing_count(&self) -> usize {
        self.playing.len()
    }

    /// Clean up finished sounds
    pub fn cleanup_finished(&mut self) {
        self.backend.cleanup_finished();
    }

    /// Check if audio feature is enabled
    pub fn is_audio_enabled() -> bool {
        cfg!(feature = "audio")
    }

    // ============ 3D Spatial Audio API ============

    /// Play a 3D spatial sound at a world position
    pub fn play_spatial(
        &mut self,
        name: &str,
        settings: SpatialSettings,
    ) -> Result<u64, AudioError> {
        if !self.enabled {
            return Ok(0);
        }

        let path = self.sounds.get(name)
            .ok_or_else(|| AudioError::SoundNotFound(name.to_string()))?
            .clone();

        let volume = self.master_volume * self.sfx_volume * settings.volume;

        let id = self.backend.play_spatial(
            &path,
            volume,
            settings.looping,
            settings.position,
            settings.max_distance,
            settings.rolloff_factor,
        ).map_err(AudioError::PlayFailed)?;

        if id > 0 {
            self.playing.insert(id, PlayingSound {
                sound_name: name.to_string(),
                is_music: false,
            });
        }

        Ok(id)
    }

    /// Update listener position and orientation (call every frame from camera)
    pub fn set_listener(&mut self, position: [f32; 3], forward: [f32; 3], up: [f32; 3]) {
        self.backend.set_listener(position, forward, up);
    }

    /// Update a spatial sound source position (for moving sources)
    pub fn set_source_position(&mut self, id: u64, position: [f32; 3]) {
        self.backend.set_source_position(id, position);
    }

    /// Update all spatial sounds (call after listener moves)
    pub fn update_spatial(&mut self) {
        self.backend.update_spatial_sounds();
    }
}

// ============ Tests ============

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
    fn test_audio_source_builder() {
        let source = AudioSource::new("explosion")
            .with_volume(0.8)
            .with_loop(true)
            .play_on_start();

        assert_eq!(source.sound_name, "explosion");
        assert_eq!(source.volume, 0.8);
        assert!(source.looping);
        assert!(source.play_on_start);
    }

    #[test]
    fn test_audio_system_stub() {
        let system = AudioSystem::new();
        assert!(system.is_ok());
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
    fn test_audio_listener() {
        let mut listener = AudioListener::new();
        assert!(listener.enabled);

        listener.update_from_transform(
            glam::Vec3::new(5.0, 10.0, 15.0),
            glam::Quat::IDENTITY,
        );

        assert_eq!(listener.position, [5.0, 10.0, 15.0]);
        // Z-up: Default forward is -Y (0, -1, 0)
        assert!((listener.forward[1] - (-1.0)).abs() < 0.01);
        // Z-up: Default up is Z (0, 0, 1)
        assert!((listener.up[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_audio_source_spatial() {
        let source = AudioSource::new("explosion")
            .with_max_distance(100.0)
            .with_rolloff(1.5);

        assert!(source.spatial);
        assert_eq!(source.max_distance, 100.0);
        assert_eq!(source.rolloff_factor, 1.5);
    }

    #[test]
    fn test_audio_source_2d() {
        let source = AudioSource::new_2d("music");
        assert!(!source.spatial);
    }
}
