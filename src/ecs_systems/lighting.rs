// 라이팅 시스템
// 라이팅 데이터 추출 및 GPU 버퍼 업데이트

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::ecs_resources::{
    RenderExtractedData, ExtractedLighting,
    GpuContext, LightManagerRes, HairExtractedData,
    Time,
};

/// 라이팅 데이터 추출 시스템
///
/// 현재는 기본 태양광 설정을 사용합니다.
/// 추후 동적 라이트 쿼리로 확장 가능합니다.
pub fn lighting_extract_system(
    mut extracted_data: ResMut<RenderExtractedData>,
) {
    // 기본 태양광 설정
    extracted_data.lighting = ExtractedLighting {
        sun_direction: Vec3::new(-0.5, -1.0, -0.3).normalize(),
        sun_color: Vec3::new(1.0, 0.98, 0.95),
        sun_intensity: 3.0,
        ambient_color: Vec3::new(0.03, 0.03, 0.05),
    };
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

/// Hair 렌더링 준비 시스템
#[allow(dead_code)]
pub fn hair_prepare_system(
    time: Option<Res<Time>>,
    extracted_data: Res<RenderExtractedData>,
    mut hair_data: ResMut<HairExtractedData>,
) {
    hair_data.elapsed_time = time
        .map(|t| t.elapsed_seconds as f32)
        .unwrap_or(0.0);

    if let Some(ref camera) = extracted_data.camera {
        hair_data.view_proj = camera.view_projection;
        hair_data.view = camera.view_matrix;
        hair_data.proj = camera.projection_matrix;
        hair_data.camera_pos = camera.position;
    }
}
