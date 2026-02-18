#[cfg(feature = "gpu")]
use wgpu;

/// Description for creating a transient texture within the render graph.
#[cfg(feature = "gpu")]
#[derive(Clone, Debug)]
pub struct RDGTextureDesc {
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
    pub depth_or_layers: u32,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
    pub mip_levels: u32,
    pub sample_count: u32,
}

#[cfg(feature = "gpu")]
impl RDGTextureDesc {
    pub fn new_2d(label: &'static str, width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            label,
            width,
            height,
            depth_or_layers: 1,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            mip_levels: 1,
            sample_count: 1,
        }
    }

    pub fn with_usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.usage = usage;
        self
    }

    pub fn with_mips(mut self, mip_levels: u32) -> Self {
        self.mip_levels = mip_levels;
        self
    }

    pub fn with_storage(mut self) -> Self {
        self.usage |= wgpu::TextureUsages::STORAGE_BINDING;
        self
    }
}

/// Description for creating a transient buffer within the render graph.
#[cfg(feature = "gpu")]
#[derive(Clone, Debug)]
pub struct RDGBufferDesc {
    pub label: &'static str,
    pub size: u64,
    pub usage: wgpu::BufferUsages,
}

/// An RDG-managed texture, either transient (created/destroyed within frame)
/// or imported (externally owned, persisted across frames).
#[cfg(feature = "gpu")]
pub enum RDGTexture {
    /// Transient texture created from a description.
    /// The actual wgpu::Texture is allocated from the pool at compile time.
    Transient {
        desc: RDGTextureDesc,
        texture: Option<wgpu::Texture>,
        view: Option<wgpu::TextureView>,
    },
    /// Imported texture (swapchain, persistent render targets, etc.)
    Imported {
        view: wgpu::TextureView,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    },
}

#[cfg(feature = "gpu")]
impl RDGTexture {
    /// Get the texture view (panics if transient and not yet allocated).
    pub fn view(&self) -> &wgpu::TextureView {
        match self {
            RDGTexture::Transient { view, .. } => {
                view.as_ref().expect("Transient texture not yet allocated")
            }
            RDGTexture::Imported { view, .. } => view,
        }
    }

    /// Get the wgpu::Texture reference (only for transient).
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        match self {
            RDGTexture::Transient { texture, .. } => texture.as_ref(),
            RDGTexture::Imported { .. } => None,
        }
    }

    pub fn width(&self) -> u32 {
        match self {
            RDGTexture::Transient { desc, .. } => desc.width,
            RDGTexture::Imported { width, .. } => *width,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            RDGTexture::Transient { desc, .. } => desc.height,
            RDGTexture::Imported { height, .. } => *height,
        }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        match self {
            RDGTexture::Transient { desc, .. } => desc.format,
            RDGTexture::Imported { format, .. } => *format,
        }
    }
}

/// An RDG-managed buffer.
#[cfg(feature = "gpu")]
pub enum RDGBuffer {
    Transient {
        desc: RDGBufferDesc,
        buffer: Option<wgpu::Buffer>,
    },
    Imported {
        buffer: wgpu::Buffer,
    },
}

#[cfg(feature = "gpu")]
impl RDGBuffer {
    pub fn buffer(&self) -> &wgpu::Buffer {
        match self {
            RDGBuffer::Transient { buffer, .. } => {
                buffer.as_ref().expect("Transient buffer not yet allocated")
            }
            RDGBuffer::Imported { buffer } => buffer,
        }
    }
}
