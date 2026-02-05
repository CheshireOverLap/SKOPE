//! SKOPE Engine - Unified Effect Renderer
//!
//! 통합 이펙트 렌더러: Flipbook, VAT, Particle을 Forward Overlay 패스에서 렌더링

use std::collections::HashMap;
use bevy_ecs::prelude::*;
use bytemuck;

use crate::data::*;
use crate::systems::emitter::ParticleEmitter;
use super::flipbook::{FlipbookCameraUniform, FlipbookRenderer};
use crate::gpu_particle::{GpuEmitterConfig, GpuForceFieldArray, GpuParticle, GpuRenderConfig};
use super::gpu_particle_pipeline::GpuParticlePipeline;
use super::particle_renderer::ParticleRenderer;
use super::vat::{VatCameraUniform, VatLightUniform, VatModelUniform, VatRenderer};

/// 이펙트 렌더 데이터 (프레임마다 수집)
#[derive(Resource, Default)]
pub struct EffectRenderData {
    /// Flipbook 인스턴스 (텍스처 이름별 그룹)
    pub flipbook_batches: HashMap<String, Vec<FlipbookInstance>>,
    /// VAT 인스턴스 (에셋 이름별 그룹)
    pub vat_instances: HashMap<String, VatRenderBatch>,
    /// CPU 파티클 이미터들
    pub cpu_emitters: Vec<CpuEmitterRef>,
    /// GPU 파티클 파이프라인 인덱스들
    pub gpu_emitter_indices: Vec<usize>,
}

impl EffectRenderData {
    pub fn clear(&mut self) {
        self.flipbook_batches.clear();
        self.vat_instances.clear();
        self.cpu_emitters.clear();
        self.gpu_emitter_indices.clear();
    }

    /// 렌더링할 데이터가 있는지 확인
    pub fn has_data(&self) -> bool {
        !self.flipbook_batches.is_empty()
            || !self.vat_instances.is_empty()
            || !self.cpu_emitters.is_empty()
            || !self.gpu_emitter_indices.is_empty()
    }
}

/// VAT 렌더 배치
pub struct VatRenderBatch {
    pub instances: Vec<VatRenderInstance>,
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub index_count: u32,
}

/// VAT 렌더 인스턴스
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct VatRenderInstance {
    pub model_matrix: [[f32; 4]; 4],
    pub current_frame: f32,
    pub frame_blend: f32,
    pub color: [f32; 4],
    pub _pad: [f32; 2],
}

/// CPU 이미터 참조
pub struct CpuEmitterRef {
    pub emitter_ptr: *const ParticleEmitter,
}

// Safety: ParticleEmitter는 렌더링 중에만 참조됨
unsafe impl Send for CpuEmitterRef {}
unsafe impl Sync for CpuEmitterRef {}

/// 이펙트 에셋 (GPU 리소스)
pub struct EffectAsset {
    pub name: String,
    pub asset_type: EffectAssetType,
}

pub enum EffectAssetType {
    Flipbook {
        meta: FlipbookMeta,
        texture_view: wgpu::TextureView,
    },
    Vat {
        meta: VatMeta,
        position_texture: wgpu::TextureView,
        normal_texture: Option<wgpu::TextureView>,
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        index_count: u32,
    },
}

/// 이펙트 에셋 레지스트리 (전역 리소스)
#[derive(Default)]
pub struct EffectAssetRegistry {
    pub assets: HashMap<String, EffectAsset>,
}

impl EffectAssetRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, asset: EffectAsset) {
        self.assets.insert(asset.name.clone(), asset);
    }

    pub fn get(&self, name: &str) -> Option<&EffectAsset> {
        self.assets.get(name)
    }

    pub fn get_flipbook(&self, name: &str) -> Option<(&FlipbookMeta, &wgpu::TextureView)> {
        self.assets.get(name).and_then(|a| match &a.asset_type {
            EffectAssetType::Flipbook { meta, texture_view } => Some((meta, texture_view)),
            _ => None,
        })
    }
}

/// GPU 파티클 렌더 파이프라인 (GpuParticle 버퍼를 빌보드로 렌더링)
pub struct GpuParticleRenderPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    render_config_buffer: wgpu::Buffer,
    /// 바인드 그룹 캐시 (파티클 버퍼별)
    bind_groups: HashMap<usize, wgpu::BindGroup>,
}

impl GpuParticleRenderPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        // 셰이더 로드
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("GPU Particle Render Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/gpu_particle_render.wgsl").into()),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("GPU Particle Render BGL"),
                entries: &[
                    // binding 0: particle storage buffer (read-only for rendering)
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // binding 1: render config uniform
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("GPU Particle Render Layout"),
            bind_group_layouts: &[camera_bind_group_layout, &bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("GPU Particle Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        // 렌더 설정 버퍼
        let render_config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("GPU Particle Render Config"),
            size: std::mem::size_of::<GpuRenderConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            render_config_buffer,
            bind_groups: HashMap::new(),
        }
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// 바인드 그룹 생성 또는 캐시에서 가져오기
    pub fn get_or_create_bind_group(
        &mut self,
        device: &wgpu::Device,
        pipeline_index: usize,
        particle_buffer: &wgpu::Buffer,
    ) -> &wgpu::BindGroup {
        if !self.bind_groups.contains_key(&pipeline_index) {
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("GPU Particle Render BG {}", pipeline_index)),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: particle_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: self.render_config_buffer.as_entire_binding(),
                    },
                ],
            });
            self.bind_groups.insert(pipeline_index, bind_group);
        }
        self.bind_groups.get(&pipeline_index).unwrap()
    }

    /// 렌더 설정 업데이트
    pub fn update_config(&self, queue: &wgpu::Queue, config: &GpuRenderConfig) {
        queue.write_buffer(&self.render_config_buffer, 0, bytemuck::cast_slice(&[*config]));
    }

    /// GPU 파티클 렌더링
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
        camera_bind_group: &'a wgpu::BindGroup,
        particle_count: u32,
    ) {
        if particle_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_bind_group(1, bind_group, &[]);

        // 각 파티클당 6개의 정점 (2 삼각형)
        render_pass.draw(0..particle_count * 6, 0..1);
    }
}

/// 통합 이펙트 렌더러
pub struct EffectRenderer {
    /// Flipbook 렌더러
    pub flipbook: FlipbookRenderer,
    /// VAT 렌더러
    pub vat: VatRenderer,
    /// CPU 파티클 렌더러
    pub particle: ParticleRenderer,
    /// GPU 파티클 Compute 파이프라인들
    pub gpu_particle_pipelines: Vec<GpuParticlePipeline>,
    /// GPU 파티클 렌더 파이프라인
    pub gpu_particle_render: Option<GpuParticleRenderPipeline>,
    /// 에셋 레지스트리
    pub assets: EffectAssetRegistry,
    /// Flipbook 파이프라인 초기화 완료 여부
    flipbook_initialized: bool,
    /// VAT 파이프라인 초기화 완료 여부
    vat_initialized: bool,
}

impl EffectRenderer {
    /// 새 이펙트 렌더러 생성
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let flipbook = FlipbookRenderer::new(device);
        let vat = VatRenderer::new(device);
        let particle = ParticleRenderer::new(device, surface_format, camera_bind_group_layout);

        // GPU 파티클 렌더 파이프라인은 나중에 초기화
        let gpu_particle_render = None;

        Self {
            flipbook,
            vat,
            particle,
            gpu_particle_pipelines: Vec::new(),
            gpu_particle_render,
            assets: EffectAssetRegistry::new(),
            flipbook_initialized: false,
            vat_initialized: false,
        }
    }

    /// Flipbook 파이프라인 초기화 (셰이더 로드 후)
    pub fn init_flipbook_pipeline(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
    ) {
        if self.flipbook_initialized {
            return;
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Flipbook Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/flipbook.wgsl").into()),
        });

        self.flipbook.create_pipelines(device, &shader, color_format);
        self.flipbook_initialized = true;

        log::info!("[EffectRenderer] Flipbook pipeline initialized");
    }

    /// VAT 파이프라인 초기화
    pub fn init_vat_pipeline(
        &mut self,
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
    ) {
        if self.vat_initialized {
            return;
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("VAT Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/vat.wgsl").into()),
        });

        self.vat.create_pipeline(device, &shader, color_format);
        self.vat_initialized = true;

        log::info!("[EffectRenderer] VAT pipeline initialized");
    }

    /// GPU 파티클 렌더 파이프라인 초기화
    pub fn init_gpu_particle_render(
        &mut self,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) {
        if self.gpu_particle_render.is_some() {
            return;
        }

        self.gpu_particle_render = Some(GpuParticleRenderPipeline::new(
            device,
            surface_format,
            camera_bind_group_layout,
        ));

        log::info!("[EffectRenderer] GPU Particle render pipeline initialized");
    }

    /// GPU 파티클 파이프라인 추가
    pub fn add_gpu_particle_pipeline(&mut self, device: &wgpu::Device, particle_count: u32) -> usize {
        let pipeline = GpuParticlePipeline::new(device, particle_count);
        let index = self.gpu_particle_pipelines.len();
        self.gpu_particle_pipelines.push(pipeline);
        index
    }

    /// Camera uniform 업데이트
    pub fn update_camera(
        &self,
        queue: &wgpu::Queue,
        view_proj: [[f32; 4]; 4],
        view: [[f32; 4]; 4],
        camera_pos: [f32; 3],
    ) {
        let flipbook_camera = FlipbookCameraUniform {
            view_proj,
            view,
            camera_pos,
            _padding: 0.0,
        };
        self.flipbook.update_camera(queue, &flipbook_camera);

        let vat_camera = VatCameraUniform {
            view_proj,
            view,
            camera_pos,
            _padding: 0.0,
        };
        self.vat.update_camera(queue, &vat_camera);
    }

    /// GPU Compute 디스패치 (렌더 루프 초반에 호출)
    pub fn dispatch_gpu_particles(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        queue: &wgpu::Queue,
        delta_time: f32,
    ) {
        for pipeline in &mut self.gpu_particle_pipelines {
            // Config 업데이트 (delta_time 반영)
            let config = GpuEmitterConfig {
                delta_time,
                particle_count: pipeline.particle_count(),
                ..Default::default()
            };
            pipeline.update_config(queue, &config);

            // Compute 디스패치
            pipeline.dispatch(encoder);
        }
    }

    /// Flipbook 렌더링 준비 (바인드 그룹 생성)
    pub fn prepare_flipbook_bind_groups(
        &mut self,
        device: &wgpu::Device,
        depth_view: &wgpu::TextureView,
        asset_names: &[String],
    ) {
        for asset_name in asset_names {
            if let Some((_, texture_view)) = self.assets.get_flipbook(asset_name) {
                // 바인드 그룹 미리 생성
                let _ = self.flipbook.get_or_create_bind_group(
                    device,
                    asset_name,
                    texture_view,
                    depth_view,
                );
            }
        }
    }

    /// CPU 파티클 렌더링
    pub fn render_cpu_particles<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        camera_bind_group: &'a wgpu::BindGroup,
        emitters: &[&ParticleEmitter],
    ) {
        if !emitters.is_empty() {
            self.particle.render(render_pass, queue, camera_bind_group, emitters);
        }
    }

    /// Flipbook 배치 렌더링
    pub fn render_flipbook_batch<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        asset_name: &str,
        instances: &[FlipbookInstance],
    ) {
        if instances.is_empty() {
            return;
        }

        // 에셋에서 메타데이터 가져오기
        let (meta, blend_mode) = if let Some((m, _)) = self.assets.get_flipbook(asset_name) {
            (m.clone(), m.blend_mode)
        } else {
            return;
        };

        // Uniform 업데이트
        let uniforms = FlipbookUniforms {
            grid: [meta.grid.0, meta.grid.1],
            frame_count: meta.frame_count,
            soft_particle: if meta.soft_particle { 1 } else { 0 },
            depth_fade_distance: meta.depth_fade_distance,
            emission_strength: meta.shader_params.emission_strength,
            _pad: [0.0, 0.0],
        };
        self.flipbook.update_uniforms(queue, &uniforms);
        self.flipbook.update_instances(queue, instances);

        // 캐시된 바인드 그룹 가져오기
        if let Some(bind_group) = self.flipbook.get_bind_group(asset_name) {
            self.flipbook.render(
                render_pass,
                bind_group,
                blend_mode,
                instances.len() as u32,
            );
        }
    }

    /// Forward Overlay 패스에서 호출 - 모든 이펙트 렌더링 (단순화 버전)
    pub fn render_all<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        camera_bind_group: &'a wgpu::BindGroup,
        render_data: &EffectRenderData,
    ) {
        // 1. CPU 파티클 렌더링
        if !render_data.cpu_emitters.is_empty() {
            // Safety: cpu_emitters는 렌더링 중에만 유효한 참조
            let emitters: Vec<&ParticleEmitter> = render_data
                .cpu_emitters
                .iter()
                .map(|e| unsafe { &*e.emitter_ptr })
                .collect();

            self.particle.render(render_pass, queue, camera_bind_group, &emitters);
        }

        // 2. Flipbook 렌더링 (준비 단계에서 바인드 그룹 생성 필요)
        for (asset_name, instances) in &render_data.flipbook_batches {
            self.render_flipbook_batch(render_pass, queue, asset_name, instances);
        }

        // 3. VAT 렌더링 (TODO: 구현)
        // 4. GPU 파티클은 별도 메서드로 렌더링 (바인드 그룹 캐시 필요)
    }

    /// GPU 파티클 렌더링 준비 (바인드 그룹 생성)
    pub fn prepare_gpu_particle_bind_groups(&mut self, device: &wgpu::Device) {
        if let Some(ref mut gpu_render) = self.gpu_particle_render {
            for (idx, pipeline) in self.gpu_particle_pipelines.iter().enumerate() {
                let _ = gpu_render.get_or_create_bind_group(
                    device,
                    idx,
                    pipeline.particle_buffer(),
                );
            }
        }
    }

    /// GPU 파티클 렌더링
    pub fn render_gpu_particles<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        camera_bind_group: &'a wgpu::BindGroup,
        render_data: &EffectRenderData,
    ) {
        if let Some(ref gpu_render) = self.gpu_particle_render {
            for &idx in &render_data.gpu_emitter_indices {
                if idx < self.gpu_particle_pipelines.len() {
                    let pipeline = &self.gpu_particle_pipelines[idx];
                    let particle_count = pipeline.particle_count();

                    // 렌더 설정 업데이트
                    let config = GpuRenderConfig {
                        particle_count,
                        soft_particle: 0,
                        depth_fade_distance: 0.5,
                        emission_strength: 1.0,
                    };
                    gpu_render.update_config(queue, &config);

                    // 바인드 그룹 가져오기 (이미 prepare에서 생성됨)
                    if let Some(bind_group) = gpu_render.bind_groups.get(&idx) {
                        gpu_render.render(
                            render_pass,
                            bind_group,
                            camera_bind_group,
                            particle_count,
                        );
                    }
                }
            }
        }
    }

    /// 파이프라인 초기화 여부
    pub fn is_flipbook_ready(&self) -> bool {
        self.flipbook_initialized
    }

    pub fn is_vat_ready(&self) -> bool {
        self.vat_initialized
    }

    pub fn is_gpu_particle_ready(&self) -> bool {
        self.gpu_particle_render.is_some()
    }
}
