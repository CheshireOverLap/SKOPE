//! World-Space Radiance Cache
//!
//! Persistent world-space probes that store SH (Spherical Harmonics)
//! coefficients representing incoming radiance from all directions.
//!
//! Probes are placed on a uniform grid and updated incrementally
//! across frames. Screen probes that miss in screen-space fall back
//! to the radiance cache for long-range indirect lighting.

use crate::types::LumenConfig;
#[cfg(feature = "gpu")]
use crate::types::RadianceCacheProbe;

/// Radiance cache grid state.
pub struct RadianceCache {
    /// Grid dimensions per axis.
    pub grid_size: u32,
    /// World-space probe spacing.
    pub probe_spacing: f32,
    /// World-space origin (center of the cache volume).
    pub origin: [f32; 3],
    /// Total number of probes in the grid.
    pub total_probes: u32,
    /// Probes to update per frame (round-robin).
    pub updates_per_frame: u32,
    /// Current update offset.
    pub update_offset: u32,
}

impl RadianceCache {
    pub fn new(config: &LumenConfig) -> Self {
        let spacing = config.radiance_cache_probe_spacing;
        // Grid covers the SDF trace distance in each direction
        let grid_size = (config.sdf_trace_max_distance * 2.0 / spacing).ceil() as u32;
        let grid_size = grid_size.min(64); // Cap at 64^3 = 262144 probes
        let total = grid_size * grid_size * grid_size;

        Self {
            grid_size,
            probe_spacing: spacing,
            origin: [0.0, 0.0, 0.0],
            total_probes: total,
            updates_per_frame: (total / 16).max(64), // Update ~6% per frame
            update_offset: 0,
        }
    }

    /// Update the cache origin to track the camera.
    pub fn update_origin(&mut self, camera_pos: [f32; 3]) {
        // Snap to grid
        self.origin = [
            (camera_pos[0] / self.probe_spacing).round() * self.probe_spacing,
            (camera_pos[1] / self.probe_spacing).round() * self.probe_spacing,
            (camera_pos[2] / self.probe_spacing).round() * self.probe_spacing,
        ];
    }

    /// Get the world position of a probe by its linear index.
    pub fn probe_world_pos(&self, index: u32) -> [f32; 3] {
        let gs = self.grid_size;
        let ix = index % gs;
        let iy = (index / gs) % gs;
        let iz = index / (gs * gs);

        let half = (gs as f32 - 1.0) * self.probe_spacing * 0.5;
        [
            self.origin[0] + ix as f32 * self.probe_spacing - half,
            self.origin[1] + iy as f32 * self.probe_spacing - half,
            self.origin[2] + iz as f32 * self.probe_spacing - half,
        ]
    }

    /// Get the range of probes to update this frame.
    pub fn update_range(&mut self) -> (u32, u32) {
        let start = self.update_offset;
        let end = (start + self.updates_per_frame).min(self.total_probes);
        self.update_offset = if end >= self.total_probes {
            0
        } else {
            end
        };
        (start, end)
    }

    /// Grid world-space half-extent.
    pub fn half_extent(&self) -> f32 {
        (self.grid_size as f32 - 1.0) * self.probe_spacing * 0.5
    }
}

/// GPU resources for the radiance cache.
#[cfg(feature = "gpu")]
pub struct RadianceCacheGpu {
    /// Buffer storing all RadianceCacheProbe entries.
    pub probe_buffer: wgpu::Buffer,
    pub total_probes: u32,
}

#[cfg(feature = "gpu")]
impl RadianceCacheGpu {
    pub fn new(device: &wgpu::Device, total_probes: u32) -> Self {
        let probe_size = std::mem::size_of::<RadianceCacheProbe>() as u64;
        let probe_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Radiance Cache Probes"),
            size: probe_size * total_probes as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Self {
            probe_buffer,
            total_probes,
        }
    }
}

/// GPU pipeline for updating radiance cache SH coefficients from screen probes.
#[cfg(feature = "gpu")]
pub struct SHUpdatePipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub params_layout: wgpu::BindGroupLayout,
    pub screen_data_layout: wgpu::BindGroupLayout,
    pub cache_data_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
}

#[cfg(feature = "gpu")]
impl SHUpdatePipeline {
    pub fn new(device: &wgpu::Device) -> Self {
        use crate::types::SHUpdateParams;

        // G0: SHUpdateParams uniform
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SH Update Params Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // G1: screen_probes (read) + filtered_irradiance (read)
        let screen_data_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SH Update Screen Data Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // G2: cache_probes (read_write)
        let cache_data_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SH Update Cache Data Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Radiance Cache SH Update Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_radiance_cache_sh_update.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SH Update Pipeline Layout"),
            bind_group_layouts: &[&params_layout, &screen_data_layout, &cache_data_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen SH Update Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SH Update Params Buffer"),
            size: std::mem::size_of::<SHUpdateParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            params_layout,
            screen_data_layout,
            cache_data_layout,
            params_buffer,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radiance_cache_grid() {
        let config = LumenConfig {
            radiance_cache_probe_spacing: 4.0,
            sdf_trace_max_distance: 200.0,
            ..Default::default()
        };
        let cache = RadianceCache::new(&config);
        // 200*2/4 = 100, capped at 64
        assert_eq!(cache.grid_size, 64);
        assert_eq!(cache.total_probes, 64 * 64 * 64);
    }

    #[test]
    fn test_probe_world_pos() {
        let config = LumenConfig {
            radiance_cache_probe_spacing: 2.0,
            sdf_trace_max_distance: 10.0,
            ..Default::default()
        };
        let mut cache = RadianceCache::new(&config);
        cache.update_origin([0.0, 0.0, 0.0]);

        let pos = cache.probe_world_pos(0);
        // First probe should be at the negative corner
        assert!(pos[0] < 0.0);
    }

    #[test]
    fn test_update_range_wraps() {
        let config = LumenConfig {
            radiance_cache_probe_spacing: 4.0,
            sdf_trace_max_distance: 8.0, // Small for testing
            ..Default::default()
        };
        let mut cache = RadianceCache::new(&config);
        // grid_size = ceil(16/4) = 4, total = 64

        let (s1, e1) = cache.update_range();
        assert_eq!(s1, 0);
        assert!(e1 <= cache.total_probes);

        // Advance enough to wrap
        for _ in 0..100 {
            cache.update_range();
        }
        // Should have wrapped at least once
    }
}
