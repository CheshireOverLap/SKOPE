// SKOPE Render Resources
// Shared bind group layouts and buffers

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

/// Shared render resources
pub struct RenderResources {
    // Bind group layouts
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
    pub lighting_bind_group_layout: wgpu::BindGroupLayout,

    // Lighting uniforms
    pub lighting_uniform_buffer: wgpu::Buffer,
    pub lighting_bind_group: wgpu::BindGroup,

    // Light buffers (placeholder initially)
    pub light_buffer: wgpu::Buffer,
    pub light_count_buffer: wgpu::Buffer,

    // Default textures
    pub default_white_texture: wgpu::Texture,
    pub default_white_view: wgpu::TextureView,
    pub default_normal_texture: wgpu::Texture,
    pub default_normal_view: wgpu::TextureView,
    pub default_sampler: wgpu::Sampler,
}

/// Camera uniform data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view: [[f32; 4]; 4],
    pub projection: [[f32; 4]; 4],
    pub view_projection: [[f32; 4]; 4],
    pub inv_view_projection: [[f32; 4]; 4],
    pub camera_position: [f32; 4],
    pub screen_size: [f32; 2],
    pub near: f32,
    pub far: f32,
}

impl CameraUniform {
    pub fn new(view: Mat4, projection: Mat4, position: Vec3, screen_size: (u32, u32), near: f32, far: f32) -> Self {
        let view_projection = projection * view;
        let inv_view_projection = view_projection.inverse();

        Self {
            view: view.to_cols_array_2d(),
            projection: projection.to_cols_array_2d(),
            view_projection: view_projection.to_cols_array_2d(),
            inv_view_projection: inv_view_projection.to_cols_array_2d(),
            camera_position: [position.x, position.y, position.z, 1.0],
            screen_size: [screen_size.0 as f32, screen_size.1 as f32],
            near,
            far,
        }
    }
}

/// Model transform uniform
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ModelUniform {
    pub model: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 4]; 4],
}

impl ModelUniform {
    pub fn new(model: Mat4) -> Self {
        let normal_matrix = model.inverse().transpose();
        Self {
            model: model.to_cols_array_2d(),
            normal_matrix: normal_matrix.to_cols_array_2d(),
        }
    }
}

/// Lighting uniform data for deferred pass
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightingUniform {
    // Camera
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_position: [f32; 4],

    // Sun/Directional light
    pub sun_direction: [f32; 4],
    pub sun_color: [f32; 4],

    // Ambient
    pub ambient_color: [f32; 4],

    // Settings
    pub screen_size: [f32; 2],
    pub time: f32,
    pub exposure: f32,
}

impl Default for LightingUniform {
    fn default() -> Self {
        Self {
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_position: [0.0, 0.0, 5.0, 1.0],
            sun_direction: [-0.5, -1.0, -0.3, 0.0],
            sun_color: [1.0, 0.98, 0.95, 1.0],
            ambient_color: [0.03, 0.03, 0.05, 1.0],
            screen_size: [1280.0, 720.0],
            time: 0.0,
            exposure: 1.0,
        }
    }
}

/// Material uniform data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MaterialUniform {
    pub base_color: [f32; 4],
    pub emissive: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub ao: f32,
    pub _pad: f32,
}

impl Default for MaterialUniform {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            emissive: [0.0, 0.0, 0.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            ao: 1.0,
            _pad: 0.0,
        }
    }
}

impl RenderResources {
    pub fn new(device: &wgpu::Device) -> Self {
        // Camera bind group layout
        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera Bind Group Layout"),
            entries: &[
                // Camera uniform
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
                // Model uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
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

        // Material bind group layout
        let material_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Material Bind Group Layout"),
            entries: &[
                // Material uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Albedo texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Normal texture
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
                // Metallic-Roughness texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
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
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Lighting bind group layout (with multi-light support)
        let lighting_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lighting Bind Group Layout"),
            entries: &[
                // Lighting uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Lights storage buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Light counts uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        // Lighting uniform buffer
        let lighting_uniform = LightingUniform::default();
        let lighting_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lighting Uniform Buffer"),
            contents: bytemuck::bytes_of(&lighting_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Placeholder light buffer (at least 80 bytes for one GpuLight)
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Buffer"),
            size: 80, // One GpuLight = 80 bytes
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Light counts buffer (8 u32s = 32 bytes)
        let light_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Count Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lighting Bind Group"),
            layout: &lighting_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: lighting_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_count_buffer.as_entire_binding(),
                },
            ],
        });

        // Default textures
        let default_white_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default White Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let default_white_view = default_white_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let default_normal_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Normal Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let default_normal_view = default_normal_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let default_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Default Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            camera_bind_group_layout,
            material_bind_group_layout,
            lighting_bind_group_layout,
            lighting_uniform_buffer,
            lighting_bind_group,
            light_buffer,
            light_count_buffer,
            default_white_texture,
            default_white_view,
            default_normal_texture,
            default_normal_view,
            default_sampler,
        }
    }

    pub fn update_lighting(&self, queue: &wgpu::Queue, uniform: &LightingUniform) {
        queue.write_buffer(&self.lighting_uniform_buffer, 0, bytemuck::bytes_of(uniform));
    }

    /// Update lighting bind group with actual light buffers from LightManager
    pub fn update_light_buffers(
        &mut self,
        device: &wgpu::Device,
        light_buffer: &wgpu::Buffer,
        light_count_buffer: &wgpu::Buffer,
    ) {
        self.lighting_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lighting Bind Group"),
            layout: &self.lighting_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.lighting_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_count_buffer.as_entire_binding(),
                },
            ],
        });
    }
}
