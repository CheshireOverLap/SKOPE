//! Global Signed Distance Field
//!
//! Manages a 3D texture representing the scene's global SDF.
//! Used for medium-range ray tracing in the gather pass.
//!
//! The SDF is stored as R16Float in a 3D texture (typically 128^3).
//! Positive values = outside surfaces, negative = inside.
//! Normalized so 1.0 = one voxel distance.

use crate::types::{GlobalSDFParams, LumenConfig};

/// CPU-side global SDF management.
pub struct GlobalSDF {
    /// SDF volume resolution per axis.
    pub resolution: u32,
    /// World-space bounds.
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    /// Voxel size in world units.
    pub voxel_size: f32,
    /// Whether the SDF needs a full rebuild.
    pub dirty: bool,
}

impl GlobalSDF {
    pub fn new(config: &LumenConfig) -> Self {
        let resolution = config.global_sdf_resolution;
        let half_extent = config.sdf_trace_max_distance;
        Self {
            resolution,
            bounds_min: [-half_extent, -half_extent, -half_extent],
            bounds_max: [half_extent, half_extent, half_extent],
            voxel_size: (half_extent * 2.0) / resolution as f32,
            dirty: true,
        }
    }

    /// Update the SDF bounds to be centered around the camera.
    pub fn update_bounds(&mut self, camera_pos: [f32; 3], half_extent: f32) {
        self.bounds_min = [
            camera_pos[0] - half_extent,
            camera_pos[1] - half_extent,
            camera_pos[2] - half_extent,
        ];
        self.bounds_max = [
            camera_pos[0] + half_extent,
            camera_pos[1] + half_extent,
            camera_pos[2] + half_extent,
        ];
        self.voxel_size = (half_extent * 2.0) / self.resolution as f32;
        self.dirty = true;
    }

    /// Get GPU params for this SDF volume.
    pub fn params(&self) -> GlobalSDFParams {
        GlobalSDFParams {
            bounds_min: self.bounds_min,
            voxel_size: self.voxel_size,
            bounds_max: self.bounds_max,
            resolution: self.resolution,
        }
    }

    /// Convert a world position to voxel coordinates.
    pub fn world_to_voxel(&self, world_pos: [f32; 3]) -> Option<[u32; 3]> {
        let rel = [
            (world_pos[0] - self.bounds_min[0]) / (self.bounds_max[0] - self.bounds_min[0]),
            (world_pos[1] - self.bounds_min[1]) / (self.bounds_max[1] - self.bounds_min[1]),
            (world_pos[2] - self.bounds_min[2]) / (self.bounds_max[2] - self.bounds_min[2]),
        ];

        if rel[0] < 0.0 || rel[0] >= 1.0
            || rel[1] < 0.0 || rel[1] >= 1.0
            || rel[2] < 0.0 || rel[2] >= 1.0
        {
            return None;
        }

        Some([
            (rel[0] * self.resolution as f32) as u32,
            (rel[1] * self.resolution as f32) as u32,
            (rel[2] * self.resolution as f32) as u32,
        ])
    }
}

/// GPU SDF volume resources.
#[cfg(feature = "gpu")]
pub struct SDFVolumeGpu {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub params_buffer: wgpu::Buffer,
}

#[cfg(feature = "gpu")]
impl SDFVolumeGpu {
    pub fn new(device: &wgpu::Device, resolution: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Lumen Global SDF"),
            size: wgpu::Extent3d {
                width: resolution,
                height: resolution,
                depth_or_array_layers: resolution,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R32Float, // R16Float doesn't support STORAGE_BINDING
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Lumen SDF Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen SDF Params"),
            size: std::mem::size_of::<GlobalSDFParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            texture,
            view,
            sampler,
            params_buffer,
        }
    }

    pub fn update_params(&self, queue: &wgpu::Queue, params: &GlobalSDFParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_world_to_voxel() {
        let config = LumenConfig::default();
        let sdf = GlobalSDF::new(&config);

        // Center of volume → center voxel
        let center = sdf.world_to_voxel([0.0, 0.0, 0.0]).unwrap();
        assert_eq!(center, [64, 64, 64]);

        // Outside bounds → None
        assert!(sdf.world_to_voxel([999.0, 0.0, 0.0]).is_none());
    }

    #[test]
    fn test_sdf_update_bounds() {
        let config = LumenConfig::default();
        let mut sdf = GlobalSDF::new(&config);
        sdf.update_bounds([100.0, 0.0, 0.0], 50.0);

        assert_eq!(sdf.bounds_min[0], 50.0);
        assert_eq!(sdf.bounds_max[0], 150.0);
        assert!(sdf.dirty);
    }
}
