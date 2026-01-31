//! Audio Backend
//!
//! Cross-platform audio backend with optional rodio support

// ============ Conditional Backend ============

#[cfg(feature = "audio")]
mod inner {
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

    /// 3D spatial audio info
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

    /// Listener state
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
mod inner {
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

pub use inner::AudioBackend;
