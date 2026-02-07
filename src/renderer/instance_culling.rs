// SKOPE Engine - GPU Instance Culling
//
// Two-pass GPU-driven instance culling inspired by UE5's InstanceCulling system.
//
// Pass 0: Frustum + Occlusion cull against previous frame's HZB
// Pass 1: Re-cull newly visible instances against current frame's HZB
//
// Output: compact visible-instance index list + indirect draw commands

#![allow(dead_code)]

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

use super::gpu_scene::GpuScene;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum visible instances per frame (limits GPU buffer size)
pub const MAX_VISIBLE_INSTANCES: u32 = 65_536;

/// Workgroup size for the cull compute shader
const CULL_WORKGROUP_SIZE: u32 = 64;

// ---------------------------------------------------------------------------
// GPU types
// ---------------------------------------------------------------------------

/// Culling parameters uploaded each frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CullingParams {
    pub view_proj: [[f32; 4]; 4],
    pub frustum_planes: [[f32; 4]; 6], // left, right, bottom, top, near, far
    pub hzb_size: [f32; 2],
    pub near_plane: f32,
    pub far_plane: f32,
    pub pass_index: u32,
    pub instance_count: u32,
    pub _pad: [u32; 2],
}

/// Indirect draw command (matches wgpu::DrawIndexedIndirect)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct DrawCommand {
    pub index_count: u32,
    pub instance_count: u32,
    pub first_index: u32,
    pub base_vertex: i32,
    pub first_instance: u32,
}

// ---------------------------------------------------------------------------
// GPU Instance Culling Pipeline
// ---------------------------------------------------------------------------

pub struct InstanceCullingPipeline {
    // Compute pipeline
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,

    // GPU buffers
    params_buffer: wgpu::Buffer,
    visible_indices_buffer: wgpu::Buffer,
    draw_commands_buffer: wgpu::Buffer,
    counters_buffer: wgpu::Buffer,
    counters_readback_buffer: wgpu::Buffer,

    // Two-pass occlusion culling buffers
    // Instances that failed HZB test in Pass 0 are written here for re-testing in Pass 1.
    pub occluded_indices_buffer: wgpu::Buffer,
    pub occluded_count_buffer: wgpu::Buffer,

    // Stats
    last_visible_count: u32,
}

impl InstanceCullingPipeline {
    pub fn new(device: &wgpu::Device, _gpu_scene: &GpuScene) -> Self {
        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Instance Culling Layout"),
            entries: &[
                // binding 0: GPU Scene instances (storage, read-only)
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
                // binding 1: culling params (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: HZB texture (read)
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
                // binding 3: visible indices (storage, read-write)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 4: draw commands (storage, read-write)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 5: counters (storage, read-write, atomic)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 6: occluded indices (storage, read-write) — two-pass
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Compute shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Instance Culling Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/instance_culling.wgsl").into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Instance Culling Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Instance Culling Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cull_instances"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // GPU Buffers
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Culling Params"),
            size: std::mem::size_of::<CullingParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let visible_indices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Visible Indices"),
            size: (MAX_VISIBLE_INSTANCES as u64) * 4, // u32 per index
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_commands_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Draw Commands"),
            size: (MAX_VISIBLE_INSTANCES as u64) * std::mem::size_of::<DrawCommand>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // counters[0] = visible_count, counters[1] = draw_command_count, counters[2] = occluded_count
        let counters_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Culling Counters"),
            size: 12, // 3 x u32
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let counters_readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Culling Counters Readback"),
            size: 12, // 3 x u32
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Two-pass: occluded instance indices from Pass 0 for re-test in Pass 1
        let occluded_indices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Occluded Indices (Two-Pass)"),
            size: (MAX_VISIBLE_INSTANCES as u64) * 4, // u32 per index
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // counters: occluded_count[0]
        let occluded_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Occluded Count (Two-Pass)"),
            size: 4, // single u32
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            visible_indices_buffer,
            draw_commands_buffer,
            counters_buffer,
            counters_readback_buffer,
            occluded_indices_buffer,
            occluded_count_buffer,
            last_visible_count: 0,
        }
    }

    /// Run a single culling pass.
    ///
    /// `pass_index`: 0 for first pass (previous HZB), 1 for second pass (new HZB)
    pub fn cull(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        gpu_scene: &GpuScene,
        hzb_view: &wgpu::TextureView,
        view: Mat4,
        proj: Mat4,
        hzb_width: u32,
        hzb_height: u32,
        pass_index: u32,
    ) {
        let view_proj = proj * view;
        let frustum_planes = extract_frustum_planes(view_proj);
        let instance_count = gpu_scene.live_count();

        if instance_count == 0 {
            return;
        }

        // Upload params
        let params = CullingParams {
            view_proj: view_proj.to_cols_array_2d(),
            frustum_planes,
            hzb_size: [hzb_width as f32, hzb_height as f32],
            near_plane: 0.1,
            far_plane: 1000.0,
            pass_index,
            instance_count,
            _pad: [0; 2],
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Clear counters to zero before culling
        if pass_index == 0 {
            queue.write_buffer(&self.counters_buffer, 0, &[0u8; 12]); // 3 x u32
        }

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Instance Culling Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu_scene.instance_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(hzb_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.visible_indices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.draw_commands_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.counters_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: self.occluded_indices_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch
        let dispatch_x = instance_count.div_ceil(CULL_WORKGROUP_SIZE);

        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(&format!("Instance Culling Pass {}", pass_index)),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(dispatch_x, 1, 1);
    }

    /// Execute Pass 0 of two-pass culling (against previous frame's HZB).
    ///
    /// After this, the caller should:
    /// 1. Render the V-Buffer for visible instances
    /// 2. Regenerate HZB from new depth
    /// 3. Call `cull_pass1()` with the new HZB
    pub fn cull_two_pass(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        gpu_scene: &GpuScene,
        prev_hzb_view: &wgpu::TextureView,
        view: Mat4,
        proj: Mat4,
        hzb_width: u32,
        hzb_height: u32,
    ) {
        // Pass 0: cull against previous frame HZB
        self.cull(
            device, queue, encoder, gpu_scene,
            prev_hzb_view, view, proj, hzb_width, hzb_height, 0,
        );
    }

    /// Execute Pass 1: re-test occluded instances from Pass 0 against current frame's HZB.
    ///
    /// Reads the occluded_indices written during Pass 0 and dispatches them for re-testing.
    /// Newly visible instances are appended to the same visible_indices + draw_commands buffers.
    pub fn cull_pass1(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        gpu_scene: &GpuScene,
        current_hzb_view: &wgpu::TextureView,
        view: Mat4,
        proj: Mat4,
        hzb_width: u32,
        hzb_height: u32,
    ) {
        // Pass 1 re-uses the same shader with pass_index = 1.
        // The shader reads from the full instance array but only processes
        // instances that were marked occluded in Pass 0.
        // Note: We do NOT clear counters[0..1] here — Pass 1 appends to the
        // visible list built by Pass 0.
        self.cull(
            device, queue, encoder, gpu_scene,
            current_hzb_view, view, proj, hzb_width, hzb_height, 1,
        );
    }

    // =======================================================================
    // Accessors
    // =======================================================================

    /// Buffer of visible instance indices (u32 array).
    pub fn visible_indices_buffer(&self) -> &wgpu::Buffer {
        &self.visible_indices_buffer
    }

    /// Buffer of indirect draw commands.
    pub fn draw_commands_buffer(&self) -> &wgpu::Buffer {
        &self.draw_commands_buffer
    }

    /// Counters buffer: [visible_count, draw_command_count, occluded_count]
    pub fn counters_buffer(&self) -> &wgpu::Buffer {
        &self.counters_buffer
    }

    /// Bind group layout for external use.
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Last known visible instance count (read back asynchronously).
    pub fn last_visible_count(&self) -> u32 {
        self.last_visible_count
    }
}

// ---------------------------------------------------------------------------
// Frustum plane extraction
// ---------------------------------------------------------------------------

/// Extract 6 frustum planes from a view-projection matrix (public API).
/// Each plane is (nx, ny, nz, d) where dot(n, p) + d >= 0 means inside.
pub fn extract_frustum_planes_pub(vp: Mat4) -> [[f32; 4]; 6] {
    extract_frustum_planes(vp)
}

/// Extract 6 frustum planes from a view-projection matrix.
/// Each plane is (nx, ny, nz, d) where dot(n, p) + d >= 0 means inside.
fn extract_frustum_planes(vp: Mat4) -> [[f32; 4]; 6] {
    let m = vp.to_cols_array_2d();

    // Row-based extraction (Gribb-Hartmann method)
    let row0 = Vec4::new(m[0][0], m[1][0], m[2][0], m[3][0]);
    let row1 = Vec4::new(m[0][1], m[1][1], m[2][1], m[3][1]);
    let row2 = Vec4::new(m[0][2], m[1][2], m[2][2], m[3][2]);
    let row3 = Vec4::new(m[0][3], m[1][3], m[2][3], m[3][3]);

    let planes = [
        row3 + row0, // Left
        row3 - row0, // Right
        row3 + row1, // Bottom
        row3 - row1, // Top
        row2,        // Near (for reverse-Z: row2)
        row3 - row2, // Far
    ];

    let mut result = [[0.0f32; 4]; 6];
    for (i, plane) in planes.iter().enumerate() {
        let len = Vec3::new(plane.x, plane.y, plane.z).length();
        if len > 0.0 {
            let inv = 1.0 / len;
            result[i] = [plane.x * inv, plane.y * inv, plane.z * inv, plane.w * inv];
        }
    }

    result
}
