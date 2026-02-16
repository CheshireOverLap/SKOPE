// 라이팅 시스템
// ECS Light 엔티티에서 라이팅 데이터 추출 + GPU 버퍼 업데이트

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::ecs_components::{Light, Transform};
use skope_core::components::light::LightType;
use crate::ecs_resources::{
    RenderExtractedData, ExtractedLighting,
    GpuContext, LightManagerRes,
};

/// ECS Light 엔티티에서 ExtractedLighting(sun) 데이터를 추출하는 시스템.
/// Sun 타입 Light가 없으면 기본 ambient만 설정.
pub fn lighting_extract_system(
    mut extracted_data: ResMut<RenderExtractedData>,
    lights: Query<(&Light, &Transform)>,
) {
    // Find the first Sun light entity
    let mut sun_found = false;
    for (light, transform) in lights.iter() {
        if light.light_type == LightType::Sun {
            // Sun direction = Transform rotation applied to -Z (forward)
            let direction = transform.rotation * Vec3::new(0.0, 0.0, -1.0);
            extracted_data.lighting = ExtractedLighting {
                sun_direction: direction.normalize(),
                sun_color: light.color,
                sun_intensity: light.intensity,
                ambient_color: Vec3::new(0.03, 0.03, 0.05),
            };
            sun_found = true;
            break;
        }
    }

    if !sun_found {
        // No sun entity — dim ambient only
        extracted_data.lighting = ExtractedLighting {
            sun_direction: Vec3::new(0.0, -1.0, 0.0),
            sun_color: Vec3::ZERO,
            sun_intensity: 0.0,
            ambient_color: Vec3::new(0.05, 0.05, 0.05),
        };
    }
}

/// ECS Light 엔티티에서 Point/Spot 라이트를 LightManager로 동기화하는 시스템.
/// 매 프레임 LightManager를 비우고 ECS 데이터로 다시 채움.
pub fn light_sync_system(
    lights: Query<(&Light, &Transform)>,
    mut light_manager: Option<ResMut<LightManagerRes>>,
) {
    let Some(ref mut lm_res) = light_manager else { return };
    let lm = &mut lm_res.manager;

    // Clear existing lights (rebuild from ECS every frame)
    lm.directional_lights.clear();
    lm.point_lights.clear();
    lm.spot_lights.clear();
    lm.mark_dirty();

    for (light, transform) in lights.iter() {
        match light.light_type {
            LightType::Sun => {
                let direction = transform.rotation * Vec3::new(0.0, 0.0, -1.0);
                lm.add_directional(skope_blitz::DirectionalLight {
                    direction: direction.normalize(),
                    color: light.color,
                    intensity: light.intensity,
                    cast_shadows: light.cast_shadows,
                    ..Default::default()
                });
            }
            LightType::Point => {
                lm.add_point(skope_blitz::PointLight {
                    position: transform.translation,
                    color: light.color,
                    intensity: light.intensity,
                    radius: light.range,
                    cast_shadows: light.cast_shadows,
                    ..Default::default()
                });
            }
            LightType::Spot => {
                let direction = transform.rotation * Vec3::new(0.0, 0.0, -1.0);
                lm.add_spot(skope_blitz::SpotLight {
                    position: transform.translation,
                    direction: direction.normalize(),
                    color: light.color,
                    intensity: light.intensity,
                    radius: light.range,
                    outer_angle: light.spot_angle,
                    inner_angle: light.spot_angle * 0.8,
                    cast_shadows: light.cast_shadows,
                    ..Default::default()
                });
            }
            LightType::Area => {
                // Area lights not yet mapped to LightManager
            }
        }
    }
}

/// LightManager GPU 버퍼 업데이트 시스템
///
/// 동적 라이트들의 GPU 버퍼를 업데이트합니다.
pub fn light_buffer_update_system(
    gpu_ctx: Option<Res<GpuContext>>,
    mut light_manager: Option<ResMut<LightManagerRes>>,
) {
    let Some(ref gpu_ctx) = gpu_ctx else { return };

    if let Some(ref mut light_manager) = light_manager {
        light_manager.manager.update_gpu_buffers(&gpu_ctx.device, &gpu_ctx.queue);
    }
}
