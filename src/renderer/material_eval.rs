// SKOPE Engine - Material Evaluation System
// V-Buffer Material Evaluation via Compute Shader
//
// Bind Groups (4개로 제한):
// Group 0: V-Buffer (triangle_id, barycentric, depth, sampler)
// Group 1: Geometry (vertices, indices, mesh_infos)
// Group 2: Materials + Lighting (materials, sampler, lighting)
// Group 3: Output (HDR storage texture)

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};
use wgpu;

use super::vbuffer::VBuffer;

/// Material 정보 (GPU용)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMaterial {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_strength: f32,
    pub normal_scale: f32,

    pub albedo_tex_idx: i32,
    pub normal_tex_idx: i32,
    pub metallic_roughness_tex_idx: i32,
    pub emissive_tex_idx: i32,
}

impl Default for GpuMaterial {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_strength: 0.0,
            normal_scale: 1.0,
            albedo_tex_idx: -1,
            normal_tex_idx: -1,
            metallic_roughness_tex_idx: -1,
            emissive_tex_idx: -1,
        }
    }
}

/// 메시 정보 (GPU용)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuMeshInfo {
    pub vertex_offset: u32,
    pub index_offset: u32,
    pub index_count: u32,
    pub material_index: u32,
}

/// 라이팅 파라미터 (GPU용)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct MaterialEvalLighting {
    pub view_pos: [f32; 3],
    pub _pad0: f32,
    pub sun_direction: [f32; 3],
    pub _pad1: f32,
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
    pub inv_view_proj: [[f32; 4]; 4],
}

impl Default for MaterialEvalLighting {
    fn default() -> Self {
        Self {
            view_pos: [0.0, 2.0, 5.0],
            _pad0: 0.0,
            sun_direction: [-0.5, -0.7, -0.5],
            _pad1: 0.0,
            sun_color: [1.0, 0.98, 0.95],
            sun_intensity: 3.0,
            ambient_color: [0.1, 0.12, 0.15],
            ambient_intensity: 0.3,
            inv_view_proj: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
}

/// Material Evaluation Pipeline (4 Bind Groups)
pub struct MaterialEvalPipeline {
    pub pipeline: wgpu::ComputePipeline,

    // Bind group layouts (4개)
    pub vbuffer_layout: wgpu::BindGroupLayout,      // Group 0
    pub geometry_layout: wgpu::BindGroupLayout,     // Group 1
    pub material_lighting_layout: wgpu::BindGroupLayout, // Group 2: Materials + Lighting
    pub output_layout: wgpu::BindGroupLayout,       // Group 3

    // Buffers
    pub lighting_buffer: wgpu::Buffer,
    pub mesh_info_buffer: wgpu::Buffer,
    pub material_buffer: wgpu::Buffer,

    // Material sampler
    pub material_sampler: wgpu::Sampler,

    // Bind group for materials + lighting (Group 2)
    pub material_lighting_bind_group: wgpu::BindGroup,

    // HDR 출력
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,
    pub output_bind_group: wgpu::BindGroup,

    // 크기
    pub width: u32,
    pub height: u32,
}

impl MaterialEvalPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        // Group 0: V-Buffer
        let vbuffer_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval VBuffer Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        // Group 1: Geometry
        let geometry_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Geometry Layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        // Group 2: Materials + Lighting (combined)
        let material_lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Material+Lighting Layout"),
            entries: &[
                // binding 0: materials storage buffer
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
                // binding 1: material sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: lighting uniform
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

        // Group 3: Output HDR
        let output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialEval Output Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba16Float,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Create buffers
        let lighting_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MaterialEval Lighting Buffer"),
            size: std::mem::size_of::<MaterialEvalLighting>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mesh_info_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MaterialEval MeshInfo Buffer"),
            size: std::mem::size_of::<GpuMeshInfo>() as u64 * 256,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("MaterialEval Material Buffer"),
            size: std::mem::size_of::<GpuMaterial>() as u64 * 128,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Material sampler
        let material_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("MaterialEval Material Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Material + Lighting bind group (Group 2)
        let material_lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Material+Lighting Bind Group"),
            layout: &material_lighting_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: material_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&material_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: lighting_buffer.as_entire_binding(),
                },
            ],
        });

        // Output texture
        let (output_texture, output_view) = Self::create_output_texture(device, width, height);

        // Output bind group (Group 3)
        let output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Output Bind Group"),
            layout: &output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&output_view),
                },
            ],
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Material Evaluation Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/material_eval.wgsl").into()),
        });

        // Pipeline layout (4 bind groups)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("MaterialEval Pipeline Layout"),
            bind_group_layouts: &[
                &vbuffer_layout,
                &geometry_layout,
                &material_lighting_layout,
                &output_layout,
            ],
            push_constant_ranges: &[],
        });

        // Compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("MaterialEval Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Self {
            pipeline,
            vbuffer_layout,
            geometry_layout,
            material_lighting_layout,
            output_layout,
            lighting_buffer,
            mesh_info_buffer,
            material_buffer,
            material_sampler,
            material_lighting_bind_group,
            output_texture,
            output_view,
            output_bind_group,
            width,
            height,
        }
    }

    fn create_output_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("MaterialEval HDR Output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;

        (self.output_texture, self.output_view) = Self::create_output_texture(device, width, height);

        self.output_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Output Bind Group"),
            layout: &self.output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.output_view),
                },
            ],
        });
    }

    pub fn update_lighting(&self, queue: &wgpu::Queue, lighting: &MaterialEvalLighting) {
        queue.write_buffer(&self.lighting_buffer, 0, bytemuck::cast_slice(&[*lighting]));
    }

    pub fn update_mesh_infos(&self, queue: &wgpu::Queue, mesh_infos: &[GpuMeshInfo]) {
        if !mesh_infos.is_empty() {
            queue.write_buffer(&self.mesh_info_buffer, 0, bytemuck::cast_slice(mesh_infos));
        }
    }

    pub fn update_materials(&self, queue: &wgpu::Queue, materials: &[GpuMaterial]) {
        if !materials.is_empty() {
            queue.write_buffer(&self.material_buffer, 0, bytemuck::cast_slice(materials));
        }
    }

    /// V-Buffer용 Bind Group 생성 (Group 0)
    pub fn create_vbuffer_bind_group(&self, device: &wgpu::Device, vbuffer: &VBuffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval VBuffer Bind Group"),
            layout: &self.vbuffer_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&vbuffer.triangle_id_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&vbuffer.barycentric_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&vbuffer.depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&vbuffer.sampler),
                },
            ],
        })
    }

    /// Geometry Bind Group 생성 (Group 1)
    pub fn create_geometry_bind_group(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        index_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MaterialEval Geometry Bind Group"),
            layout: &self.geometry_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.mesh_info_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Dispatch compute shader
    pub fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        vbuffer_bind_group: &wgpu::BindGroup,
        geometry_bind_group: &wgpu::BindGroup,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("MaterialEval Compute Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, vbuffer_bind_group, &[]);
        pass.set_bind_group(1, geometry_bind_group, &[]);
        pass.set_bind_group(2, &self.material_lighting_bind_group, &[]);
        pass.set_bind_group(3, &self.output_bind_group, &[]);

        let dispatch_x = (self.width + 7) / 8;
        let dispatch_y = (self.height + 7) / 8;
        pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);
    }
}
