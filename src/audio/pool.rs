//! Audio Pool
//!
//! Audio pool for caching frequently used sounds
//! Reduces load time by pre-decoding audio files

#[cfg(feature = "audio")]
mod inner {
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
        #[allow(dead_code)]
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
        #[allow(dead_code)]
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
mod inner {
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

#[allow(unused_imports)]
pub use inner::*;
