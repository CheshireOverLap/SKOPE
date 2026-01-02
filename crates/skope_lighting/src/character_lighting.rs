// SKOPE Engine - Character Lighting System
// Face Shadow, Fill Light, Rim Light, Subsurface Scattering

use glam::{Vec3, Vec4};
use bytemuck::{Pod, Zeroable};

/// 캐릭터 전용 라이팅 파라미터
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CharacterLightingParams {
    // Fill Light (보조광)
    pub fill_light_direction: [f32; 4],     // xyz = direction, w = intensity
    pub fill_light_color: [f32; 4],         // xyz = color, w = shadow_softness

    // Rim Light (역광)
    pub rim_light_direction: [f32; 4],      // xyz = direction, w = intensity
    pub rim_light_color: [f32; 4],          // xyz = color, w = power (rim falloff)

    // Face Shadow
    pub face_forward: [f32; 4],             // xyz = face forward direction
    pub face_shadow_params: [f32; 4],       // x = offset, y = softness, z = strength

    // Additional
    pub ambient_multiplier: f32,
    pub shadow_color_saturation: f32,
    pub highlight_intensity: f32,
    pub _pad: f32,
}

impl Default for CharacterLightingParams {
    fn default() -> Self {
        Self {
            fill_light_direction: [0.3, 0.5, -0.8, 0.3],
            fill_light_color: [0.9, 0.95, 1.0, 0.5],
            rim_light_direction: [-0.5, 0.2, -0.8, 0.8],
            rim_light_color: [1.0, 0.95, 0.9, 4.0],
            face_forward: [0.0, 0.0, 1.0, 0.0],
            face_shadow_params: [0.0, 0.3, 0.8, 0.0],
            ambient_multiplier: 1.2,
            shadow_color_saturation: 0.7,
            highlight_intensity: 1.0,
            _pad: 0.0,
        }
    }
}

impl CharacterLightingParams {
    pub fn from_vec4s(
        fill_dir: Vec4,
        fill_color: Vec4,
        rim_dir: Vec4,
        rim_color: Vec4,
        face_fwd: Vec4,
        face_shadow: Vec4,
        ambient_mult: f32,
        shadow_sat: f32,
        highlight: f32,
    ) -> Self {
        Self {
            fill_light_direction: fill_dir.to_array(),
            fill_light_color: fill_color.to_array(),
            rim_light_direction: rim_dir.to_array(),
            rim_light_color: rim_color.to_array(),
            face_forward: face_fwd.to_array(),
            face_shadow_params: face_shadow.to_array(),
            ambient_multiplier: ambient_mult,
            shadow_color_saturation: shadow_sat,
            highlight_intensity: highlight,
            _pad: 0.0,
        }
    }
}

/// Face Shadow 처리를 위한 SDF 기반 시스템
pub struct FaceShadowMap {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl FaceShadowMap {
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Face Shadow SDF"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 2,  // Left and right face SDF
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
        }
    }
}

/// 캐릭터별 라이팅 설정
#[derive(Debug, Clone)]
pub struct CharacterLightingConfig {
    pub params: CharacterLightingParams,

    // SSS (Subsurface Scattering)
    pub sss_color: Vec3,
    pub sss_radius: f32,
    pub sss_strength: f32,

    // Hair specular
    pub hair_specular_shift: f32,
    pub hair_specular_width: f32,
    pub hair_specular_color: Vec3,
    pub hair_specular_intensity: f32,

    // Eye
    pub eye_highlight_size: f32,
    pub eye_highlight_intensity: f32,
}

impl Default for CharacterLightingConfig {
    fn default() -> Self {
        Self {
            params: CharacterLightingParams::default(),
            sss_color: Vec3::new(1.0, 0.4, 0.25),
            sss_radius: 0.01,
            sss_strength: 0.5,
            hair_specular_shift: -0.1,
            hair_specular_width: 0.15,
            hair_specular_color: Vec3::new(1.0, 0.95, 0.9),
            hair_specular_intensity: 1.0,
            eye_highlight_size: 0.1,
            eye_highlight_intensity: 2.0,
        }
    }
}

/// 캐릭터 라이팅 매니저
pub struct CharacterLightingManager {
    configs: Vec<CharacterLightingConfig>,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl CharacterLightingManager {
    pub fn new(device: &wgpu::Device) -> Self {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Character Lighting Uniforms"),
            size: std::mem::size_of::<CharacterLightingParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Character Lighting Bind Group Layout"),
            entries: &[
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
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Character Lighting Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
            ],
        });

        Self {
            configs: vec![CharacterLightingConfig::default()],
            uniform_buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn set_config(&mut self, index: usize, config: CharacterLightingConfig) {
        if index >= self.configs.len() {
            self.configs.resize(index + 1, CharacterLightingConfig::default());
        }
        self.configs[index] = config;
    }

    pub fn update_uniforms(&self, queue: &wgpu::Queue, config_index: usize) {
        if let Some(config) = self.configs.get(config_index) {
            queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::bytes_of(&config.params),
            );
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}

/// 캐릭터 라이팅 프리셋
pub struct CharacterLightingPresets;

impl CharacterLightingPresets {
    /// 야외 낮 (밝은 태양광)
    pub fn outdoor_day() -> CharacterLightingConfig {
        CharacterLightingConfig {
            params: CharacterLightingParams {
                fill_light_direction: [0.4, 0.3, -0.8, 0.25],
                fill_light_color: [0.7, 0.85, 1.0, 0.4],
                rim_light_direction: [-0.6, 0.4, -0.7, 0.6],
                rim_light_color: [1.0, 0.95, 0.85, 3.5],
                face_forward: [0.0, 0.0, 1.0, 0.0],
                face_shadow_params: [0.0, 0.25, 0.7, 0.0],
                ambient_multiplier: 1.3,
                shadow_color_saturation: 0.6,
                highlight_intensity: 1.2,
                _pad: 0.0,
            },
            sss_color: Vec3::new(1.0, 0.5, 0.3),
            sss_radius: 0.012,
            sss_strength: 0.4,
            ..Default::default()
        }
    }

    /// 실내 (부드러운 조명)
    pub fn indoor_soft() -> CharacterLightingConfig {
        CharacterLightingConfig {
            params: CharacterLightingParams {
                fill_light_direction: [0.2, 0.6, -0.7, 0.4],
                fill_light_color: [1.0, 0.95, 0.9, 0.6],
                rim_light_direction: [-0.4, 0.3, -0.8, 0.5],
                rim_light_color: [1.0, 0.98, 0.95, 3.0],
                face_forward: [0.0, 0.0, 1.0, 0.0],
                face_shadow_params: [0.0, 0.4, 0.5, 0.0],
                ambient_multiplier: 1.5,
                shadow_color_saturation: 0.8,
                highlight_intensity: 0.8,
                _pad: 0.0,
            },
            sss_color: Vec3::new(1.0, 0.6, 0.4),
            sss_radius: 0.015,
            sss_strength: 0.6,
            ..Default::default()
        }
    }

    /// 야간 (달빛 + 인공조명)
    pub fn night() -> CharacterLightingConfig {
        CharacterLightingConfig {
            params: CharacterLightingParams {
                fill_light_direction: [0.3, 0.4, -0.8, 0.2],
                fill_light_color: [0.6, 0.7, 1.0, 0.3],
                rim_light_direction: [-0.5, 0.3, -0.7, 0.7],
                rim_light_color: [0.9, 0.95, 1.0, 4.0],
                face_forward: [0.0, 0.0, 1.0, 0.0],
                face_shadow_params: [0.0, 0.35, 0.85, 0.0],
                ambient_multiplier: 0.8,
                shadow_color_saturation: 0.5,
                highlight_intensity: 1.5,
                _pad: 0.0,
            },
            sss_color: Vec3::new(0.8, 0.4, 0.3),
            sss_radius: 0.008,
            sss_strength: 0.3,
            ..Default::default()
        }
    }

    /// 감성 컷신 (영화적 조명)
    pub fn cinematic() -> CharacterLightingConfig {
        CharacterLightingConfig {
            params: CharacterLightingParams {
                fill_light_direction: [0.5, 0.4, -0.75, 0.35],
                fill_light_color: [0.9, 0.85, 0.8, 0.5],
                rim_light_direction: [-0.7, 0.2, -0.6, 1.0],
                rim_light_color: [1.0, 0.9, 0.8, 5.0],
                face_forward: [0.0, 0.0, 1.0, 0.0],
                face_shadow_params: [0.1, 0.2, 0.9, 0.0],
                ambient_multiplier: 0.9,
                shadow_color_saturation: 0.4,
                highlight_intensity: 1.8,
                _pad: 0.0,
            },
            sss_color: Vec3::new(1.0, 0.5, 0.35),
            sss_radius: 0.01,
            sss_strength: 0.5,
            hair_specular_intensity: 1.5,
            ..Default::default()
        }
    }

    /// 보스전 (드라마틱)
    pub fn boss_battle() -> CharacterLightingConfig {
        CharacterLightingConfig {
            params: CharacterLightingParams {
                fill_light_direction: [0.0, 0.5, -0.85, 0.15],
                fill_light_color: [0.5, 0.6, 1.0, 0.2],
                rim_light_direction: [-0.3, 0.1, -0.9, 1.2],
                rim_light_color: [1.0, 0.4, 0.2, 6.0],
                face_forward: [0.0, 0.0, 1.0, 0.0],
                face_shadow_params: [-0.05, 0.15, 1.0, 0.0],
                ambient_multiplier: 0.6,
                shadow_color_saturation: 0.3,
                highlight_intensity: 2.0,
                _pad: 0.0,
            },
            sss_color: Vec3::new(1.0, 0.3, 0.2),
            sss_radius: 0.008,
            sss_strength: 0.35,
            ..Default::default()
        }
    }
}
