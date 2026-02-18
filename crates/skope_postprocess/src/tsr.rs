// SKOPE Engine - Temporal Super Resolution (TSR)
//
// 6-phase compute pipeline for temporal upscaling:
// Phase 1:   Motion analysis + disocclusion detection
// Phase 1.5: Velocity dilation (3x3 closest depth)
// Phase 2:   History reprojection with Lanczos upsampling
// Phase 2.5: Shading rejection (neighborhood color clamping)
// Phase 3:   History resolve + anti-flickering
// Phase 4:   RCAS (Robust Contrast-Adaptive Sharpening)

use bytemuck::{Pod, Zeroable};

use super::taa::halton_sequence;

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// TSR quality mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TsrMode {
    /// Disabled (use standard TAA instead)
    Off,
    /// Higher quality, more compute cost
    Quality,
    /// Default balanced mode
    #[default]
    Balanced,
    /// Lower quality, less compute cost
    Performance,
}

/// TSR configuration parameters
#[derive(Debug, Clone)]
pub struct TsrConfig {
    pub mode: TsrMode,
    /// RCAS sharpening strength (0.0 = off, 1.0 = maximum)
    pub sharpness: f32,
    /// Anti-flicker strength (0.0 = off, 1.0 = maximum)
    pub anti_flicker: f32,
    /// History blend weight (0.8 = responsive, 0.98 = stable)
    pub history_weight: f32,
}

impl Default for TsrConfig {
    fn default() -> Self {
        Self {
            mode: TsrMode::Balanced,
            sharpness: 0.5,
            anti_flicker: 0.5,
            history_weight: 0.92,
        }
    }
}

// ---------------------------------------------------------------------------
// GPU uniform struct
// ---------------------------------------------------------------------------

/// TSR parameters (matches WGSL TsrParams struct)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TsrParams {
    pub internal_size: [f32; 2],
    pub output_size: [f32; 2],
    pub inv_internal_size: [f32; 2],
    pub inv_output_size: [f32; 2],
    pub jitter_offset: [f32; 2],
    pub prev_jitter_offset: [f32; 2],
    pub scale_factor: f32,
    pub sharpness: f32,
    pub anti_flicker: f32,
    pub history_weight: f32,
    pub frame_index: u32,
    pub _pad: [u32; 3],
}

// ---------------------------------------------------------------------------
// Halton jitter for TSR (32-frame sequence, wider coverage)
// ---------------------------------------------------------------------------

/// Number of Halton samples for TSR subpixel jitter
const TSR_HALTON_SAMPLES: u32 = 32;

/// Compute jitter offset in NDC space for TSR.
/// Uses a 32-frame Halton(2,3) sequence for wider subpixel coverage
/// compared to standard TAA's 16-frame sequence.
pub fn tsr_halton_jitter(frame_index: u32, internal_width: u32, internal_height: u32) -> [f32; 2] {
    let idx = frame_index % TSR_HALTON_SAMPLES;
    [
        (halton_sequence(idx + 1, 2) - 0.5) * 2.0 / internal_width as f32,
        (halton_sequence(idx + 1, 3) - 0.5) * 2.0 / internal_height as f32,
    ]
}

// ---------------------------------------------------------------------------
// TSR Pipeline
// ---------------------------------------------------------------------------

/// Temporal Super Resolution pipeline.
///
/// Executes 6 compute passes per frame:
/// 1.  Motion analysis: depth-based disocclusion detection + motion coherence
/// 1.5 Velocity dilation: 3x3 closest-depth velocity propagation for thin geometry
/// 2.  Reproject: Lanczos-filtered history reprojection at output resolution
/// 2.5 Shading rejection: color-neighborhood rejection mask for ghosting prevention
/// 3.  Resolve: variance clipping in YCoCg + anti-flicker luminance clamping
/// 4.  Sharpen: RCAS to compensate upscale blur
pub struct TsrPipeline {
    // Phase 1: Motion analysis + disocclusion mask
    pub motion_analysis_pipeline: wgpu::ComputePipeline,
    pub motion_analysis_layout: wgpu::BindGroupLayout,

    // Phase 2: History reprojection (Lanczos upscale)
    pub reproject_pipeline: wgpu::ComputePipeline,
    pub reproject_layout: wgpu::BindGroupLayout,

    // Phase 3: History resolve + anti-flickering
    pub resolve_pipeline: wgpu::ComputePipeline,
    pub resolve_layout: wgpu::BindGroupLayout,

    // Phase 1.5: Velocity dilation (3x3 closest depth)
    pub dilate_velocity_pipeline: wgpu::ComputePipeline,
    pub dilate_velocity_layout: wgpu::BindGroupLayout,

    // Phase 2.5: Shading rejection mask
    pub reject_shading_pipeline: wgpu::ComputePipeline,
    pub reject_shading_layout: wgpu::BindGroupLayout,

    // Phase 1.7: Thin geometry detection
    pub thin_geometry_pipeline: wgpu::ComputePipeline,
    pub thin_geometry_layout: wgpu::BindGroupLayout,

    // Phase 1.8: Flickering luma measurement
    pub flickering_luma_pipeline: wgpu::ComputePipeline,
    pub flickering_luma_layout: wgpu::BindGroupLayout,

    // Phase 4: RCAS sharpening
    pub sharpen_pipeline: wgpu::ComputePipeline,
    pub sharpen_layout: wgpu::BindGroupLayout,

    // Phase 5: Spatial anti-aliasing
    pub spatial_aa_pipeline: wgpu::ComputePipeline,
    pub spatial_aa_layout: wgpu::BindGroupLayout,

    // History color buffers (output resolution, ping-pong)
    pub history_textures: [wgpu::Texture; 2],
    pub history_views: [wgpu::TextureView; 2],

    // History depth buffers (internal resolution, ping-pong)
    pub history_depth: [wgpu::Texture; 2],
    pub history_depth_views: [wgpu::TextureView; 2],

    pub current_history_index: usize,

    // Internal buffers at internal resolution
    pub disocclusion_mask: wgpu::Texture,
    pub disocclusion_mask_view: wgpu::TextureView,
    pub motion_confidence: wgpu::Texture,
    pub motion_confidence_view: wgpu::TextureView,

    // Dilated velocity (internal resolution, RG16Float)
    pub dilated_velocity: wgpu::Texture,
    pub dilated_velocity_view: wgpu::TextureView,

    // Rejection mask (internal resolution, R8Unorm)
    pub rejection_mask: wgpu::Texture,
    pub rejection_mask_view: wgpu::TextureView,

    // Thin geometry mask (internal resolution, R8Unorm)
    pub thin_geometry_mask: wgpu::Texture,
    pub thin_geometry_mask_view: wgpu::TextureView,

    // Flicker map (internal resolution, R8Unorm, ping-pong)
    pub flicker_textures: [wgpu::Texture; 2],
    pub flicker_views: [wgpu::TextureView; 2],

    // Reprojected history at output resolution (intermediate)
    pub reprojected_texture: wgpu::Texture,
    pub reprojected_view: wgpu::TextureView,

    // Resolved output at output resolution (intermediate before sharpen)
    pub resolved_texture: wgpu::Texture,
    pub resolved_view: wgpu::TextureView,

    // Sharpened output (intermediate before spatial AA)
    pub sharpened_texture: wgpu::Texture,
    pub sharpened_view: wgpu::TextureView,

    // Final output at output resolution
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,

    // Params
    pub params_buffer: wgpu::Buffer,
    pub sampler: wgpu::Sampler,

    // Config
    pub config: TsrConfig,
    pub internal_width: u32,
    pub internal_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub frame_index: u32,
    pub prev_jitter: [f32; 2],
}

impl TsrPipeline {
    pub fn new(
        device: &wgpu::Device,
        internal_width: u32,
        internal_height: u32,
        output_width: u32,
        output_height: u32,
        config: TsrConfig,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TSR Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TSR Params Buffer"),
            size: std::mem::size_of::<TsrParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Bind group layouts ---

        let motion_analysis_layout = Self::create_motion_analysis_layout(device);
        let dilate_velocity_layout = Self::create_dilate_velocity_layout(device);
        let thin_geometry_layout = Self::create_thin_geometry_layout(device);
        let flickering_luma_layout = Self::create_flickering_luma_layout(device);
        let reject_shading_layout = Self::create_reject_shading_layout(device);
        let reproject_layout = Self::create_reproject_layout(device);
        let resolve_layout = Self::create_resolve_layout(device);
        let sharpen_layout = Self::create_sharpen_layout(device);
        let spatial_aa_layout = Self::create_spatial_aa_layout(device);

        // --- Shaders ---

        let motion_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Motion Analysis Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_motion_analysis.wgsl").into(),
            ),
        });

        let dilate_velocity_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Dilate Velocity Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_dilate_velocity.wgsl").into(),
            ),
        });

        let reject_shading_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Reject Shading Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_reject_shading.wgsl").into(),
            ),
        });

        let reproject_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Reproject Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_reproject.wgsl").into(),
            ),
        });

        let resolve_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Resolve Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_resolve.wgsl").into(),
            ),
        });

        let thin_geometry_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Thin Geometry Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_thin_geometry.wgsl").into(),
            ),
        });

        let flickering_luma_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Flickering Luma Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_flickering_luma.wgsl").into(),
            ),
        });

        let sharpen_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Sharpen Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_sharpen.wgsl").into(),
            ),
        });

        let spatial_aa_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TSR Spatial AA Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("shaders/tsr_spatial_aa.wgsl").into(),
            ),
        });

        // --- Compute pipelines ---

        let motion_analysis_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Motion Analysis",
            &motion_shader,
            &motion_analysis_layout,
        );

        let dilate_velocity_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Dilate Velocity",
            &dilate_velocity_shader,
            &dilate_velocity_layout,
        );

        let reject_shading_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Reject Shading",
            &reject_shading_shader,
            &reject_shading_layout,
        );

        let reproject_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Reproject",
            &reproject_shader,
            &reproject_layout,
        );

        let resolve_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Resolve",
            &resolve_shader,
            &resolve_layout,
        );

        let thin_geometry_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Thin Geometry",
            &thin_geometry_shader,
            &thin_geometry_layout,
        );

        let flickering_luma_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Flickering Luma",
            &flickering_luma_shader,
            &flickering_luma_layout,
        );

        let sharpen_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Sharpen",
            &sharpen_shader,
            &sharpen_layout,
        );

        let spatial_aa_pipeline = Self::create_compute_pipeline(
            device,
            "TSR Spatial AA",
            &spatial_aa_shader,
            &spatial_aa_layout,
        );

        // --- Textures ---

        let history_textures = Self::create_history_color_textures(device, output_width, output_height);
        let history_views = std::array::from_fn(|i| {
            history_textures[i].create_view(&wgpu::TextureViewDescriptor::default())
        });

        let history_depth = Self::create_history_depth_textures(device, internal_width, internal_height);
        let history_depth_views = std::array::from_fn(|i| {
            history_depth[i].create_view(&wgpu::TextureViewDescriptor::default())
        });

        let disocclusion_mask = Self::create_r8_texture(device, internal_width, internal_height, "TSR Disocclusion Mask");
        let disocclusion_mask_view = disocclusion_mask.create_view(&wgpu::TextureViewDescriptor::default());

        let motion_confidence = Self::create_r8_texture(device, internal_width, internal_height, "TSR Motion Confidence");
        let motion_confidence_view = motion_confidence.create_view(&wgpu::TextureViewDescriptor::default());

        let dilated_velocity = Self::create_rg16_texture(device, internal_width, internal_height, "TSR Dilated Velocity");
        let dilated_velocity_view = dilated_velocity.create_view(&wgpu::TextureViewDescriptor::default());

        let rejection_mask = Self::create_r8_texture(device, internal_width, internal_height, "TSR Rejection Mask");
        let rejection_mask_view = rejection_mask.create_view(&wgpu::TextureViewDescriptor::default());

        let thin_geometry_mask = Self::create_r8_texture(device, internal_width, internal_height, "TSR Thin Geometry Mask");
        let thin_geometry_mask_view = thin_geometry_mask.create_view(&wgpu::TextureViewDescriptor::default());

        let flicker_textures: [wgpu::Texture; 2] = std::array::from_fn(|i| {
            Self::create_r8_texture(device, internal_width, internal_height, &format!("TSR Flicker {}", i))
        });
        let flicker_views: [wgpu::TextureView; 2] = std::array::from_fn(|i| {
            flicker_textures[i].create_view(&wgpu::TextureViewDescriptor::default())
        });

        let reprojected_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Reprojected");
        let reprojected_view = reprojected_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let resolved_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Resolved");
        let resolved_view = resolved_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sharpened_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Sharpened");
        let sharpened_view = sharpened_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let output_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Output");
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            motion_analysis_pipeline,
            motion_analysis_layout,
            dilate_velocity_pipeline,
            dilate_velocity_layout,
            thin_geometry_pipeline,
            thin_geometry_layout,
            flickering_luma_pipeline,
            flickering_luma_layout,
            reject_shading_pipeline,
            reject_shading_layout,
            reproject_pipeline,
            reproject_layout,
            resolve_pipeline,
            resolve_layout,
            sharpen_pipeline,
            sharpen_layout,
            spatial_aa_pipeline,
            spatial_aa_layout,
            history_textures,
            history_views,
            history_depth,
            history_depth_views,
            current_history_index: 0,
            dilated_velocity,
            dilated_velocity_view,
            rejection_mask,
            rejection_mask_view,
            thin_geometry_mask,
            thin_geometry_mask_view,
            flicker_textures,
            flicker_views,
            disocclusion_mask,
            disocclusion_mask_view,
            motion_confidence,
            motion_confidence_view,
            reprojected_texture,
            reprojected_view,
            resolved_texture,
            resolved_view,
            sharpened_texture,
            sharpened_view,
            output_texture,
            output_view,
            params_buffer,
            sampler,
            config,
            internal_width,
            internal_height,
            output_width,
            output_height,
            frame_index: 0,
            prev_jitter: [0.0; 2],
        }
    }

    /// Execute all 4 TSR phases.
    ///
    /// Inputs (all at internal resolution unless noted):
    /// - `color_input_view`: current frame HDR color (internal res)
    /// - `depth_view`: current frame depth (internal res, Depth32Float)
    /// - `velocity_view`: motion vectors (internal res)
    /// - `normal_roughness_view`: packed normals + roughness (internal res)
    pub fn execute(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        color_input_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        velocity_view: &wgpu::TextureView,
        normal_roughness_view: &wgpu::TextureView,
    ) {
        // Frame 0: history_depth starts as zeroes (cleared on creation).
        // The motion analysis shader writes current depth to depth_history_out each frame,
        // so history is populated after the first pass. Zeroed history causes full disocclusion
        // detection on frame 0, which is correct behavior (no valid history exists yet).
        // Note: Depth32Float → R32Float copy is not allowed in wgpu (not copy-compatible).

        // Update params
        let jitter = tsr_halton_jitter(self.frame_index, self.internal_width, self.internal_height);
        let params = TsrParams {
            internal_size: [self.internal_width as f32, self.internal_height as f32],
            output_size: [self.output_width as f32, self.output_height as f32],
            inv_internal_size: [1.0 / self.internal_width as f32, 1.0 / self.internal_height as f32],
            inv_output_size: [1.0 / self.output_width as f32, 1.0 / self.output_height as f32],
            jitter_offset: jitter,
            prev_jitter_offset: self.prev_jitter,
            scale_factor: self.output_width as f32 / self.internal_width as f32,
            sharpness: self.config.sharpness,
            anti_flicker: self.config.anti_flicker,
            history_weight: self.config.history_weight,
            frame_index: self.frame_index,
            _pad: [0; 3],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        let history_read_idx = 1 - self.current_history_index;

        // ---- Phase 1: Motion Analysis ----
        let history_write_idx = self.current_history_index;
        let motion_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Motion Analysis Bind Group"),
            layout: &self.motion_analysis_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.history_depth_views[history_read_idx]) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(velocity_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.disocclusion_mask_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.motion_confidence_view) },
                wgpu::BindGroupEntry { binding: 6, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.history_depth_views[history_write_idx]) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 1: Motion Analysis"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.motion_analysis_pipeline);
            pass.set_bind_group(0, &motion_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.internal_width, self.internal_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 1.5: Velocity Dilation ----
        let dilate_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Dilate Velocity Bind Group"),
            layout: &self.dilate_velocity_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(velocity_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.dilated_velocity_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 1.5: Dilate Velocity"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.dilate_velocity_pipeline);
            pass.set_bind_group(0, &dilate_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.internal_width, self.internal_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 1.7: Thin Geometry Detection ----
        let thin_geometry_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Thin Geometry Bind Group"),
            layout: &self.thin_geometry_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(normal_roughness_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.thin_geometry_mask_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 1.7: Thin Geometry Detection"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.thin_geometry_pipeline);
            pass.set_bind_group(0, &thin_geometry_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.internal_width, self.internal_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 1.8: Flickering Luma Measurement ----
        let flicker_read_idx = 1 - self.current_history_index;
        let flicker_write_idx = self.current_history_index;
        let flicker_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Flickering Luma Bind Group"),
            layout: &self.flickering_luma_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(color_input_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.history_views[history_read_idx]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.dilated_velocity_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.flicker_views[flicker_read_idx]) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.flicker_views[flicker_write_idx]) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 1.8: Flickering Luma"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.flickering_luma_pipeline);
            pass.set_bind_group(0, &flicker_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.internal_width, self.internal_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 2: Reproject ----
        // Use dilated velocity instead of raw velocity for better edge handling
        let reproject_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Reproject Bind Group"),
            layout: &self.reproject_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.history_views[history_read_idx]) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.dilated_velocity_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.disocclusion_mask_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.reprojected_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 5, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 2: Reproject"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.reproject_pipeline);
            pass.set_bind_group(0, &reproject_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.output_width, self.output_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 2.5: Shading Rejection ----
        let reject_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Reject Shading Bind Group"),
            layout: &self.reject_shading_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(color_input_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.history_views[history_read_idx]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.dilated_velocity_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.rejection_mask_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 2.5: Reject Shading"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.reject_shading_pipeline);
            pass.set_bind_group(0, &reject_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.internal_width, self.internal_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 3: Resolve ----
        // Use the flicker map that was just written by phase 1.8
        let flicker_current_idx = self.current_history_index;
        let resolve_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Resolve Bind Group"),
            layout: &self.resolve_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(color_input_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.reprojected_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.disocclusion_mask_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.motion_confidence_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.resolved_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: self.params_buffer.as_entire_binding() },
                // Phase 1.7: thin geometry mask — reduces blend weight at thin edges
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&self.thin_geometry_mask_view) },
                // Phase 1.8: flicker map — suppresses temporal luminance oscillation
                wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::TextureView(&self.flicker_views[flicker_current_idx]) },
                // Phase 2.5: rejection mask — suppresses ghosting from color-neighborhood rejection
                wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::TextureView(&self.rejection_mask_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 3: Resolve"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.resolve_pipeline);
            pass.set_bind_group(0, &resolve_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.output_width, self.output_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 4: Sharpen (RCAS) ----
        let sharpen_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Sharpen Bind Group"),
            layout: &self.sharpen_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.resolved_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.sharpened_view) },
                wgpu::BindGroupEntry { binding: 2, resource: self.params_buffer.as_entire_binding() },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 4: Sharpen (RCAS)"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.sharpen_pipeline);
            pass.set_bind_group(0, &sharpen_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.output_width, self.output_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // ---- Phase 5: Spatial Anti-Aliasing ----
        let spatial_aa_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TSR Spatial AA Bind Group"),
            layout: &self.spatial_aa_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.sharpened_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.output_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TSR Phase 5: Spatial AA"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.spatial_aa_pipeline);
            pass.set_bind_group(0, &spatial_aa_bind_group, &[]);
            let (dx, dy) = dispatch_size(self.output_width, self.output_height, 8);
            pass.dispatch_workgroups(dx, dy, 1);
        }

        // Copy resolved to history for next frame
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.history_textures[self.current_history_index],
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.output_width,
                height: self.output_height,
                depth_or_array_layers: 1,
            },
        );

        // Update state
        self.prev_jitter = jitter;
        self.current_history_index = 1 - self.current_history_index;
        self.frame_index = self.frame_index.wrapping_add(1);
    }

    /// Get the final output texture view
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.output_view
    }

    /// Get current frame's jitter offset in NDC space
    pub fn get_jitter(&self) -> [f32; 2] {
        tsr_halton_jitter(self.frame_index, self.internal_width, self.internal_height)
    }

    /// Apply TSR subpixel jitter to a projection matrix.
    /// The jitter is added as an NDC offset to proj[2][0] and proj[2][1],
    /// identical to how TAA applies its jitter.
    pub fn jitter_projection(&self, proj: glam::Mat4) -> glam::Mat4 {
        let jitter = self.get_jitter();
        let cols = proj.to_cols_array_2d();
        let mut new_cols = cols;
        new_cols[2][0] += jitter[0];
        new_cols[2][1] += jitter[1];
        glam::Mat4::from_cols_array_2d(&new_cols)
    }

    /// Resize all textures when resolution changes
    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        internal_width: u32,
        internal_height: u32,
        output_width: u32,
        output_height: u32,
    ) {
        if self.internal_width == internal_width
            && self.internal_height == internal_height
            && self.output_width == output_width
            && self.output_height == output_height
        {
            return;
        }

        self.internal_width = internal_width;
        self.internal_height = internal_height;
        self.output_width = output_width;
        self.output_height = output_height;
        self.frame_index = 0;
        self.prev_jitter = [0.0; 2];

        // Recreate output-resolution textures
        self.history_textures = Self::create_history_color_textures(device, output_width, output_height);
        self.history_views = std::array::from_fn(|i| {
            self.history_textures[i].create_view(&wgpu::TextureViewDescriptor::default())
        });

        self.history_depth = Self::create_history_depth_textures(device, internal_width, internal_height);
        self.history_depth_views = std::array::from_fn(|i| {
            self.history_depth[i].create_view(&wgpu::TextureViewDescriptor::default())
        });

        self.reprojected_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Reprojected");
        self.reprojected_view = self.reprojected_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.resolved_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Resolved");
        self.resolved_view = self.resolved_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.sharpened_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Sharpened");
        self.sharpened_view = self.sharpened_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.output_texture = Self::create_rgba16_texture(device, output_width, output_height, "TSR Output");
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Recreate internal-resolution textures
        self.dilated_velocity = Self::create_rg16_texture(device, internal_width, internal_height, "TSR Dilated Velocity");
        self.dilated_velocity_view = self.dilated_velocity.create_view(&wgpu::TextureViewDescriptor::default());

        self.rejection_mask = Self::create_r8_texture(device, internal_width, internal_height, "TSR Rejection Mask");
        self.rejection_mask_view = self.rejection_mask.create_view(&wgpu::TextureViewDescriptor::default());

        self.thin_geometry_mask = Self::create_r8_texture(device, internal_width, internal_height, "TSR Thin Geometry Mask");
        self.thin_geometry_mask_view = self.thin_geometry_mask.create_view(&wgpu::TextureViewDescriptor::default());

        self.flicker_textures = std::array::from_fn(|i| {
            Self::create_r8_texture(device, internal_width, internal_height, &format!("TSR Flicker {}", i))
        });
        self.flicker_views = std::array::from_fn(|i| {
            self.flicker_textures[i].create_view(&wgpu::TextureViewDescriptor::default())
        });

        self.disocclusion_mask = Self::create_r8_texture(device, internal_width, internal_height, "TSR Disocclusion Mask");
        self.disocclusion_mask_view = self.disocclusion_mask.create_view(&wgpu::TextureViewDescriptor::default());

        self.motion_confidence = Self::create_r8_texture(device, internal_width, internal_height, "TSR Motion Confidence");
        self.motion_confidence_view = self.motion_confidence.create_view(&wgpu::TextureViewDescriptor::default());
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn create_compute_pipeline(
        device: &wgpu::Device,
        label: &str,
        shader: &wgpu::ShaderModule,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> wgpu::ComputePipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{} Pipeline Layout", label)),
            bind_group_layouts: &[bind_group_layout],
            immediate_size: 0,
        });

        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(&format!("{} Pipeline", label)),
            layout: Some(&pipeline_layout),
            module: shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        })
    }

    fn create_motion_analysis_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Motion Analysis Layout"),
            entries: &[
                // binding 0: current depth
                bgl_texture_entry(0, wgpu::TextureSampleType::Depth),
                // binding 1: previous depth (history, R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(1),
                // binding 2: velocity
                bgl_float_texture_entry(2),
                // binding 3: normal + roughness
                bgl_float_texture_entry(3),
                // binding 4: disocclusion mask (output, R32Float storage)
                bgl_storage_texture_entry(4, wgpu::TextureFormat::R32Float),
                // binding 5: motion confidence (output, R32Float storage)
                bgl_storage_texture_entry(5, wgpu::TextureFormat::R32Float),
                // binding 6: params uniform
                bgl_uniform_entry(6),
                // binding 7: history depth write (R32Float storage, current frame depth for next frame)
                bgl_storage_texture_entry(7, wgpu::TextureFormat::R32Float),
            ],
        })
    }

    fn create_dilate_velocity_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Dilate Velocity Layout"),
            entries: &[
                // binding 0: params uniform
                bgl_uniform_entry(0),
                // binding 1: depth texture
                bgl_texture_entry(1, wgpu::TextureSampleType::Depth),
                // binding 2: velocity texture
                bgl_float_texture_entry(2),
                // binding 3: dilated velocity output (Rg32Float storage)
                bgl_storage_texture_entry(3, wgpu::TextureFormat::Rg32Float),
            ],
        })
    }

    fn create_thin_geometry_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Thin Geometry Layout"),
            entries: &[
                // binding 0: params uniform
                bgl_uniform_entry(0),
                // binding 1: depth texture
                bgl_texture_entry(1, wgpu::TextureSampleType::Depth),
                // binding 2: normal + roughness texture
                bgl_float_texture_entry(2),
                // binding 3: thin geometry mask output (R32Float storage)
                bgl_storage_texture_entry(3, wgpu::TextureFormat::R32Float),
            ],
        })
    }

    fn create_flickering_luma_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Flickering Luma Layout"),
            entries: &[
                // binding 0: params uniform
                bgl_uniform_entry(0),
                // binding 1: current color
                bgl_float_texture_entry(1),
                // binding 2: history color
                bgl_float_texture_entry(2),
                // binding 3: dilated velocity (Rg32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(3),
                // binding 4: previous flicker map (R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(4),
                // binding 5: output flicker map (R32Float storage)
                bgl_storage_texture_entry(5, wgpu::TextureFormat::R32Float),
            ],
        })
    }

    fn create_reject_shading_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Reject Shading Layout"),
            entries: &[
                // binding 0: params uniform
                bgl_uniform_entry(0),
                // binding 1: current color
                bgl_float_texture_entry(1),
                // binding 2: history color
                bgl_float_texture_entry(2),
                // binding 3: depth
                bgl_texture_entry(3, wgpu::TextureSampleType::Depth),
                // binding 4: dilated velocity (Rg32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(4),
                // binding 5: rejection mask output (R32Float storage)
                bgl_storage_texture_entry(5, wgpu::TextureFormat::R32Float),
            ],
        })
    }

    fn create_reproject_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Reproject Layout"),
            entries: &[
                // binding 0: history color
                bgl_float_texture_entry(0),
                // binding 1: velocity (Rg32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(1),
                // binding 2: disocclusion mask (R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(2),
                // binding 3: reprojected output (Rgba16Float storage)
                bgl_storage_texture_entry(3, wgpu::TextureFormat::Rgba16Float),
                // binding 4: sampler
                bgl_sampler_entry(4),
                // binding 5: params uniform
                bgl_uniform_entry(5),
            ],
        })
    }

    fn create_resolve_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Resolve Layout"),
            entries: &[
                // binding 0: current color (internal res)
                bgl_float_texture_entry(0),
                // binding 1: reprojected history (output res)
                bgl_float_texture_entry(1),
                // binding 2: disocclusion mask (R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(2),
                // binding 3: motion confidence (R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(3),
                // binding 4: resolved output (Rgba16Float storage)
                bgl_storage_texture_entry(4, wgpu::TextureFormat::Rgba16Float),
                // binding 5: sampler
                bgl_sampler_entry(5),
                // binding 6: params uniform
                bgl_uniform_entry(6),
                // binding 7: thin geometry mask (R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(7),
                // binding 8: flicker map (R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(8),
                // binding 9: rejection mask (R32Float — textureLoad only)
                bgl_unfilterable_float_texture_entry(9),
            ],
        })
    }

    fn create_sharpen_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Sharpen Layout"),
            entries: &[
                // binding 0: resolved color
                bgl_float_texture_entry(0),
                // binding 1: sharpened output (Rgba16Float storage)
                bgl_storage_texture_entry(1, wgpu::TextureFormat::Rgba16Float),
                // binding 2: params uniform
                bgl_uniform_entry(2),
            ],
        })
    }

    fn create_spatial_aa_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("TSR Spatial AA Layout"),
            entries: &[
                // binding 0: params uniform
                bgl_uniform_entry(0),
                // binding 1: sharpened color input
                bgl_float_texture_entry(1),
                // binding 2: final output (Rgba16Float storage)
                bgl_storage_texture_entry(2, wgpu::TextureFormat::Rgba16Float),
            ],
        })
    }

    fn create_history_color_textures(device: &wgpu::Device, width: u32, height: u32) -> [wgpu::Texture; 2] {
        std::array::from_fn(|i| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("TSR History Color {}", i)),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba16Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
        })
    }

    fn create_history_depth_textures(device: &wgpu::Device, width: u32, height: u32) -> [wgpu::Texture; 2] {
        std::array::from_fn(|i| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&format!("TSR History Depth {}", i)),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::STORAGE_BINDING,
                view_formats: &[],
            })
        })
    }

    fn create_rgba16_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn create_rg16_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        })
    }

    fn create_r8_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING,
            view_formats: &[],
        })
    }
}

// ---------------------------------------------------------------------------
// Bind group layout entry helpers
// ---------------------------------------------------------------------------

fn bgl_float_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Non-filterable float texture entry (for R32Float/Rg32Float that only use textureLoad)
fn bgl_unfilterable_float_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_texture_entry(binding: u32, sample_type: wgpu::TextureSampleType) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_storage_texture_entry(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn bgl_sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn bgl_uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

// ---------------------------------------------------------------------------
// Utility
// ---------------------------------------------------------------------------

/// Compute dispatch size for workgroup_size(8, 8)
fn dispatch_size(width: u32, height: u32, workgroup: u32) -> (u32, u32) {
    (
        (width + workgroup - 1) / workgroup,
        (height + workgroup - 1) / workgroup,
    )
}
