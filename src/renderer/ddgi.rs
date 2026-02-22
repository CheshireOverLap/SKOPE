// SKOPE Engine - Dynamic Diffuse Global Illumination (DDGI)
//
// Architecture:
// - 3-level probe cascades (2m, 8m, 32m spacing)
// - Octahedral irradiance maps (8x8 per probe)
// - Octahedral visibility maps (16x16 per probe)
// - Screen-space + SDF hybrid ray tracing
//
// Reference: "Dynamic Diffuse Global Illumination with Ray-Traced Irradiance Fields"
// (JCGT 2019, Majercik et al.)

mod probe_grid;
mod pipeline;

pub use probe_grid::{ProbeGrid, ProbeGridConfig, CascadeLevel};
pub use pipeline::{DdgiPipeline, DdgiParams, DdgiCameraUniform, RayResult, DdgiProbeGridParams};

use wgpu;
use glam::{Vec3, Mat4, IVec3};

/// DDGI System - manages all cascades and atlases
#[allow(dead_code)]
pub struct DdgiSystem {
    /// Probe grids for each cascade level
    pub cascades: [ProbeGrid; 3],

    /// Combined irradiance atlas (all cascades)
    pub irradiance_atlas: wgpu::Texture,
    pub irradiance_view: wgpu::TextureView,

    /// Combined visibility atlas (all cascades)
    pub visibility_atlas: wgpu::Texture,
    pub visibility_view: wgpu::TextureView,

    /// Probe state buffer (active/inactive/sleeping)
    pub probe_state_buffer: wgpu::Buffer,

    /// Ray dispatch pipelines (to be implemented)
    // pub ray_trace_pipeline: wgpu::ComputePipeline,
    // pub irradiance_update_pipeline: wgpu::ComputePipeline,
    // pub visibility_update_pipeline: wgpu::ComputePipeline,

    /// Current center position (camera position)
    pub center: Vec3,

    /// Configuration
    pub config: DdgiConfig,
}

/// DDGI Configuration
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DdgiConfig {
    /// Rays per probe per frame
    pub rays_per_probe: u32,

    /// Hysteresis for irradiance blending (0.97 = slow)
    pub irradiance_hysteresis: f32,

    /// Hysteresis for visibility blending (0.85 = fast)
    pub visibility_hysteresis: f32,

    /// Maximum ray distance
    pub max_ray_distance: f32,

    /// Enable screen-space tracing
    pub enable_screen_space: bool,

    /// Enable SDF tracing fallback
    pub enable_sdf_fallback: bool,
}

impl Default for DdgiConfig {
    fn default() -> Self {
        Self {
            rays_per_probe: 128,
            irradiance_hysteresis: 0.97,
            visibility_hysteresis: 0.85,
            max_ray_distance: 500.0,
            enable_screen_space: true,
            enable_sdf_fallback: true,
        }
    }
}

impl DdgiSystem {
    /// Create new DDGI system
    pub fn new(device: &wgpu::Device, config: DdgiConfig) -> Self {
        // Create cascade configurations
        let cascade_configs = [
            ProbeGridConfig::new(CascadeLevel::Near),   // 2m spacing, 64m range
            ProbeGridConfig::new(CascadeLevel::Medium), // 8m spacing, 256m range
            ProbeGridConfig::new(CascadeLevel::Far),    // 32m spacing, 1km range
        ];

        // Create probe grids
        let cascades = [
            ProbeGrid::new(device, cascade_configs[0].clone()),
            ProbeGrid::new(device, cascade_configs[1].clone()),
            ProbeGrid::new(device, cascade_configs[2].clone()),
        ];

        // Calculate total atlas size
        let total_probes: u32 = cascades.iter().map(|c| c.total_probes()).sum();

        // Irradiance atlas: 8x8 octahedral per probe
        // E5B9G9R9 would be ideal but not universally supported, use Rgba16Float
        let irradiance_size = Self::calculate_atlas_size(total_probes, 8);
        let irradiance_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DDGI Irradiance Atlas"),
            size: wgpu::Extent3d {
                width: irradiance_size.0,
                height: irradiance_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let irradiance_view = irradiance_atlas.create_view(&wgpu::TextureViewDescriptor::default());

        // Visibility atlas: 16x16 octahedral per probe
        let visibility_size = Self::calculate_atlas_size(total_probes, 16);
        let visibility_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DDGI Visibility Atlas"),
            size: wgpu::Extent3d {
                width: visibility_size.0,
                height: visibility_size.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // NOTE: Rg16Float doesn't support STORAGE_BINDING on all GPUs
            format: wgpu::TextureFormat::Rgba16Float,  // Mean distance, variance (using Rgba for compatibility)
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        });
        let visibility_view = visibility_atlas.create_view(&wgpu::TextureViewDescriptor::default());

        // Probe state buffer: 1 byte per probe (active/inactive/sleeping)
        let probe_state_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DDGI Probe State"),
            size: total_probes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        log::info!(
            "[DDGI] Initialized: {} total probes, irradiance {}x{}, visibility {}x{}",
            total_probes,
            irradiance_size.0, irradiance_size.1,
            visibility_size.0, visibility_size.1
        );

        Self {
            cascades,
            irradiance_atlas,
            irradiance_view,
            visibility_atlas,
            visibility_view,
            probe_state_buffer,
            center: Vec3::ZERO,
            config,
        }
    }

    /// Calculate atlas texture size for given probe count and octahedral resolution
    fn calculate_atlas_size(probe_count: u32, octahedral_res: u32) -> (u32, u32) {
        // Pack probes in a square-ish atlas
        let probes_per_row = (probe_count as f32).sqrt().ceil() as u32;
        let rows = probe_count.div_ceil(probes_per_row);

        (probes_per_row * octahedral_res, rows * octahedral_res)
    }

    /// Update probe grid center (should be called when camera moves significantly)
    pub fn update_center(&mut self, new_center: Vec3) {
        let delta = new_center - self.center;

        // Only update if moved more than half a probe spacing
        let min_spacing = self.cascades[0].config.spacing;
        if delta.length() > min_spacing * 0.5 {
            self.center = new_center;
            for cascade in &mut self.cascades {
                cascade.update_center(new_center);
            }
        }
    }

    /// Get total VRAM usage estimate
    pub fn vram_usage(&self) -> usize {
        let irradiance_bytes = (self.irradiance_atlas.width() * self.irradiance_atlas.height() * 8) as usize;
        let visibility_bytes = (self.visibility_atlas.width() * self.visibility_atlas.height() * 4) as usize;
        let state_bytes = self.cascades.iter().map(|c| c.total_probes()).sum::<u32>() as usize;

        irradiance_bytes + visibility_bytes + state_bytes
    }

}
