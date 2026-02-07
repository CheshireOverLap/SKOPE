//! Nanite Rasterization Pipelines (Mesh Shader + SW)
//!
//! Two rasterization paths for visible clusters:
//!
//! - **Mesh Shader Rasterizer**: Task + Mesh shader for clusters > 32px.
//!   Task shader dispatches one mesh workgroup per visible cluster.
//!   Mesh shader emits meshlet vertices/triangles directly.
//!   Outputs to V-Buffer render targets (triangle_id R32Uint + barycentrics RG16Float).
//!
//! - **SW Rasterizer**: Compute shader for sub-pixel/small clusters.
//!   Outputs via `atomicMin` on a u32 storage buffer (depth16|payload16).
//!
//! After both passes, a resolve step merges results into the final V-Buffer.

#[cfg(feature = "gpu")]
use crate::types::NaniteCameraUniform;

// ── Mesh Shader Rasterization Pipeline ────────────────────────────────────

/// Mesh shader rasterization pipeline: task + mesh shader → V-Buffer.
///
/// Uses `Device::create_mesh_pipeline` (wgpu 28 EXPERIMENTAL_MESH_SHADER).
/// Task shader dispatches one mesh workgroup per visible cluster.
/// Mesh shader emits meshlet geometry directly — no prefix sum needed.
///
/// Bind groups:
/// - G0: Camera uniform
/// - G1: Geometry (vertices, meshlet triangles, meshlets)
/// - G2: Instance + visible cluster data + counters
///
/// Render targets:
/// - Color0: R32Uint (triangle_id)
/// - Color1: RG16Float (barycentrics)
/// - Depth: Depth32Float
#[cfg(feature = "gpu")]
pub struct NaniteMeshRasterPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub camera_layout: wgpu::BindGroupLayout,
    pub geometry_layout: wgpu::BindGroupLayout,
    pub instance_layout: wgpu::BindGroupLayout,
    pub camera_buffer: wgpu::Buffer,
}

#[cfg(feature = "gpu")]
impl NaniteMeshRasterPipeline {
    pub fn new(device: &wgpu::Device, depth_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Nanite Mesh Rasterize Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/nanite_rasterize_mesh.wgsl").into(),
            ),
        });

        // ── Group 0: Camera ──────────────────────────────────────
        // Visible to both task and mesh shader stages
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite Mesh G0: Camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::MESH,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<NaniteCameraUniform>() as u64,
                    ),
                },
                count: None,
            }],
        });

        // ── Group 1: Geometry data ───────────────────────────────
        // Mesh shader reads vertices, triangles, and meshlet metadata
        let geometry_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite Mesh G1: Geometry"),
            entries: &[
                // Vertices
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::MESH,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Meshlet triangles (packed u8 indices)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::MESH,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Meshlets metadata
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::MESH,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // ── Group 2: Instance + visible clusters + counters ──────
        // Task shader reads counters and visible_clusters
        // Mesh shader reads instances and visible_clusters
        let instance_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite Mesh G2: Instances"),
            entries: &[
                // Instances
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::MESH | wgpu::ShaderStages::TASK,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Visible clusters
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::MESH | wgpu::ShaderStages::TASK,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Counters (read by task shader for dispatch sizing)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::TASK,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // ── Pipeline ─────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Nanite Mesh Rasterize Layout"),
            bind_group_layouts: &[&camera_layout, &geometry_layout, &instance_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_mesh_pipeline(&wgpu::MeshPipelineDescriptor {
            label: Some("Nanite Mesh Rasterize Pipeline"),
            layout: Some(&pipeline_layout),
            task: Some(wgpu::TaskState {
                module: &shader,
                entry_point: Some("task_main"),
                compilation_options: Default::default(),
            }),
            mesh: wgpu::MeshState {
                module: &shader,
                entry_point: Some("mesh_main"),
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[
                    // Color0: Triangle ID (R32Uint)
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::R32Uint,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                    // Color1: Barycentrics (RGBA16Float; Rg16Float doesn't support STORAGE_BINDING)
                    Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba16Float,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    }),
                ],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Culling done in compute
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::GreaterEqual, // Reverse-Z
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Mesh Camera Uniform"),
            size: std::mem::size_of::<NaniteCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            camera_layout,
            geometry_layout,
            instance_layout,
            camera_buffer,
        }
    }

    /// Create camera bind group (G0).
    pub fn create_camera_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Mesh G0"),
            layout: &self.camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.camera_buffer.as_entire_binding(),
            }],
        })
    }

    /// Create geometry bind group (G1).
    pub fn create_geometry_bind_group(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        triangle_buffer: &wgpu::Buffer,
        meshlet_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Mesh G1"),
            layout: &self.geometry_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: triangle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: meshlet_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create instance bind group (G2).
    pub fn create_instance_bind_group(
        &self,
        device: &wgpu::Device,
        instance_buffer: &wgpu::Buffer,
        visible_clusters_buffer: &wgpu::Buffer,
        counters_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Mesh G2"),
            layout: &self.instance_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: visible_clusters_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: counters_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Upload camera data for this frame.
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &NaniteCameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(camera));
    }

    /// Record mesh shader rasterization pass.
    ///
    /// The task shader reads `counters[1]` (HW cluster count) to determine
    /// how many mesh workgroups to dispatch. Each mesh workgroup renders
    /// one meshlet from the visible_clusters list.
    ///
    /// `task_group_count` = ceil(hw_cluster_count / 32) — computed by CPU
    /// after reading back counters, or use `draw_mesh_tasks_indirect`.
    pub fn draw_mesh_tasks(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        triangle_id_view: &wgpu::TextureView,
        barycentrics_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        camera_bg: &wgpu::BindGroup,
        geometry_bg: &wgpu::BindGroup,
        instance_bg: &wgpu::BindGroup,
        task_group_count: u32,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Nanite Mesh Rasterize Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: triangle_id_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                }),
                Some(wgpu::RenderPassColorAttachment {
                    view: barycentrics_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0), // Reverse-Z: 0.0 = far
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        pass.set_bind_group(1, geometry_bg, &[]);
        pass.set_bind_group(2, instance_bg, &[]);
        pass.draw_mesh_tasks(task_group_count, 1, 1);
    }
}

// ── SW Rasterization Pipeline ───────────────────────────────────────────

/// Software rasterization pipeline: compute → atomic visibility buffer.
///
/// Bind groups:
/// - G0: Camera uniform
/// - G1: Geometry (vertices, meshlet triangles, meshlets)
/// - G2: Instance + visible clusters + counters
/// - G3: Visibility buffer (atomic u32 storage)
///
/// Each workgroup (128 threads) handles one cluster.
/// Each thread scan-converts one triangle via bounding-box traversal.
#[cfg(feature = "gpu")]
pub struct NaniteSwRasterPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub camera_layout: wgpu::BindGroupLayout,
    pub geometry_layout: wgpu::BindGroupLayout,
    pub instance_layout: wgpu::BindGroupLayout,
    pub vis_buffer_layout: wgpu::BindGroupLayout,
    pub camera_buffer: wgpu::Buffer,
    pub vis_buffer: wgpu::Buffer,
    pub width: u32,
    pub height: u32,
}

#[cfg(feature = "gpu")]
impl NaniteSwRasterPipeline {
    pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Nanite SW Rasterize Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/nanite_rasterize_sw.wgsl").into(),
            ),
        });

        // ── Group 0: Camera ──────────────────────────────────────
        let camera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite SW G0: Camera"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(
                        std::mem::size_of::<NaniteCameraUniform>() as u64,
                    ),
                },
                count: None,
            }],
        });

        // ── Group 1: Geometry ────────────────────────────────────
        let geometry_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite SW G1: Geometry"),
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

        // ── Group 2: Instance + visible clusters + counters ──────
        let instance_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite SW G2: Instances"),
            entries: &[
                // Instances (read)
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
                // Visible clusters (read)
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
                // Counters (read_write, atomic)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        // ── Group 3: Visibility buffer ───────────────────────────
        let vis_buffer_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Nanite SW G3: Vis Buffer"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // ── Pipeline ─────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Nanite SW Rasterize Layout"),
            bind_group_layouts: &[
                &camera_layout,
                &geometry_layout,
                &instance_layout,
                &vis_buffer_layout,
            ],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Nanite SW Rasterize Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Buffers ──────────────────────────────────────────────
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite SW Camera Uniform"),
            size: std::mem::size_of::<NaniteCameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // One u32 per pixel, initialized to 0xFFFFFFFF (max depth = far plane)
        let pixel_count = (width as u64) * (height as u64);
        let vis_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite SW Vis Buffer"),
            size: pixel_count * 4, // u32 per pixel
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            camera_layout,
            geometry_layout,
            instance_layout,
            vis_buffer_layout,
            camera_buffer,
            vis_buffer,
            width,
            height,
        }
    }

    /// Recreate the visibility buffer when viewport size changes.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let pixel_count = (width as u64) * (height as u64);
        self.vis_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite SW Vis Buffer"),
            size: pixel_count * 4,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
    }

    /// Clear the vis buffer to 0xFFFFFFFF (max depth = nothing visible).
    pub fn clear_vis_buffer(&self, queue: &wgpu::Queue) {
        let pixel_count = (self.width as usize) * (self.height as usize);
        let clear_data = vec![0xFFFFFFFFu32; pixel_count];
        queue.write_buffer(&self.vis_buffer, 0, bytemuck::cast_slice(&clear_data));
    }

    /// Create camera bind group (G0).
    pub fn create_camera_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite SW G0"),
            layout: &self.camera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.camera_buffer.as_entire_binding(),
            }],
        })
    }

    /// Create geometry bind group (G1).
    pub fn create_geometry_bind_group(
        &self,
        device: &wgpu::Device,
        vertex_buffer: &wgpu::Buffer,
        triangle_buffer: &wgpu::Buffer,
        meshlet_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite SW G1"),
            layout: &self.geometry_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: triangle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: meshlet_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create instance bind group (G2).
    pub fn create_instance_bind_group(
        &self,
        device: &wgpu::Device,
        instance_buffer: &wgpu::Buffer,
        visible_clusters_buffer: &wgpu::Buffer,
        counters_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite SW G2"),
            layout: &self.instance_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: instance_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: visible_clusters_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: counters_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create visibility buffer bind group (G3).
    pub fn create_vis_buffer_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite SW G3"),
            layout: &self.vis_buffer_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.vis_buffer.as_entire_binding(),
            }],
        })
    }

    /// Upload camera data for this frame.
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &NaniteCameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(camera));
    }

    /// Dispatch SW rasterization using indirect dispatch.
    ///
    /// The `indirect_buffer` contains `IndirectDispatchArgs` set by the cull shader.
    /// Each workgroup handles one SW cluster.
    pub fn dispatch_indirect(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        camera_bg: &wgpu::BindGroup,
        geometry_bg: &wgpu::BindGroup,
        instance_bg: &wgpu::BindGroup,
        vis_buffer_bg: &wgpu::BindGroup,
        indirect_buffer: &wgpu::Buffer,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Nanite SW Rasterize Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        pass.set_bind_group(1, geometry_bg, &[]);
        pass.set_bind_group(2, instance_bg, &[]);
        pass.set_bind_group(3, vis_buffer_bg, &[]);
        pass.dispatch_workgroups_indirect(indirect_buffer, 0);
    }
}
