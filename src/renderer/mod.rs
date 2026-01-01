// SKOPE Integrated Renderer
// V-Buffer Rendering Pipeline with PBR
//
// V-Buffer 렌더링 파이프라인:
// 1. Visibility Pass: 메시 렌더링 → V-Buffer (Triangle ID, Barycentric, Depth)
// 2. Material Eval: Compute Shader로 Material 평가 → HDR
// 3. Post Processing: Bloom, Tonemapping
// 4. Blit: HDR → Screen

#![allow(dead_code)]
#![allow(unused_imports)]

mod resources;
mod vbuffer;
mod material_eval;

pub use resources::{RenderResources, CameraUniform, ModelUniform, LightingUniform, MaterialUniform};
pub use vbuffer::{VBuffer, VisibilityPipeline, VisibilityParams, encode_triangle_id, decode_mesh_index, decode_primitive_index, INVALID_TRIANGLE_ID};
pub use material_eval::{MaterialEvalPipeline, MaterialEvalLighting, GpuMaterial, GpuMeshInfo};

use glam::{Vec3, Mat4};

/// V-Buffer 기반 렌더러
pub struct Renderer {
    // V-Buffer
    pub vbuffer: VBuffer,
    pub visibility_pipeline: VisibilityPipeline,

    // Material Evaluation (Compute)
    pub material_eval: MaterialEvalPipeline,

    // Shared resources
    pub resources: RenderResources,

    // Geometry data (for material eval compute)
    pub geometry_buffer: Option<GeometryBuffer>,

    // Blit (HDR → Screen)
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_bind_group: wgpu::BindGroup,
    blit_sampler: wgpu::Sampler,
    blit_params_buffer: wgpu::Buffer,

    // Settings
    pub settings: RenderSettings,

    width: u32,
    height: u32,
}

/// Geometry data for material evaluation
pub struct GeometryBuffer {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub geometry_bind_group: wgpu::BindGroup,
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
            enable_shadows: true,
            enable_bloom: true,
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
        _shadow_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        // V-Buffer
        let vbuffer = VBuffer::new(device, width, height);

        // Visibility Pipeline
        let visibility_pipeline = VisibilityPipeline::new(device);

        // Material Evaluation Pipeline
        let material_eval = MaterialEvalPipeline::new(device, width, height);

        // Shared resources
        let resources = RenderResources::new(device);

        // Blit pipeline (HDR to screen)
        let (blit_pipeline, blit_bind_group_layout, blit_sampler) =
            Self::create_blit_pipeline(device, surface_format);

        // Blit params buffer
        let blit_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Blit Params Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let blit_bind_group = Self::create_blit_bind_group(
            device,
            &blit_bind_group_layout,
            &material_eval.output_view,
            &blit_sampler,
            &vbuffer.depth_view,
            &blit_params_buffer,
        );

        Self {
            vbuffer,
            visibility_pipeline,
            material_eval,
            resources,
            geometry_buffer: None,
            blit_pipeline,
            blit_bind_group_layout,
            blit_bind_group,
            blit_sampler,
            blit_params_buffer,
            settings,
            width,
            height,
        }
    }

    fn create_blit_pipeline(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout, wgpu::Sampler) {
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

            struct BlitParams {
                debug_mode: u32,
                _pad: vec3<u32>,
            }

            @group(0) @binding(0) var hdr_texture: texture_2d<f32>;
            @group(0) @binding(1) var tex_sampler: sampler;
            @group(0) @binding(2) var depth_texture: texture_depth_2d;
            @group(0) @binding(3) var<uniform> blit_params: BlitParams;

            // ACES Filmic Tonemapping
            fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
                let a = 2.51;
                let b = 0.03;
                let c = 2.43;
                let d = 0.59;
                let e = 0.14;
                return saturate((x * (a * x + b)) / (x * (c * x + d) + e));
            }

            // Sobel edge detection on depth
            fn sobel_depth(pixel: vec2<i32>) -> f32 {
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

            fn detect_edges(pixel: vec2<i32>) -> f32 {
                let depth = textureLoad(depth_texture, pixel, 0);
                if (depth >= 1.0) { return 0.0; }

                let depth_e = sobel_depth(pixel);
                let threshold = 0.008;
                return smoothstep(threshold, threshold * 3.0, depth_e);
            }

            @fragment
            fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                let hdr = textureSample(hdr_texture, tex_sampler, in.uv).rgb;

                // Debug mode: skip tonemapping
                if (blit_params.debug_mode > 0u) {
                    return vec4<f32>(clamp(hdr, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
                }

                // Edge detection for outlines
                let tex_size = textureDimensions(depth_texture);
                let pixel = vec2<i32>(i32(in.uv.x * f32(tex_size.x)), i32(in.uv.y * f32(tex_size.y)));
                let edge = detect_edges(pixel);

                var combined = hdr;

                // Outline (dark edge)
                let outline_color = vec3<f32>(0.02, 0.01, 0.01);
                combined = mix(combined, outline_color, edge * 0.85);

                // ACES Tonemapping
                let tonemapped = aces_tonemap(combined);

                // Gamma correction
                let gamma = pow(tonemapped, vec3<f32>(1.0 / 2.2));

                return vec4<f32>(gamma, 1.0);
            }
        "#;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("V-Buffer Blit Shader"),
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
                // Blit params
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
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
        params_buffer: &wgpu::Buffer,
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
                    resource: params_buffer.as_entire_binding(),
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

        self.vbuffer.resize(device, width, height);
        self.material_eval.resize(device, width, height);

        // Recreate blit bind group
        self.blit_bind_group = Self::create_blit_bind_group(
            device,
            &self.blit_bind_group_layout,
            &self.material_eval.output_view,
            &self.blit_sampler,
            &self.vbuffer.depth_view,
            &self.blit_params_buffer,
        );
    }

    /// Update blit params (debug_mode)
    pub fn update_blit_params(&self, queue: &wgpu::Queue, debug_mode: u32) {
        let data: [u32; 8] = [debug_mode, 0, 0, 0, 0, 0, 0, 0];
        queue.write_buffer(&self.blit_params_buffer, 0, bytemuck::cast_slice(&data));
    }

    /// Update lighting uniforms for material evaluation
    pub fn update_lighting(
        &self,
        queue: &wgpu::Queue,
        camera_view: Mat4,
        camera_proj: Mat4,
        camera_pos: Vec3,
        sun_direction: Vec3,
        sun_color: Vec3,
        sun_intensity: f32,
        _intensity_scale: f32,
        _d_ggx_max: f32,
        _specular_max: f32,
        _roughness_min: f32,
        _debug_mode: u32,
    ) {
        let view_proj = camera_proj * camera_view;
        let inv_view_proj = view_proj.inverse();

        let lighting = MaterialEvalLighting {
            view_pos: [camera_pos.x, camera_pos.y, camera_pos.z],
            _pad0: 0.0,
            sun_direction: [sun_direction.x, sun_direction.y, sun_direction.z],
            _pad1: 0.0,
            sun_color: [sun_color.x, sun_color.y, sun_color.z],
            sun_intensity,
            ambient_color: [0.03, 0.03, 0.05],
            ambient_intensity: 0.3,
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
        };

        self.material_eval.update_lighting(queue, &lighting);
    }

    /// Setup geometry buffers for material evaluation
    pub fn setup_geometry_buffers(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        mesh_infos: &[GpuMeshInfo],
        materials: &[GpuMaterial],
    ) {
        // Create geometry bind group
        let geometry_bind_group = self.material_eval.create_geometry_bind_group(
            device,
            &vertex_buffer,
            &index_buffer,
        );

        self.geometry_buffer = Some(GeometryBuffer {
            vertex_buffer,
            index_buffer,
            geometry_bind_group,
        });

        // Update mesh infos and materials in material_eval's internal buffers
        self.material_eval.update_mesh_infos(queue, mesh_infos);
        self.material_eval.update_materials(queue, materials);
    }

    /// Update mesh infos
    pub fn update_mesh_infos(&self, queue: &wgpu::Queue, mesh_infos: &[GpuMeshInfo]) {
        self.material_eval.update_mesh_infos(queue, mesh_infos);
    }

    /// Update materials
    pub fn update_materials(&self, queue: &wgpu::Queue, materials: &[GpuMaterial]) {
        self.material_eval.update_materials(queue, materials);
    }

    /// Main render function (V-Buffer pipeline)
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        meshes: &[MeshRenderData],
        _shadow_bind_group: &wgpu::BindGroup,
    ) {
        // 1. Visibility Pass (render to V-Buffer)
        {
            let mut visibility_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Visibility Pass"),
                color_attachments: &self.vbuffer.color_attachments(),
                depth_stencil_attachment: Some(self.vbuffer.depth_attachment()),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            visibility_pass.set_pipeline(&self.visibility_pipeline.pipeline);

            for (mesh_idx, mesh) in meshes.iter().enumerate() {
                visibility_pass.set_bind_group(0, mesh.camera_bind_group, &[]);
                visibility_pass.set_bind_group(1, &self.visibility_pipeline.params_bind_group, &[]);
                visibility_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                visibility_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

                // Note: mesh_index should be set via queue.write_buffer before each draw
                // For now, we use the loop index
                let _ = mesh_idx;

                visibility_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        // 2. Material Evaluation - 이 함수에서는 device가 없으므로 스킵
        // render_vbuffer() 함수를 대신 사용하세요
        let _ = &self.geometry_buffer;

        // 3. Blit to screen
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

    /// Render with full V-Buffer pipeline (visibility + material eval + blit)
    pub fn render_vbuffer(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        meshes: &[MeshRenderData],
        queue: &wgpu::Queue,
    ) {
        // DEBUG: 첫 프레임만 로깅
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            log::info!("[V-Buffer] render_vbuffer() called with {} meshes", meshes.len());
            log::info!("[V-Buffer] geometry_buffer is_some: {}", self.geometry_buffer.is_some());
        });

        // 1. Visibility Pass
        {
            let mut visibility_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Visibility Pass"),
                color_attachments: &self.vbuffer.color_attachments(),
                depth_stencil_attachment: Some(self.vbuffer.depth_attachment()),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            visibility_pass.set_pipeline(&self.visibility_pipeline.pipeline);

            for (mesh_idx, mesh) in meshes.iter().enumerate() {
                // Update mesh index for this draw call
                self.visibility_pipeline.update_mesh_index(queue, mesh_idx as u32);

                visibility_pass.set_bind_group(0, mesh.camera_bind_group, &[]);
                visibility_pass.set_bind_group(1, &self.visibility_pipeline.params_bind_group, &[]);
                visibility_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                visibility_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                visibility_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        // 2. Material Evaluation (Compute)
        if let Some(ref geom) = self.geometry_buffer {
            let vbuffer_bind_group = self.material_eval.create_vbuffer_bind_group(device, &self.vbuffer);

            self.material_eval.dispatch(
                encoder,
                &vbuffer_bind_group,
                &geom.geometry_bind_group,
            );
        }

        // 3. Blit to screen
        {
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("V-Buffer Blit Pass"),
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

    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.visibility_pipeline.camera_bind_group_layout
    }

    pub fn material_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        // For compatibility, return resources layout
        // In V-Buffer, materials are handled via storage buffer
        &self.resources.material_bind_group_layout
    }

    /// Get depth view for external use
    pub fn depth_view(&self) -> &wgpu::TextureView {
        &self.vbuffer.depth_view
    }

    /// Update light buffers (compatibility method)
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
