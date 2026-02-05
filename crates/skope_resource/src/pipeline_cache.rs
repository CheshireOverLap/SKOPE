//! Pipeline state cache — avoids recreating identical GPU pipelines.
//!
//! WGPU pipeline creation can cause frame hitches. This cache stores
//! previously created render and compute pipelines keyed by a hash
//! of their descriptor fields.

#[cfg(feature = "gpu")]
use std::collections::HashMap;

/// Hash-based cache for wgpu render and compute pipelines.
#[cfg(feature = "gpu")]
pub struct PipelineCache {
    render_pipelines: HashMap<u64, wgpu::RenderPipeline>,
    compute_pipelines: HashMap<u64, wgpu::ComputePipeline>,
    stats: PipelineCacheStats,
}

#[cfg(feature = "gpu")]
#[derive(Default, Clone, Debug)]
pub struct PipelineCacheStats {
    pub render_hits: u64,
    pub render_misses: u64,
    pub compute_hits: u64,
    pub compute_misses: u64,
}

#[cfg(feature = "gpu")]
impl PipelineCache {
    pub fn new() -> Self {
        Self {
            render_pipelines: HashMap::new(),
            compute_pipelines: HashMap::new(),
            stats: PipelineCacheStats::default(),
        }
    }

    /// Get a cached render pipeline, or create and cache a new one.
    ///
    /// `key` should be a stable hash of the pipeline descriptor fields.
    /// The caller is responsible for computing a consistent hash.
    pub fn get_or_create_render_pipeline<F>(
        &mut self,
        key: u64,
        create_fn: F,
    ) -> &wgpu::RenderPipeline
    where
        F: FnOnce() -> wgpu::RenderPipeline,
    {
        if self.render_pipelines.contains_key(&key) {
            self.stats.render_hits += 1;
            return &self.render_pipelines[&key];
        }

        self.stats.render_misses += 1;
        log::debug!("PipelineCache: render pipeline miss (key={:#018x})", key);
        let pipeline = create_fn();
        self.render_pipelines.entry(key).or_insert(pipeline)
    }

    /// Get a cached compute pipeline, or create and cache a new one.
    pub fn get_or_create_compute_pipeline<F>(
        &mut self,
        key: u64,
        create_fn: F,
    ) -> &wgpu::ComputePipeline
    where
        F: FnOnce() -> wgpu::ComputePipeline,
    {
        if self.compute_pipelines.contains_key(&key) {
            self.stats.compute_hits += 1;
            return &self.compute_pipelines[&key];
        }

        self.stats.compute_misses += 1;
        log::debug!("PipelineCache: compute pipeline miss (key={:#018x})", key);
        let pipeline = create_fn();
        self.compute_pipelines.entry(key).or_insert(pipeline)
    }

    /// Invalidate a specific render pipeline (e.g., after shader hot-reload).
    pub fn invalidate_render(&mut self, key: u64) -> bool {
        self.render_pipelines.remove(&key).is_some()
    }

    /// Invalidate a specific compute pipeline.
    pub fn invalidate_compute(&mut self, key: u64) -> bool {
        self.compute_pipelines.remove(&key).is_some()
    }

    /// Clear all cached pipelines (e.g., on device lost).
    pub fn clear(&mut self) {
        self.render_pipelines.clear();
        self.compute_pipelines.clear();
        log::info!("PipelineCache: cleared all cached pipelines");
    }

    pub fn stats(&self) -> &PipelineCacheStats {
        &self.stats
    }

    pub fn render_pipeline_count(&self) -> usize {
        self.render_pipelines.len()
    }

    pub fn compute_pipeline_count(&self) -> usize {
        self.compute_pipelines.len()
    }
}

#[cfg(feature = "gpu")]
impl Default for PipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple FNV-1a hash utility for building pipeline keys.
pub fn fnv1a_hash(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Combine multiple hash values into a single key.
pub fn hash_combine(seed: u64, value: u64) -> u64 {
    seed ^ (value.wrapping_add(0x9e3779b97f4a7c15).wrapping_add(seed << 6).wrapping_add(seed >> 2))
}
