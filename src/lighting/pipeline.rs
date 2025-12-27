// SKOPE Engine - Deferred Lighting Pipeline
// Combines all lighting systems into unified rendering

use super::*;
use glam::{Vec3, Vec4, Mat4};
use bytemuck::{Pod, Zeroable};

/// 라이팅 파이프라인 설정
#[derive(Debug, Clone)]
pub struct LightingPipelineConfig {
    pub enable_shadows: bool,
    pub enable_ssao: bool,
    pub enable_ibl: bool,
    pub enable_light_probes: bool,
    pub enable_clustered: bool,
    pub debug_mode: LightingDebugMode,
}

impl Default for LightingPipelineConfig {
    fn default() -> Self {
        Self {
            enable_shadows: true,
            enable_ssao: true,
            enable_ibl: true,
            enable_light_probes: true,
            enable_clustered: true,
            debug_mode: LightingDebugMode::None,
        }
    }
}

/// 디버그 시각화 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingDebugMode {
    None,
    Albedo,
    Normal,
    Metallic,
    Roughness,
    AO,
    Depth,
    Shadow,
    Clusters,
    Cascade,
    LightHeatmap,
}

/// GPU용 라이팅 유니폼
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightingUniforms {
    pub view_matrix: [[f32; 4]; 4],
    pub proj_matrix: [[f32; 4]; 4],
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_position: [f32; 4],
    pub ambient_color: [f32; 4],
    pub sun_direction: [f32; 4],
    pub sun_color: [f32; 4],
    pub screen_size: [f32; 2],
    pub time: f32,
    pub debug_mode: u32,
    pub enable_flags: u32,  // bitfield
    pub ibl_intensity: f32,
    pub ao_intensity: f32,
    pub shadow_intensity: f32,
}

impl LightingUniforms {
    pub fn new(
        view: Mat4,
        proj: Mat4,
        camera_pos: Vec3,
        ambient: Vec3,
        sun_dir: Vec3,
        sun_color_intensity: Vec4,
        screen_width: u32,
        screen_height: u32,
        time: f32,
        debug_mode: u32,
        enable_flags: u32,
        ibl_intensity: f32,
        ao_intensity: f32,
        shadow_intensity: f32,
    ) -> Self {
        let inv_view_proj = (proj * view).inverse();
        Self {
            view_matrix: view.to_cols_array_2d(),
            proj_matrix: proj.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_position: [camera_pos.x, camera_pos.y, camera_pos.z, 1.0],
            ambient_color: [ambient.x, ambient.y, ambient.z, 1.0],
            sun_direction: [sun_dir.x, sun_dir.y, sun_dir.z, 0.0],
            sun_color: sun_color_intensity.to_array(),
            screen_size: [screen_width as f32, screen_height as f32],
            time,
            debug_mode,
            enable_flags,
            ibl_intensity,
            ao_intensity,
            shadow_intensity,
        }
    }
}

/// Deferred Lighting Pipeline
pub struct DeferredLightingPipeline {
    config: LightingPipelineConfig,

    // Core systems
    light_manager: LightManager,
    csm: Option<CascadedShadowMap>,
    clustered: Option<ClusteredLighting>,
    ibl: Option<IBLEnvironment>,
    light_probe_grid: Option<LightProbeGrid>,
    character_lighting: CharacterLightingManager,

    // GPU resources
    uniform_buffer: wgpu::Buffer,
    lighting_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,

    // Placeholder resources (for initial bind group)
    _placeholder_tex: wgpu::Texture,
    _placeholder_depth: wgpu::Texture,
    _placeholder_light_buf: wgpu::Buffer,
    _placeholder_count_buf: wgpu::Buffer,
    sampler: wgpu::Sampler,

    // Output
    output_texture: Option<wgpu::Texture>,
    output_view: Option<wgpu::TextureView>,
}

impl DeferredLightingPipeline {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: LightingPipelineConfig,
        screen_width: u32,
        screen_height: u32,
    ) -> Self {
        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lighting Uniforms"),
            size: std::mem::size_of::<LightingUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout (G-Buffer inputs + lighting data)
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Deferred Lighting Bind Group Layout"),
            entries: &[
                // Uniforms
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
                // G-Buffer: Albedo
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
                // G-Buffer: Normal
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
                // G-Buffer: Material (Metallic, Roughness, AO)
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
                // G-Buffer: Depth
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Light Buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Light Count
                wgpu::BindGroupLayoutEntry {
                    binding: 7,
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

        // Fullscreen quad shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Deferred Lighting Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/deferred_lighting.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Deferred Lighting Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let lighting_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Deferred Lighting Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Initialize subsystems
        let light_manager = LightManager::new();

        let csm = if config.enable_shadows {
            Some(CascadedShadowMap::new(device, CascadedShadowConfig::default()))
        } else {
            None
        };

        let clustered = if config.enable_clustered {
            Some(ClusteredLighting::new(
                device,
                ClusterConfig::default(),
                screen_width,
                screen_height,
            ))
        } else {
            None
        };

        let ibl = if config.enable_ibl {
            Some(IBLEnvironment::new(device, queue, 512))
        } else {
            None
        };

        let light_probe_grid = if config.enable_light_probes {
            Some(LightProbeGrid::new(
                Vec3::new(-50.0, -10.0, -50.0),
                Vec3::new(50.0, 30.0, 50.0),
                [8, 4, 8],
            ))
        } else {
            None
        };

        let character_lighting = CharacterLightingManager::new(device);

        // Sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Lighting Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create placeholder textures for initial bind group
        let placeholder_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Placeholder"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let placeholder_view = placeholder_tex.create_view(&Default::default());

        let placeholder_depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Placeholder Depth"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let placeholder_depth_view = placeholder_depth.create_view(&Default::default());

        // Placeholder light buffer
        let placeholder_light_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Placeholder Light Buffer"),
            size: std::mem::size_of::<GpuLight>() as u64,
            usage: wgpu::BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let placeholder_count_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Placeholder Count Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Deferred Lighting Bind Group (Placeholder)"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&placeholder_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&placeholder_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&placeholder_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&placeholder_depth_view) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: placeholder_light_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: placeholder_count_buf.as_entire_binding() },
            ],
        });

        Self {
            config,
            light_manager,
            csm,
            clustered,
            ibl,
            light_probe_grid,
            character_lighting,
            uniform_buffer,
            lighting_pipeline,
            bind_group_layout,
            bind_group,
            _placeholder_tex: placeholder_tex,
            _placeholder_depth: placeholder_depth,
            _placeholder_light_buf: placeholder_light_buf,
            _placeholder_count_buf: placeholder_count_buf,
            sampler,
            output_texture: None,
            output_view: None,
        }
    }

    /// G-Buffer 바인딩 업데이트
    pub fn update_gbuffer_bindings(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        albedo: &wgpu::TextureView,
        normal: &wgpu::TextureView,
        material: &wgpu::TextureView,
        depth: &wgpu::TextureView,
    ) {
        self.light_manager.update_gpu_buffers(device, queue);

        let light_buffer = self.light_manager.light_buffer().unwrap();
        let light_count_buffer = self.light_manager.light_count_buffer().unwrap();

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Deferred Lighting Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: self.uniform_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(albedo) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(normal) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(material) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(depth) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: light_buffer.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: light_count_buffer.as_entire_binding() },
            ],
        });
    }

    /// 유니폼 업데이트
    pub fn update_uniforms(
        &self,
        queue: &wgpu::Queue,
        view: Mat4,
        proj: Mat4,
        camera_pos: Vec3,
        sun_dir: Vec3,
        sun_color: Vec3,
        screen_width: u32,
        screen_height: u32,
        time: f32,
    ) {
        let mut enable_flags = 0u32;
        if self.config.enable_shadows { enable_flags |= 1; }
        if self.config.enable_ssao { enable_flags |= 2; }
        if self.config.enable_ibl { enable_flags |= 4; }
        if self.config.enable_light_probes { enable_flags |= 8; }
        if self.config.enable_clustered { enable_flags |= 16; }

        let uniforms = LightingUniforms::new(
            view,
            proj,
            camera_pos,
            Vec3::new(0.03, 0.03, 0.05),
            sun_dir,
            Vec4::new(sun_color.x, sun_color.y, sun_color.z, 1.0),
            screen_width,
            screen_height,
            time,
            self.config.debug_mode as u32,
            enable_flags,
            self.ibl.as_ref().map(|i| i.intensity()).unwrap_or(1.0),
            1.0,
            1.0,
        );

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// 라이팅 패스 실행
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        output: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Deferred Lighting Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&self.lighting_pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);  // Fullscreen triangle
    }

    // Accessors
    pub fn light_manager(&self) -> &LightManager {
        &self.light_manager
    }

    pub fn light_manager_mut(&mut self) -> &mut LightManager {
        &mut self.light_manager
    }

    pub fn csm(&self) -> Option<&CascadedShadowMap> {
        self.csm.as_ref()
    }

    pub fn csm_mut(&mut self) -> Option<&mut CascadedShadowMap> {
        self.csm.as_mut()
    }

    pub fn clustered(&self) -> Option<&ClusteredLighting> {
        self.clustered.as_ref()
    }

    pub fn ibl(&self) -> Option<&IBLEnvironment> {
        self.ibl.as_ref()
    }

    pub fn light_probe_grid(&self) -> Option<&LightProbeGrid> {
        self.light_probe_grid.as_ref()
    }

    pub fn light_probe_grid_mut(&mut self) -> Option<&mut LightProbeGrid> {
        self.light_probe_grid.as_mut()
    }

    pub fn character_lighting(&self) -> &CharacterLightingManager {
        &self.character_lighting
    }

    pub fn set_debug_mode(&mut self, mode: LightingDebugMode) {
        self.config.debug_mode = mode;
    }
}

/// 라이팅 프리셋
pub struct LightingPresets;

impl LightingPresets {
    /// 야외 낮 (맑은 날)
    pub fn outdoor_day(pipeline: &mut DeferredLightingPipeline) {
        let light_mgr = pipeline.light_manager_mut();
        light_mgr.directional_lights.clear();

        // 태양
        light_mgr.add_directional(DirectionalLight {
            direction: Vec3::new(-0.5, -0.8, -0.3).normalize(),
            color: Vec3::new(1.0, 0.95, 0.85),
            intensity: 3.0,
            angular_diameter: 0.53,
            cast_shadows: true,
            shadow_cascade_count: 4,
            shadow_distance: 100.0,
        });

        light_mgr.mark_dirty();
    }

    /// 야외 일몰
    pub fn outdoor_sunset(pipeline: &mut DeferredLightingPipeline) {
        let light_mgr = pipeline.light_manager_mut();
        light_mgr.directional_lights.clear();

        // 석양
        light_mgr.add_directional(DirectionalLight {
            direction: Vec3::new(-0.8, -0.3, -0.5).normalize(),
            color: Vec3::new(1.0, 0.5, 0.2),
            intensity: 2.0,
            angular_diameter: 1.0,  // 큰 태양
            cast_shadows: true,
            shadow_cascade_count: 4,
            shadow_distance: 80.0,
        });

        light_mgr.mark_dirty();
    }

    /// 실내
    pub fn indoor(pipeline: &mut DeferredLightingPipeline) {
        let light_mgr = pipeline.light_manager_mut();
        light_mgr.directional_lights.clear();
        light_mgr.point_lights.clear();
        light_mgr.spot_lights.clear();

        // 약한 간접광 (창문)
        light_mgr.add_directional(DirectionalLight {
            direction: Vec3::new(0.3, -0.5, -0.8).normalize(),
            color: Vec3::new(0.7, 0.8, 1.0),
            intensity: 0.5,
            angular_diameter: 2.0,
            cast_shadows: false,
            ..Default::default()
        });

        // 실내 조명들
        light_mgr.add_point(PointLight {
            position: Vec3::new(0.0, 3.0, 0.0),
            color: Vec3::new(1.0, 0.95, 0.8),
            intensity: 5.0,
            radius: 8.0,
            source_radius: 0.1,
            ..Default::default()
        });

        light_mgr.mark_dirty();
    }

    /// 야간
    pub fn night(pipeline: &mut DeferredLightingPipeline) {
        let light_mgr = pipeline.light_manager_mut();
        light_mgr.directional_lights.clear();

        // 달빛
        light_mgr.add_directional(DirectionalLight {
            direction: Vec3::new(0.4, -0.7, -0.5).normalize(),
            color: Vec3::new(0.6, 0.7, 1.0),
            intensity: 0.3,
            angular_diameter: 0.5,
            cast_shadows: true,
            shadow_cascade_count: 2,
            shadow_distance: 50.0,
        });

        light_mgr.mark_dirty();
    }

    /// 보스 아레나
    pub fn boss_arena(pipeline: &mut DeferredLightingPipeline) {
        let light_mgr = pipeline.light_manager_mut();
        light_mgr.directional_lights.clear();
        light_mgr.point_lights.clear();

        // 드라마틱한 상단 조명
        light_mgr.add_directional(DirectionalLight {
            direction: Vec3::new(0.0, -1.0, 0.0),
            color: Vec3::new(0.4, 0.5, 1.0),
            intensity: 1.0,
            cast_shadows: true,
            ..Default::default()
        });

        // 붉은 악센트 조명
        light_mgr.add_point(PointLight {
            position: Vec3::new(0.0, 2.0, -10.0),
            color: Vec3::new(1.0, 0.2, 0.1),
            intensity: 8.0,
            radius: 15.0,
            ..Default::default()
        });

        light_mgr.mark_dirty();
    }
}
