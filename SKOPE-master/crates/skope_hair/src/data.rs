// SKOPE Engine - Hybrid Hair Data Structures
// Phase 12: Card + Strand Hybrid

use bytemuck::{Pod, Zeroable};

/// Hybrid Hair 설정
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HybridHairConfig {
    // === Card 설정 ===
    /// Card 레이어 수 (3~5 권장)
    pub card_layer_count: u32,
    /// Alpha test threshold
    pub card_alpha_cutoff: f32,
    /// Card 스펙큘러 집중도 (0=분산, 1=Shiny Band)
    pub card_spec_concentration: f32,
    /// Card Shiny Band 위치 (0=루트, 1=팁)
    pub card_spec_band_position: f32,

    // === Strand 설정 ===
    /// Flyaway strand 개수
    pub flyaway_count: u32,
    /// Silhouette strand 개수
    pub silhouette_count: u32,
    /// Silhouette spawn threshold (Card alpha 기준)
    pub silhouette_threshold: f32,
    /// Strand 물리 시뮬레이션 활성화
    pub strand_simulation: u32,  // bool as u32

    // === 블렌딩 ===
    /// Card-Strand 블렌딩 영역 크기
    pub blend_width: f32,
    /// Strand가 Card를 덮는 정도 (0=투명, 1=불투명)
    pub strand_over_card_opacity: f32,

    pub _pad: [f32; 2],
}

impl Default for HybridHairConfig {
    fn default() -> Self {
        Self {
            // Card
            card_layer_count: 4,
            card_alpha_cutoff: 0.5,
            card_spec_concentration: 0.7,  // 스타일라이즈드
            card_spec_band_position: 0.3,

            // Strand
            flyaway_count: 1000,
            silhouette_count: 2000,
            silhouette_threshold: 0.3,
            strand_simulation: 1,

            // Blend
            blend_width: 0.05,
            strand_over_card_opacity: 0.8,

            _pad: [0.0; 2],
        }
    }
}

/// Hair Card 버텍스 데이터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HairCardVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],  // xyz + bitangent sign
    pub uv: [f32; 2],
    pub uv2: [f32; 2],      // Lightmap UV 또는 Flow map UV
}

#[cfg(feature = "gpu")]
impl HairCardVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<HairCardVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Tangent
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // UV
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // UV2
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// Silhouette Strand spawn 포인트
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct StrandSpawnPoint {
    pub position: [f32; 3],
    pub card_alpha: f32,
    pub tangent: [f32; 3],
    pub strand_length: f32,
    pub card_uv: [f32; 2],
    pub _pad: [f32; 2],
}

/// Strand 버텍스 (위치 + 두께)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct StrandVertex {
    pub position: [f32; 3],
    pub thickness: f32,
    pub velocity: [f32; 3],
    pub _pad: f32,
}

/// Flyaway Strand 설정
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FlyawayParams {
    /// 잔머리 길이 범위
    pub length_min: f32,
    pub length_max: f32,
    /// 곡률 (curl)
    pub curl_amount: f32,
    pub curl_frequency: f32,
    /// 흩날림 정도
    pub spread_angle: f32,
    /// 루트 위치 랜덤 오프셋
    pub root_offset: f32,
    pub _pad: [f32; 2],
}

impl Default for FlyawayParams {
    fn default() -> Self {
        Self {
            length_min: 0.02,
            length_max: 0.08,
            curl_amount: 0.3,
            curl_frequency: 2.0,
            spread_angle: 0.5,  // radians
            root_offset: 0.005,
            _pad: [0.0; 2],
        }
    }
}

/// Card 셰이딩 파라미터
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct CardShadeParams {
    pub spec_concentration: f32,  // 스펙큘러 집중도
    pub spec_band_position: f32,  // Shiny band 위치
    pub spec_intensity: f32,
    pub _pad1: f32,
    pub spec_color: [f32; 3],
    pub _pad2: f32,
}

impl Default for CardShadeParams {
    fn default() -> Self {
        Self {
            spec_concentration: 0.7,
            spec_band_position: 0.3,
            spec_intensity: 1.0,
            _pad1: 0.0,
            spec_color: [1.0, 0.95, 0.9],  // 약간 따뜻한 화이트
            _pad2: 0.0,
        }
    }
}

/// LOD 레벨
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HairLOD {
    /// Full Hybrid (0~3m)
    Full,
    /// Reduced Hybrid (3~8m)
    Reduced,
    /// Card + Silhouette Only (8~15m)
    CardSilhouette,
    /// Card Only (15m+)
    CardOnly,
}

impl HairLOD {
    pub fn from_distance(distance: f32) -> Self {
        if distance < 3.0 {
            HairLOD::Full
        } else if distance < 8.0 {
            HairLOD::Reduced
        } else if distance < 15.0 {
            HairLOD::CardSilhouette
        } else {
            HairLOD::CardOnly
        }
    }

    /// Flyaway strand 비율 (0.0 ~ 1.0)
    pub fn flyaway_ratio(&self) -> f32 {
        match self {
            HairLOD::Full => 1.0,
            HairLOD::Reduced => 0.5,
            HairLOD::CardSilhouette => 0.0,
            HairLOD::CardOnly => 0.0,
        }
    }

    /// Silhouette strand 비율 (0.0 ~ 1.0)
    pub fn silhouette_ratio(&self) -> f32 {
        match self {
            HairLOD::Full => 1.0,
            HairLOD::Reduced => 1.0,
            HairLOD::CardSilhouette => 0.5,
            HairLOD::CardOnly => 0.0,
        }
    }
}

/// Hair Card용 Camera Uniform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HairCameraUniform {
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _pad: f32,
}

impl Default for HairCameraUniform {
    fn default() -> Self {
        Self {
            view: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            view_proj: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            camera_pos: [0.0, 0.0, 5.0],
            _pad: 0.0,
        }
    }
}

/// Hair Card용 Model Transform
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HairModelTransform {
    pub model: [[f32; 4]; 4],
    pub model_inv_transpose: [[f32; 4]; 4],
}

impl Default for HairModelTransform {
    fn default() -> Self {
        Self {
            model: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
            model_inv_transpose: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]],
        }
    }
}

/// Hair Card용 Light Params
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct HairLightParams {
    pub sun_direction: [f32; 3],
    pub _pad0: f32,
    pub sun_color: [f32; 3],
    pub sun_intensity: f32,
    pub ambient_color: [f32; 3],
    pub ambient_intensity: f32,
}

impl Default for HairLightParams {
    fn default() -> Self {
        Self {
            sun_direction: [-0.5, -1.0, -0.3],
            _pad0: 0.0,
            sun_color: [1.0, 0.98, 0.95],
            sun_intensity: 1.0,
            ambient_color: [0.15, 0.15, 0.15],
            ambient_intensity: 1.0,
        }
    }
}
