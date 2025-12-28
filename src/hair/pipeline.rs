// SKOPE Engine - Hybrid Hair Render Pipeline
// Phase 12: Card + Strand Rendering

use wgpu::util::DeviceExt;

use super::{HybridHairConfig, FlyawayParams, CardShadeParams, MarschnerParams, HairCardVertex, HairLOD};

/// Hybrid Hair 버퍼들
pub struct HybridHairBuffers {
    // Config
    pub config_buffer: wgpu::Buffer,
    pub flyaway_params_buffer: wgpu::Buffer,
    pub card_shade_buffer: wgpu::Buffer,
    pub marschner_buffer: wgpu::Buffer,

    // Card
    pub card_vertex_buffer: Option<wgpu::Buffer>,
    pub card_index_buffer: Option<wgpu::Buffer>,
    pub card_index_count: u32,

    // Strand
    pub spawn_points_buffer: wgpu::Buffer,
    pub strand_vertices_buffer: wgpu::Buffer,
    pub strand_counter_buffer: wgpu::Buffer,

    // Scalp points (flyaway 루트 위치)
    pub scalp_points_buffer: wgpu::Buffer,
}

/// Strand 셰이딩 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StrandShadeParams {
    pub base_color: [f32; 3],
    pub roughness: f32,
    pub specular_intensity: f32,
    pub ambient_occlusion: f32,
    pub _pad: [f32; 2],
}

impl Default for StrandShadeParams {
    fn default() -> Self {
        Self {
            base_color: [0.15, 0.1, 0.05],  // 갈색
            roughness: 0.4,
            specular_intensity: 0.8,
            ambient_occlusion: 0.5,
            _pad: [0.0; 2],
        }
    }
}

/// Hybrid Hair 렌더러
pub struct HybridHairRenderer {
    // 렌더 파이프라인들
    pub card_pipeline: wgpu::RenderPipeline,
    pub strand_pipeline: wgpu::RenderPipeline,

    // Compute 파이프라인
    pub flyaway_pipeline: wgpu::ComputePipeline,

    // Bind group layouts
    pub uniform_bind_group_layout: wgpu::BindGroupLayout,
    pub flyaway_bind_group_layout: wgpu::BindGroupLayout,
    pub strand_bind_group_layout: wgpu::BindGroupLayout,

    // Bind groups (런타임에 생성)
    pub uniform_bind_group: Option<wgpu::BindGroup>,
    pub flyaway_bind_group: Option<wgpu::BindGroup>,
    pub strand_bind_group: Option<wgpu::BindGroup>,

    // 버퍼
    pub buffers: HybridHairBuffers,

    // 추가 버퍼
    pub strand_shade_buffer: wgpu::Buffer,
    pub time_buffer: wgpu::Buffer,
    pub segments_buffer: wgpu::Buffer,
    pub strand_count_buffer: wgpu::Buffer,

    // 설정
    pub config: HybridHairConfig,
    pub segments_per_strand: u32,
    pub max_flyaway: u32,
    pub max_silhouette: u32,
    pub current_lod: HairLOD,
}

impl HybridHairRenderer {
    pub fn new(
        device: &wgpu::Device,
        config_format: wgpu::TextureFormat,
        max_flyaway: u32,
        max_silhouette: u32,
        segments_per_strand: u32,
    ) -> Self {
        let config = HybridHairConfig::default();

        // === Bind Group Layouts ===
        let uniform_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Hair Uniform Bind Group Layout"),
                entries: &[
                    // Config
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
                    // Card shade params
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Marschner params
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
            },
        );

        // === 버퍼 생성 ===
        let config_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hair Config Buffer"),
            contents: bytemuck::cast_slice(&[config]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let flyaway_params = FlyawayParams::default();
        let flyaway_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Flyaway Params Buffer"),
            contents: bytemuck::cast_slice(&[flyaway_params]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let card_shade = CardShadeParams::default();
        let card_shade_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Card Shade Params Buffer"),
            contents: bytemuck::cast_slice(&[card_shade]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let marschner = MarschnerParams::default();
        let marschner_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Marschner Params Buffer"),
            contents: bytemuck::cast_slice(&[marschner]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Spawn points (silhouette)
        let spawn_points_size = max_silhouette as usize * std::mem::size_of::<super::StrandSpawnPoint>();
        let spawn_points_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Spawn Points Buffer"),
            size: spawn_points_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Strand vertices
        let total_strands = max_flyaway + max_silhouette;
        let vertices_per_strand = segments_per_strand + 1;
        let strand_vertices_size = (total_strands * vertices_per_strand) as usize
            * std::mem::size_of::<super::StrandVertex>();
        let strand_vertices_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Strand Vertices Buffer"),
            size: strand_vertices_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        // Counter buffer
        let strand_counter_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Strand Counter Buffer"),
            size: 4,  // u32
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Scalp points (flyaway roots)
        let scalp_points_size = max_flyaway as usize * 16;  // vec4<f32>
        let scalp_points_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Scalp Points Buffer"),
            size: scalp_points_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // === 추가 버퍼들 ===
        let strand_shade = StrandShadeParams::default();
        let strand_shade_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Strand Shade Params Buffer"),
            contents: bytemuck::cast_slice(&[strand_shade]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let time_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Time Buffer"),
            contents: bytemuck::cast_slice(&[0.0f32]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let segments_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Segments Per Strand Buffer"),
            contents: bytemuck::cast_slice(&[segments_per_strand]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let strand_count_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Strand Count Uniform Buffer"),
            contents: bytemuck::cast_slice(&[max_flyaway]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // === Flyaway Compute Bind Group Layout ===
        let flyaway_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Flyaway Compute Bind Group Layout"),
                entries: &[
                    // FlyawayParams
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
                    // Scalp points (read)
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
                    // Strand vertices (read_write)
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
                    // Segments per strand
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Time
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Strand count
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            },
        );

        // === Strand Render Bind Group Layout ===
        let strand_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("Strand Render Bind Group Layout"),
                entries: &[
                    // Camera uniforms (placeholder - 실제로는 메인 카메라 버퍼 사용)
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
                    // Strand shade params
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Strand vertices (read)
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Segments per strand
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Strand count
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            },
        );

        // === Card Pipeline ===
        let card_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hair Card Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/hair_card.wgsl").into()),
        });

        let card_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Hair Card Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let card_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Hair Card Pipeline"),
            layout: Some(&card_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &card_shader,
                entry_point: Some("vs_main"),
                buffers: &[HairCardVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &card_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,  // 양면 렌더링
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // === Flyaway Compute Pipeline ===
        let flyaway_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Flyaway Generate Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/hair_flyaway_generate.wgsl").into()),
        });

        let flyaway_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Flyaway Pipeline Layout"),
            bind_group_layouts: &[&flyaway_bind_group_layout],
            push_constant_ranges: &[],
        });

        let flyaway_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Flyaway Compute Pipeline"),
            layout: Some(&flyaway_pipeline_layout),
            module: &flyaway_shader,
            entry_point: Some("generate_flyaway"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        // === Strand Render Pipeline ===
        let strand_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Strand Rasterize Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/hair_strand_rasterize.wgsl").into()),
        });

        let strand_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Strand Pipeline Layout"),
            bind_group_layouts: &[&strand_bind_group_layout],
            push_constant_ranges: &[],
        });

        let strand_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Strand Render Pipeline"),
            layout: Some(&strand_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &strand_shader,
                entry_point: Some("vs_main"),
                buffers: &[],  // 버퍼에서 직접 읽음
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &strand_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let buffers = HybridHairBuffers {
            config_buffer,
            flyaway_params_buffer,
            card_shade_buffer,
            marschner_buffer,
            card_vertex_buffer: None,
            card_index_buffer: None,
            card_index_count: 0,
            spawn_points_buffer,
            strand_vertices_buffer,
            strand_counter_buffer,
            scalp_points_buffer,
        };

        log::info!("Created Hybrid Hair Renderer:");
        log::info!("  Max flyaway: {}", max_flyaway);
        log::info!("  Max silhouette: {}", max_silhouette);
        log::info!("  Segments per strand: {}", segments_per_strand);

        Self {
            card_pipeline,
            strand_pipeline,
            flyaway_pipeline,
            uniform_bind_group_layout,
            flyaway_bind_group_layout,
            strand_bind_group_layout,
            uniform_bind_group: None,
            flyaway_bind_group: None,
            strand_bind_group: None,
            buffers,
            strand_shade_buffer,
            time_buffer,
            segments_buffer,
            strand_count_buffer: strand_count_uniform_buffer,
            config,
            segments_per_strand,
            max_flyaway,
            max_silhouette,
            current_lod: HairLOD::Full,
        }
    }

    /// Bind groups 생성 (초기화 후 호출)
    pub fn create_bind_groups(&mut self, device: &wgpu::Device) {
        // Uniform bind group (for card pipeline)
        self.uniform_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Hair Uniform Bind Group"),
            layout: &self.uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.buffers.config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.buffers.card_shade_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.buffers.marschner_buffer.as_entire_binding(),
                },
            ],
        }));

        // Flyaway compute bind group
        self.flyaway_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Flyaway Compute Bind Group"),
            layout: &self.flyaway_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.buffers.flyaway_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.buffers.scalp_points_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.buffers.strand_vertices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.segments_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.time_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: self.strand_count_buffer.as_entire_binding(),
                },
            ],
        }));
    }

    /// Strand bind group 생성 (카메라 버퍼가 필요하므로 별도)
    pub fn create_strand_bind_group(&mut self, device: &wgpu::Device, camera_buffer: &wgpu::Buffer) {
        self.strand_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Strand Render Bind Group"),
            layout: &self.strand_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.strand_shade_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.buffers.strand_vertices_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.segments_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: self.strand_count_buffer.as_entire_binding(),
                },
            ],
        }));
    }

    /// 시간 업데이트 (애니메이션용)
    pub fn update_time(&self, queue: &wgpu::Queue, time: f32) {
        queue.write_buffer(&self.time_buffer, 0, bytemuck::cast_slice(&[time]));
    }

    /// Flyaway params 업데이트
    pub fn update_flyaway_params(&self, queue: &wgpu::Queue, params: FlyawayParams) {
        queue.write_buffer(&self.buffers.flyaway_params_buffer, 0, bytemuck::cast_slice(&[params]));
    }

    /// Strand shade params 업데이트
    pub fn update_strand_shade(&self, queue: &wgpu::Queue, params: StrandShadeParams) {
        queue.write_buffer(&self.strand_shade_buffer, 0, bytemuck::cast_slice(&[params]));
    }

    /// Scalp points 업데이트 (flyaway 루트 위치)
    pub fn set_scalp_points(&self, queue: &wgpu::Queue, points: &[[f32; 4]]) {
        queue.write_buffer(
            &self.buffers.scalp_points_buffer,
            0,
            bytemuck::cast_slice(points),
        );
        queue.write_buffer(
            &self.strand_count_buffer,
            0,
            bytemuck::cast_slice(&[points.len() as u32]),
        );
    }

    /// LOD 설정
    pub fn set_lod(&mut self, lod: HairLOD) {
        self.current_lod = lod;
    }

    /// Flyaway strand 생성 (compute pass)
    pub fn dispatch_flyaway_generation<'a>(&'a self, compute_pass: &mut wgpu::ComputePass<'a>) {
        if let Some(ref bind_group) = self.flyaway_bind_group {
            compute_pass.set_pipeline(&self.flyaway_pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);

            // 64 threads per workgroup
            let workgroups = (self.max_flyaway + 63) / 64;
            compute_pass.dispatch_workgroups(workgroups, 1, 1);
        }
    }

    /// Card 렌더링
    pub fn render_cards<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        // LOD 체크: CardOnly 이상에서만 카드 렌더링
        match self.current_lod {
            HairLOD::Full | HairLOD::Reduced | HairLOD::CardSilhouette | HairLOD::CardOnly => {}
        }

        if let (Some(ref vb), Some(ref ib), Some(ref bg)) = (
            &self.buffers.card_vertex_buffer,
            &self.buffers.card_index_buffer,
            &self.uniform_bind_group,
        ) {
            render_pass.set_pipeline(&self.card_pipeline);
            render_pass.set_bind_group(0, bg, &[]);
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.buffers.card_index_count, 0, 0..1);
        }
    }

    /// Strand 렌더링
    pub fn render_strands<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        // LOD 체크: Full/Reduced에서만 strand 렌더링
        match self.current_lod {
            HairLOD::Full | HairLOD::Reduced => {}
            HairLOD::CardSilhouette | HairLOD::CardOnly => return,
        }

        if let Some(ref bind_group) = self.strand_bind_group {
            render_pass.set_pipeline(&self.strand_pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);

            // 각 strand마다 (segments * 6) vertices (2 triangles per segment)
            let vertices_per_strand = self.segments_per_strand * 6;
            let total_vertices = vertices_per_strand * self.max_flyaway;
            render_pass.draw(0..total_vertices, 0..1);
        }
    }

    /// Card 메시 설정
    pub fn set_card_mesh(
        &mut self,
        device: &wgpu::Device,
        vertices: &[HairCardVertex],
        indices: &[u32],
    ) {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hair Card Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hair Card Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.buffers.card_vertex_buffer = Some(vertex_buffer);
        self.buffers.card_index_buffer = Some(index_buffer);
        self.buffers.card_index_count = indices.len() as u32;

        log::info!("Set hair card mesh: {} vertices, {} indices",
            vertices.len(), indices.len());
    }

    /// Config 업데이트
    pub fn update_config(&mut self, queue: &wgpu::Queue, config: HybridHairConfig) {
        self.config = config;
        queue.write_buffer(&self.buffers.config_buffer, 0, bytemuck::cast_slice(&[config]));
    }

    /// Card shade params 업데이트
    pub fn update_card_shade(&self, queue: &wgpu::Queue, params: CardShadeParams) {
        queue.write_buffer(&self.buffers.card_shade_buffer, 0, bytemuck::cast_slice(&[params]));
    }

    /// Marschner params 업데이트
    pub fn update_marschner(&self, queue: &wgpu::Queue, params: MarschnerParams) {
        queue.write_buffer(&self.buffers.marschner_buffer, 0, bytemuck::cast_slice(&[params]));
    }
}
