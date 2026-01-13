// SKOPE Audio System
// Cross-platform audio engine with optional backend
// Supports 3D spatial audio with distance attenuation
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ============ Conditional Backend ============

#[cfg(feature = "audio")]
mod backend {
    use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, SpatialSink, Source};
    use std::fs::File;
    use std::io::BufReader;
    use std::path::Path;
    use std::collections::HashMap;

    /// Wrapper for both regular and spatial sinks
    enum SinkType {
        Regular(Sink),
        Spatial(SpatialSink),
    }

    /// 3D 공간 오디오 정보
    pub struct SpatialInfo {
        pub position: [f32; 3],
        pub max_distance: f32,
        pub rolloff_factor: f32,
    }

    impl Default for SpatialInfo {
        fn default() -> Self {
            Self {
                position: [0.0, 0.0, 0.0],
                max_distance: 50.0,
                rolloff_factor: 1.0,
            }
        }
    }

    /// 리스너 상태
    pub struct ListenerState {
        pub position: [f32; 3],
        pub forward: [f32; 3],
        pub up: [f32; 3],
    }

    impl Default for ListenerState {
        fn default() -> Self {
            Self {
                position: [0.0, 0.0, 0.0],
                forward: [0.0, 0.0, -1.0],
                up: [0.0, 1.0, 0.0],
            }
        }
    }

    pub struct AudioBackend {
        _stream: OutputStream,
        stream_handle: OutputStreamHandle,
        sinks: HashMap<u64, SinkType>,
        spatial_info: HashMap<u64, SpatialInfo>,
        next_id: u64,
        listener: ListenerState,
    }

    impl AudioBackend {
        pub fn new() -> Result<Self, String> {
            let (stream, stream_handle) = OutputStream::try_default()
                .map_err(|e| format!("Failed to create audio output: {}", e))?;
            Ok(Self {
                _stream: stream,
                stream_handle,
                sinks: HashMap::new(),
                spatial_info: HashMap::new(),
                next_id: 1,
                listener: ListenerState::default(),
            })
        }

        /// Play 2D sound (non-spatial)
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
            self.sinks.insert(id, SinkType::Regular(sink));

            Ok(id)
        }

        /// Play 3D spatial sound
        pub fn play_spatial(
            &mut self,
            path: &Path,
            volume: f32,
            looping: bool,
            position: [f32; 3],
            max_distance: f32,
            rolloff_factor: f32,
        ) -> Result<u64, String> {
            let file = File::open(path)
                .map_err(|e| format!("Failed to open audio file: {}", e))?;
            let reader = BufReader::new(file);
            let source = Decoder::new(reader)
                .map_err(|e| format!("Failed to decode audio: {}", e))?;

            // Calculate ear positions based on listener orientation
            let (left_ear, right_ear) = self.calculate_ear_positions();

            let sink = SpatialSink::try_new(
                &self.stream_handle,
                position,
                left_ear,
                right_ear,
            ).map_err(|e| format!("Failed to create spatial sink: {}", e))?;

            // Apply distance attenuation
            let distance = self.calculate_distance(position);
            let attenuation = self.calculate_attenuation(distance, max_distance, rolloff_factor);
            sink.set_volume(volume * attenuation);

            if looping {
                sink.append(source.repeat_infinite());
            } else {
                sink.append(source);
            }

            let id = self.next_id;
            self.next_id += 1;
            self.sinks.insert(id, SinkType::Spatial(sink));
            self.spatial_info.insert(id, SpatialInfo {
                position,
                max_distance,
                rolloff_factor,
            });

            Ok(id)
        }

        /// Calculate ear positions from listener state
        fn calculate_ear_positions(&self) -> ([f32; 3], [f32; 3]) {
            let pos = self.listener.position;
            let fwd = self.listener.forward;
            let up = self.listener.up;

            // Right vector = forward × up
            let right = [
                fwd[1] * up[2] - fwd[2] * up[1],
                fwd[2] * up[0] - fwd[0] * up[2],
                fwd[0] * up[1] - fwd[1] * up[0],
            ];

            // Ear offset (typical head width ~0.2m)
            let ear_offset = 0.1;

            let left_ear = [
                pos[0] - right[0] * ear_offset,
                pos[1] - right[1] * ear_offset,
                pos[2] - right[2] * ear_offset,
            ];
            let right_ear = [
                pos[0] + right[0] * ear_offset,
                pos[1] + right[1] * ear_offset,
                pos[2] + right[2] * ear_offset,
            ];

            (left_ear, right_ear)
        }

        /// Calculate distance from listener to position
        fn calculate_distance(&self, position: [f32; 3]) -> f32 {
            let dx = position[0] - self.listener.position[0];
            let dy = position[1] - self.listener.position[1];
            let dz = position[2] - self.listener.position[2];
            (dx * dx + dy * dy + dz * dz).sqrt()
        }

        /// Calculate distance attenuation (inverse square with rolloff)
        fn calculate_attenuation(&self, distance: f32, max_distance: f32, rolloff_factor: f32) -> f32 {
            if distance >= max_distance {
                return 0.0;
            }
            if distance < 1.0 {
                return 1.0;
            }

            // Inverse distance attenuation with rolloff
            let ref_distance = 1.0;
            let attenuation = ref_distance / (ref_distance + rolloff_factor * (distance - ref_distance));

            // Smooth falloff near max distance
            let fade = 1.0 - (distance / max_distance).powi(2);

            attenuation * fade.max(0.0)
        }

        /// Update listener position and orientation
        pub fn set_listener(&mut self, position: [f32; 3], forward: [f32; 3], up: [f32; 3]) {
            self.listener.position = position;
            self.listener.forward = forward;
            self.listener.up = up;
        }

        /// Update source position (for moving sources)
        pub fn set_source_position(&mut self, id: u64, position: [f32; 3]) {
            // First, read the spatial info (immutable borrow)
            let (max_dist, rolloff) = match self.spatial_info.get(&id) {
                Some(info) => (info.max_distance, info.rolloff_factor),
                None => return,
            };

            // Calculate distance and attenuation with only immutable self borrow
            let distance = self.calculate_distance(position);
            let attenuation = self.calculate_attenuation(distance, max_dist, rolloff);

            // Now update the position (mutable borrow)
            if let Some(info) = self.spatial_info.get_mut(&id) {
                info.position = position;
            }

            // Update the sink
            if let Some(SinkType::Spatial(sink)) = self.sinks.get(&id) {
                sink.set_emitter_position(position);
                sink.set_volume(attenuation);
            }
        }

        /// Update all spatial sounds (call when listener moves)
        pub fn update_spatial_sounds(&mut self) {
            let (left_ear, right_ear) = self.calculate_ear_positions();

            for (id, sink) in &self.sinks {
                if let SinkType::Spatial(spatial) = sink {
                    spatial.set_left_ear_position(left_ear);
                    spatial.set_right_ear_position(right_ear);

                    // Update attenuation based on new listener position
                    if let Some(info) = self.spatial_info.get(id) {
                        let distance = self.calculate_distance(info.position);
                        let attenuation = self.calculate_attenuation(
                            distance,
                            info.max_distance,
                            info.rolloff_factor,
                        );
                        spatial.set_volume(attenuation);
                    }
                }
            }
        }

        pub fn stop(&mut self, id: u64) {
            if let Some(sink) = self.sinks.remove(&id) {
                match sink {
                    SinkType::Regular(s) => s.stop(),
                    SinkType::Spatial(s) => s.stop(),
                }
            }
            self.spatial_info.remove(&id);
        }

        pub fn pause(&mut self, id: u64) {
            if let Some(sink) = self.sinks.get(&id) {
                match sink {
                    SinkType::Regular(s) => s.pause(),
                    SinkType::Spatial(s) => s.pause(),
                }
            }
        }

        pub fn resume(&mut self, id: u64) {
            if let Some(sink) = self.sinks.get(&id) {
                match sink {
                    SinkType::Regular(s) => s.play(),
                    SinkType::Spatial(s) => s.play(),
                }
            }
        }

        pub fn set_volume(&mut self, id: u64, volume: f32) {
            if let Some(sink) = self.sinks.get(&id) {
                match sink {
                    SinkType::Regular(s) => s.set_volume(volume),
                    SinkType::Spatial(s) => s.set_volume(volume),
                }
            }
        }

        pub fn is_playing(&self, id: u64) -> bool {
            self.sinks.get(&id).map(|s| {
                match s {
                    SinkType::Regular(sink) => !sink.is_paused() && !sink.empty(),
                    SinkType::Spatial(sink) => !sink.is_paused() && !sink.empty(),
                }
            }).unwrap_or(false)
        }

        pub fn cleanup_finished(&mut self) {
            let finished: Vec<u64> = self.sinks.iter()
                .filter(|(_, sink)| {
                    match sink {
                        SinkType::Regular(s) => s.empty(),
                        SinkType::Spatial(s) => s.empty(),
                    }
                })
                .map(|(id, _)| *id)
                .collect();

            for id in finished {
                self.sinks.remove(&id);
                self.spatial_info.remove(&id);
            }
        }

        pub fn stop_all(&mut self) {
            for (_, sink) in self.sinks.drain() {
                match sink {
                    SinkType::Regular(s) => s.stop(),
                    SinkType::Spatial(s) => s.stop(),
                }
            }
            self.spatial_info.clear();
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

        pub fn play_spatial(
            &mut self,
            _path: &Path,
            _volume: f32,
            _looping: bool,
            _position: [f32; 3],
            _max_distance: f32,
            _rolloff_factor: f32,
        ) -> Result<u64, String> {
            Ok(0) // Stub - no audio
        }

        pub fn set_listener(&mut self, _position: [f32; 3], _forward: [f32; 3], _up: [f32; 3]) {}
        pub fn set_source_position(&mut self, _id: u64, _position: [f32; 3]) {}
        pub fn update_spatial_sounds(&mut self) {}
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

        // Calculate forward and up vectors from quaternion (Z-up 좌표계)
        let forward = rotation * glam::Vec3::NEG_Y;  // Z-up: forward is -Y
        let up = rotation * glam::Vec3::Z;           // Z-up: up is +Z

        self.forward = [forward.x, forward.y, forward.z];
        self.up = [up.x, up.y, up.z];
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

// ============ Audio Pool ============

/// Audio pool for caching frequently used sounds
/// Reduces load time by pre-decoding audio files
#[cfg(feature = "audio")]
pub mod pool {
    use rodio::{Decoder, Source};
    use std::collections::HashMap;
    use std::io::BufReader;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    /// Cached audio data (decoded samples)
    pub struct CachedSound {
        /// Raw audio samples (interleaved)
        samples: Arc<Vec<i16>>,
        /// Sample rate
        sample_rate: u32,
        /// Number of channels (1 = mono, 2 = stereo)
        channels: u16,
        /// File path (for debugging)
        path: PathBuf,
        /// Size in bytes (for memory tracking)
        size_bytes: usize,
    }

    impl CachedSound {
        /// Load and decode an audio file
        pub fn load(path: &Path) -> Result<Self, String> {
            let file = std::fs::File::open(path)
                .map_err(|e| format!("Failed to open file {:?}: {}", path, e))?;
            let reader = BufReader::new(file);
            let decoder = Decoder::new(reader)
                .map_err(|e| format!("Failed to decode {:?}: {}", path, e))?;

            let sample_rate = decoder.sample_rate();
            let channels = decoder.channels();

            // Collect all samples into memory
            let samples: Vec<i16> = decoder.collect();
            let size_bytes = samples.len() * std::mem::size_of::<i16>();

            Ok(Self {
                samples: Arc::new(samples),
                sample_rate,
                channels,
                path: path.to_path_buf(),
                size_bytes,
            })
        }

        /// Get the size in bytes
        pub fn size_bytes(&self) -> usize {
            self.size_bytes
        }

        /// Get the duration in seconds (approximate)
        pub fn duration_secs(&self) -> f32 {
            let total_samples = self.samples.len() / self.channels as usize;
            total_samples as f32 / self.sample_rate as f32
        }
    }

    /// Iterator over cached sound samples
    pub struct CachedSoundSource {
        samples: Arc<Vec<i16>>,
        sample_rate: u32,
        channels: u16,
        position: usize,
    }

    impl CachedSoundSource {
        fn new(cached: &CachedSound) -> Self {
            Self {
                samples: Arc::clone(&cached.samples),
                sample_rate: cached.sample_rate,
                channels: cached.channels,
                position: 0,
            }
        }
    }

    impl Iterator for CachedSoundSource {
        type Item = i16;

        fn next(&mut self) -> Option<Self::Item> {
            if self.position < self.samples.len() {
                let sample = self.samples[self.position];
                self.position += 1;
                Some(sample)
            } else {
                None
            }
        }
    }

    impl Source for CachedSoundSource {
        fn current_frame_len(&self) -> Option<usize> {
            None
        }

        fn channels(&self) -> u16 {
            self.channels
        }

        fn sample_rate(&self) -> u32 {
            self.sample_rate
        }

        fn total_duration(&self) -> Option<std::time::Duration> {
            let total_samples = self.samples.len() / self.channels as usize;
            let duration_secs = total_samples as f64 / self.sample_rate as f64;
            Some(std::time::Duration::from_secs_f64(duration_secs))
        }
    }

    /// Audio pool manager
    pub struct AudioPool {
        /// Cached sounds
        cache: HashMap<String, CachedSound>,
        /// Maximum cache size in bytes
        max_size: usize,
        /// Current cache size in bytes
        current_size: usize,
        /// Access order for LRU eviction
        access_order: Vec<String>,
    }

    impl AudioPool {
        /// Create a new audio pool with maximum size in MB
        pub fn new(max_size_mb: usize) -> Self {
            Self {
                cache: HashMap::new(),
                max_size: max_size_mb * 1024 * 1024,
                current_size: 0,
                access_order: Vec::new(),
            }
        }

        /// Preload a sound into the pool
        pub fn preload(&mut self, name: &str, path: &Path) -> Result<(), String> {
            if self.cache.contains_key(name) {
                return Ok(()); // Already cached
            }

            let cached = CachedSound::load(path)?;
            let size = cached.size_bytes();

            // Evict if necessary
            while self.current_size + size > self.max_size && !self.access_order.is_empty() {
                self.evict_lru();
            }

            self.current_size += size;
            self.cache.insert(name.to_string(), cached);
            self.access_order.push(name.to_string());

            log::debug!("[AudioPool] Preloaded '{}' ({:.1} KB)", name, size as f32 / 1024.0);
            Ok(())
        }

        /// Check if a sound is cached
        pub fn is_cached(&self, name: &str) -> bool {
            self.cache.contains_key(name)
        }

        /// Get a source for playback (None if not cached)
        pub fn get_source(&mut self, name: &str) -> Option<CachedSoundSource> {
            // Update access order first (mutable borrow)
            self.touch(name);

            // Then get the cached sound (immutable borrow)
            self.cache.get(name).map(CachedSoundSource::new)
        }

        /// Get cache statistics
        pub fn stats(&self) -> (usize, usize, usize) {
            (
                self.cache.len(),
                self.current_size,
                self.max_size,
            )
        }

        /// Clear the entire cache
        pub fn clear(&mut self) {
            self.cache.clear();
            self.access_order.clear();
            self.current_size = 0;
            log::info!("[AudioPool] Cache cleared");
        }

        /// Evict least recently used entry
        fn evict_lru(&mut self) {
            if let Some(name) = self.access_order.first().cloned() {
                if let Some(cached) = self.cache.remove(&name) {
                    self.current_size -= cached.size_bytes();
                    self.access_order.remove(0);
                    log::debug!("[AudioPool] Evicted '{}' (LRU)", name);
                }
            }
        }

        /// Update access order (move to end)
        fn touch(&mut self, name: &str) {
            if let Some(pos) = self.access_order.iter().position(|n| n == name) {
                self.access_order.remove(pos);
                self.access_order.push(name.to_string());
            }
        }
    }

    impl Default for AudioPool {
        fn default() -> Self {
            Self::new(64) // 64 MB default
        }
    }
}

/// Audio pool stub for when audio feature is disabled
#[cfg(not(feature = "audio"))]
pub mod pool {
    use std::path::Path;

    pub struct AudioPool;

    impl AudioPool {
        pub fn new(_max_size_mb: usize) -> Self { Self }
        pub fn preload(&mut self, _name: &str, _path: &Path) -> Result<(), String> { Ok(()) }
        pub fn is_cached(&self, _name: &str) -> bool { false }
        pub fn stats(&self) -> (usize, usize, usize) { (0, 0, 0) }
        pub fn clear(&mut self) {}
    }

    impl Default for AudioPool {
        fn default() -> Self { Self }
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
