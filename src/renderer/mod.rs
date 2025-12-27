// SKOPE Integrated Renderer
// Deferred Rendering Pipeline with PBR

#![allow(dead_code)]

mod gbuffer;
mod resources;

pub use gbuffer::GBuffer;
pub use resources::{RenderResources, CameraUniform, ModelUniform, LightingUniform};

use glam::{Vec3, Mat4};

/// Main integrated renderer (simplified deferred pipeline)
pub struct Renderer {
    // Core
    pub gbuffer: GBuffer,
    pub resources: RenderResources,

    // Passes
    geometry_pipeline: wgpu::RenderPipeline,
    lighting_pipeline: wgpu::RenderPipeline,

    // HDR buffer for lighting output
    hdr_texture: wgpu::Texture,
    hdr_view: wgpu::TextureView,

    // Simple fullscreen blit for now
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_bind_group: wgpu::BindGroup,
    blit_sampler: wgpu::Sampler,

    // Settings
    pub settings: RenderSettings,

    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
pub struct RenderSettings {
    pub enable_shadows: bool,
    pub enable_bloom: bool,
    pub exposure: f32,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            enable_shadows: false,  // Start with shadows disabled
            enable_bloom: false,    // Start with bloom disabled
            exposure: 1.0,
        }
    }
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        settings: RenderSettings,
    ) -> Self {
        // G-Buffer
        let gbuffer = GBuffer::new(device, width, height);

        // Shared resources
        let resources = RenderResources::new(device);

        // HDR buffer for lighting output
        let (hdr_texture, hdr_view) = Self::create_hdr_buffer(device, width, height);

        // Geometry pass pipeline
        let geometry_pipeline = Self::create_geometry_pipeline(device, &gbuffer, &resources);

        // Lighting pass pipeline
        let lighting_pipeline = Self::create_lighting_pipeline(device, &gbuffer, &resources);

        // Blit pipeline (HDR to screen)
        let (blit_pipeline, blit_bind_group_layout, blit_sampler) =
            Self::create_blit_pipeline(device, surface_format);

        let blit_bind_group = Self::create_blit_bind_group(
            device,
            &blit_bind_group_layout,
            &hdr_view,
            &blit_sampler,
            &gbuffer.depth_view,
            &gbuffer.normal_roughness_view,  // G-Buffer RT1 (Normal + Roughness)
        );

        Self {
            gbuffer,
            resources,
            geometry_pipeline,
            lighting_pipeline,
            hdr_texture,
            hdr_view,
            blit_pipeline,
            blit_bind_group_layout,
            blit_bind_group,
            blit_sampler,
            settings,
            width,
            height,
        }
    }

    fn create_hdr_buffer(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR Buffer"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    fn create_geometry_pipeline(
        device: &wgpu::Device,
        gbuffer: &GBuffer,
        resources: &RenderResources,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Geometry Pass Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/geometry_pass.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Geometry Pipeline Layout"),
            bind_group_layouts: &[
                &resources.camera_bind_group_layout,
                &resources.material_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Geometry Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    // Position, Normal, Tangent, UV (matches gltf_loader::Vertex::desc())
                    wgpu::VertexBufferLayout {
                        array_stride: 48, // 3 + 3 + 4 + 2 = 12 floats
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            // Position
                            wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            // Normal
                            wgpu::VertexAttribute {
                                offset: 12,
                                shader_location: 1,
                                format: wgpu::VertexFormat::Float32x3,
                            },
                            // Tangent
                            wgpu::VertexAttribute {
                                offset: 24,
                                shader_location: 2,
                                format: wgpu::VertexFormat::Float32x4,
                            },
                            // UV
                            wgpu::VertexAttribute {
                                offset: 40,
                                shader_location: 3,
                                format: wgpu::VertexFormat::Float32x2,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &gbuffer.color_targets(),
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn create_lighting_pipeline(
        device: &wgpu::Device,
        gbuffer: &GBuffer,
        resources: &RenderResources,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Deferred Lighting Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/deferred_lighting.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lighting Pipeline Layout"),
            bind_group_layouts: &[
                &gbuffer.bind_group_layout,
                &resources.lighting_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lighting Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],  // Fullscreen quad, no vertex input
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn create_blit_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout, wgpu::Sampler) {
        // Enhanced blit shader with ACES tonemapping, bloom, and edge detection outlines
        let shader_src = r#"
            struct VertexOutput {
                @builtin(position) position: vec4<f32>,
                @location(0) uv: vec2<f32>,
            }

            @vertex
            fn vs_main(@builtin(vertex_index) vertex_idx: u32) -> VertexOutput {
                var out: VertexOutput;
                var positions = array<vec2<f32>, 6>(
                    vec2<f32>(-1.0, -1.0),
                    vec2<f32>( 1.0, -1.0),
                    vec2<f32>(-1.0,  1.0),
                    vec2<f32>(-1.0,  1.0),
                    vec2<f32>( 1.0, -1.0),
                    vec2<f32>( 1.0,  1.0),
                );
                var uvs = array<vec2<f32>, 6>(
                    vec2<f32>(0.0, 1.0),
                    vec2<f32>(1.0, 1.0),
                    vec2<f32>(0.0, 0.0),
                    vec2<f32>(0.0, 0.0),
                    vec2<f32>(1.0, 1.0),
                    vec2<f32>(1.0, 0.0),
                );
                out.position = vec4<f32>(positions[vertex_idx], 0.0, 1.0);
                out.uv = uvs[vertex_idx];
                return out;
            }

            @group(0) @binding(0) var hdr_texture: texture_2d<f32>;
            @group(0) @binding(1) var tex_sampler: sampler;
            @group(0) @binding(2) var depth_texture: texture_depth_2d;
            @group(0) @binding(3) var normal_texture: texture_2d<f32>;

            // ACES Filmic Tonemapping
            fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
                let a = 2.51;
                let b = 0.03;
                let c = 2.43;
                let d = 0.59;
                let e = 0.14;
                return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
            }

            // Simple bloom extraction (threshold-based glow)
            fn extract_bloom(color: vec3<f32>, threshold: f32) -> vec3<f32> {
                let brightness = max(max(color.r, color.g), color.b);
                let soft_threshold = threshold * 0.7;
                let contribution = smoothstep(soft_threshold, threshold, brightness);
                return color * contribution;
            }

            // Sobel edge detection on depth
            fn sobel_depth(pixel: vec2<i32>) -> f32 {
                // texture_depth_2d returns f32 directly, no .r accessor needed
                let d00 = textureLoad(depth_texture, pixel + vec2<i32>(-1, -1), 0);
                let d10 = textureLoad(depth_texture, pixel + vec2<i32>( 0, -1), 0);
                let d20 = textureLoad(depth_texture, pixel + vec2<i32>( 1, -1), 0);
                let d01 = textureLoad(depth_texture, pixel + vec2<i32>(-1,  0), 0);
                let d21 = textureLoad(depth_texture, pixel + vec2<i32>( 1,  0), 0);
                let d02 = textureLoad(depth_texture, pixel + vec2<i32>(-1,  1), 0);
                let d12 = textureLoad(depth_texture, pixel + vec2<i32>( 0,  1), 0);
                let d22 = textureLoad(depth_texture, pixel + vec2<i32>( 1,  1), 0);

                let gx = -d00 - 2.0*d01 - d02 + d20 + 2.0*d21 + d22;
                let gy = -d00 - 2.0*d10 - d20 + d02 + 2.0*d12 + d22;

                return sqrt(gx*gx + gy*gy);
            }

            // Decode octahedron normal
            fn decode_octahedron(encoded: vec2<f32>) -> vec3<f32> {
                let f = encoded * 2.0 - 1.0;
                var n = vec3<f32>(f.x, f.y, 1.0 - abs(f.x) - abs(f.y));
                let t = saturate(-n.z);
                if (n.x >= 0.0) { n.x -= t; } else { n.x += t; }
                if (n.y >= 0.0) { n.y -= t; } else { n.y += t; }
                return normalize(n);
            }

            // Normal discontinuity detection
            fn normal_edge(pixel: vec2<i32>) -> f32 {
                let n_center_enc = textureLoad(normal_texture, pixel, 0).rg;
                let n_center = decode_octahedron(n_center_enc);

                var max_diff = 0.0;
                for (var dy = -1; dy <= 1; dy++) {
                    for (var dx = -1; dx <= 1; dx++) {
                        if (dx == 0 && dy == 0) { continue; }
                        let neighbor_enc = textureLoad(normal_texture, pixel + vec2<i32>(dx, dy), 0).rg;
                        let neighbor = decode_octahedron(neighbor_enc);
                        let diff = 1.0 - dot(n_center, neighbor);
                        max_diff = max(max_diff, diff);
                    }
                }
                return max_diff;
            }

            // Combined edge detection
            fn detect_edges(pixel: vec2<i32>) -> f32 {
                let depth = textureLoad(depth_texture, pixel, 0);
                if (depth >= 1.0) { return 0.0; }  // Skip background

                var edge = 0.0;

                // Depth edges (silhouette)
                let depth_e = sobel_depth(pixel);
                let depth_threshold = 0.008;
                if (depth_e > depth_threshold) {
                    edge = max(edge, smoothstep(depth_threshold, depth_threshold * 3.0, depth_e));
                }

                // Normal edges (internal detail)
                let normal_e = normal_edge(pixel);
                let normal_threshold = 0.4;
                if (normal_e > normal_threshold) {
                    edge = max(edge, smoothstep(normal_threshold, normal_threshold * 1.5, normal_e) * 0.6);
                }

                return edge;
            }

            @fragment
            fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                let hdr = textureSample(hdr_texture, tex_sampler, in.uv).rgb;

                // Simple bloom: sample nearby pixels (fixed texel size for 1080p-ish)
                let texel_size = vec2<f32>(1.0 / 1920.0, 1.0 / 1080.0);

                // Extract bloom from bright areas and blur
                var bloom = extract_bloom(hdr, 1.0);

                // 5-tap cross blur
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2(-4.0, 0.0) * texel_size).rgb, 1.0) * 0.15;
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2( 4.0, 0.0) * texel_size).rgb, 1.0) * 0.15;
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2(0.0, -4.0) * texel_size).rgb, 1.0) * 0.15;
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2(0.0,  4.0) * texel_size).rgb, 1.0) * 0.15;

                // Wider bloom
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2(-12.0, 0.0) * texel_size).rgb, 1.2) * 0.1;
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2( 12.0, 0.0) * texel_size).rgb, 1.2) * 0.1;
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2(0.0, -12.0) * texel_size).rgb, 1.2) * 0.1;
                bloom += extract_bloom(textureSample(hdr_texture, tex_sampler, in.uv + vec2(0.0,  12.0) * texel_size).rgb, 1.2) * 0.1;

                // Combine HDR + bloom
                let bloom_intensity = 0.25;
                var combined = hdr + bloom * bloom_intensity;

                // Edge detection for outlines
                let tex_size = textureDimensions(depth_texture);
                let pixel = vec2<i32>(i32(in.uv.x * f32(tex_size.x)), i32(in.uv.y * f32(tex_size.y)));
                let edge = detect_edges(pixel);

                // Outline color (dark, slightly warm)
                let outline_color = vec3<f32>(0.02, 0.01, 0.01);

                // Apply outline (blend towards outline color based on edge strength)
                combined = mix(combined, outline_color, edge * 0.85);

                // ACES Tonemapping
                let tonemapped = aces_tonemap(combined);

                // Gamma correction
                let gamma = pow(tonemapped, vec3<f32>(1.0 / 2.2));

                return vec4<f32>(gamma, 1.0);
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Blit Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Blit Bind Group Layout"),
            entries: &[
                // HDR texture
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
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
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Depth texture (for edge detection)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Normal texture (G-Buffer RT1 for edge detection)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Blit Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Blit Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Blit Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        (pipeline, bind_group_layout, sampler)
    }

    fn create_blit_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        hdr_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        depth_view: &wgpu::TextureView,
        normal_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Blit Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(normal_view),
                },
            ],
        })
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.width = width;
        self.height = height;

        self.gbuffer.resize(device, width, height);
        (self.hdr_texture, self.hdr_view) = Self::create_hdr_buffer(device, width, height);

        // Recreate blit bind group with new HDR view and G-Buffer views
        self.blit_bind_group = Self::create_blit_bind_group(
            device,
            &self.blit_bind_group_layout,
            &self.hdr_view,
            &self.blit_sampler,
            &self.gbuffer.depth_view,
            &self.gbuffer.normal_roughness_view,
        );
    }

    /// Update lighting uniforms
    pub fn update_lighting(
        &self,
        queue: &wgpu::Queue,
        camera_view: Mat4,
        camera_proj: Mat4,
        camera_pos: Vec3,
        sun_direction: Vec3,
        sun_color: Vec3,
        sun_intensity: f32,
    ) {
        let view_proj = camera_proj * camera_view;
        let inv_view_proj = view_proj.inverse();

        let uniform = LightingUniform {
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_position: [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z, 0.0],
            sun_color: [sun_color.x, sun_color.y, sun_color.z, sun_intensity],
            ambient_color: [0.03, 0.03, 0.05, 1.0],
            screen_size: [self.width as f32, self.height as f32],
            time: 0.0,
            exposure: self.settings.exposure,
        };

        self.resources.update_lighting(queue, &uniform);
    }

    /// Main render function
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        meshes: &[MeshRenderData],
    ) {
        // 1. Geometry Pass (render to G-Buffer)
        {
            let mut geometry_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Geometry Pass"),
                color_attachments: &self.gbuffer.color_attachments(),
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.gbuffer.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            geometry_pass.set_pipeline(&self.geometry_pipeline);

            for mesh in meshes {
                geometry_pass.set_bind_group(0, mesh.camera_bind_group, &[]);
                geometry_pass.set_bind_group(1, mesh.material_bind_group, &[]);
                geometry_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                geometry_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                geometry_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        // 2. Lighting Pass (G-Buffer -> HDR)
        {
            let mut lighting_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Lighting Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            lighting_pass.set_pipeline(&self.lighting_pipeline);
            lighting_pass.set_bind_group(0, &self.gbuffer.bind_group, &[]);
            lighting_pass.set_bind_group(1, &self.resources.lighting_bind_group, &[]);

            // Draw fullscreen quad (6 vertices, no buffer)
            lighting_pass.draw(0..6, 0..1);
        }

        // 3. Blit to screen (HDR -> LDR with simple tonemapping)
        {
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Blit Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            blit_pass.set_pipeline(&self.blit_pipeline);
            blit_pass.set_bind_group(0, &self.blit_bind_group, &[]);
            blit_pass.draw(0..6, 0..1);
        }
    }

    pub fn geometry_pipeline(&self) -> &wgpu::RenderPipeline {
        &self.geometry_pipeline
    }

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.resources.camera_bind_group_layout
    }

    pub fn material_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.resources.material_bind_group_layout
    }

    /// Update light buffers from LightManager
    pub fn update_light_buffers(
        &mut self,
        device: &wgpu::Device,
        light_buffer: &wgpu::Buffer,
        light_count_buffer: &wgpu::Buffer,
    ) {
        self.resources.update_light_buffers(device, light_buffer, light_count_buffer);
    }
}

/// Per-mesh render data
pub struct MeshRenderData<'a> {
    pub vertex_buffer: &'a wgpu::Buffer,
    pub index_buffer: &'a wgpu::Buffer,
    pub index_count: u32,
    pub camera_bind_group: &'a wgpu::BindGroup,
    pub material_bind_group: &'a wgpu::BindGroup,
}
