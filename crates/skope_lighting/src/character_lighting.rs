// SKOPE Engine - Character Lighting System
// Face Shadow, Fill Light, Rim Light, Subsurface Scattering

use glam::Vec3;
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

