//! Debug UI Types
//!
//! Data structures for debug UI system

use glam::{Vec3, Quat};

/// Entity information for hierarchy view
#[derive(Clone, Debug)]
pub struct EntityInfo {
    pub id: u64,
    pub name: String,
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub components: Vec<String>,
}

impl EntityInfo {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            components: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DebugView {
    #[default]
    None,
    Albedo,
    Normal,
    Depth,
    Metallic,
    Roughness,
    LightingRaw,      // Raw lighting clamped 0-1
    LightingLog,      // Lighting magnitude (log scale)
    LightingScaled,   // Lighting * 0.01
    UniformValues,    // Show PBR debug uniform values as RGB
    SimpleLambert,    // Simple N dot L * albedo
    SunColor,         // Show sun_color to verify struct alignment
    ExposureTime,     // Show exposure, time, screen_size
    SpecularOnly,     // Specular contribution only (슬라이더 2,3,4 테스트용)
    SpecularLog,      // Specular log scale
    Wireframe,
    // V-Buffer 디버그
    Barycentric,      // Barycentric 좌표 (100)
    TriangleId,       // Triangle ID 시각화 (101)
    VBufferCheck,     // 삼각형 존재 확인 - 빨강 (102)
    UvCoords,         // UV 좌표 시각화 (103)
    TextureOnly,      // Albedo 텍스처만 (라이팅 없이) (104)
    UvChecker,        // UV 체커보드 패턴 (105)
    TextureFlippedV,  // V 좌표 flip 테스트 (106)
    V0UvDirect,       // v0.uv 직접 출력 (storage buffer 검증) (107)
    V0PositionXY,     // v0.position.xy 출력 (버텍스 데이터 검증) (108)
    V0NormalXYZ,      // v0.normal 출력 (offset 16 검증) (109)
    V0TangentXYZ,     // v0.tangent 출력 (offset 32 검증) (110)
    IndexValues,      // i0, i1, i2 인덱스 값 (111)
    MeshInfoValues,   // vertex_offset, index_offset, prim_idx (112)
    RawIndexValues,   // base_index, raw_index, mesh_idx (113)
    // World Space UV 디버그 (114-121)
    WorldUvDebug,       // World UV fract() 시각화 (115)
    WorldMatrixPos,     // world_matrix translation (116)
    WorldMatrixScale,   // world_matrix scale (117)
    MeshIdxDebug,       // mesh_idx 시각화 (118)
    LocalPosition,      // 로컬 스페이스 position (119)
    WorldPosDiff,       // 깊이 재구성 vs 행렬 변환 차이 (120)
    WorldPosRaw,        // 깊이 재구성 월드 좌표 raw (121)
    // Normal 디버그 (130-134)
    TangentW,           // Tangent handedness (w) 시각화 (130)
    Bitangent,          // Bitangent 벡터 시각화 (131)
    FinalNormal,        // 노말맵 적용 후 최종 노말 (132)
    NormalMapRaw,       // 노말맵 텍스처 원본 값 (133)
    NdotL,              // dot(normal, light) 라이팅 방향 체크 (134)
    // Motion Vector / TAA 디버그 (200+) - 별도 패스로 처리됨
    MotionVectors,      // Motion Vector 방향 색상화 (200)
    MotionVectorsMagnitude, // Motion Vector 크기 히트맵 (201)
}

/// 디버깅에 유용한 핵심 DebugView만 순회 대상으로 포함
pub const ALL_DEBUG_VIEWS: &[DebugView] = &[
    DebugView::None,
    DebugView::Albedo,
    DebugView::Normal,
    DebugView::Depth,
    DebugView::Metallic,
    DebugView::Roughness,
    DebugView::TangentW,
    DebugView::Bitangent,
    DebugView::FinalNormal,
    DebugView::NormalMapRaw,
    DebugView::NdotL,
    DebugView::Barycentric,
    DebugView::TriangleId,
    DebugView::UvCoords,
    DebugView::MotionVectors,
    DebugView::MotionVectorsMagnitude,
];

impl DebugView {
    /// 다음 debug view (순회)
    pub fn next(self) -> Self {
        let idx = ALL_DEBUG_VIEWS.iter().position(|&v| v == self).unwrap_or(0);
        ALL_DEBUG_VIEWS[(idx + 1) % ALL_DEBUG_VIEWS.len()]
    }

    /// 이전 debug view (순회)
    pub fn prev(self) -> Self {
        let idx = ALL_DEBUG_VIEWS.iter().position(|&v| v == self).unwrap_or(0);
        ALL_DEBUG_VIEWS[(idx + ALL_DEBUG_VIEWS.len() - 1) % ALL_DEBUG_VIEWS.len()]
    }

    /// 사람이 읽을 수 있는 이름
    pub fn name(self) -> &'static str {
        match self {
            DebugView::None => "None",
            DebugView::Albedo => "Albedo",
            DebugView::Normal => "Normal",
            DebugView::Depth => "Depth",
            DebugView::Metallic => "Metallic",
            DebugView::Roughness => "Roughness",
            DebugView::LightingRaw => "Lighting Raw",
            DebugView::LightingLog => "Lighting Log",
            DebugView::LightingScaled => "Lighting Scaled",
            DebugView::UniformValues => "Uniform Values",
            DebugView::SimpleLambert => "Simple Lambert",
            DebugView::SunColor => "Sun Color",
            DebugView::ExposureTime => "Exposure Time",
            DebugView::SpecularOnly => "Specular Only",
            DebugView::SpecularLog => "Specular Log",
            DebugView::Wireframe => "Wireframe",
            DebugView::Barycentric => "Barycentric",
            DebugView::TriangleId => "Triangle ID",
            DebugView::VBufferCheck => "VBuffer Check",
            DebugView::UvCoords => "UV Coords",
            DebugView::TextureOnly => "Texture Only",
            DebugView::UvChecker => "UV Checker",
            DebugView::TextureFlippedV => "Texture Flipped V",
            DebugView::V0UvDirect => "V0 UV Direct",
            DebugView::V0PositionXY => "V0 Position XY",
            DebugView::V0NormalXYZ => "V0 Normal XYZ",
            DebugView::V0TangentXYZ => "V0 Tangent XYZ",
            DebugView::IndexValues => "Index Values",
            DebugView::MeshInfoValues => "Mesh Info Values",
            DebugView::RawIndexValues => "Raw Index Values",
            DebugView::WorldUvDebug => "World UV Debug",
            DebugView::WorldMatrixPos => "World Matrix Pos",
            DebugView::WorldMatrixScale => "World Matrix Scale",
            DebugView::MeshIdxDebug => "Mesh Idx Debug",
            DebugView::LocalPosition => "Local Position",
            DebugView::WorldPosDiff => "World Pos Diff",
            DebugView::WorldPosRaw => "World Pos Raw",
            DebugView::TangentW => "Tangent W",
            DebugView::Bitangent => "Bitangent",
            DebugView::FinalNormal => "Final Normal",
            DebugView::NormalMapRaw => "Normal Map Raw",
            DebugView::NdotL => "NdotL",
            DebugView::MotionVectors => "Motion Vectors",
            DebugView::MotionVectorsMagnitude => "Motion Vectors Magnitude",
        }
    }

    /// Convert to u32 for shader debug_mode
    pub fn to_shader_mode(&self) -> u32 {
        match self {
            DebugView::None => 0,
            DebugView::Albedo => 1,
            DebugView::Normal => 2,
            DebugView::Roughness => 3,
            DebugView::Metallic => 4,
            DebugView::Depth => 5,
            DebugView::LightingRaw => 6,
            DebugView::LightingLog => 7,
            DebugView::LightingScaled => 8,
            DebugView::UniformValues => 10,
            DebugView::SimpleLambert => 11,
            DebugView::SunColor => 12,
            DebugView::ExposureTime => 13,
            DebugView::SpecularOnly => 14,
            DebugView::SpecularLog => 15,
            DebugView::Wireframe => 0, // Wireframe is handled separately
            // V-Buffer debug modes
            DebugView::Barycentric => 100,
            DebugView::TriangleId => 101,
            DebugView::VBufferCheck => 102,
            DebugView::UvCoords => 103,
            DebugView::TextureOnly => 104,
            DebugView::UvChecker => 105,
            DebugView::TextureFlippedV => 106,
            DebugView::V0UvDirect => 107,
            DebugView::V0PositionXY => 108,
            DebugView::V0NormalXYZ => 109,
            DebugView::V0TangentXYZ => 110,
            DebugView::IndexValues => 111,
            DebugView::MeshInfoValues => 112,
            DebugView::RawIndexValues => 113,
            // World Space UV debug modes
            DebugView::WorldUvDebug => 115,
            DebugView::WorldMatrixPos => 116,
            DebugView::WorldMatrixScale => 117,
            DebugView::MeshIdxDebug => 118,
            DebugView::LocalPosition => 119,
            DebugView::WorldPosDiff => 120,
            DebugView::WorldPosRaw => 121,
            // Normal debug modes
            DebugView::TangentW => 130,
            DebugView::Bitangent => 131,
            DebugView::FinalNormal => 132,
            DebugView::NormalMapRaw => 133,
            DebugView::NdotL => 134,
            // Motion Vector modes - handled by separate pass, not shader debug_mode
            DebugView::MotionVectors => 200,
            DebugView::MotionVectorsMagnitude => 201,
        }
    }

    /// Check if this debug view requires a separate render pass (not shader debug_mode)
    pub fn is_separate_pass(&self) -> bool {
        matches!(self, DebugView::MotionVectors | DebugView::MotionVectorsMagnitude)
    }
}

#[derive(Clone)]
pub struct ConsoleMessage {
    pub level: LogLevel,
    pub text: String,
    pub timestamp: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Console command action (returned for external handling)
#[derive(Clone, Debug)]
pub enum ConsoleAction {
    ReloadScene,
    ExecuteLua(String),
    SpawnEntity(String),
    SpawnParticle(String),  // fire, smoke, explosion, sparkle
}
