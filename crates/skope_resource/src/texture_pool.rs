//! Texture pool — reuse GPU textures by matching format, size, and usage.

#[cfg(feature = "gpu")]
use std::collections::HashMap;

/// Key for texture pool buckets.
#[cfg(feature = "gpu")]
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
struct TexPoolKey {
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    mip_levels: u32,
    sample_count: u32,
}

/// Pool of reusable GPU textures.
#[cfg(feature = "gpu")]
pub struct TexturePool {
    pools: HashMap<TexPoolKey, Vec<wgpu::Texture>>,
    stats: TexturePoolStats,
}

#[cfg(feature = "gpu")]
#[derive(Default, Clone, Debug)]
pub struct TexturePoolStats {
    pub hits: u64,
    pub misses: u64,
    pub returns: u64,
}

#[cfg(feature = "gpu")]
impl TexturePool {
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            stats: TexturePoolStats::default(),
        }
    }

    /// Acquire a texture matching the given parameters.
    pub fn acquire(
        &mut self,
        device: &wgpu::Device,
        label: &str,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
        mip_levels: u32,
        sample_count: u32,
    ) -> wgpu::Texture {
        let key = TexPoolKey {
            width,
            height,
            format,
            usage,
            mip_levels,
            sample_count,
        };

        if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(texture) = pool.pop() {
                self.stats.hits += 1;
                return texture;
            }
        }

        self.stats.misses += 1;
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: mip_levels,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    /// Return a texture to the pool.
    pub fn release(&mut self, texture: wgpu::Texture) {
        let size = texture.size();
        let key = TexPoolKey {
            width: size.width,
            height: size.height,
            format: texture.format(),
            usage: texture.usage(),
            mip_levels: texture.mip_level_count(),
            sample_count: texture.sample_count(),
        };
        self.stats.returns += 1;
        self.pools.entry(key).or_default().push(texture);
    }

    pub fn trim(&mut self) {
        let count: usize = self.pools.values().map(|v| v.len()).sum();
        self.pools.clear();
        if count > 0 {
            log::debug!("TexturePool: trimmed {} textures", count);
        }
    }

    pub fn stats(&self) -> &TexturePoolStats {
        &self.stats
    }

    pub fn total_pooled(&self) -> usize {
        self.pools.values().map(|v| v.len()).sum()
    }
}

#[cfg(feature = "gpu")]
impl Default for TexturePool {
    fn default() -> Self {
        Self::new()
    }
}
