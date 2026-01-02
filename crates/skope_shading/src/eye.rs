//! SKOPE Engine - Eye Shading
//! 물리 기반 + 스타일라이즈드 눈 렌더링

use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};

/// 동공 크기에 영향을 주는 감정/상태
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PupilEmotion {
    /// 평상시
    Normal,
    /// 흥분, 관심 (동공 확대)
    Excited,
    /// 공포 (동공 최대 확대)
    Fearful,
    /// 집중 (동공 축소)
    Focused,
    /// 편안함
    Relaxed,
}

/// 눈 셰이딩 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct EyeShadeParams {
    /// 각막 곡률 (굴절 강도)
    pub cornea_curvature: f32,
    /// 동공 크기 (0~1)
    pub pupil_size: f32,
    /// 동공 깊이 (Parallax용)
    pub pupil_depth: f32,
    /// 홍채 크기
    pub iris_size: f32,
    /// 홍채 색상
    pub iris_color: [f32; 3],
    pub _pad0: f32,
    /// 림발 링 색상 (홍채 외곽 어두운 링)
    pub limbal_ring_color: [f32; 3],
    /// 림발 링 강도
    pub limbal_ring_intensity: f32,
    /// 각막 스펙큘러 강도
    pub cornea_specular: f32,
    /// 각막 굴절률 (IOR)
    pub cornea_ior: f32,
    /// 습기 효과 강도
    pub wetness: f32,
    /// Caustics 강도
    pub caustics_intensity: f32,
    /// 하이라이트 크기 (스타일라이즈드)
    pub highlight_size: f32,
    /// 하이라이트 위치 오프셋
    pub highlight_offset: [f32; 2],
    /// 눈 through hair 투명도
    pub see_through_alpha: f32,
}

impl Default for EyeShadeParams {
    fn default() -> Self {
        Self {
            cornea_curvature: 0.8,
            pupil_size: 0.3,
            pupil_depth: 0.05,
            iris_size: 0.7,
            iris_color: [0.4, 0.25, 0.1],
            _pad0: 0.0,
            limbal_ring_color: [0.1, 0.05, 0.02],
            limbal_ring_intensity: 0.8,
            cornea_specular: 0.8,
            cornea_ior: 1.376,
            wetness: 0.3,
            caustics_intensity: 0.15,
            highlight_size: 0.15,
            highlight_offset: [0.1, 0.1],
            see_through_alpha: 0.7,
        }
    }
}

impl EyeShadeParams {
    /// 파란 눈
    pub fn blue() -> Self {
        Self {
            iris_color: [0.2, 0.4, 0.7],
            limbal_ring_color: [0.05, 0.1, 0.2],
            ..Default::default()
        }
    }

    /// 녹색 눈
    pub fn green() -> Self {
        Self {
            iris_color: [0.3, 0.5, 0.3],
            limbal_ring_color: [0.05, 0.15, 0.05],
            ..Default::default()
        }
    }

    /// 갈색 눈
    pub fn brown() -> Self {
        Self::default()
    }

    /// 밝은/연한 눈 (애니메이션 스타일)
    pub fn anime_light() -> Self {
        Self {
            iris_color: [0.6, 0.5, 0.8],
            pupil_size: 0.25,
            highlight_size: 0.2,
            limbal_ring_intensity: 0.5,
            ..Default::default()
        }
    }

    /// 동공 크기 설정
    pub fn with_pupil_size(mut self, size: f32) -> Self {
        self.pupil_size = size.clamp(0.1, 0.6);
        self
    }

    /// 조명 강도에 따른 동공 크기 동적 조절
    pub fn update_pupil_for_lighting(&mut self, light_intensity: f32, adaptation_speed: f32) {
        const BASE_SIZE: f32 = 0.35;
        const MIN_SIZE: f32 = 0.15;
        const MAX_SIZE: f32 = 0.55;

        let normalized = (1.0 + light_intensity).ln() / (1.0 + 10.0_f32).ln();
        let clamped = normalized.clamp(0.0, 1.0);
        let target_size = MAX_SIZE - (MAX_SIZE - MIN_SIZE) * clamped;
        let speed = adaptation_speed.clamp(0.0, 1.0);
        self.pupil_size = self.pupil_size + (target_size - self.pupil_size) * speed;
        self.pupil_size = self.pupil_size.clamp(MIN_SIZE, MAX_SIZE);
    }

    /// 감정/상태에 따른 동공 크기 조절
    pub fn set_pupil_emotion(&mut self, emotion: PupilEmotion) {
        self.pupil_size = match emotion {
            PupilEmotion::Normal => 0.3,
            PupilEmotion::Excited => 0.45,
            PupilEmotion::Fearful => 0.5,
            PupilEmotion::Focused => 0.2,
            PupilEmotion::Relaxed => 0.35,
        };
    }
}

/// 홍채 Parallax 계산
pub fn compute_iris_parallax(
    uv: Vec2,
    view_dir: Vec3,
    iris_depth: f32,
    cornea_curvature: f32,
) -> Vec2 {
    let refract_dir = refract_ray(view_dir, Vec3::Z, 1.0 / 1.376);
    let height = iris_depth * cornea_curvature;
    let offset = refract_dir.truncate() * height / refract_dir.z.abs().max(0.001);
    uv + offset
}

fn refract_ray(incident: Vec3, normal: Vec3, eta: f32) -> Vec3 {
    let cos_i = -normal.dot(incident);
    let sin_t2 = eta * eta * (1.0 - cos_i * cos_i);

    if sin_t2 > 1.0 {
        return incident + 2.0 * cos_i * normal;
    }

    let cos_t = (1.0 - sin_t2).sqrt();
    eta * incident + (eta * cos_i - cos_t) * normal
}

/// 눈의 영역 마스크 계산
pub struct EyeMasks {
    pub iris_mask: f32,
    pub pupil_mask: f32,
    pub limbal_ring_mask: f32,
    pub sclera_mask: f32,
}

pub fn compute_eye_masks(uv: Vec2, params: &EyeShadeParams) -> EyeMasks {
    let centered_uv = uv - Vec2::splat(0.5);
    let dist = centered_uv.length();

    let iris_mask = smoothstep(params.iris_size, params.iris_size - 0.02, dist);
    let pupil_mask = smoothstep(params.pupil_size, params.pupil_size - 0.01, dist);

    let limbal_inner = params.iris_size - 0.08;
    let limbal_outer = params.iris_size - 0.05;
    let limbal_ring_mask = smoothstep(params.iris_size, limbal_outer, dist)
        * (1.0 - smoothstep(limbal_outer, limbal_inner, dist));

    let sclera_mask = 1.0 - iris_mask;

    EyeMasks {
        iris_mask,
        pupil_mask,
        limbal_ring_mask,
        sclera_mask,
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// 스타일라이즈드 하이라이트 계산
pub fn compute_stylized_highlight(
    uv: Vec2,
    highlight_size: f32,
    highlight_offset: Vec2,
) -> f32 {
    let centered_uv = uv - Vec2::splat(0.5) - highlight_offset;
    let dist = centered_uv.length();
    smoothstep(highlight_size, 0.0, dist)
}
