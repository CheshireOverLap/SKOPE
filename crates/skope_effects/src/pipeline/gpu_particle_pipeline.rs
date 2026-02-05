//! GPU Particle Pipeline
//!
//! Compute shader 기반 파티클 시뮬레이션 파이프라인

use wgpu::util::DeviceExt;

use crate::gpu_particle::{GpuEmitterConfig, GpuForceFieldArray, GpuParticle, MAX_FORCE_FIELDS};

/// GPU Particle System Pipeline
pub struct GpuParticlePipeline {
    /// Compute pipeline
    pipeline: wgpu::ComputePipeline,
    /// Bind group layout
    bind_group_layout: wgpu::BindGroupLayout,
    /// Particle storage buffer
    particle_buffer: wgpu::Buffer,
    /// Emitter config uniform buffer
    config_buffer: wgpu::Buffer,
    /// Force field array uniform buffer
    force_field_buffer: wgpu::Buffer,
    /// Current bind group
    bind_group: wgpu::BindGroup,
    /// Particle count
    particle_count: u32,
}

impl GpuParticlePipeline {
    /// 새 GPU 파티클 파이프라인 생성
    pub fn new(device: &wgpu::Device, particle_count: u32) -> Self {
        // Shader 로드
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("particle_update_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/particle_update.wgsl").into()),
        });

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu_particle_bind_group_layout"),
            entries: &[
                // Particle storage buffer (read/write)
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
                // Emitter config uniform
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
                // Force fields uniform
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

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu_particle_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        // Compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gpu_particle_compute_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // 파티클 버퍼 생성 (초기화 안됨)
        let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_particle_buffer"),
            size: (std::mem::size_of::<GpuParticle>() * particle_count as usize) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Config uniform buffer
        let config_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gpu_emitter_config_buffer"),
            contents: bytemuck::cast_slice(&[GpuEmitterConfig {
                particle_count,
                ..Default::default()
            }]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Force field uniform buffer
        let force_field_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("gpu_force_field_buffer"),
            contents: bytemuck::cast_slice(&[GpuForceFieldArray::default()]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_particle_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: force_field_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            pipeline,
            bind_group_layout,
            particle_buffer,
            config_buffer,
            force_field_buffer,
            bind_group,
            particle_count,
        }
    }

    /// 파티클 초기화 (CPU에서 초기 상태 설정)
    pub fn init_particles(&self, queue: &wgpu::Queue, particles: &[GpuParticle]) {
        let count = particles.len().min(self.particle_count as usize);
        queue.write_buffer(&self.particle_buffer, 0, bytemuck::cast_slice(&particles[..count]));
    }

    /// 이미터 설정 업데이트
    pub fn update_config(&self, queue: &wgpu::Queue, config: &GpuEmitterConfig) {
        queue.write_buffer(&self.config_buffer, 0, bytemuck::cast_slice(&[*config]));
    }

    /// Force field 배열 업데이트
    pub fn update_force_fields(&self, queue: &wgpu::Queue, force_fields: &GpuForceFieldArray) {
        queue.write_buffer(&self.force_field_buffer, 0, bytemuck::cast_slice(&[*force_fields]));
    }

    /// Compute shader 실행
    pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("gpu_particle_compute_pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, &self.bind_group, &[]);

        // workgroup size = 64, 총 파티클 수만큼 dispatch
        let workgroups = self.particle_count.div_ceil(64);
        compute_pass.dispatch_workgroups(workgroups, 1, 1);
    }

    /// 파티클 버퍼 반환 (렌더링용)
    pub fn particle_buffer(&self) -> &wgpu::Buffer {
        &self.particle_buffer
    }

    /// 파티클 수 반환
    pub fn particle_count(&self) -> u32 {
        self.particle_count
    }

    /// 파티클 버퍼 크기 변경 (새 버퍼 생성)
    pub fn resize(&mut self, device: &wgpu::Device, new_count: u32) {
        if new_count == self.particle_count {
            return;
        }

        self.particle_count = new_count;

        // 새 파티클 버퍼 생성
        self.particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_particle_buffer"),
            size: (std::mem::size_of::<GpuParticle>() * new_count as usize) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Bind group 재생성
        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_particle_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.particle_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.force_field_buffer.as_entire_binding(),
                },
            ],
        });

        log::info!("[GpuParticle] Resized buffer to {} particles", new_count);
    }
}
