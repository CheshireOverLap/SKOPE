//! Lumen Screen Probe System
//!
//! Manages the screen-space probe grid for irradiance gathering.
//! The probe grid covers the screen at regular intervals (e.g., every 16 pixels).
//!
//! Pipeline per frame:
//! 1. Place probes on visible surfaces (reads depth + normal)
//! 2. Gather radiance per probe (HZB + SDF trace)
//! 3. Filter (spatial bilateral + temporal EMA)
//! 4. Store filtered irradiance for composite

use crate::types::{ScreenProbeParams, LumenConfig};
#[cfg(feature = "gpu")]
use crate::types::ScreenProbe;

/// Maximum probes at 1080p with spacing=16: (1920/16)*(1080/16) = 120*67.5 ≈ 8100
pub const MAX_SCREEN_PROBES: u32 = 32768;

/// Directions per probe for radiance gathering.
pub const DIRECTIONS_PER_PROBE: u32 = 8;

/// Screen probe grid state.
pub struct ScreenProbeGrid {
    pub spacing: u32,
    pub probes_x: u32,
    pub probes_y: u32,
    pub frame_index: u64,
}

impl ScreenProbeGrid {
    pub fn new(config: &LumenConfig, screen_width: u32, screen_height: u32) -> Self {
        let spacing = config.screen_probe_spacing;
        Self {
            spacing,
            probes_x: (screen_width + spacing - 1) / spacing,
            probes_y: (screen_height + spacing - 1) / spacing,
            frame_index: 0,
        }
    }

    /// Update grid dimensions on viewport resize.
    pub fn resize(&mut self, screen_width: u32, screen_height: u32) {
        self.probes_x = (screen_width + self.spacing - 1) / self.spacing;
        self.probes_y = (screen_height + self.spacing - 1) / self.spacing;
    }

    /// Total number of probes in the grid.
    pub fn total_probes(&self) -> u32 {
        self.probes_x * self.probes_y
    }

    /// Compute jitter offset for temporal accumulation.
    pub fn jitter(&self) -> (f32, f32) {
        // Halton sequence for subpixel jitter
        let frame = self.frame_index as u32;
        let jx = halton(frame, 2) * self.spacing as f32 - self.spacing as f32 * 0.5;
        let jy = halton(frame, 3) * self.spacing as f32 - self.spacing as f32 * 0.5;
        (jx, jy)
    }

    /// Build placement shader params.
    pub fn placement_params(
        &self,
        screen_width: u32,
        screen_height: u32,
        sdf_max_steps: u32,
        max_trace_distance: f32,
    ) -> ScreenProbeParams {
        let (jx, jy) = self.jitter();
        ScreenProbeParams {
            probe_spacing: self.spacing,
            jitter_x: jx,
            jitter_y: jy,
            screen_width,
            screen_height,
            frame_index: self.frame_index as u32,
            sdf_max_steps,
            max_trace_distance,
        }
    }

    /// Advance to next frame.
    pub fn next_frame(&mut self) {
        self.frame_index += 1;
    }
}

/// Halton sequence for quasi-random sampling.
fn halton(index: u32, base: u32) -> f32 {
    let mut f = 1.0f32;
    let mut r = 0.0f32;
    let mut i = index;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }
    r
}

/// GPU resources for the screen probe pipeline.
#[cfg(feature = "gpu")]
pub struct ScreenProbePipeline {
    // Placement pass
    pub place_pipeline: wgpu::ComputePipeline,
    pub place_params_layout: wgpu::BindGroupLayout,
    pub place_gbuffer_layout: wgpu::BindGroupLayout,
    pub place_output_layout: wgpu::BindGroupLayout,

    // Gather pass
    pub gather_pipeline: wgpu::ComputePipeline,
    pub gather_params_layout: wgpu::BindGroupLayout,
    pub gather_input_layout: wgpu::BindGroupLayout,
    pub gather_output_layout: wgpu::BindGroupLayout,

    // Filter pass
    pub filter_pipeline: wgpu::ComputePipeline,
    pub filter_params_layout: wgpu::BindGroupLayout,
    pub filter_input_layout: wgpu::BindGroupLayout,
    pub filter_history_layout: wgpu::BindGroupLayout,
    pub filter_output_layout: wgpu::BindGroupLayout,

    // Buffers
    pub params_buffer: wgpu::Buffer,
    pub probe_buffer: wgpu::Buffer,
    pub probe_count_buffer: wgpu::Buffer,
    pub radiance_buffer: wgpu::Buffer,
    pub history_buffer: wgpu::Buffer,
    pub filtered_buffer: wgpu::Buffer,
}

#[cfg(feature = "gpu")]
impl ScreenProbePipeline {
    pub fn new(
        device: &wgpu::Device,
        max_probes: u32,
    ) -> Self {
        // ── Shaders ──────────────────────────────────────────────
        let place_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Screen Probe Place"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_screen_probe_place.wgsl").into(),
            ),
        });

        let gather_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Screen Probe Gather"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_screen_probe_gather.wgsl").into(),
            ),
        });

        let filter_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lumen Screen Probe Filter"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/lumen_screen_probe_filter.wgsl").into(),
            ),
        });

        // ── Placement layouts ────────────────────────────────────
        let place_params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Place G0"),
            entries: &[
                // ScreenProbeParams
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ScreenProbeParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // CameraData
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let place_gbuffer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Place G1: GBuffer"),
            entries: &[
                // Depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Normal texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let place_output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Place G2: Output"),
            entries: &[
                // Probe buffer (read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Probe count (atomic)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(4),
                    },
                    count: None,
                },
            ],
        });

        let place_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Place Layout"),
            bind_group_layouts: &[&place_params_layout, &place_gbuffer_layout, &place_output_layout],
            immediate_size: 0,
        });

        let place_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Screen Probe Place Pipeline"),
            layout: Some(&place_pipeline_layout),
            module: &place_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Gather layouts ───────────────────────────────────────
        let gather_params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Gather G0"),
            entries: &[
                // GatherParams
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // CameraData
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // SDFVolumeParams
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let gather_input_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Gather G1: Input"),
            entries: &[
                // Probes (read)
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
                // HZB texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // HZB sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
                // SDF volume
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                // SDF sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let gather_output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Gather G2: Output"),
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

        let gather_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Gather Layout"),
            bind_group_layouts: &[&gather_params_layout, &gather_input_layout, &gather_output_layout],
            immediate_size: 0,
        });

        let gather_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Screen Probe Gather Pipeline"),
            layout: Some(&gather_pipeline_layout),
            module: &gather_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Filter layouts ───────────────────────────────────────
        let filter_params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Filter G0"),
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

        let filter_input_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Filter G1"),
            entries: &[
                // Probes (read)
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
                // Raw radiance (read)
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

        let filter_history_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Filter G2: History"),
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

        let filter_output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lumen Filter G3: Output"),
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

        let filter_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lumen Filter Layout"),
            bind_group_layouts: &[
                &filter_params_layout,
                &filter_input_layout,
                &filter_history_layout,
                &filter_output_layout,
            ],
            immediate_size: 0,
        });

        let filter_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Lumen Screen Probe Filter Pipeline"),
            layout: Some(&filter_pipeline_layout),
            module: &filter_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Buffers ──────────────────────────────────────────────
        let probe_size = std::mem::size_of::<ScreenProbe>() as u64;
        let probe_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Screen Probes"),
            size: probe_size * max_probes as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let probe_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Probe Count"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Screen Probe Params"),
            size: 256, // Generous, covers all param structs
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Per-probe radiance: 8 directions * vec4<f32>
        let radiance_entries = max_probes as u64 * DIRECTIONS_PER_PROBE as u64;
        let radiance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Probe Radiance"),
            size: radiance_entries * 16, // vec4<f32> = 16 bytes
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // History + filtered: one vec4 per probe
        let per_probe_size = max_probes as u64 * 16;
        let history_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Probe History"),
            size: per_probe_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let filtered_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lumen Filtered Irradiance"),
            size: per_probe_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Self {
            place_pipeline,
            place_params_layout,
            place_gbuffer_layout,
            place_output_layout,
            gather_pipeline,
            gather_params_layout,
            gather_input_layout,
            gather_output_layout,
            filter_pipeline,
            filter_params_layout,
            filter_input_layout,
            filter_history_layout,
            filter_output_layout,
            params_buffer,
            probe_buffer,
            probe_count_buffer,
            radiance_buffer,
            history_buffer,
            filtered_buffer,
        }
    }

    /// Reset probe counter before a new frame.
    pub fn reset_counter(&self, queue: &wgpu::Queue) {
        queue.write_buffer(&self.probe_count_buffer, 0, &[0u8; 4]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halton_sequence() {
        let h0 = halton(0, 2);
        let h1 = halton(1, 2);
        let h2 = halton(2, 2);
        assert!((h0 - 0.0).abs() < 0.001);
        assert!((h1 - 0.5).abs() < 0.001);
        assert!((h2 - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_screen_probe_grid() {
        let config = LumenConfig {
            screen_probe_spacing: 16,
            ..Default::default()
        };
        let grid = ScreenProbeGrid::new(&config, 1920, 1080);
        assert_eq!(grid.probes_x, 120);
        assert_eq!(grid.probes_y, 68);
        assert_eq!(grid.total_probes(), 120 * 68);
    }
}
