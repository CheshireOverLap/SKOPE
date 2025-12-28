// SKOPE Audio System
// Cross-platform audio engine with optional backend
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============ Conditional Backend ============

#[cfg(feature = "audio")]
mod backend {
    use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;
    use std::collections::HashMap;

    pub struct AudioBackend {
        _stream: OutputStream,
        stream_handle: OutputStreamHandle,
        sinks: HashMap<u64, Sink>,
        next_id: u64,
    }

    impl AudioBackend {
        pub fn new() -> Result<Self, String> {
            let (stream, stream_handle) = OutputStream::try_default()
                .map_err(|e| format!("Failed to create audio output: {}", e))?;
            Ok(Self {
                _stream: stream,
                stream_handle,
                sinks: HashMap::new(),
                next_id: 1,
            })
        }

        pub fn play(&mut self, path: &Path, volume: f32, looping: bool) -> Result<u64, String> {
            let file = File::open(path)
                .map_err(|e| format!("Failed to open audio file: {}", e))?;
            let reader = BufReader::new(file);
            let source = Decoder::new(reader)
                .map_err(|e| format!("Failed to decode audio: {}", e))?;

            let sink = Sink::try_new(&self.stream_handle)
                .map_err(|e| format!("Failed to create sink: {}", e))?;

            sink.set_volume(volume);

            if looping {
                sink.append(source.repeat_infinite());
            } else {
                sink.append(source);
            }

            let id = self.next_id;
            self.next_id += 1;
            self.sinks.insert(id, sink);

            Ok(id)
        }

        pub fn stop(&mut self, id: u64) {
            if let Some(sink) = self.sinks.remove(&id) {
                sink.stop();
            }
        }

        pub fn pause(&mut self, id: u64) {
            if let Some(sink) = self.sinks.get(&id) {
                sink.pause();
            }
        }

        pub fn resume(&mut self, id: u64) {
            if let Some(sink) = self.sinks.get(&id) {
                sink.play();
            }
        }

        pub fn set_volume(&mut self, id: u64, volume: f32) {
            if let Some(sink) = self.sinks.get(&id) {
                sink.set_volume(volume);
            }
        }

        pub fn is_playing(&self, id: u64) -> bool {
            self.sinks.get(&id).map(|s| !s.is_paused() && !s.empty()).unwrap_or(false)
        }

        pub fn cleanup_finished(&mut self) {
            self.sinks.retain(|_, sink| !sink.empty());
        }

        pub fn stop_all(&mut self) {
            for (_, sink) in self.sinks.drain() {
                sink.stop();
            }
        }
    }
}

#[cfg(not(feature = "audio"))]
mod backend {
    use std::path::Path;

    pub struct AudioBackend;

    impl AudioBackend {
        pub fn new() -> Result<Self, String> {
            log::info!("[Audio] Audio disabled (compile with --features audio)");
            Ok(Self)
        }

        pub fn play(&mut self, _path: &Path, _volume: f32, _looping: bool) -> Result<u64, String> {
            Ok(0) // Stub - no audio
        }

        pub fn stop(&mut self, _id: u64) {}
        pub fn pause(&mut self, _id: u64) {}
        pub fn resume(&mut self, _id: u64) {}
        pub fn set_volume(&mut self, _id: u64, _volume: f32) {}
        pub fn is_playing(&self, _id: u64) -> bool { false }
        pub fn cleanup_finished(&mut self) {}
        pub fn stop_all(&mut self) {}
    }
}

use backend::AudioBackend;

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
            .map_err(|e| AudioError::InitFailed(e))?;

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
            .map_err(|e| AudioError::PlayFailed(e))?;

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
}

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

// ============ Audio Components ============

/// Audio source component - attach to entities that emit sound
#[derive(Component, Clone)]
pub struct AudioSource {
    pub sound_name: String,
    pub volume: f32,
    pub looping: bool,
    pub spatial: bool,
    pub play_on_start: bool,
    pub playing_id: Option<u64>,
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

    pub fn play_on_start(mut self) -> Self {
        self.play_on_start = true;
        self
    }
}

/// Audio listener component - attach to camera/player
#[derive(Component, Clone, Default)]
pub struct AudioListener {
    pub enabled: bool,
}

impl AudioListener {
    pub fn new() -> Self {
        Self { enabled: true }
    }
}

// ============ Error Types ============

#[derive(Debug)]
pub enum AudioError {
    InitFailed(String),
    SoundNotFound(String),
    PlayFailed(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioError::InitFailed(e) => write!(f, "Audio init failed: {}", e),
            AudioError::SoundNotFound(name) => write!(f, "Sound not found: {}", name),
            AudioError::PlayFailed(e) => write!(f, "Play failed: {}", e),
        }
    }
}

impl std::error::Error for AudioError {}

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
}
