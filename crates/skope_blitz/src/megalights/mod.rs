// SKOPE Engine - MegaLights System
// Stochastic light sampling for thousands of lights via RIS
//
// Pipeline:
// 1. Tile Classification: classify screen tiles by light density
// 2. RIS Sampling: per-pixel reservoir sampling of candidate lights
// 3. Denoising: spatiotemporal bilateral filter for noise reduction

mod types;
mod tile_classify;
mod light_sampling;
mod denoiser;
pub mod visible_light_hash;

pub use types::*;
pub use tile_classify::*;
pub use light_sampling::*;
pub use denoiser::*;
pub use visible_light_hash::VisibleLightHash;

use bytemuck;
use glam::Mat4;

/// MegaLights configuration
#[derive(Debug, Clone)]
pub struct MegaLightsConfig {
    pub max_lights: u32,              // Maximum supported lights (10,000+)
    pub samples_per_pixel: u32,       // RIS candidate multiplier (1-4, default 1)
    pub enable_temporal_reuse: bool,  // Temporal accumulation
    pub enable_spatial_reuse: bool,   // Spatial bilateral filter
    pub spatial_radius: u32,          // Bilateral filter radius (pixels)
    pub tile_size: u32,               // Classification tile size (8x8)
    pub temporal_blend: f32,          // Temporal blend factor (0.0-1.0)
}

impl Default for MegaLightsConfig {
    fn default() -> Self {
        Self {
            max_lights: 10_000,
            samples_per_pixel: 1,
            enable_temporal_reuse: true,
            enable_spatial_reuse: true,
            spatial_radius: 2, // 5x5 kernel
            tile_size: 8,
            temporal_blend: 0.8,
        }
    }
}

/// MegaLights stochastic light sampling system
///
/// Handles thousands of dynamic lights efficiently using:
/// - Tile-based classification for adaptive evaluation
/// - Resampled Importance Sampling (RIS) with reservoir sampling
/// - Spatiotemporal denoising for noise-free results at 1 spp
pub struct MegaLightsSystem {
    // Compute passes
    pub classify_pass: TileClassifyPass,
    pub sample_pass: LightSamplingPass,
    pub denoise_pass: DenoiserPass,

    // Buffers
    pub params_buffer: wgpu::Buffer,
    pub reservoir_buffer: wgpu::Buffer,
    pub prev_reservoir_buffer: wgpu::Buffer,
    pub tile_class_buffer: wgpu::Buffer,

    // Output textures
    pub sample_output: wgpu::Texture,
    pub sample_output_view: wgpu::TextureView,
    pub denoised_output: wgpu::Texture,
    pub denoised_output_view: wgpu::TextureView,

    // Previous frame for temporal reuse
    pub prev_output: wgpu::Texture,
    pub prev_output_view: wgpu::TextureView,

    pub config: MegaLightsConfig,
    pub width: u32,
    pub height: u32,
    pub frame_index: u32,
}

impl MegaLightsSystem {
    pub fn new(
        device: &wgpu::Device,
        config: MegaLightsConfig,
        width: u32,
        height: u32,
    ) -> Self {
        let tile_count_x = width.div_ceil(config.tile_size);
        let tile_count_y = height.div_ceil(config.tile_size);
        let total_tiles = (tile_count_x * tile_count_y) as usize;
        let total_pixels = (width * height) as usize;

        // Create compute passes
        let classify_pass = TileClassifyPass::new(device);
        let sample_pass = LightSamplingPass::new(device);
        let denoise_pass = DenoiserPass::new(device);

        // Parameters buffer
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MegaLights Params"),
            size: std::mem::size_of::<MegaLightsParams>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Per-pixel reservoir buffer (current frame)
        let reservoir_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MegaLights Reservoirs"),
            size: (total_pixels * std::mem::size_of::<Reservoir>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Per-pixel reservoir buffer (previous frame, for temporal reuse)
        let prev_reservoir_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MegaLights Prev Reservoirs"),
            size: (total_pixels * std::mem::size_of::<Reservoir>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Per-tile classification buffer
        let tile_class_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MegaLights Tile Classification"),
            size: (total_tiles * std::mem::size_of::<TileClassification>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Sampled lighting output texture
        let sample_output = Self::create_output_texture(device, width, height, "MegaLights Sample Output");
        let sample_output_view = sample_output.create_view(&wgpu::TextureViewDescriptor::default());

        // Denoised output texture
        let denoised_output = Self::create_output_texture(device, width, height, "MegaLights Denoised Output");
        let denoised_output_view = denoised_output.create_view(&wgpu::TextureViewDescriptor::default());

        // Previous frame output (temporal history)
        let prev_output = Self::create_output_texture(device, width, height, "MegaLights Prev Output");
        let prev_output_view = prev_output.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            classify_pass,
            sample_pass,
            denoise_pass,
            params_buffer,
            reservoir_buffer,
            prev_reservoir_buffer,
            tile_class_buffer,
            sample_output,
            sample_output_view,
            denoised_output,
            denoised_output_view,
            prev_output,
            prev_output_view,
            config,
            width,
            height,
            frame_index: 0,
        }
    }

    fn create_output_texture(device: &wgpu::Device, width: u32, height: u32, label: &str) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    /// Update parameters and upload to GPU
    fn update_params(&self, queue: &wgpu::Queue, light_count: u32, inv_view_proj: Mat4) {
        let tile_count_x = self.width.div_ceil(self.config.tile_size);
        let tile_count_y = self.height.div_ceil(self.config.tile_size);

        let params = MegaLightsParams {
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            screen_size: [self.width, self.height],
            tile_size: self.config.tile_size,
            max_lights: light_count.min(self.config.max_lights),
            samples_per_pixel: self.config.samples_per_pixel,
            spatial_radius: if self.config.enable_spatial_reuse {
                self.config.spatial_radius
            } else {
                0
            },
            temporal_blend: if self.config.enable_temporal_reuse {
                self.config.temporal_blend
            } else {
                0.0
            },
            frame_index: self.frame_index,
            tile_count: [tile_count_x, tile_count_y],
            _pad: [0; 2],
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
    }

    /// Run tile classification pass
    pub fn classify_tiles(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        depth_view: &wgpu::TextureView,
        light_buffer: &wgpu::Buffer,
        light_count: u32,
        inv_view_proj: Mat4,
    ) {
        self.update_params(queue, light_count, inv_view_proj);

        let bind_group = self.classify_pass.create_bind_group(
            device,
            &self.params_buffer,
            light_buffer,
            &self.tile_class_buffer,
            depth_view,
        );

        let tile_count_x = self.width.div_ceil(self.config.tile_size);
        let tile_count_y = self.height.div_ceil(self.config.tile_size);

        self.classify_pass.dispatch(encoder, &bind_group, tile_count_x, tile_count_y);
    }

    /// Run RIS light sampling pass
    pub fn sample_lights(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        light_buffer: &wgpu::Buffer,
        light_count: u32,
        inv_view_proj: Mat4,
    ) {
        self.update_params(queue, light_count, inv_view_proj);

        let bind_group = self.sample_pass.create_bind_group(
            device,
            &self.params_buffer,
            light_buffer,
            &self.reservoir_buffer,
            &self.tile_class_buffer,
            depth_view,
            normal_view,
            &self.sample_output_view,
        );

        self.sample_pass.dispatch(encoder, &bind_group, self.width, self.height);
    }

    /// Run spatiotemporal denoising pass
    pub fn denoise(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        velocity_view: &wgpu::TextureView,
        inv_view_proj: Mat4,
    ) {
        self.update_params(queue, 0, inv_view_proj); // light_count not needed for denoise

        let bind_group = self.denoise_pass.create_bind_group(
            device,
            &self.params_buffer,
            &self.sample_output_view,
            depth_view,
            normal_view,
            velocity_view,
            &self.prev_output_view,
            &self.denoised_output_view,
        );

        self.denoise_pass.dispatch(encoder, &bind_group, self.width, self.height);
    }

    /// Copy current denoised output to previous frame buffer for next frame's temporal reuse
    pub fn swap_history(&mut self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.denoised_output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.prev_output,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.frame_index = self.frame_index.wrapping_add(1);
    }

    /// Run full MegaLights pipeline: classify -> sample -> denoise -> swap
    pub fn execute(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
        velocity_view: &wgpu::TextureView,
        light_buffer: &wgpu::Buffer,
        light_count: u32,
        inv_view_proj: Mat4,
    ) {
        // Step 1: Classify tiles
        self.classify_tiles(device, encoder, queue, depth_view, light_buffer, light_count, inv_view_proj);

        // Step 2: Sample lights (RIS)
        self.sample_lights(device, encoder, queue, depth_view, normal_view, light_buffer, light_count, inv_view_proj);

        // Step 3: Denoise (spatial + temporal)
        self.denoise(device, encoder, queue, depth_view, normal_view, velocity_view, inv_view_proj);

        // Step 4: Swap history for next frame
        self.swap_history(encoder);
    }

    /// Recreate size-dependent resources on window resize
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }

        self.width = width;
        self.height = height;

        let tile_count_x = width.div_ceil(self.config.tile_size);
        let tile_count_y = height.div_ceil(self.config.tile_size);
        let total_tiles = (tile_count_x * tile_count_y) as usize;
        let total_pixels = (width * height) as usize;

        // Recreate buffers
        self.reservoir_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MegaLights Reservoirs"),
            size: (total_pixels * std::mem::size_of::<Reservoir>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.prev_reservoir_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MegaLights Prev Reservoirs"),
            size: (total_pixels * std::mem::size_of::<Reservoir>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        self.tile_class_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MegaLights Tile Classification"),
            size: (total_tiles * std::mem::size_of::<TileClassification>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Recreate textures
        self.sample_output = Self::create_output_texture(device, width, height, "MegaLights Sample Output");
        self.sample_output_view = self.sample_output.create_view(&wgpu::TextureViewDescriptor::default());

        self.denoised_output = Self::create_output_texture(device, width, height, "MegaLights Denoised Output");
        self.denoised_output_view = self.denoised_output.create_view(&wgpu::TextureViewDescriptor::default());

        self.prev_output = Self::create_output_texture(device, width, height, "MegaLights Prev Output");
        self.prev_output_view = self.prev_output.create_view(&wgpu::TextureViewDescriptor::default());

        // Reset frame counter to invalidate temporal history
        self.frame_index = 0;

        log::info!(
            "MegaLights resized to {}x{} ({} tiles, {} pixels)",
            width, height, total_tiles, total_pixels
        );
    }

    /// Get the final denoised lighting output view
    pub fn output_view(&self) -> &wgpu::TextureView {
        &self.denoised_output_view
    }

    /// Get the raw (pre-denoise) sampled lighting view
    pub fn raw_sample_view(&self) -> &wgpu::TextureView {
        &self.sample_output_view
    }

    /// Get tile classification buffer for external reads
    pub fn tile_class_buffer(&self) -> &wgpu::Buffer {
        &self.tile_class_buffer
    }

    /// Get reservoir buffer for external reads
    pub fn reservoir_buffer(&self) -> &wgpu::Buffer {
        &self.reservoir_buffer
    }
}
