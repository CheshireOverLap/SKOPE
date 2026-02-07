//! Nanite GPU Culling Pipeline
//!
//! Compute shader that performs per-cluster frustum culling, LOD selection,
//! HZB occlusion culling, and normal cone backface culling.
//!
//! Outputs a list of visible clusters classified into HW (mesh shader)
//! and SW (compute rasterizer) buckets.

#[cfg(feature = "gpu")]
use crate::types::{CullParams, NaniteConfig, IndirectDrawArgs, IndirectDispatchArgs, MeshMeshletRange};

/// Maximum number of visible clusters per frame.
pub const MAX_VISIBLE_CLUSTERS: u32 = 1_000_000;

/// GPU visible cluster entry (matches WGSL VisibleCluster struct).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VisibleCluster {
    pub instance_id: u32,
    pub meshlet_id: u32,
    pub material_id: u32,
    pub flags: u32,
}

impl VisibleCluster {
    pub const SW_RASTER_FLAG: u32 = 1;

    pub fn is_sw_raster(&self) -> bool {
        self.flags & Self::SW_RASTER_FLAG != 0
    }
}

/// Counter buffer layout (matches WGSL counters array).
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CullCounters {
    /// Total visible clusters
    pub total_visible: u32,
    /// HW rasterized cluster count (mesh shader dispatch size)
    pub hw_cluster_count: u32,
    /// SW rasterized cluster count
    pub sw_cluster_count: u32,
    pub _pad: u32,
}

/// Nanite GPU culling pipeline.
///
/// Bind groups:
/// - G0: CullParams (uniform) + Instances (storage, read)
/// - G1: Meshlets (storage, read)
/// - G2: HZB texture + sampler
/// - G3: Outputs (visible clusters, HW indirect, SW indirect, counters)
#[cfg(feature = "gpu")]
pub struct NaniteCullPipeline {
    pub pipeline: wgpu::ComputePipeline,
    pub params_layout: wgpu::BindGroupLayout,
    pub meshlet_layout: wgpu::BindGroupLayout,
    pub hzb_layout: wgpu::BindGroupLayout,
    pub output_layout: wgpu::BindGroupLayout,
    pub params_buffer: wgpu::Buffer,
    pub visible_clusters_buffer: wgpu::Buffer,
    pub hw_indirect_buffer: wgpu::Buffer,
    pub sw_indirect_buffer: wgpu::Buffer,
    pub counters_buffer: wgpu::Buffer,
    pub max_visible_clusters: u32,
}

#[cfg(feature = "gpu")]
impl NaniteCullPipeline {
    pub fn new(device: &wgpu::Device, config: &NaniteConfig) -> Self {
        let max_clusters = config.max_visible_meshlets;

        // ── Shader ───────────────────────────────────────────────
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Nanite Cull Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../shaders/nanite_cull.wgsl").into(),
            ),
        });

        // ── Group 0: CullParams + Instances ──────────────────────
        let params_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite Cull G0: Params + Instances"),
            entries: &[
                // CullParams uniform
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<CullParams>() as u64,
                        ),
                    },
                    count: None,
                },
                // Instances storage (read)
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
            ],
        });

        // ── Group 1: Meshlets + MeshRanges ────────────────────────
        let meshlet_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite Cull G1: Meshlets + Ranges"),
            entries: &[
                // binding 0: Meshlets (storage, read)
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
                // binding 1: MeshMeshletRange table (storage, read)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<MeshMeshletRange>() as u64,
                        ),
                    },
                    count: None,
                },
            ],
        });

        // ── Group 2: HZB ─────────────────────────────────────────
        let hzb_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite Cull G2: HZB"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });

        // ── Group 3: Outputs ─────────────────────────────────────
        let output_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Nanite Cull G3: Outputs"),
            entries: &[
                // Visible clusters (storage, read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // HW indirect draw args (storage, read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<IndirectDrawArgs>() as u64,
                        ),
                    },
                    count: None,
                },
                // SW indirect dispatch args (storage, read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<IndirectDispatchArgs>() as u64,
                        ),
                    },
                    count: None,
                },
                // Counters (storage, read_write) — atomic u32 array
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<CullCounters>() as u64,
                        ),
                    },
                    count: None,
                },
            ],
        });

        // ── Pipeline ─────────────────────────────────────────────
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Nanite Cull Pipeline Layout"),
            bind_group_layouts: &[&params_layout, &meshlet_layout, &hzb_layout, &output_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Nanite Cull Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── Buffers ──────────────────────────────────────────────
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite CullParams"),
            size: std::mem::size_of::<CullParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let visible_clusters_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Visible Clusters"),
            size: (std::mem::size_of::<VisibleCluster>() as u64) * max_clusters as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let hw_indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite HW Indirect Args"),
            size: std::mem::size_of::<IndirectDrawArgs>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sw_indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite SW Indirect Args"),
            size: std::mem::size_of::<IndirectDispatchArgs>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::INDIRECT
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let counters_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Nanite Cull Counters"),
            size: std::mem::size_of::<CullCounters>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            params_layout,
            meshlet_layout,
            hzb_layout,
            output_layout,
            params_buffer,
            visible_clusters_buffer,
            hw_indirect_buffer,
            sw_indirect_buffer,
            counters_buffer,
            max_visible_clusters: max_clusters,
        }
    }

    /// Create Group 0 bind group: CullParams + Instances.
    pub fn create_params_bind_group(
        &self,
        device: &wgpu::Device,
        instance_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Cull G0"),
            layout: &self.params_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instance_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create Group 1 bind group: Meshlets + MeshRanges.
    pub fn create_meshlet_bind_group(
        &self,
        device: &wgpu::Device,
        meshlet_buffer: &wgpu::Buffer,
        mesh_ranges_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Cull G1"),
            layout: &self.meshlet_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: meshlet_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: mesh_ranges_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create Group 2 bind group: HZB.
    pub fn create_hzb_bind_group(
        &self,
        device: &wgpu::Device,
        hzb_view: &wgpu::TextureView,
        hzb_sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Cull G2"),
            layout: &self.hzb_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(hzb_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(hzb_sampler),
                },
            ],
        })
    }

    /// Create Group 3 bind group: Outputs.
    pub fn create_output_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Nanite Cull G3"),
            layout: &self.output_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.visible_clusters_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.hw_indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.sw_indirect_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.counters_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Clear counters and indirect args before a new culling pass.
    pub fn reset_buffers(&self, queue: &wgpu::Queue) {
        let zero_counters = CullCounters {
            total_visible: 0,
            hw_cluster_count: 0,
            sw_cluster_count: 0,
            _pad: 0,
        };
        queue.write_buffer(
            &self.counters_buffer,
            0,
            bytemuck::bytes_of(&zero_counters),
        );

        let zero_hw = IndirectDrawArgs {
            vertex_count: 0,
            instance_count: 0,
            first_vertex: 0,
            first_instance: 0,
        };
        queue.write_buffer(
            &self.hw_indirect_buffer,
            0,
            bytemuck::bytes_of(&zero_hw),
        );

        let zero_sw = IndirectDispatchArgs {
            x: 0,
            y: 1,
            z: 1,
            _pad: 0,
        };
        queue.write_buffer(
            &self.sw_indirect_buffer,
            0,
            bytemuck::bytes_of(&zero_sw),
        );
    }

    /// Upload cull params for this frame.
    pub fn update_params(&self, queue: &wgpu::Queue, params: &CullParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(params));
    }

    /// Dispatch the culling compute shader with 2D dispatch.
    ///
    /// `gid.x` = local meshlet index (within the mesh with most meshlets),
    /// `gid.y` = instance index.
    ///
    /// Call this after setting up bind groups and uploading params.
    pub fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        params_bg: &wgpu::BindGroup,
        meshlet_bg: &wgpu::BindGroup,
        hzb_bg: &wgpu::BindGroup,
        output_bg: &wgpu::BindGroup,
        max_meshlets_per_mesh: u32,
        instance_count: u32,
    ) {
        if instance_count == 0 || max_meshlets_per_mesh == 0 {
            return;
        }

        let workgroup_x = (max_meshlets_per_mesh + 63) / 64;

        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Nanite Cull Pass"),
                timestamp_writes: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, params_bg, &[]);
            pass.set_bind_group(1, meshlet_bg, &[]);
            pass.set_bind_group(2, hzb_bg, &[]);
            pass.set_bind_group(3, output_bg, &[]);
            pass.dispatch_workgroups(workgroup_x, instance_count, 1);
        }

        // Copy SW cluster count (counters[2]) → sw_indirect.x after cull pass
        encoder.copy_buffer_to_buffer(
            &self.counters_buffer,
            8, // offset of counters[2] (sw_cluster_count)
            &self.sw_indirect_buffer,
            0, // offset of IndirectDispatchArgs.x
            4,
        );
    }
}
