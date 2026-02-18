#[cfg(feature = "gpu")]
use crate::resource::{RDGTextureDesc, RDGBufferDesc};

/// Pool for reusing transient GPU resources across frames.
///
/// Textures and buffers that are only needed within a single frame can be
/// allocated from this pool and returned at the end. This avoids expensive
/// GPU allocation/deallocation per frame.
#[cfg(feature = "gpu")]
pub struct TransientResourcePool {
    free_textures: Vec<PooledTexture>,
    free_buffers: Vec<PooledBuffer>,
    frame_index: u64,
}

#[cfg(feature = "gpu")]
struct PooledTexture {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
    mip_levels: u32,
    last_used_frame: u64,
}

#[cfg(feature = "gpu")]
struct PooledBuffer {
    buffer: wgpu::Buffer,
    size: u64,
    usage: wgpu::BufferUsages,
    last_used_frame: u64,
}

#[cfg(feature = "gpu")]
impl TransientResourcePool {
    /// Number of frames a resource can remain unused before being freed.
    const MAX_UNUSED_FRAMES: u64 = 8;

    pub fn new() -> Self {
        Self {
            free_textures: Vec::new(),
            free_buffers: Vec::new(),
            frame_index: 0,
        }
    }

    /// Try to find a matching texture in the pool, or create a new one.
    pub fn acquire_texture(
        &mut self,
        device: &wgpu::Device,
        desc: &RDGTextureDesc,
    ) -> wgpu::Texture {
        // Search for a compatible texture.
        if let Some(idx) = self.free_textures.iter().position(|t| {
            t.width == desc.width
                && t.height == desc.height
                && t.format == desc.format
                && t.usage.contains(desc.usage)
                && t.mip_levels == desc.mip_levels
        }) {
            let pooled = self.free_textures.swap_remove(idx);
            log::trace!("RDG Pool: reused texture '{}' ({}x{})", desc.label, desc.width, desc.height);
            return pooled.texture;
        }

        // Create new texture.
        log::trace!("RDG Pool: new texture '{}' ({}x{} {:?})", desc.label, desc.width, desc.height, desc.format);
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(desc.label),
            size: wgpu::Extent3d {
                width: desc.width,
                height: desc.height,
                depth_or_array_layers: desc.depth_or_layers,
            },
            mip_level_count: desc.mip_levels,
            sample_count: desc.sample_count,
            dimension: if desc.depth_or_layers > 1 {
                wgpu::TextureDimension::D3
            } else {
                wgpu::TextureDimension::D2
            },
            format: desc.format,
            usage: desc.usage,
            view_formats: &[],
        })
    }

    /// Try to find a matching buffer in the pool, or create a new one.
    pub fn acquire_buffer(
        &mut self,
        device: &wgpu::Device,
        desc: &RDGBufferDesc,
    ) -> wgpu::Buffer {
        // Round up to power-of-two size class for better reuse.
        let size_class = desc.size.next_power_of_two();

        if let Some(idx) = self.free_buffers.iter().position(|b| {
            b.size >= desc.size
                && b.size <= size_class
                && b.usage.contains(desc.usage)
        }) {
            let pooled = self.free_buffers.swap_remove(idx);
            log::trace!("RDG Pool: reused buffer '{}' ({} bytes)", desc.label, desc.size);
            return pooled.buffer;
        }

        log::trace!("RDG Pool: new buffer '{}' ({} bytes)", desc.label, size_class);
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(desc.label),
            size: size_class,
            usage: desc.usage,
            mapped_at_creation: false,
        })
    }

    /// Return a texture to the pool for future reuse.
    pub fn return_texture(&mut self, texture: wgpu::Texture) {
        let size = texture.size();
        self.free_textures.push(PooledTexture {
            width: size.width,
            height: size.height,
            format: texture.format(),
            usage: texture.usage(),
            mip_levels: texture.mip_level_count(),
            texture,
            last_used_frame: self.frame_index,
        });
    }

    /// Return a buffer to the pool for future reuse.
    pub fn return_buffer(&mut self, buffer: wgpu::Buffer) {
        self.free_buffers.push(PooledBuffer {
            size: buffer.size(),
            usage: buffer.usage(),
            buffer,
            last_used_frame: self.frame_index,
        });
    }

    /// Advance frame counter and release resources unused for too many frames.
    pub fn trim(&mut self) {
        self.frame_index += 1;
        let frame = self.frame_index;
        let max_age = Self::MAX_UNUSED_FRAMES;

        let old_tex = self.free_textures.len();
        self.free_textures
            .retain(|t| frame - t.last_used_frame < max_age);
        let old_buf = self.free_buffers.len();
        self.free_buffers
            .retain(|b| frame - b.last_used_frame < max_age);

        let freed_tex = old_tex - self.free_textures.len();
        let freed_buf = old_buf - self.free_buffers.len();
        if freed_tex > 0 || freed_buf > 0 {
            log::debug!(
                "RDG Pool trim: freed {} textures, {} buffers",
                freed_tex,
                freed_buf
            );
        }
    }

    pub fn free_texture_count(&self) -> usize {
        self.free_textures.len()
    }

    pub fn free_buffer_count(&self) -> usize {
        self.free_buffers.len()
    }
}

#[cfg(feature = "gpu")]
impl Default for TransientResourcePool {
    fn default() -> Self {
        Self::new()
    }
}
