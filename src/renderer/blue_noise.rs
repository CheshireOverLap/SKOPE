// SKOPE Engine — Blue Noise Generator
//
// Provides spatiotemporally stratified blue noise for stochastic
// sampling across the renderer (TAA jitter, GTAO directions,
// SSR ray offsets, MegaLights sample selection, TSR jitter, etc.)
//
// Instead of storing precomputed textures, we use a compute shader
// to generate high-quality blue noise sequences using the
// R2 quasi-random sequence (generalized golden ratio) with
// per-frame temporal offset.
//
// UE5 equivalent: FBlueNoise in BlueNoise.h/cpp
// Reference: "A Low-Discrepancy Sampler that Distributes Monte Carlo
//             Errors as a Blue Noise in Screen Space" (Heitz & Belcour 2019)

use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Blue noise configuration
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BlueNoiseParams {
    /// Screen dimensions
    pub screen_width: u32,
    pub screen_height: u32,
    /// Frame index (for temporal offset)
    pub frame_index: u32,
    /// Number of dimensions to generate (1-4)
    pub dimensions: u32,
}

/// Blue noise sample (up to 4 dimensions)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct BlueNoiseSample {
    pub values: [f32; 4],
}

/// Precomputed R2 sequence constants
const PHI2: f64 = 1.3247179572447460; // Plastic constant (real root of x^3 = x + 1)
const ALPHA_1: f64 = 1.0 / 1.3247179572447460;
const ALPHA_2: f64 = 1.0 / (1.3247179572447460 * 1.3247179572447460);

// ---------------------------------------------------------------------------
// CPU-side Blue Noise
// ---------------------------------------------------------------------------

/// CPU blue noise generator (for jitter patterns, etc.)
pub struct BlueNoiseGenerator {
    frame_index: u32,
}

impl BlueNoiseGenerator {
    pub fn new() -> Self {
        Self { frame_index: 0 }
    }

    /// Advance to next frame
    pub fn next_frame(&mut self) {
        self.frame_index = self.frame_index.wrapping_add(1);
    }

    /// Get current frame index
    pub fn frame_index(&self) -> u32 {
        self.frame_index
    }

    /// R2 quasi-random sample (2D) for a given pixel and frame
    /// Returns values in [0, 1)
    pub fn r2_sample(&self, pixel_x: u32, pixel_y: u32) -> [f32; 2] {
        // Combine pixel position with frame index for temporal variation
        let seed = pixel_x.wrapping_mul(1973) ^ pixel_y.wrapping_mul(9277) ^ self.frame_index.wrapping_mul(26699);
        let n = seed as f64 + 0.5;

        let x = (n * ALPHA_1).fract() as f32;
        let y = (n * ALPHA_2).fract() as f32;
        [x, y]
    }

    /// Halton sequence sample (for TAA jitter patterns)
    /// base 2 and base 3
    pub fn halton_sample(&self, index: u32) -> [f32; 2] {
        [halton(index, 2), halton(index, 3)]
    }

    /// Get TAA/TSR jitter offset for current frame
    /// Returns offset in [-0.5, 0.5) range (pixel units)
    pub fn jitter_offset(&self, sequence_length: u32) -> [f32; 2] {
        let idx = self.frame_index % sequence_length;
        let h = self.halton_sample(idx + 1); // Halton is 1-indexed
        [h[0] - 0.5, h[1] - 0.5]
    }

    /// Get a 1D blue noise value for the current frame at a pixel
    pub fn sample_1d(&self, pixel_x: u32, pixel_y: u32) -> f32 {
        self.r2_sample(pixel_x, pixel_y)[0]
    }

    /// Get stratified samples across N frames for temporal accumulation
    pub fn stratified_temporal(&self, total_samples: u32) -> Vec<[f32; 2]> {
        let mut samples = Vec::with_capacity(total_samples as usize);
        for i in 0..total_samples {
            samples.push(self.halton_sample(self.frame_index * total_samples + i + 1));
        }
        samples
    }
}

impl Default for BlueNoiseGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// GPU-side Blue Noise
// ---------------------------------------------------------------------------

/// GPU blue noise texture (computed per-frame)
pub struct GpuBlueNoise {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,

    /// Blue noise texture (Rgba32Float, screen-resolution / 4 tiled)
    pub noise_texture: wgpu::Texture,
    pub noise_view: wgpu::TextureView,

    noise_width: u32,
    noise_height: u32,
}

impl GpuBlueNoise {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blue Noise Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba32Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blue Noise Shader"),
            source: wgpu::ShaderSource::Wgsl(BLUE_NOISE_SHADER.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blue Noise Pipeline Layout"),
            bind_group_layouts: &[&layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Blue Noise Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blue Noise Params"),
            size: std::mem::size_of::<BlueNoiseParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Quarter-resolution noise texture (tiled)
        let noise_width = (width + 3) / 4;
        let noise_height = (height + 3) / 4;

        let noise_texture = create_noise_texture(device, noise_width, noise_height);
        let noise_view = noise_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            pipeline,
            layout,
            params_buffer,
            noise_texture,
            noise_view,
            noise_width,
            noise_height,
        }
    }

    /// Generate blue noise for this frame
    pub fn generate(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame_index: u32,
    ) {
        let params = BlueNoiseParams {
            screen_width: self.noise_width,
            screen_height: self.noise_height,
            frame_index,
            dimensions: 4,
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blue Noise Bind Group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.noise_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Blue Noise Generate"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(
                (self.noise_width + 7) / 8,
                (self.noise_height + 7) / 8,
                1,
            );
        }
    }

    /// Resize noise texture
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let new_w = (width + 3) / 4;
        let new_h = (height + 3) / 4;
        if new_w == self.noise_width && new_h == self.noise_height {
            return;
        }
        self.noise_width = new_w;
        self.noise_height = new_h;
        self.noise_texture = create_noise_texture(device, new_w, new_h);
        self.noise_view = self.noise_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Halton sequence (Van der Corput)
fn halton(mut index: u32, base: u32) -> f32 {
    let mut f = 1.0f32;
    let mut r = 0.0f32;
    let inv_base = 1.0 / base as f32;

    while index > 0 {
        f *= inv_base;
        r += f * (index % base) as f32;
        index /= base;
    }

    r
}

fn create_noise_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Blue Noise Texture"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

// Inline shader for GPU blue noise generation
const BLUE_NOISE_SHADER: &str = r#"
// Blue Noise Generation — R2 quasi-random with temporal offset
//
// Generates spatiotemporally stratified blue noise using the
// generalized golden ratio (R2 sequence) with per-frame rotation.

struct BlueNoiseParams {
    screen_width:  u32,
    screen_height: u32,
    frame_index:   u32,
    dimensions:    u32,
};

@group(0) @binding(0) var<uniform> params: BlueNoiseParams;
@group(0) @binding(1) var output: texture_storage_2d<rgba32float, write>;

// R2 sequence constants
const PHI2: f32 = 1.3247179572447460;
const ALPHA1: f32 = 0.7548776662466928; // 1/PHI2
const ALPHA2: f32 = 0.5698402909980532; // 1/PHI2^2

// Owen-scrambled Sobol-like hash for decorrelation
fn hash_wang(seed: u32) -> u32 {
    var s = seed;
    s = (s ^ 61u) ^ (s >> 16u);
    s = s * 9u;
    s = s ^ (s >> 4u);
    s = s * 0x27d4eb2du;
    s = s ^ (s >> 15u);
    return s;
}

fn hash_to_float(h: u32) -> f32 {
    return f32(h & 0x00FFFFFFu) / f32(0x01000000u);
}

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.screen_width || gid.y >= params.screen_height {
        return;
    }

    // Spatial seed from pixel position
    let spatial_seed = gid.x + gid.y * params.screen_width;

    // Temporal offset using frame index
    let temporal = f32(params.frame_index) + 0.5;

    // R2 sequence with spatial hashing for decorrelation
    let h = hash_wang(spatial_seed);
    let offset = hash_to_float(h);

    let n = f32(spatial_seed) + temporal;
    let r2_x = fract(n * ALPHA1 + offset);
    let r2_y = fract(n * ALPHA2 + offset * 0.7);

    // Additional dimensions using different hashes
    let h2 = hash_wang(spatial_seed ^ 0xDEADBEEFu);
    let h3 = hash_wang(spatial_seed ^ 0xCAFEBABEu);
    let r2_z = fract(f32(h2) / 4294967296.0 + temporal * ALPHA1);
    let r2_w = fract(f32(h3) / 4294967296.0 + temporal * ALPHA2);

    let pixel = vec2<i32>(i32(gid.x), i32(gid.y));
    textureStore(output, pixel, vec4<f32>(r2_x, r2_y, r2_z, r2_w));
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halton_base2() {
        // Halton(1, 2) = 0.5
        assert!((halton(1, 2) - 0.5).abs() < 1e-6);
        // Halton(2, 2) = 0.25
        assert!((halton(2, 2) - 0.25).abs() < 1e-6);
        // Halton(3, 2) = 0.75
        assert!((halton(3, 2) - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_halton_base3() {
        // Halton(1, 3) = 1/3
        assert!((halton(1, 3) - 1.0 / 3.0).abs() < 1e-6);
        // Halton(2, 3) = 2/3
        assert!((halton(2, 3) - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_blue_noise_generator() {
        let mut gen = BlueNoiseGenerator::new();
        let s1 = gen.r2_sample(0, 0);
        gen.next_frame();
        let s2 = gen.r2_sample(0, 0);

        // Samples should differ between frames
        assert!(s1 != s2);

        // Samples should be in [0, 1)
        assert!(s1[0] >= 0.0 && s1[0] < 1.0);
        assert!(s1[1] >= 0.0 && s1[1] < 1.0);
    }

    #[test]
    fn test_jitter_offset() {
        let gen = BlueNoiseGenerator::new();
        let jitter = gen.jitter_offset(8);
        // Jitter should be in [-0.5, 0.5)
        assert!(jitter[0] >= -0.5 && jitter[0] < 0.5);
        assert!(jitter[1] >= -0.5 && jitter[1] < 0.5);
    }
}
