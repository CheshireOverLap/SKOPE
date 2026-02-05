// SKOPE Engine - Magic Circle SDF Rendering
//
// Procedural magic circle rendering using Signed Distance Fields.
// Features: animated runes, layered rings, glow effects.
//
// Reference: "Painting with Math" (Inigo Quilez)

use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Vec4, Mat4};

/// Magic Circle Parameters
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MagicCircleParams {
    /// World position (xyz) + scale (w)
    pub position_scale: [f32; 4],
    /// Rotation (radians) around Y axis
    pub rotation: f32,
    /// Animation time
    pub time: f32,
    /// Ring count (1-5)
    pub ring_count: u32,
    /// Symbol count per ring (4-16)
    pub symbol_count: u32,
    /// Primary color (RGB) + intensity
    pub color_primary: [f32; 4],
    /// Secondary color (RGB) + glow intensity
    pub color_secondary: [f32; 4],
    /// Animation speeds: outer ring, inner ring, symbols, pulse
    pub anim_speeds: [f32; 4],
    /// Rune style: 0=Runic, 1=Arcane, 2=Demonic, 3=Divine
    pub rune_style: u32,
    /// Opacity
    pub opacity: f32,
    /// Ground fade distance
    pub fade_distance: f32,
    pub _pad: f32,
}

impl Default for MagicCircleParams {
    fn default() -> Self {
        Self {
            position_scale: [0.0, 0.1, 0.0, 3.0],  // Slightly above ground, 3m radius
            rotation: 0.0,
            time: 0.0,
            ring_count: 3,
            symbol_count: 8,
            color_primary: [0.2, 0.5, 1.0, 2.0],    // Blue, intensity 2
            color_secondary: [0.8, 0.2, 1.0, 1.5],  // Purple, glow 1.5
            anim_speeds: [0.5, -0.3, 0.2, 1.0],     // Rotation speeds + pulse
            rune_style: 1,  // Arcane
            opacity: 0.9,
            fade_distance: 5.0,
            _pad: 0.0,
        }
    }
}

/// Magic Circle Instance Data
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MagicCircleInstance {
    /// Model matrix (for world transform)
    pub model: [[f32; 4]; 4],
    /// Parameters
    pub params: MagicCircleParams,
}

impl MagicCircleInstance {
    pub fn new(position: Vec3, scale: f32, params: MagicCircleParams) -> Self {
        let model = Mat4::from_translation(position) * Mat4::from_scale(Vec3::splat(scale));
        let mut p = params;
        p.position_scale = [position.x, position.y, position.z, scale];
        Self {
            model: model.to_cols_array_2d(),
            params: p,
        }
    }
}

/// Rune Style presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuneStyle {
    Runic = 0,    // Nordic/Viking style
    Arcane = 1,   // Classic wizard circles
    Demonic = 2,  // Dark/sinister patterns
    Divine = 3,   // Holy/angelic patterns
}

impl RuneStyle {
    pub fn to_u32(self) -> u32 {
        self as u32
    }
}

/// Magic Circle Pipeline
pub struct MagicCirclePipeline {
    /// Render pipeline (billboard quad with SDF)
    render_pipeline: wgpu::RenderPipeline,
    /// Bind group layout
    bind_group_layout: wgpu::BindGroupLayout,
    /// Camera uniform buffer
    camera_buffer: wgpu::Buffer,
    /// Instance buffer (supports multiple circles)
    instance_buffer: wgpu::Buffer,
    /// Instance count uniform
    count_buffer: wgpu::Buffer,
    /// Noise texture for procedural effects
    noise_texture: wgpu::Texture,
    noise_view: wgpu::TextureView,
    /// Sampler
    sampler: wgpu::Sampler,
    /// Max instances
    max_instances: u32,
}

/// Camera data for magic circles
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MagicCircleCameraData {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub screen_size: [f32; 2],
    pub time: f32,
    pub _pad: f32,
}

impl MagicCirclePipeline {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat) -> Self {
        const MAX_INSTANCES: u32 = 64;

        // Create noise texture (256x256 RGBA)
        let noise_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Magic Circle Noise"),
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Generate procedural noise
        let noise_data = Self::generate_noise(256, 256);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &noise_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &noise_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4),
                rows_per_image: Some(256),
            },
            wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
        );

        let noise_view = noise_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Magic Circle Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Buffers
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Camera Buffer"),
            size: std::mem::size_of::<MagicCircleCameraData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Instance Buffer"),
            size: (MAX_INSTANCES as usize * std::mem::size_of::<MagicCircleInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Magic Circle Count Buffer"),
            size: 16,  // u32 + padding
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Magic Circle Bind Group Layout"),
            entries: &[
                // Camera
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Instances
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Noise texture
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Count
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Pipeline
        let render_pipeline = Self::create_pipeline(device, &bind_group_layout, target_format);

        log::info!("[MagicCircle] Pipeline initialized, max {} instances", MAX_INSTANCES);

        Self {
            render_pipeline,
            bind_group_layout,
            camera_buffer,
            instance_buffer,
            count_buffer,
            noise_texture,
            noise_view,
            sampler,
            max_instances: MAX_INSTANCES,
        }
    }

    fn create_pipeline(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        target_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Magic Circle Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/magic_circle.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Magic Circle Pipeline Layout"),
            bind_group_layouts: &[bind_group_layout],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Magic Circle Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],  // Procedural quad
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,  // Additive for glow
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Max,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                cull_mode: None,  // Double-sided
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,  // Don't write depth for transparency
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Generate procedural noise texture
    fn generate_noise(width: u32, height: u32) -> Vec<u8> {
        let mut data = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            for x in 0..width {
                // Multi-octave simplex-like noise approximation
                let fx = x as f32 / width as f32;
                let fy = y as f32 / height as f32;

                let noise1 = Self::hash_noise(fx * 4.0, fy * 4.0);
                let noise2 = Self::hash_noise(fx * 8.0, fy * 8.0) * 0.5;
                let noise3 = Self::hash_noise(fx * 16.0, fy * 16.0) * 0.25;
                let noise4 = Self::hash_noise(fx * 32.0 + 100.0, fy * 32.0 + 100.0) * 0.125;

                let combined = (noise1 + noise2 + noise3 + noise4) / 1.875;
                let value = ((combined * 0.5 + 0.5) * 255.0) as u8;

                // Different noise for each channel
                let r = value;
                let g = ((Self::hash_noise(fx * 4.0 + 50.0, fy * 4.0 + 50.0) * 0.5 + 0.5) * 255.0) as u8;
                let b = ((Self::hash_noise(fx * 4.0 + 100.0, fy * 4.0 + 100.0) * 0.5 + 0.5) * 255.0) as u8;
                let a = 255u8;

                data.extend_from_slice(&[r, g, b, a]);
            }
        }

        data
    }

    /// Simple hash-based noise
    fn hash_noise(x: f32, y: f32) -> f32 {
        let xi = x.floor() as i32;
        let yi = y.floor() as i32;
        let xf = x.fract();
        let yf = y.fract();

        // Smoothstep interpolation
        let u = xf * xf * (3.0 - 2.0 * xf);
        let v = yf * yf * (3.0 - 2.0 * yf);

        let h00 = Self::hash2d(xi, yi);
        let h10 = Self::hash2d(xi + 1, yi);
        let h01 = Self::hash2d(xi, yi + 1);
        let h11 = Self::hash2d(xi + 1, yi + 1);

        let mix_x0 = h00 + (h10 - h00) * u;
        let mix_x1 = h01 + (h11 - h01) * u;

        mix_x0 + (mix_x1 - mix_x0) * v
    }

    fn hash2d(x: i32, y: i32) -> f32 {
        let n = (x.wrapping_mul(127).wrapping_add(y.wrapping_mul(311))) as u32;
        let n = n.wrapping_mul(n).wrapping_mul(n);
        let n = n.wrapping_mul(15731).wrapping_add(789221).wrapping_mul(1376312589);
        (n as f32) / (u32::MAX as f32) * 2.0 - 1.0
    }

    /// Update camera data
    pub fn update_camera(
        &self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: Vec3,
        screen_size: (f32, f32),
        time: f32,
    ) {
        let data = MagicCircleCameraData {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            screen_size: [screen_size.0, screen_size.1],
            time,
            _pad: 0.0,
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&data));
    }

    /// Update instances
    pub fn update_instances(&self, queue: &wgpu::Queue, instances: &[MagicCircleInstance]) {
        let count = instances.len().min(self.max_instances as usize);
        if count > 0 {
            queue.write_buffer(
                &self.instance_buffer,
                0,
                bytemuck::cast_slice(&instances[..count]),
            );
        }
        queue.write_buffer(
            &self.count_buffer,
            0,
            bytemuck::cast_slice(&[count as u32, 0u32, 0u32, 0u32]),
        );
    }

    /// Create bind group
    pub fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Magic Circle Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.count_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Render magic circles
    pub fn render<'a>(
        &'a self,
        _device: &wgpu::Device,
        render_pass: &mut wgpu::RenderPass<'a>,
        instance_count: u32,
        bind_group: &'a wgpu::BindGroup,
    ) {
        if instance_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        // 4 vertices per quad (triangle strip), instanced
        render_pass.draw(0..4, 0..instance_count);
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn max_instances(&self) -> u32 {
        self.max_instances
    }
}

/// Preset magic circle configurations
pub mod presets {
    use super::*;

    pub fn summoning_circle() -> MagicCircleParams {
        MagicCircleParams {
            ring_count: 4,
            symbol_count: 12,
            color_primary: [0.8, 0.2, 0.2, 3.0],    // Red
            color_secondary: [1.0, 0.5, 0.0, 2.0],  // Orange
            anim_speeds: [0.3, -0.5, 0.15, 1.5],
            rune_style: RuneStyle::Demonic.to_u32(),
            ..Default::default()
        }
    }

    pub fn healing_circle() -> MagicCircleParams {
        MagicCircleParams {
            ring_count: 2,
            symbol_count: 6,
            color_primary: [0.2, 1.0, 0.4, 2.5],    // Green
            color_secondary: [0.8, 1.0, 0.8, 1.5],  // Light green
            anim_speeds: [0.2, 0.3, 0.1, 2.0],
            rune_style: RuneStyle::Divine.to_u32(),
            ..Default::default()
        }
    }

    pub fn frost_circle() -> MagicCircleParams {
        MagicCircleParams {
            ring_count: 3,
            symbol_count: 8,
            color_primary: [0.3, 0.7, 1.0, 2.0],    // Ice blue
            color_secondary: [0.9, 0.95, 1.0, 1.2], // White-blue
            anim_speeds: [0.1, -0.15, 0.05, 0.5],   // Slow, icy
            rune_style: RuneStyle::Arcane.to_u32(),
            ..Default::default()
        }
    }

    pub fn ancient_rune() -> MagicCircleParams {
        MagicCircleParams {
            ring_count: 5,
            symbol_count: 16,
            color_primary: [0.7, 0.5, 0.2, 1.5],    // Gold/bronze
            color_secondary: [0.9, 0.8, 0.5, 1.0],
            anim_speeds: [0.05, -0.03, 0.02, 0.3],  // Very slow
            rune_style: RuneStyle::Runic.to_u32(),
            ..Default::default()
        }
    }
}
