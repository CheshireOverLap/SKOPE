// SKOPE Engine — Debug Visualization Pipeline
//
// Fullscreen compute shader overlay that visualizes intermediate
// render targets for debugging: depth, normals, VSM pages,
// MegaLights tiles, TSR masks, DF shadows, etc.
//
// Usage:
//   debug_viz.dispatch(device, queue, encoder, mode, input_view, debug_view, depth_view, output_view);

use bytemuck::{Pod, Zeroable};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Maps DebugView enum to shader mode index
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugOverlayMode {
    Off = 0,
    Depth = 1,
    Normals = 2,
    #[allow(dead_code)]
    MotionVectors = 3,
    VsmShadowFactor = 4,
    VsmClipmapLevel = 5,
    MegaLightsTileCount = 6,
    TsrRejectionMask = 7,
    TsrThinGeometry = 8,
    DfShadows = 9,
    DfAO = 10,
    DBufferAlbedo = 11,
    DBufferNormal = 12,
    LumenScreenProbes = 13,
    AerialPerspective = 14,
    ProfilerOverlay = 15,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DebugParams {
    pub screen_width: u32,
    pub screen_height: u32,
    pub debug_mode: u32,
    pub near_plane: f32,
    pub far_plane: f32,
    pub _pad: [f32; 3],
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

pub struct DebugVisualization {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    params_buffer: wgpu::Buffer,
    /// Output texture (Rgba8Unorm, storage + texture binding)
    pub output_texture: wgpu::Texture,
    pub output_view: wgpu::TextureView,
    screen_width: u32,
    screen_height: u32,
}

impl DebugVisualization {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Debug Viz Layout"),
            entries: &[
                // binding 0: params uniform
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
                // binding 1: input color texture
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
                // binding 2: debug texture A (float)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 3: debug texture B (depth)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // binding 4: output storage texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Debug Overlay Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/debug_overlay.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Debug Viz Pipeline Layout"),
            bind_group_layouts: &[&layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Debug Viz Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Debug Viz Params"),
            size: std::mem::size_of::<DebugParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let output_texture = create_debug_output_texture(device, width, height);
        let output_view = output_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            pipeline,
            layout,
            params_buffer,
            output_texture,
            output_view,
            screen_width: width,
            screen_height: height,
        }
    }

    /// Dispatch the debug overlay compute pass
    /// Dispatch the debug overlay compute pass
    ///
    /// Writes result to `self.output_view` (Rgba8Unorm).
    /// Use `self.output_view` as blit source to display.
    pub fn dispatch(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        mode: DebugOverlayMode,
        near_plane: f32,
        far_plane: f32,
        input_color_view: &wgpu::TextureView,
        debug_tex_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) {
        if mode == DebugOverlayMode::Off {
            return;
        }

        let params = DebugParams {
            screen_width: self.screen_width,
            screen_height: self.screen_height,
            debug_mode: mode as u32,
            near_plane,
            far_plane,
            _pad: [0.0; 3],
        };

        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Debug Viz Bind Group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.params_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(input_color_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(debug_tex_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.output_view) },
            ],
        });

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Debug Visualization"),
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

    /// Resize output texture and dimensions
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if self.screen_width == width && self.screen_height == height {
            return;
        }
        self.screen_width = width;
        self.screen_height = height;
        self.output_texture = create_debug_output_texture(device, width, height);
        self.output_view = self.output_texture.create_view(&wgpu::TextureViewDescriptor::default());
    }
}

fn create_debug_output_texture(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Debug Viz Output"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}
