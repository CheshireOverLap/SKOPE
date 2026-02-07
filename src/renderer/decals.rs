// SKOPE Engine — DBuffer Decals
//
// Deferred decal system using DBuffer approach:
// 1. Project box-volume decals onto depth buffer
// 2. Write albedo/normal/roughness overlays to DBuffer textures
// 3. Composite in material_eval pass
//
// Reference: UE5 PostProcessDeferredDecals.cpp

use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Per-decal GPU data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DecalData {
    /// Transform from world space to decal-local [-1,1]^3
    pub world_to_decal: [[f32; 4]; 4],
    /// RGBA tint (A = opacity)
    pub albedo_tint: [f32; 4],
    /// Normal perturbation strength
    pub normal_strength: f32,
    /// Roughness override value
    pub roughness_value: f32,
    /// Roughness blend factor (0 = no override, 1 = full)
    pub roughness_blend: f32,
    pub _pad: f32,
}

/// Decal pass uniform params
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DecalParams {
    pub screen_width: u32,
    pub screen_height: u32,
    pub decal_count: u32,
    pub _pad: u32,
    pub inv_view_proj: [[f32; 4]; 4],
}

/// Maximum decals per frame
pub const MAX_DECALS: usize = 256;

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

pub struct DBufferDecalPipeline {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,

    // DBuffer textures
    pub dbuffer_albedo: wgpu::Texture,
    pub dbuffer_albedo_view: wgpu::TextureView,
    pub dbuffer_normal: wgpu::Texture,
    pub dbuffer_normal_view: wgpu::TextureView,
    pub dbuffer_roughness: wgpu::Texture,
    pub dbuffer_roughness_view: wgpu::TextureView,

    // GPU buffers
    params_buffer: wgpu::Buffer,
    decal_buffer: wgpu::Buffer,

    screen_width: u32,
    screen_height: u32,
}

impl DBufferDecalPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DBuffer Decal Layout"),
            entries: &[
                // binding 0: params uniform
                bgl_uniform(0),
                // binding 1: decal data (storage)
                bgl_storage_read(1),
                // binding 2: depth texture
                bgl_depth_tex(2),
                // binding 3: DBuffer albedo (Rgba8Unorm storage)
                bgl_storage_tex(3, wgpu::TextureFormat::Rgba8Unorm),
                // binding 4: DBuffer normal (Rgba8Snorm storage)
                bgl_storage_tex(4, wgpu::TextureFormat::Rgba8Snorm),
                // binding 5: DBuffer roughness (Rgba8Unorm storage; rg channels used)
                bgl_storage_tex(5, wgpu::TextureFormat::Rgba8Unorm),
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DBuffer Decal Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/dbuffer_decal.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DBuffer Decal Pipeline Layout"),
            bind_group_layouts: &[&layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("DBuffer Decal Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DBuffer Decal Params"),
            size: std::mem::size_of::<DecalParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let decal_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DBuffer Decal Data"),
            size: (std::mem::size_of::<DecalData>() * MAX_DECALS) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dbuffer_albedo = create_dbuffer_texture(device, width, height, wgpu::TextureFormat::Rgba8Unorm, "DBuffer Albedo");
        let dbuffer_albedo_view = dbuffer_albedo.create_view(&wgpu::TextureViewDescriptor::default());

        let dbuffer_normal = create_dbuffer_texture(device, width, height, wgpu::TextureFormat::Rgba8Snorm, "DBuffer Normal");
        let dbuffer_normal_view = dbuffer_normal.create_view(&wgpu::TextureViewDescriptor::default());

        let dbuffer_roughness = create_dbuffer_texture(device, width, height, wgpu::TextureFormat::Rgba8Unorm, "DBuffer Roughness");
        let dbuffer_roughness_view = dbuffer_roughness.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            pipeline,
            layout,
            dbuffer_albedo,
            dbuffer_albedo_view,
            dbuffer_normal,
            dbuffer_normal_view,
            dbuffer_roughness,
            dbuffer_roughness_view,
            params_buffer,
            decal_buffer,
            screen_width: width,
            screen_height: height,
        }
    }

    /// Project decals onto the DBuffer
    pub fn project(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        decals: &[DecalData],
        inv_view_proj: [[f32; 4]; 4],
        depth_view: &wgpu::TextureView,
    ) {
        if decals.is_empty() {
            return;
        }

        let count = decals.len().min(MAX_DECALS);
        let params = DecalParams {
            screen_width: self.screen_width,
            screen_height: self.screen_height,
            decal_count: count as u32,
            _pad: 0,
            inv_view_proj,
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        queue.write_buffer(
            &self.decal_buffer,
            0,
            bytemuck::cast_slice(&decals[..count]),
        );

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DBuffer Decal Bind Group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: self.decal_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.dbuffer_albedo_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.dbuffer_normal_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&self.dbuffer_roughness_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DBuffer Decal Projection"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(
                (self.screen_width + 7) / 8,
                (self.screen_height + 7) / 8,
                1,
            );
        }
    }

    /// Resize DBuffer textures when resolution changes
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.screen_width == width && self.screen_height == height {
            return;
        }
        self.screen_width = width;
        self.screen_height = height;

        self.dbuffer_albedo = create_dbuffer_texture(device, width, height, wgpu::TextureFormat::Rgba8Unorm, "DBuffer Albedo");
        self.dbuffer_albedo_view = self.dbuffer_albedo.create_view(&wgpu::TextureViewDescriptor::default());

        self.dbuffer_normal = create_dbuffer_texture(device, width, height, wgpu::TextureFormat::Rgba8Snorm, "DBuffer Normal");
        self.dbuffer_normal_view = self.dbuffer_normal.create_view(&wgpu::TextureViewDescriptor::default());

        self.dbuffer_roughness = create_dbuffer_texture(device, width, height, wgpu::TextureFormat::Rgba8Unorm, "DBuffer Roughness");
        self.dbuffer_roughness_view = self.dbuffer_roughness.create_view(&wgpu::TextureViewDescriptor::default());
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn create_dbuffer_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::STORAGE_BINDING,
        view_formats: &[],
    })
}

fn bgl_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
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

fn bgl_storage_read(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn bgl_depth_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn bgl_storage_tex(binding: u32, format: wgpu::TextureFormat) -> wgpu::BindGroupLayoutEntry {
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
