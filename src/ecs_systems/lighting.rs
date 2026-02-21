// 라이팅 시스템
// ECS Light 엔티티에서 라이팅 데이터 추출 + GPU 버퍼 업데이트

use skope_ecs::prelude::*;
use glam::Vec3;

use crate::ecs_components::{Light, Transform, SunPositionDriver};
use skope_core::components::light::LightType;
use crate::ecs_resources::{
    RenderExtractedData, ExtractedLighting,
    GpuContext, LightManagerRes, Time,
};

/// SunPositionDriver가 있는 Sun Light의 Transform.rotation을 NOAA 태양 위치로 업데이트.
pub fn sun_position_update_system(
    time: Res<Time>,
    mut query: Query<(&Light, &mut SunPositionDriver, &mut Transform)>,
) {
    for (light, mut driver, mut transform) in query.iter_mut() {
        if light.light_type != LightType::Sun {
            continue;
        }

        // Auto-advance time if enabled
        if driver.auto_advance {
            driver.time_of_day += time.delta_seconds * driver.time_speed / 3600.0;
            driver.time_of_day %= 24.0;
            if driver.time_of_day < 0.0 {
                driver.time_of_day += 24.0;
            }
        }

        // Decompose fractional time-of-day
        let (hours, minutes, seconds) = skope_lighting::decompose_time(driver.time_of_day);

        // Convert day_of_year → (month, day)
        let (month, day) = skope_lighting::day_of_year_to_month_day(driver.day_of_year, driver.year);

        // Calculate NOAA sun position
        let sun_pos = skope_lighting::calculate_sun_position(
            driver.latitude,
            driver.longitude,
            driver.timezone,
            driver.year,
            month,
            day,
            hours,
            minutes,
            seconds,
        );

        // Convert angles to world direction
        let dir = skope_lighting::sun_angles_to_direction(
            sun_pos.corrected_elevation,
            sun_pos.azimuth,
        );

        // Update transform rotation: the light looks along -Z, so we
        // compute the quaternion that rotates -Z to the desired direction.
        transform.rotation = glam::Quat::from_rotation_arc(Vec3::NEG_Z, dir);
    }
}

/// ECS Light 엔티티에서 ExtractedLighting(sun) 데이터를 추출하는 시스템.
/// Sun 타입 Light가 없으면 기본 ambient만 설정.
/// UE5 확장 필드 포함.
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

            // Compute effective color (with optional Kelvin temperature)
            let effective_color = skope_lighting::compute_effective_color(
                light.color,
                light.temperature,
                light.use_temperature,
            );

            extracted_data.lighting = ExtractedLighting {
                sun_direction: direction.normalize(),
                sun_color: effective_color,
                sun_intensity: light.intensity,
                ambient_color: Vec3::new(0.03, 0.03, 0.05),
                // UE5 extension fields
                sun_source_radius: skope_lighting::light_source_angle_to_source_radius(
                    light.light_source_angle,
                ),
                sun_specular_scale: light.specular_scale,
                sun_diffuse_scale: light.diffuse_scale,
                sun_shadow_amount: light.shadow_amount,
                cascade_distribution_exponent: light.cascade_distribution_exponent,
                dynamic_shadow_distance: light.dynamic_shadow_distance,
                shadow_cascade_count: light.shadow_cascade_count,
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
            ..Default::default()
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
    lm.rect_area_lights.clear();
    lm.disk_area_lights.clear();
    lm.mark_dirty();

    for (light, transform) in lights.iter() {
        match light.light_type {
            LightType::Sun => {
                let direction = transform.rotation * Vec3::new(0.0, 0.0, -1.0);
                let effective_color = skope_lighting::compute_effective_color(
                    light.color,
                    light.temperature,
                    light.use_temperature,
                );
                lm.add_directional(skope_lighting::DirectionalLight {
                    direction: direction.normalize(),
                    color: effective_color,
                    intensity: light.intensity,
                    angular_diameter: light.light_source_angle,
                    cast_shadows: light.cast_shadows,
                    shadow_cascade_count: light.shadow_cascade_count,
                    shadow_distance: light.dynamic_shadow_distance,
                });
            }
            LightType::Point => {
                lm.add_point(skope_lighting::PointLight {
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
                lm.add_spot(skope_lighting::SpotLight {
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
