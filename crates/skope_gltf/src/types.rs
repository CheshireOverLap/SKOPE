//! glTF Data Types
//!
//! Core data structures for glTF model representation

// Transform 구조체 (위치, 회전, 스케일)
#[derive(Debug, Clone)]
pub struct Transform {
    pub translation: [f32; 3],
    pub rotation: [f32; 4],     // quaternion (x, y, z, w)
    pub scale: [f32; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0],  // identity quaternion
            scale: [1.0, 1.0, 1.0],
        }
    }
}

// Scene Node (Transform + Mesh)
#[derive(Debug, Clone)]
pub struct SceneNode {
    #[allow(dead_code)]
    pub name: String,
    pub transform: Transform,
    pub mesh_index: Option<usize>,
    pub skin_index: Option<usize>,  // 스킨이 있으면 skinned_meshes 인덱스
    pub children: Vec<usize>,  // 자식 노드 인덱스
}

#[derive(Debug)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub skinned_meshes: Vec<SkinnedMesh>,  // 스켈레탈 메시들
    pub skins: Vec<Skin>,                   // 스킨(스켈레톤) 데이터
    pub animations: Vec<Animation>,         // 애니메이션 클립들
    pub materials: Vec<Material>,
    pub textures: Vec<TextureData>,
    pub nodes: Vec<SceneNode>,
    pub root_nodes: Vec<usize>,  // Scene의 루트 노드들
}

#[derive(Debug, Clone)]
pub struct Material {
    #[allow(dead_code)]
    pub name: String,
    // PBR Metallic-Roughness
    pub base_color_factor: [f32; 4],  // RGBA (기본값: [1,1,1,1])
    pub base_color_texture: Option<usize>,
    pub metallic_factor: f32,         // 기본값: 1.0
    pub roughness_factor: f32,        // 기본값: 1.0
    pub metallic_roughness_texture: Option<usize>,

    // Additional maps
    pub normal_texture: Option<usize>,
    pub occlusion_texture: Option<usize>,
    pub emissive_texture: Option<usize>,
    pub emissive_factor: [f32; 3],    // RGB (기본값: [0,0,0])
}

impl Default for Material {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
            emissive_factor: [0.0, 0.0, 0.0],
        }
    }
}

#[derive(Debug)]
pub struct TextureData {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// 정적 메시 (스키닝 없음)
#[derive(Debug)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
    /// 모프 타겟 데이터 (Shape Keys)
    pub morph_targets: Option<MorphTargetData>,
}

/// 스켈레탈 메시 (스키닝 있음)
#[derive(Debug)]
pub struct SkinnedMesh {
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
    pub skin_index: usize,  // 이 메시가 사용하는 Skin
    /// 모프 타겟 데이터 (Shape Keys) - 스킨드 메시도 모프 가능
    pub morph_targets: Option<MorphTargetData>,
}

/// 본/조인트 정보
#[derive(Debug, Clone)]
pub struct Joint {
    pub name: String,
    pub node_index: usize,              // glTF node index
    pub inverse_bind_matrix: [[f32; 4]; 4],  // 역 바인드 행렬
}

/// 스킨 (스켈레톤) 정보
#[derive(Debug, Clone)]
pub struct Skin {
    pub name: String,
    pub joints: Vec<Joint>,
    pub root_joint_index: Option<usize>,  // 스켈레톤 루트
}

// ============ Animation Data Structures ============

/// 애니메이션 보간 방식
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}

/// 애니메이션 타겟 속성
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationProperty {
    Translation,
    Rotation,
    Scale,
    MorphTargetWeights,  // Shape Keys
}

/// 키프레임 데이터
#[derive(Debug, Clone)]
pub struct Keyframe {
    pub time: f32,
    pub value: KeyframeValue,
}

/// 키프레임 값 (Translation/Scale은 Vec3, Rotation은 Quat, MorphWeights는 Vec<f32>)
#[derive(Debug, Clone)]
pub enum KeyframeValue {
    Vec3([f32; 3]),
    Quat([f32; 4]),  // (x, y, z, w)
    Weights(Vec<f32>),  // Shape Key/Morph Target weights
}

/// 애니메이션 채널 (하나의 노드, 하나의 속성)
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    pub node_index: usize,
    pub property: AnimationProperty,
    pub interpolation: Interpolation,
    pub keyframes: Vec<Keyframe>,
}

/// 애니메이션 클립
#[derive(Debug, Clone)]
pub struct Animation {
    pub name: String,
    pub channels: Vec<AnimationChannel>,
    pub duration: f32,  // 전체 길이 (초)
}

// ============ Morph Target (Shape Key) Data Structures ============

/// 모프 타겟 (Shape Key) - Blender의 Shape Key에 해당
/// 각 타겟은 base mesh 대비 position/normal/tangent의 델타 값 보유
#[derive(Debug, Clone)]
pub struct MorphTarget {
    /// Shape Key 이름 (glTF에서 제공될 경우)
    pub name: String,
    /// Position 델타 (base 위치에서의 오프셋)
    pub position_deltas: Vec<[f32; 3]>,
    /// Normal 델타 (optional)
    pub normal_deltas: Option<Vec<[f32; 3]>>,
    /// Tangent 델타 (optional)
    pub tangent_deltas: Option<Vec<[f32; 3]>>,
}

/// 모프 타겟이 있는 메시의 추가 데이터
#[derive(Debug, Clone)]
pub struct MorphTargetData {
    /// 모프 타겟들
    pub targets: Vec<MorphTarget>,
    /// 기본 가중치 (glTF에서 지정될 경우)
    pub default_weights: Vec<f32>,
    /// 현재 가중치 (런타임에서 변경)
    pub current_weights: Vec<f32>,
}

impl MorphTargetData {
    pub fn new(targets: Vec<MorphTarget>, default_weights: Vec<f32>) -> Self {
        let current_weights = default_weights.clone();
        Self {
            targets,
            default_weights,
            current_weights,
        }
    }

    /// 가중치 설정
    pub fn set_weight(&mut self, index: usize, weight: f32) {
        if index < self.current_weights.len() {
            self.current_weights[index] = weight.clamp(0.0, 1.0);
        }
    }

    /// 모든 가중치 설정
    pub fn set_weights(&mut self, weights: &[f32]) {
        for (i, &w) in weights.iter().enumerate() {
            if i < self.current_weights.len() {
                self.current_weights[i] = w.clamp(0.0, 1.0);
            }
        }
    }

    /// 기본값으로 리셋
    pub fn reset_weights(&mut self) {
        self.current_weights = self.default_weights.clone();
    }
}

/// 기본 정적 메시용 Vertex
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub _pad1: f32,            // WGSL vec3 정렬용 패딩
    pub normal: [f32; 3],
    pub _pad2: f32,            // WGSL vec3 정렬용 패딩
    pub tangent: [f32; 4],     // xyz = tangent vector, w = handedness (±1)
    pub tex_coords: [f32; 2],
    pub _pad3: [f32; 2],       // WGSL vec2 뒤 정렬용 패딩
}  // Total: 64 bytes (WGSL Vertex와 일치)

unsafe impl bytemuck::Pod for Vertex {}
unsafe impl bytemuck::Zeroable for Vertex {}

/// 스켈레탈 메시용 Vertex (joints + weights 포함)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SkinnedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],
    pub tex_coords: [f32; 2],
    pub joints: [u32; 4],      // 본 인덱스 (최대 4개)
    pub weights: [f32; 4],     // 본 가중치 (합 = 1.0)
}

unsafe impl bytemuck::Pod for SkinnedVertex {}
unsafe impl bytemuck::Zeroable for SkinnedVertex {}

impl SkinnedVertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkinnedVertex>() as wgpu::BufferAddress,
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
                // Joints (4 bone indices)
                wgpu::VertexAttribute {
                    offset: 48,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                // Weights (4 bone weights)
                wgpu::VertexAttribute {
                    offset: 64,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

impl Vertex {
    pub fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
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
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Tangent
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // UV 좌표
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2 + std::mem::size_of::<[f32; 4]>()) as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}
