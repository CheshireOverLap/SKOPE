// SKOPE Engine - Clustered Shading
// 16x16x24 Grid for efficient light culling
// Phase 14: Full Integration with Material Eval

use glam::{Vec3, Mat4, UVec3};
use bytemuck::{Pod, Zeroable};

/// Cluster Grid 설정
#[derive(Debug, Clone, Copy)]
pub struct ClusterConfig {
    pub tile_size: u32,        // 16 pixels
    pub depth_slices: u32,     // 24 slices (exponential)
    pub max_lights_per_cluster: u32,
    pub near_plane: f32,
    pub far_plane: f32,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            tile_size: 16,
            depth_slices: 24,
            max_lights_per_cluster: 64,
            near_plane: 0.1,
            far_plane: 100.0,
        }
    }
}

/// Material Eval에서 사용할 클러스터 파라미터 (읽기 전용)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ClusterReadParams {
    pub grid_size: [u32; 3],
    pub tile_size: u32,
    pub screen_size: [u32; 2],
    pub near_plane: f32,
    pub far_plane: f32,
}

/// GPU용 Cluster 정보
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ClusterUniforms {
    pub grid_size: [u32; 3],
    pub tile_size: u32,
    pub screen_size: [u32; 2],
    pub near_plane: f32,
    pub far_plane: f32,
    pub log_depth_ratio: f32,  // log(far/near) / depth_slices
    pub _pad: [f32; 3],
}

/// Light Index 리스트 (per cluster)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightGrid {
    pub offset: u32,
    pub count: u32,
}

/// Clustered Lighting System
pub struct ClusteredLighting {
    config: ClusterConfig,

    // Grid dimensions
    grid_size: UVec3,

    // GPU buffers
    cluster_buffer: wgpu::Buffer,        // ClusterUniforms
    light_grid_buffer: wgpu::Buffer,     // LightGrid per cluster
    light_index_buffer: wgpu::Buffer,    // Light indices
    light_counter_buffer: wgpu::Buffer,  // Atomic counter for indices

    // Compute pipeline
    cluster_cull_pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    // CPU-side data for rebuilding
    light_grid: Vec<LightGrid>,
    light_indices: Vec<u32>,
}

impl ClusteredLighting {
    pub fn new(
        device: &wgpu::Device,
        config: ClusterConfig,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        // Grid dimensions
        let grid_x = (screen_width + config.tile_size - 1) / config.tile_size;
        let grid_y = (screen_height + config.tile_size - 1) / config.tile_size;
        let grid_z = config.depth_slices;
        let grid_size = UVec3::new(grid_x, grid_y, grid_z);

        let total_clusters = (grid_x * grid_y * grid_z) as usize;
        let max_indices = total_clusters * config.max_lights_per_cluster as usize;

        // Cluster uniforms
        let _uniforms = ClusterUniforms {
            grid_size: [grid_x, grid_y, grid_z],
            tile_size: config.tile_size,
            screen_size: [screen_width, screen_height],
            near_plane: config.near_plane,
            far_plane: config.far_plane,
            log_depth_ratio: (config.far_plane / config.near_plane).ln() / config.depth_slices as f32,
            _pad: [0.0; 3],
        };

        let cluster_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Cluster Uniforms"),
            size: std::mem::size_of::<ClusterUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_grid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Grid"),
            size: (total_clusters * std::mem::size_of::<LightGrid>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Indices"),
            size: (max_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_counter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Counter"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Compute shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cluster Cull Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/cluster_cull.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Clustered Lighting Bind Group Layout"),
            entries: &[
                // Cluster uniforms
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Light grid
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Light indices
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Atomic counter
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
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cluster Cull Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let cluster_cull_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Cluster Cull Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("cull_lights"),
            compilation_options: Default::default(),
            cache: None,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Clustered Lighting Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: cluster_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_grid_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: light_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: light_counter_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            config,
            grid_size,
            cluster_buffer,
            light_grid_buffer,
            light_index_buffer,
            light_counter_buffer,
            cluster_cull_pipeline,
            bind_group_layout,
            bind_group,
            light_grid: vec![LightGrid { offset: 0, count: 0 }; total_clusters],
            light_indices: vec![0; max_indices],
        }
    }

    /// 화면 크기 변경 시 재구성
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let grid_x = (width + self.config.tile_size - 1) / self.config.tile_size;
        let grid_y = (height + self.config.tile_size - 1) / self.config.tile_size;
        self.grid_size = UVec3::new(grid_x, grid_y, self.config.depth_slices);

        let total_clusters = (grid_x * grid_y * self.config.depth_slices) as usize;
        let max_indices = total_clusters * self.config.max_lights_per_cluster as usize;

        // Recreate buffers
        self.light_grid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Grid"),
            size: (total_clusters * std::mem::size_of::<LightGrid>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.light_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Indices"),
            size: (max_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.light_grid.resize(total_clusters, LightGrid { offset: 0, count: 0 });
        self.light_indices.resize(max_indices, 0);

        // Recreate bind group
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Clustered Lighting Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.cluster_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.light_grid_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.light_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.light_counter_buffer.as_entire_binding(),
                },
            ],
        });
    }

    /// 깊이 슬라이스 계산 (exponential)
    pub fn depth_slice(&self, view_z: f32) -> u32 {
        if view_z <= self.config.near_plane {
            return 0;
        }
        if view_z >= self.config.far_plane {
            return self.config.depth_slices - 1;
        }

        let log_near = self.config.near_plane.ln();
        let log_ratio = (self.config.far_plane / self.config.near_plane).ln();
        let slice = ((view_z.ln() - log_near) / log_ratio * self.config.depth_slices as f32) as u32;
        slice.min(self.config.depth_slices - 1)
    }

    /// Cluster 인덱스 계산
    pub fn cluster_index(&self, x: u32, y: u32, z: u32) -> u32 {
        z * self.grid_size.x * self.grid_size.y + y * self.grid_size.x + x
    }

    /// CPU에서 라이트 컬링 (fallback)
    pub fn cull_lights_cpu(
        &mut self,
        lights: &[super::lights::GpuLight],
        view_matrix: Mat4,
        _proj_matrix: Mat4,
    ) {
        // Reset
        for grid in &mut self.light_grid {
            *grid = LightGrid { offset: 0, count: 0 };
        }
        self.light_indices.fill(0);

        let mut global_index = 0u32;

        for z in 0..self.grid_size.z {
            for y in 0..self.grid_size.y {
                for x in 0..self.grid_size.x {
                    let cluster_idx = self.cluster_index(x, y, z) as usize;

                    // Cluster AABB 계산
                    let (min_depth, max_depth) = self.slice_depth_range(z);

                    let _tile_min_x = x * self.config.tile_size;
                    let _tile_max_x = (x + 1) * self.config.tile_size;
                    let _tile_min_y = y * self.config.tile_size;
                    let _tile_max_y = (y + 1) * self.config.tile_size;

                    // 각 라이트와 교차 테스트
                    let start_offset = global_index;
                    let mut count = 0u32;

                    for (light_idx, light) in lights.iter().enumerate() {
                        let light_type = light.position_type[3] as u32;

                        // Point/Spot light만 클러스터링
                        if light_type == 1 || light_type == 2 {
                            let light_pos = Vec3::new(
                                light.position_type[0],
                                light.position_type[1],
                                light.position_type[2],
                            );
                            let radius = light.direction_radius[3];

                            // View space로 변환
                            let view_pos = view_matrix.transform_point3(light_pos);

                            // 간단한 Sphere-AABB 교차 (정확한 frustum 대신)
                            if view_pos.z - radius <= -min_depth && view_pos.z + radius >= -max_depth {
                                // Screen space bounds 체크 (간소화)
                                if count < self.config.max_lights_per_cluster {
                                    self.light_indices[(start_offset + count) as usize] = light_idx as u32;
                                    count += 1;
                                }
                            }
                        }
                    }

                    self.light_grid[cluster_idx] = LightGrid {
                        offset: start_offset,
                        count,
                    };

                    global_index += count;
                }
            }
        }
    }

    /// 깊이 슬라이스의 near/far 범위
    fn slice_depth_range(&self, slice: u32) -> (f32, f32) {
        let log_near = self.config.near_plane.ln();
        let log_ratio = (self.config.far_plane / self.config.near_plane).ln();

        let near = (log_near + (slice as f32 / self.config.depth_slices as f32) * log_ratio).exp();
        let far = (log_near + ((slice + 1) as f32 / self.config.depth_slices as f32) * log_ratio).exp();

        (near, far)
    }

    /// GPU 버퍼 업데이트
    pub fn update_buffers(&self, queue: &wgpu::Queue, screen_width: u32, screen_height: u32) {
        let uniforms = ClusterUniforms {
            grid_size: [self.grid_size.x, self.grid_size.y, self.grid_size.z],
            tile_size: self.config.tile_size,
            screen_size: [screen_width, screen_height],
            near_plane: self.config.near_plane,
            far_plane: self.config.far_plane,
            log_depth_ratio: (self.config.far_plane / self.config.near_plane).ln()
                / self.config.depth_slices as f32,
            _pad: [0.0; 3],
        };

        queue.write_buffer(&self.cluster_buffer, 0, bytemuck::bytes_of(&uniforms));
        queue.write_buffer(&self.light_grid_buffer, 0, bytemuck::cast_slice(&self.light_grid));
        queue.write_buffer(&self.light_index_buffer, 0, bytemuck::cast_slice(&self.light_indices));

        // Reset counter
        queue.write_buffer(&self.light_counter_buffer, 0, &[0u8; 4]);
    }

    /// GPU 컬링 디스패치
    pub fn dispatch_cull(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Cluster Cull Pass"),
            timestamp_writes: None,
        });

        pass.set_pipeline(&self.cluster_cull_pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);

        // 각 클러스터당 1 workgroup
        let dispatch_x = (self.grid_size.x + 7) / 8;
        let dispatch_y = (self.grid_size.y + 7) / 8;
        let dispatch_z = self.grid_size.z;

        pass.dispatch_workgroups(dispatch_x, dispatch_y, dispatch_z);
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn grid_size(&self) -> UVec3 {
        self.grid_size
    }

    /// Material Eval에서 사용할 버퍼 참조
    pub fn cluster_params_buffer(&self) -> &wgpu::Buffer {
        &self.cluster_buffer
    }

    pub fn light_grid_buffer(&self) -> &wgpu::Buffer {
        &self.light_grid_buffer
    }

    pub fn light_index_buffer(&self) -> &wgpu::Buffer {
        &self.light_index_buffer
    }

    /// ClusterReadParams 생성 (현재 상태 기반)
    pub fn get_read_params(&self, screen_width: u32, screen_height: u32) -> ClusterReadParams {
        ClusterReadParams {
            grid_size: [self.grid_size.x, self.grid_size.y, self.grid_size.z],
            tile_size: self.config.tile_size,
            screen_size: [screen_width, screen_height],
            near_plane: self.config.near_plane,
            far_plane: self.config.far_plane,
        }
    }

    /// Material Eval용 Bind Group Layout 생성 (읽기 전용)
    pub fn create_material_eval_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Clustered Lighting Material Eval Layout"),
            entries: &[
                // binding 0: cluster_params (uniform)
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
                // binding 1: light_grid (storage, read-only)
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
                // binding 2: light_indices (storage, read-only)
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
                // binding 3: lights (storage, read-only)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        })
    }

    /// Material Eval용 Bind Group 생성
    pub fn create_material_eval_bind_group(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        light_buffer: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Clustered Lighting Material Eval Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.cluster_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.light_grid_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.light_index_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        })
    }
}

/// AABB (Axis-Aligned Bounding Box)
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Sphere와 교차 테스트
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        let closest = Vec3::new(
            center.x.clamp(self.min.x, self.max.x),
            center.y.clamp(self.min.y, self.max.y),
            center.z.clamp(self.min.z, self.max.z),
        );

        let dist_sq = (closest - center).length_squared();
        dist_sq <= radius * radius
    }

    /// Cone과 교차 테스트 (Spotlight용)
    pub fn intersects_cone(
        &self,
        apex: Vec3,
        direction: Vec3,
        range: f32,
        cos_angle: f32,
    ) -> bool {
        // Simplified: sphere intersection + angle check
        let center = (self.min + self.max) * 0.5;
        let half_extent = (self.max - self.min) * 0.5;
        let aabb_radius = half_extent.length();

        // 구 교차
        let to_center = center - apex;
        let dist = to_center.length();

        if dist - aabb_radius > range {
            return false;
        }

        // 각도 체크 (간소화)
        if dist > 0.001 {
            let cos_to_center = to_center.normalize().dot(direction);
            let sin_aabb = aabb_radius / dist;
            let cos_aabb = (1.0 - sin_aabb * sin_aabb).sqrt();

            // Effective angle
            if cos_to_center * cos_aabb - (1.0 - cos_to_center * cos_to_center).sqrt() * sin_aabb < cos_angle {
                return false;
            }
        }

        true
    }
}
