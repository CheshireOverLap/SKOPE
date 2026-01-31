//! SKOPE Engine - Face Shading
//! 노멀 시프트 + 색조작 기반 스타일라이즈드 얼굴 렌더링

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// 얼굴 셰이딩 파라미터
/// SKOPE 목표: Snowbreak ~ Stellar Blade 중간
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FaceShadeParams {
    // === 노멀 시프트 ===
    /// 노멀 평탄화 강도 (0.0 = 실사, 1.0 = 완전 flat)
    /// SKOPE 권장: 0.2 ~ 0.3
    pub normal_flatten: f32,

    /// 정면 방향 (보통 캐릭터 forward 또는 카메라 방향)
    pub front_direction: [f32; 3],

    // === 라이팅 조정 ===
    /// 측면 오버 익스포즈 보정 강도
    pub side_exposure_fix: f32,

    /// GI 플리커 방지용 SH 차수 제한 (1~3)
    pub gi_sh_order: u32,

    /// 라이팅 트랜지션 샤프니스 (0=soft, 1=hard)
    /// SKOPE 권장: 0.25 ~ 0.35
    pub transition_sharpness: f32,

    pub _pad0: f32,

    // === 색 조작 ===
    /// 그림자 영역 채도 부스트
    /// SKOPE 권장: 0.1 ~ 0.2
    pub shadow_saturation_boost: f32,

    /// 그림자 영역 Hue 시프트 (따뜻한 방향)
    /// SKOPE 권장: 0.01 ~ 0.03
    pub shadow_hue_shift: f32,

    /// 하이라이트 채도 감소
    pub highlight_saturation_reduce: f32,

    pub _pad1: f32,
}

impl Default for FaceShadeParams {
    fn default() -> Self {
        Self {
            normal_flatten: 0.25,
            front_direction: [0.0, 0.0, 1.0],
            side_exposure_fix: 0.6,
            gi_sh_order: 2,
            transition_sharpness: 0.3,
            _pad0: 0.0,
            shadow_saturation_boost: 0.15,
            shadow_hue_shift: 0.02,
            highlight_saturation_reduce: 0.1,
            _pad1: 0.0,
        }
    }
}

impl FaceShadeParams {
    /// Snowbreak 스타일에 가까움 (더 스타일라이즈드)
    pub fn more_stylized() -> Self {
        Self {
            normal_flatten: 0.4,
            transition_sharpness: 0.5,
            shadow_saturation_boost: 0.25,
            shadow_hue_shift: 0.03,
            ..Default::default()
        }
    }

    /// Stellar Blade 스타일에 가까움 (더 리얼리스틱)
    pub fn more_realistic() -> Self {
        Self {
            normal_flatten: 0.1,
            transition_sharpness: 0.15,
            shadow_saturation_boost: 0.05,
            shadow_hue_shift: 0.01,
            ..Default::default()
        }
    }

    /// 정면 방향 설정
    pub fn with_front_direction(mut self, dir: Vec3) -> Self {
        let normalized = dir.normalize();
        self.front_direction = [normalized.x, normalized.y, normalized.z];
        self
    }
}

/// 노멀 시프트 계산 (CPU 참조용)
pub fn compute_normal_shift(
    geometry_normal: Vec3,
    front_direction: Vec3,
    shift_mask: f32,
    flatten_strength: f32,
) -> Vec3 {
    let shift_amount = shift_mask * flatten_strength;
    let shifted = geometry_normal.lerp(front_direction, shift_amount);
    shifted.normalize()
}

/// 측면 보정 계산
pub fn compute_side_correction(
    view_dir: Vec3,
    light_dir: Vec3,
    _face_normal: Vec3,
    geometry_normal: Vec3,
    correction_strength: f32,
) -> f32 {
    let v_dot_l = view_dir.dot(light_dir);
    let side_factor = 1.0 - geometry_normal.dot(view_dir).abs();
    let attenuation = 1.0 - (side_factor * correction_strength * (1.0 - v_dot_l.max(0.0)));
    attenuation.max(0.0)
}

/// 스타일라이즈드 디퓨즈 (소프트~하드 트랜지션)
pub fn stylized_diffuse(n_dot_l: f32, sharpness: f32) -> f32 {
    let half_lambert = n_dot_l * 0.5 + 0.5;
    let transition_start = 0.5 - sharpness * 0.3;
    let transition_end = 0.5 + sharpness * 0.3;
    let t = (half_lambert - transition_start) / (transition_end - transition_start);
    let sharp_edge = t.clamp(0.0, 1.0);
    let sharp_edge = sharp_edge * sharp_edge * (3.0 - 2.0 * sharp_edge);
    half_lambert * (1.0 - sharpness) + sharp_edge * sharpness
}
