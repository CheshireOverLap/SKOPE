//! Nanite Visibility Resolve
//!
//! Merges HW rasterization (render target) and SW rasterization (atomic storage buffer)
//! into a unified V-Buffer that can be consumed by material evaluation.
//!
//! The resolve pass is a fullscreen compute shader that:
//! 1. Reads the HW V-Buffer render target (triangle_id + barycentrics + depth)
//! 2. Reads the SW atomic vis buffer (depth16|payload16)
//! 3. Compares depths and writes the closer result to the final V-Buffer
//!
//! For now, HW and SW use separate output surfaces. A future optimization
//! could share a single atomic buffer, but HW rasterization to render targets
//! is faster for large triangles.

/// V-Buffer render targets for Nanite output.
#[cfg(feature = "gpu")]
pub struct NaniteVBuffer {
    /// Triangle ID texture (R32Uint) — HW rasterizer output.
    pub triangle_id_texture: wgpu::Texture,
    pub triangle_id_view: wgpu::TextureView,
    /// Barycentrics texture (RG16Float) — HW rasterizer output.
    pub barycentrics_texture: wgpu::Texture,
    pub barycentrics_view: wgpu::TextureView,
    /// Depth texture (Depth32Float) — shared depth buffer.
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

#[cfg(feature = "gpu")]
impl NaniteVBuffer {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let triangle_id_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Nanite V-Buffer Triangle ID"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Uint,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let barycentrics_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Nanite V-Buffer Barycentrics"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float, // Rg16Float doesn't support STORAGE_BINDING
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Nanite V-Buffer Depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let triangle_id_view =
            triangle_id_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let barycentrics_view =
            barycentrics_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            triangle_id_texture,
            triangle_id_view,
            barycentrics_texture,
            barycentrics_view,
            depth_texture,
            depth_view,
            width,
            height,
        }
    }

    /// Recreate all textures when viewport size changes.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        *self = Self::new(device, width, height);
    }
}

/// Nanite rendering statistics collected after a frame.
#[cfg(feature = "gpu")]
pub struct NaniteFrameResult {
    pub hw_clusters_rendered: u32,
    pub sw_clusters_rendered: u32,
    pub total_triangles_hw: u32,
    pub total_triangles_sw: u32,
}
