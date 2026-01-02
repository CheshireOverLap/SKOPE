// SKOPE Engine - Outline Data Structures
// 하이브리드 아웃라인 파라미터

use bytemuck::{Pod, Zeroable};

/// Inverted Hull 아웃라인 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct OutlineParams {
    /// 기본 두께 (월드 스페이스)
    pub base_thickness: f32,
    /// 거리에 따른 두께 스케일
    pub distance_scale: f32,
    /// 최소 두께 (스크린 스페이스 픽셀)
    pub min_thickness_pixels: f32,
    /// 최대 두께 (스크린 스페이스 픽셀)
    pub max_thickness_pixels: f32,

    /// 기본 색상
    pub color: [f32; 4],

    /// 환경 블렌딩 강도 (0 = 고정색, 1 = 완전 적응)
    pub env_blend: f32,
    /// 조명 영향도
    pub light_influence: f32,

    pub _pad: [f32; 2],
}

impl Default for OutlineParams {
    fn default() -> Self {
        Self {
            base_thickness: 0.002,
            distance_scale: 0.001,
            min_thickness_pixels: 1.0,
            max_thickness_pixels: 2.5,
            color: [0.05, 0.03, 0.02, 1.0], // 거의 검은색
            env_blend: 0.3,
            light_influence: 0.2,
            _pad: [0.0; 2],
        }
    }
}

/// Edge Detection 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct EdgeDetectionParams {
    /// Depth edge 민감도
    pub depth_threshold: f32,
    /// Normal edge 민감도 (dot product)
    pub normal_threshold: f32,
    /// Object ID edge 활성화
    pub use_object_id: u32,
    /// Color edge 민감도 (0 = 비활성화)
    pub color_threshold: f32,

    /// 결과 라인 색상
    pub line_color: [f32; 4],

    /// 라인 두께 (픽셀)
    pub line_width: f32,
    /// 라인 강도
    pub line_intensity: f32,

    pub _pad: [f32; 2],
}

impl Default for EdgeDetectionParams {
    fn default() -> Self {
        Self {
            depth_threshold: 0.01,
            normal_threshold: 0.5, // cos(60°)
            use_object_id: 1,
            color_threshold: 0.0, // 비활성화
            line_color: [0.0, 0.0, 0.0, 0.8],
            line_width: 1.0,
            line_intensity: 0.7,
            _pad: [0.0; 2],
        }
    }
}

/// 하이브리드 아웃라인 통합 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HybridOutlineParams {
    // === Inverted Hull (실루엣) ===
    pub hull_thickness: f32,
    pub hull_distance_scale: f32,
    pub hull_min_pixels: f32,
    pub hull_max_pixels: f32,

    pub hull_color: [f32; 4],

    // === Edge Detection (내부) ===
    pub edge_depth_threshold: f32,
    pub edge_normal_threshold: f32,
    pub edge_use_object_id: u32,
    pub _pad0: f32,

    pub edge_color: [f32; 4],

    // === 공통 ===
    /// 실루엣 vs 내부 라인 밸런스 (0 = 실루엣만, 1 = 둘 다)
    pub internal_line_strength: f32,
    /// 환경 적응 강도
    pub env_adaptation: f32,
    /// 거리 페이드 시작
    pub fade_start_distance: f32,
    /// 거리 페이드 끝
    pub fade_end_distance: f32,

    // === 부위별 설정 ===
    /// 얼굴 라인 강도 (보통 약하게)
    pub face_line_strength: f32,
    /// 머리카락 라인 강도
    pub hair_line_strength: f32,
    /// 몸/옷 라인 강도
    pub body_line_strength: f32,

    pub _pad1: f32,
}

impl Default for HybridOutlineParams {
    fn default() -> Self {
        Self {
            // Hull
            hull_thickness: 0.0015,
            hull_distance_scale: 0.0008,
            hull_min_pixels: 1.0,
            hull_max_pixels: 2.5,
            hull_color: [0.02, 0.01, 0.01, 1.0],

            // Edge
            edge_depth_threshold: 0.008,
            edge_normal_threshold: 0.45,
            edge_use_object_id: 1,
            _pad0: 0.0,
            edge_color: [0.05, 0.03, 0.02, 0.7],

            // Common
            internal_line_strength: 0.5,
            env_adaptation: 0.25,
            fade_start_distance: 15.0,
            fade_end_distance: 30.0,

            // Per-part
            face_line_strength: 0.3, // 얼굴은 약하게
            hair_line_strength: 0.8,
            body_line_strength: 1.0,

            _pad1: 0.0,
        }
    }
}

impl HybridOutlineParams {
    /// 귀여운 캐릭터 스타일 (더 강한 라인)
    pub fn cute_style() -> Self {
        Self {
            hull_thickness: 0.002,
            face_line_strength: 0.5,
            internal_line_strength: 0.7,
            ..Default::default()
        }
    }

    /// 시리어스 캐릭터 스타일 (은은한 라인)
    pub fn serious_style() -> Self {
        Self {
            hull_thickness: 0.001,
            face_line_strength: 0.2,
            internal_line_strength: 0.3,
            ..Default::default()
        }
    }

    /// 보스/몬스터 스타일 (강한 라인)
    pub fn boss_style() -> Self {
        Self {
            hull_thickness: 0.003,
            hull_color: [0.1, 0.0, 0.0, 1.0], // 붉은 기운
            face_line_strength: 1.0,
            internal_line_strength: 0.9,
            ..Default::default()
        }
    }

    /// Stellar Blade 스타일 (거의 없음)
    pub fn minimal_style() -> Self {
        Self {
            hull_thickness: 0.0008,
            hull_color: [0.02, 0.02, 0.02, 0.5],
            face_line_strength: 0.1,
            internal_line_strength: 0.2,
            ..Default::default()
        }
    }
}

/// 버텍스별 아웃라인 데이터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct OutlineVertexData {
    /// 스무딩된 노멀 (실루엣용)
    pub smooth_normal: [f32; 3],
    /// 두께 배율 (0 = 아웃라인 없음, 1 = 기본, 2 = 두 배)
    pub thickness_scale: f32,
}

impl Default for OutlineVertexData {
    fn default() -> Self {
        Self {
            smooth_normal: [0.0, 1.0, 0.0],
            thickness_scale: 1.0,
        }
    }
}

/// Outline Composite 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct OutlineCompositeParams {
    pub hull_color: [f32; 4],
    pub edge_color: [f32; 4],
    pub internal_line_strength: f32,
    pub env_adaptation: f32,
    pub _pad: [f32; 2],
}

impl Default for OutlineCompositeParams {
    fn default() -> Self {
        Self {
            hull_color: [0.02, 0.01, 0.01, 1.0],
            edge_color: [0.05, 0.03, 0.02, 0.7],
            internal_line_strength: 0.5,
            env_adaptation: 0.25,
            _pad: [0.0; 2],
        }
    }
}
