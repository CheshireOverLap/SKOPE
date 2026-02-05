//! Bind group layout deduplication.
//!
//! WGPU creates new BindGroupLayout objects even when the descriptor
//! is identical. This cache ensures we reuse layouts with matching entries.

#[cfg(feature = "gpu")]
use std::collections::HashMap;

/// Cache for deduplicating bind group layouts.
#[cfg(feature = "gpu")]
pub struct BindGroupLayoutCache {
    layouts: HashMap<u64, wgpu::BindGroupLayout>,
}

#[cfg(feature = "gpu")]
impl BindGroupLayoutCache {
    pub fn new() -> Self {
        Self {
            layouts: HashMap::new(),
        }
    }

    /// Get a cached layout, or create and cache a new one.
    ///
    /// `key` should be a hash of the BindGroupLayoutDescriptor entries.
    pub fn get_or_create<F>(
        &mut self,
        key: u64,
        create_fn: F,
    ) -> &wgpu::BindGroupLayout
    where
        F: FnOnce() -> wgpu::BindGroupLayout,
    {
        self.layouts.entry(key).or_insert_with(create_fn)
    }

    pub fn clear(&mut self) {
        self.layouts.clear();
    }

    pub fn count(&self) -> usize {
        self.layouts.len()
    }
}

#[cfg(feature = "gpu")]
impl Default for BindGroupLayoutCache {
    fn default() -> Self {
        Self::new()
    }
}
