//! SKOPE Engine - Character Shadow System
//! Hair Shadow Proxy + Face Shadow Averaging

use bytemuck::{Pod, Zeroable};

/// Hair Shadow Proxy 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HairShadowProxyParams {
    /// 프록시 메쉬 오프셋 (라이트 방향으로)
    pub offset_distance: f32,
    /// 그림자 강도
    pub shadow_intensity: f32,
    /// 역광에서도 그림자 유지 정도
    pub backlight_shadow_keep: f32,
    /// 그림자 엣지 샤프니스
    pub edge_sharpness: f32,
}

impl Default for HairShadowProxyParams {
    fn default() -> Self {
        Self {
            offset_distance: 0.02,
            shadow_intensity: 0.7,
            backlight_shadow_keep: 0.3,
            edge_sharpness: 0.8,
        }
    }
}

/// 얼굴 영역 정보 (그림자 평균화용)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FaceBounds {
    /// 월드 스페이스 얼굴 중심
    pub center: [f32; 3],
    /// 얼굴 반경
    pub radius: f32,
    /// 얼굴 정면 방향
    pub forward: [f32; 3],
    /// 캐릭터 ID (temporal 비교용)
    pub character_id: u32,
}

impl Default for FaceBounds {
    fn default() -> Self {
        Self {
            center: [0.0, 1.6, 0.0],
            radius: 0.15,
            forward: [0.0, 0.0, 1.0],
            character_id: 0,
        }
    }
}

/// 얼굴 그림자 결과
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FaceShadowResult {
    /// 평균 그림자 값 (0 = 완전 빛, 1 = 완전 그림자)
    pub average_shadow: f32,
    /// 이전 프레임 값 (temporal smoothing용)
    pub prev_shadow: f32,
    /// 동적 오브젝트 셀프 섀도 제외 여부
    pub exclude_self_shadow: u32,
    pub _pad: f32,
}

impl Default for FaceShadowResult {
    fn default() -> Self {
        Self {
            average_shadow: 0.0,
            prev_shadow: 0.0,
            exclude_self_shadow: 1,
            _pad: 0.0,
        }
    }
}

/// Temporal smoothing 적용
pub fn apply_temporal_smoothing(
    current: f32,
    previous: f32,
    blend_factor: f32,
) -> f32 {
    previous + (current - previous) * blend_factor
}

/// 얼굴 그림자 평균화 계산
pub struct FaceShadowAverager {
    shadow_samples: Vec<f32>,
    sample_count: usize,
}

impl FaceShadowAverager {
    pub fn new(sample_count: usize) -> Self {
        Self {
            shadow_samples: vec![0.0; sample_count],
            sample_count,
        }
    }

    pub fn add_sample(&mut self, index: usize, shadow_value: f32) {
        if index < self.sample_count {
            self.shadow_samples[index] = shadow_value;
        }
    }

    pub fn compute_average(&self) -> f32 {
        if self.shadow_samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.shadow_samples.iter().sum();
        sum / self.shadow_samples.len() as f32
    }

    pub fn compute_median(&self) -> f32 {
        if self.shadow_samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.shadow_samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            (sorted[mid - 1] + sorted[mid]) / 2.0
        } else {
            sorted[mid]
        }
    }

    pub fn compute_weighted_average(&self, weights: &[f32]) -> f32 {
        if self.shadow_samples.is_empty() || weights.len() != self.shadow_samples.len() {
            return self.compute_average();
        }
        let weighted_sum: f32 = self
            .shadow_samples
            .iter()
            .zip(weights.iter())
            .map(|(s, w)| s * w)
            .sum();
        let weight_sum: f32 = weights.iter().sum();
        if weight_sum > 0.0 {
            weighted_sum / weight_sum
        } else {
            0.0
        }
    }
}
