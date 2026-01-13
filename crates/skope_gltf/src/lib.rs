//! glTF/GLB 모델 로딩 크레이트
//!
//! 정적 메시, 스켈레탈 메시, 애니메이션, 머티리얼, 텍스처 로딩 지원
//!
//! ## 좌표계
//! - glTF 표준: Y-up, -Z forward (오른손 좌표계)
//! - SKOPE 엔진: Z-up, -Y forward (Blender와 동일)
//! - 로딩 시 자동 변환됨

#![allow(dead_code)]

use std::path::Path;

// ============================================
// Y-up → Z-up 좌표계 변환 함수들
// glTF (Y-up) → Blender/SKOPE (Z-up)
// 변환: (x, y, z) → (x, -z, y)
// ============================================

/// Position/Vector 변환: (x, y, z) → (x, -z, y)
#[inline]
fn convert_vec3(v: [f32; 3]) -> [f32; 3] {
    [v[0], -v[2], v[1]]
}

/// Tangent 변환: xyz는 벡터처럼, w(handedness)는 부호 반전
#[inline]
fn convert_tangent(t: [f32; 4]) -> [f32; 4] {
    [t[0], -t[2], t[1], -t[3]]
}

/// Quaternion 변환: (x, y, z, w) → (x, -z, y, w)
#[inline]
fn convert_quat(q: [f32; 4]) -> [f32; 4] {
    [q[0], -q[2], q[1], q[3]]
}

/// 4x4 행렬 변환 (inverse bind matrix 등)
fn convert_matrix(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // 좌표계 변환 행렬 S와 그 역행렬 S^-1
    // S: Y-up → Z-up 변환
    // result = S * M * S^-1
    let s = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, -1.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let s_inv = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];

    let temp = mat4_mul(s, m);
    mat4_mul(temp, s_inv)
}

/// 4x4 행렬 곱셈 헬퍼
fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                result[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    result
}

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

pub fn load_gltf<P: AsRef<Path>>(path: P) -> Result<Model, Box<dyn std::error::Error>> {
    let path = path.as_ref();
    let (document, buffers, images) = gltf::import(path)?;

    let mut meshes = Vec::new();
    let mut skinned_meshes = Vec::new();
    let mut skins = Vec::new();
    let mut animations = Vec::new();
    let mut materials = Vec::new();
    let mut textures = Vec::new();

    // 노드별 스킨 인덱스 매핑 (나중에 사용)
    let mut node_to_skin: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();

    // 1. 텍스처 로딩 (RGBA8로 변환)
    for image in images {
        log::info!("[glTF] Image {}x{}, format: {:?}, data_len: {}",
            image.width, image.height, image.format, image.pixels.len());

        let rgba_data = match image.format {
            gltf::image::Format::R8 => {
                // Grayscale → RGBA
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for &gray in &image.pixels {
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R8G8 => {
                // RG → RGBA (B=0)
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(2) {
                    rgba.push(chunk[0]);
                    rgba.push(chunk[1]);
                    rgba.push(0);
                    rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R8G8B8 => {
                // RGB → RGBA (명시적 인덱싱으로 정확한 변환)
                let pixel_count = (image.width * image.height) as usize;
                let mut rgba = vec![0u8; pixel_count * 4];
                for i in 0..pixel_count {
                    let src_idx = i * 3;
                    let dst_idx = i * 4;
                    rgba[dst_idx] = image.pixels[src_idx];
                    rgba[dst_idx + 1] = image.pixels[src_idx + 1];
                    rgba[dst_idx + 2] = image.pixels[src_idx + 2];
                    rgba[dst_idx + 3] = 255;
                }
                rgba
            }
            gltf::image::Format::R8G8B8A8 => {
                // 이미 RGBA8
                image.pixels
            }
            gltf::image::Format::R16 => {
                // 16-bit grayscale → RGBA8
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(2) {
                    let val = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let gray = (val >> 8) as u8;
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R16G16 => {
                // 16-bit RG → RGBA8
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(4) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    rgba.push((r >> 8) as u8);
                    rgba.push((g >> 8) as u8);
                    rgba.push(0);
                    rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R16G16B16 => {
                // 16-bit RGB → RGBA8
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(6) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let b = u16::from_le_bytes([chunk[4], chunk[5]]);
                    rgba.push((r >> 8) as u8);
                    rgba.push((g >> 8) as u8);
                    rgba.push((b >> 8) as u8);
                    rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R16G16B16A16 => {
                // 16-bit RGBA → RGBA8
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(8) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let b = u16::from_le_bytes([chunk[4], chunk[5]]);
                    let a = u16::from_le_bytes([chunk[6], chunk[7]]);
                    rgba.push((r >> 8) as u8);
                    rgba.push((g >> 8) as u8);
                    rgba.push((b >> 8) as u8);
                    rgba.push((a >> 8) as u8);
                }
                rgba
            }
            gltf::image::Format::R32G32B32FLOAT => {
                // 32-bit float RGB → RGBA8
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(12) {
                    let r = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let g = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                    let b = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
                    rgba.push((r.clamp(0.0, 1.0) * 255.0) as u8);
                    rgba.push((g.clamp(0.0, 1.0) * 255.0) as u8);
                    rgba.push((b.clamp(0.0, 1.0) * 255.0) as u8);
                    rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R32G32B32A32FLOAT => {
                // 32-bit float RGBA → RGBA8
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(16) {
                    let r = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    let g = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                    let b = f32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
                    let a = f32::from_le_bytes([chunk[12], chunk[13], chunk[14], chunk[15]]);
                    rgba.push((r.clamp(0.0, 1.0) * 255.0) as u8);
                    rgba.push((g.clamp(0.0, 1.0) * 255.0) as u8);
                    rgba.push((b.clamp(0.0, 1.0) * 255.0) as u8);
                    rgba.push((a.clamp(0.0, 1.0) * 255.0) as u8);
                }
                rgba
            }
        };

        log::info!("[glTF] Converted to RGBA8: {} bytes (expected: {})",
            rgba_data.len(), image.width * image.height * 4);

        textures.push(TextureData {
            data: rgba_data,
            width: image.width,
            height: image.height,
        });
    }

    // 2. Materials 파싱
    // 주의: texture().index()는 텍스처 인덱스, texture().source().index()는 이미지 인덱스
    // textures 배열은 images 배열에서 직접 생성했으므로 이미지 인덱스를 사용해야 함
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();

        // 텍스처 → 이미지 인덱스 매핑 함수
        let get_image_index = |tex_info: Option<gltf::texture::Info>| -> Option<usize> {
            tex_info.map(|info| info.texture().source().index())
        };

        let get_normal_image_index = |tex_info: Option<gltf::material::NormalTexture>| -> Option<usize> {
            tex_info.map(|info| info.texture().source().index())
        };

        let get_occlusion_image_index = |tex_info: Option<gltf::material::OcclusionTexture>| -> Option<usize> {
            tex_info.map(|info| info.texture().source().index())
        };

        let mat = Material {
            name: material.name().unwrap_or("Unnamed").to_string(),
            base_color_factor: pbr.base_color_factor(),
            base_color_texture: get_image_index(pbr.base_color_texture()),
            metallic_factor: pbr.metallic_factor(),
            roughness_factor: pbr.roughness_factor(),
            metallic_roughness_texture: get_image_index(pbr.metallic_roughness_texture()),
            normal_texture: get_normal_image_index(material.normal_texture()),
            occlusion_texture: get_occlusion_image_index(material.occlusion_texture()),
            emissive_texture: get_image_index(material.emissive_texture()),
            emissive_factor: material.emissive_factor(),
        };

        log::info!("[glTF] Material '{}': base_color_tex={:?}, normal_tex={:?}, mr_tex={:?}",
            mat.name, mat.base_color_texture, mat.normal_texture, mat.metallic_roughness_texture);

        materials.push(mat);
    }

    // Materials가 없으면 기본 머티리얼 추가
    if materials.is_empty() {
        materials.push(Material::default());
    }

    // 3. Skins (스켈레톤) 로딩
    for skin in document.skins() {
        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));

        // Inverse bind matrices 읽기
        let inverse_bind_matrices: Vec<[[f32; 4]; 4]> = reader
            .read_inverse_bind_matrices()
            .map(|iter| iter.collect())
            .unwrap_or_else(|| {
                // 없으면 항등 행렬 사용
                vec![
                    [[1.0, 0.0, 0.0, 0.0],
                     [0.0, 1.0, 0.0, 0.0],
                     [0.0, 0.0, 1.0, 0.0],
                     [0.0, 0.0, 0.0, 1.0]];
                    skin.joints().count()
                ]
            });

        // Joints 수집 (Y-up → Z-up 변환 적용)
        let joints: Vec<Joint> = skin.joints()
            .zip(inverse_bind_matrices.iter())
            .map(|(joint_node, ibm)| Joint {
                name: joint_node.name().unwrap_or("unnamed").to_string(),
                node_index: joint_node.index(),
                inverse_bind_matrix: convert_matrix(*ibm),
            })
            .collect();

        let skin_data = Skin {
            name: skin.name().unwrap_or("unnamed").to_string(),
            root_joint_index: skin.skeleton().map(|n| n.index()),
            joints,
        };

        skins.push(skin_data);
    }

    // 스킨을 사용하는 노드들 미리 파악
    for node in document.nodes() {
        if let Some(skin) = node.skin() {
            node_to_skin.insert(node.index(), skin.index());
        }
    }

    log::info!("Loaded {} skins", skins.len());

    // 4. Meshes 로딩 (정적 + 스킨드 분리)
    // 메시 인덱스 → 스킨 인덱스 매핑 (노드를 통해)
    let mut mesh_to_skin: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for node in document.nodes() {
        if let (Some(mesh), Some(skin)) = (node.mesh(), node.skin()) {
            mesh_to_skin.insert(mesh.index(), skin.index());
        }
    }

    for mesh in document.meshes() {
        let mesh_idx = mesh.index();
        let has_skin = mesh_to_skin.contains_key(&mesh_idx);

        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            // Position 읽기
            let positions = reader
                .read_positions()
                .ok_or("Missing positions")?
                .collect::<Vec<[f32; 3]>>();

            // Normal 읽기 (없으면 기본값)
            let normals = reader
                .read_normals()
                .map(|iter| iter.collect::<Vec<[f32; 3]>>())
                .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

            // Tangent 읽기 (없으면 계산)
            let tangents = if let Some(tangent_iter) = reader.read_tangents() {
                tangent_iter.collect::<Vec<[f32; 4]>>()
            } else {
                // Tangent가 없으면 계산
                log::debug!("No tangents in glTF, calculating...");
                let uvs: Vec<[f32; 2]> = reader
                    .read_tex_coords(0)
                    .map(|iter| iter.into_f32().collect())
                    .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

                let indices_for_tangent: Vec<u32> = reader
                    .read_indices()
                    .map(|iter| iter.into_u32().collect())
                    .unwrap_or_default();

                calculate_tangents(&positions, &normals, &uvs, &indices_for_tangent)
            };

            // UV 읽기
            let tex_coords = reader
                .read_tex_coords(0)
                .map(|iter| iter.into_f32().collect::<Vec<[f32; 2]>>())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            // Indices 읽기 (없으면 자동 생성)
            let indices: Vec<u32> = if let Some(indices_reader) = reader.read_indices() {
                indices_reader.into_u32().collect()
            } else {
                // 인덱스가 없는 경우 순차적으로 생성 (non-indexed mesh)
                (0..positions.len() as u32).collect()
            };

            // Material index
            let material_index = primitive.material().index();

            // Morph Targets (Shape Keys) 파싱
            let morph_targets = parse_morph_targets(&primitive, &buffers, positions.len());

            // 스킨드 메시인지 확인 (JOINTS_0, WEIGHTS_0 존재 여부)
            let joints_opt = reader.read_joints(0);
            let weights_opt = reader.read_weights(0);

            if let (true, Some(joints_reader), Some(weights_reader)) =
                (has_skin, joints_opt, weights_opt)
            {
                // 스킨드 메시 (Y-up → Z-up 좌표계 변환 적용)
                let joints: Vec<[u16; 4]> = joints_reader.into_u16().collect();
                let weights: Vec<[f32; 4]> = weights_reader.into_f32().collect();

                let skinned_vertices: Vec<SkinnedVertex> = positions
                    .iter()
                    .zip(normals.iter())
                    .zip(tangents.iter())
                    .zip(tex_coords.iter())
                    .zip(joints.iter())
                    .zip(weights.iter())
                    .map(|(((((pos, norm), tan), uv), jnt), wgt)| SkinnedVertex {
                        position: convert_vec3(*pos),
                        normal: convert_vec3(*norm),
                        tangent: convert_tangent(*tan),
                        tex_coords: *uv,
                        joints: [jnt[0] as u32, jnt[1] as u32, jnt[2] as u32, jnt[3] as u32],
                        weights: *wgt,
                    })
                    .collect();

                let skin_index = mesh_to_skin[&mesh_idx];

                let morph_count = morph_targets.as_ref().map(|m| m.targets.len()).unwrap_or(0);

                skinned_meshes.push(SkinnedMesh {
                    vertices: skinned_vertices,
                    indices,
                    material_index,
                    skin_index,
                    morph_targets,
                });

                log::debug!("Loaded skinned mesh: {} ({} verts, {} joints, {} morphs)",
                    mesh.name().unwrap_or("unnamed"),
                    positions.len(),
                    skins[skin_index].joints.len(),
                    morph_count);
            } else {
                // 정적 메시 (Y-up → Z-up 좌표계 변환 적용)
                let vertices: Vec<Vertex> = positions
                    .iter()
                    .zip(normals.iter())
                    .zip(tangents.iter())
                    .zip(tex_coords.iter())
                    .map(|(((pos, norm), tan), uv)| Vertex {
                        position: convert_vec3(*pos),
                        _pad1: 0.0,
                        normal: convert_vec3(*norm),
                        _pad2: 0.0,
                        tangent: convert_tangent(*tan),
                        tex_coords: *uv,
                        _pad3: [0.0, 0.0],
                    })
                    .collect();

                let morph_count = morph_targets.as_ref().map(|m| m.targets.len()).unwrap_or(0);

                meshes.push(Mesh {
                    vertices,
                    indices,
                    material_index,
                    morph_targets,
                });

                if morph_count > 0 {
                    log::debug!("Loaded static mesh with {} morph targets: {}",
                        morph_count, mesh.name().unwrap_or("unnamed"));
                }
            }
        }
    }

    // 5. Nodes 파싱 (Scene hierarchy)
    let mut nodes = Vec::new();

    for node in document.nodes() {
        let (trans, rot, scale) = node.transform().decomposed();

        // Y-up → Z-up 좌표계 변환 적용
        let scene_node = SceneNode {
            name: node.name().unwrap_or("Unnamed").to_string(),
            transform: Transform {
                translation: convert_vec3(trans),
                rotation: convert_quat(rot),
                scale,  // scale은 축 독립적이므로 변환 불필요
            },
            mesh_index: node.mesh().map(|m| m.index()),
            skin_index: node.skin().map(|s| s.index()),
            children: node.children().map(|c| c.index()).collect(),
        };

        nodes.push(scene_node);
    }

    // 6. Root nodes 찾기 (Scene에 직접 속한 노드들)
    let root_nodes: Vec<usize> = document
        .default_scene()
        .map(|scene| scene.nodes().map(|n| n.index()).collect())
        .unwrap_or_else(|| (0..nodes.len()).collect());  // 기본: 모든 노드

    // 7. Animations 파싱
    for anim in document.animations() {
        let mut channels = Vec::new();
        let mut max_time = 0.0f32;

        for channel in anim.channels() {
            let target = channel.target();
            let node_index = target.node().index();
            let sampler = channel.sampler();

            // 보간 방식
            let interpolation = match sampler.interpolation() {
                gltf::animation::Interpolation::Linear => Interpolation::Linear,
                gltf::animation::Interpolation::Step => Interpolation::Step,
                gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
            };

            // 타겟 속성
            let property = match target.property() {
                gltf::animation::Property::Translation => AnimationProperty::Translation,
                gltf::animation::Property::Rotation => AnimationProperty::Rotation,
                gltf::animation::Property::Scale => AnimationProperty::Scale,
                gltf::animation::Property::MorphTargetWeights => AnimationProperty::MorphTargetWeights,
            };

            // 키프레임 데이터 읽기
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));

            let times: Vec<f32> = reader
                .read_inputs()
                .map(|iter| iter.collect())
                .unwrap_or_default();

            // 최대 시간 업데이트
            if let Some(&t) = times.last() {
                if t > max_time {
                    max_time = t;
                }
            }

            // Y-up → Z-up 좌표계 변환 적용
            let keyframes: Vec<Keyframe> = match property {
                AnimationProperty::Translation => {
                    // Translation은 좌표계 변환 필요
                    let outputs: Vec<[f32; 3]> = reader
                        .read_outputs()
                        .map(|out| match out {
                            gltf::animation::util::ReadOutputs::Translations(iter) => {
                                iter.collect()
                            }
                            _ => Vec::new(),
                        })
                        .unwrap_or_default();

                    times.iter()
                        .zip(outputs.iter())
                        .map(|(&time, &value)| Keyframe {
                            time,
                            value: KeyframeValue::Vec3(convert_vec3(value)),
                        })
                        .collect()
                }
                AnimationProperty::Scale => {
                    // Scale은 축 독립적이므로 변환 불필요
                    let outputs: Vec<[f32; 3]> = reader
                        .read_outputs()
                        .map(|out| match out {
                            gltf::animation::util::ReadOutputs::Scales(iter) => {
                                iter.collect()
                            }
                            _ => Vec::new(),
                        })
                        .unwrap_or_default();

                    times.iter()
                        .zip(outputs.iter())
                        .map(|(&time, &value)| Keyframe {
                            time,
                            value: KeyframeValue::Vec3(value),
                        })
                        .collect()
                }
                AnimationProperty::Rotation => {
                    // Rotation은 쿼터니언 좌표계 변환 필요
                    let outputs: Vec<[f32; 4]> = reader
                        .read_outputs()
                        .map(|out| match out {
                            gltf::animation::util::ReadOutputs::Rotations(iter) => {
                                iter.into_f32().collect()
                            }
                            _ => Vec::new(),
                        })
                        .unwrap_or_default();

                    times.iter()
                        .zip(outputs.iter())
                        .map(|(&time, &value)| Keyframe {
                            time,
                            value: KeyframeValue::Quat(convert_quat(value)),
                        })
                        .collect()
                }
                AnimationProperty::MorphTargetWeights => {
                    // Morph Target Weights (Shape Key 애니메이션)
                    // glTF에서 weights는 flat array로 제공됨
                    // 각 키프레임마다 N개의 weight가 연속으로 저장
                    let outputs: Vec<f32> = reader
                        .read_outputs()
                        .map(|out| match out {
                            gltf::animation::util::ReadOutputs::MorphTargetWeights(weights) => {
                                // MorphTargetWeights는 into_f32()로 Iterator 변환
                                weights.into_f32().collect()
                            }
                            _ => Vec::new(),
                        })
                        .unwrap_or_default();

                    if outputs.is_empty() || times.is_empty() {
                        Vec::new()
                    } else {
                        // Weight 개수 계산 (전체 출력 / 키프레임 수)
                        let weight_count = outputs.len() / times.len();

                        times.iter()
                            .enumerate()
                            .map(|(i, &time)| {
                                let start = i * weight_count;
                                let end = start + weight_count;
                                let weights: Vec<f32> = outputs[start..end].to_vec();
                                Keyframe {
                                    time,
                                    value: KeyframeValue::Weights(weights),
                                }
                            })
                            .collect()
                    }
                }
            };

            if !keyframes.is_empty() {
                channels.push(AnimationChannel {
                    node_index,
                    property,
                    interpolation,
                    keyframes,
                });
            }
        }

        if !channels.is_empty() {
            animations.push(Animation {
                name: anim.name().unwrap_or("unnamed").to_string(),
                channels,
                duration: max_time,
            });
        }
    }

    log::info!("Loaded {} meshes, {} skinned meshes, {} skins, {} animations, {} materials, {} textures, {} nodes ({} roots)",
             meshes.len(), skinned_meshes.len(), skins.len(), animations.len(),
             materials.len(), textures.len(), nodes.len(), root_nodes.len());

    Ok(Model {
        meshes,
        skinned_meshes,
        skins,
        animations,
        materials,
        textures,
        nodes,
        root_nodes,
    })
}

/// Morph Targets (Shape Keys) 파싱
/// glTF primitive에서 모프 타겟 데이터를 추출
fn parse_morph_targets(
    primitive: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
    vertex_count: usize,
) -> Option<MorphTargetData> {
    let morph_targets: Vec<gltf::mesh::MorphTarget> = primitive.morph_targets().collect();

    if morph_targets.is_empty() {
        return None;
    }

    let mut targets = Vec::new();

    // glTF extras에서 Shape Key 이름 가져오기 시도
    // Blender는 extras.targetNames에 Shape Key 이름을 저장
    let target_names: Vec<String> = if primitive.morph_targets().next().is_some() {
        // glTF 표준에서는 이름이 없으므로 인덱스 기반 이름 생성
        // TODO: extras에서 이름 파싱 (Blender export 시)
        (0..morph_targets.len())
            .map(|i| format!("Key_{}", i))
            .collect()
    } else {
        Vec::new()
    };

    for (i, morph_target) in morph_targets.iter().enumerate() {
        // Position deltas 읽기
        let position_deltas: Vec<[f32; 3]> = if let Some(accessor) = morph_target.positions() {
            let view = accessor.view().expect("Position accessor should have a view");
            let buffer = &buffers[view.buffer().index()];
            let offset = view.offset() + accessor.offset();
            let stride = view.stride().unwrap_or(std::mem::size_of::<[f32; 3]>());

            (0..accessor.count())
                .map(|idx| {
                    let start = offset + idx * stride;
                    let bytes = &buffer[start..start + 12];
                    let x = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    let y = f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                    let z = f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                    // Y-up → Z-up 좌표계 변환
                    convert_vec3([x, y, z])
                })
                .collect()
        } else {
            // Position delta가 없으면 0으로 채움
            vec![[0.0, 0.0, 0.0]; vertex_count]
        };

        // Normal deltas 읽기 (optional)
        let normal_deltas: Option<Vec<[f32; 3]>> = morph_target.normals().map(|accessor| {
            let view = accessor.view().expect("Normal accessor should have a view");
            let buffer = &buffers[view.buffer().index()];
            let offset = view.offset() + accessor.offset();
            let stride = view.stride().unwrap_or(std::mem::size_of::<[f32; 3]>());

            (0..accessor.count())
                .map(|idx| {
                    let start = offset + idx * stride;
                    let bytes = &buffer[start..start + 12];
                    let x = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    let y = f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                    let z = f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                    convert_vec3([x, y, z])
                })
                .collect()
        });

        // Tangent deltas 읽기 (optional)
        let tangent_deltas: Option<Vec<[f32; 3]>> = morph_target.tangents().map(|accessor| {
            let view = accessor.view().expect("Tangent accessor should have a view");
            let buffer = &buffers[view.buffer().index()];
            let offset = view.offset() + accessor.offset();
            let stride = view.stride().unwrap_or(std::mem::size_of::<[f32; 3]>());

            (0..accessor.count())
                .map(|idx| {
                    let start = offset + idx * stride;
                    let bytes = &buffer[start..start + 12];
                    let x = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    let y = f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
                    let z = f32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
                    convert_vec3([x, y, z])
                })
                .collect()
        });

        let name = target_names.get(i).cloned().unwrap_or_else(|| format!("Key_{}", i));

        targets.push(MorphTarget {
            name,
            position_deltas,
            normal_deltas,
            tangent_deltas,
        });
    }

    // 기본 가중치 (glTF mesh에서 weights 속성 확인)
    // primitive의 부모 mesh에서 가져와야 하지만, primitive에서 직접 접근 불가
    // 일단 0.0으로 초기화
    let default_weights = vec![0.0; targets.len()];

    log::info!("[glTF] Parsed {} morph targets (Shape Keys)", targets.len());
    for (i, target) in targets.iter().enumerate() {
        log::debug!("  [{}] '{}': {} position deltas, normal: {}, tangent: {}",
            i, target.name, target.position_deltas.len(),
            target.normal_deltas.is_some(),
            target.tangent_deltas.is_some());
    }

    Some(MorphTargetData::new(targets, default_weights))
}

// Tangent 계산 함수 (MikkTSpace 알고리즘 간소화 버전)
fn calculate_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    let mut tangents = vec![[0.0f32; 3]; positions.len()];
    let mut bitangents = vec![[0.0f32; 3]; positions.len()];

    // 각 삼각형에 대해 tangent 계산
    for tri in indices.chunks(3) {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        let v0 = positions[i0];
        let v1 = positions[i1];
        let v2 = positions[i2];

        let uv0 = uvs[i0];
        let uv1 = uvs[i1];
        let uv2 = uvs[i2];

        let delta_pos1 = [v1[0] - v0[0], v1[1] - v0[1], v1[2] - v0[2]];
        let delta_pos2 = [v2[0] - v0[0], v2[1] - v0[1], v2[2] - v0[2]];

        let delta_uv1 = [uv1[0] - uv0[0], uv1[1] - uv0[1]];
        let delta_uv2 = [uv2[0] - uv0[0], uv2[1] - uv0[1]];

        let r = 1.0 / (delta_uv1[0] * delta_uv2[1] - delta_uv1[1] * delta_uv2[0] + 0.00001);

        let tangent = [
            r * (delta_uv2[1] * delta_pos1[0] - delta_uv1[1] * delta_pos2[0]),
            r * (delta_uv2[1] * delta_pos1[1] - delta_uv1[1] * delta_pos2[1]),
            r * (delta_uv2[1] * delta_pos1[2] - delta_uv1[1] * delta_pos2[2]),
        ];

        let bitangent = [
            r * (-delta_uv2[0] * delta_pos1[0] + delta_uv1[0] * delta_pos2[0]),
            r * (-delta_uv2[0] * delta_pos1[1] + delta_uv1[0] * delta_pos2[1]),
            r * (-delta_uv2[0] * delta_pos1[2] + delta_uv1[0] * delta_pos2[2]),
        ];

        // 삼각형의 3개 vertex에 누적
        for &idx in &[i0, i1, i2] {
            tangents[idx][0] += tangent[0];
            tangents[idx][1] += tangent[1];
            tangents[idx][2] += tangent[2];

            bitangents[idx][0] += bitangent[0];
            bitangents[idx][1] += bitangent[1];
            bitangents[idx][2] += bitangent[2];
        }
    }

    // 정규화 및 handedness 계산
    tangents
        .iter()
        .zip(normals.iter())
        .zip(bitangents.iter())
        .map(|((t, n), b)| {
            // Gram-Schmidt orthogonalize
            let n_dot_t = n[0] * t[0] + n[1] * t[1] + n[2] * t[2];
            let tangent = [
                t[0] - n[0] * n_dot_t,
                t[1] - n[1] * n_dot_t,
                t[2] - n[2] * n_dot_t,
            ];

            // Normalize
            let len = (tangent[0] * tangent[0] + tangent[1] * tangent[1] + tangent[2] * tangent[2]).sqrt();
            let tangent = if len > 0.00001 {
                [tangent[0] / len, tangent[1] / len, tangent[2] / len]
            } else {
                [1.0, 0.0, 0.0]
            };

            // Calculate handedness
            let cross = [
                n[1] * tangent[2] - n[2] * tangent[1],
                n[2] * tangent[0] - n[0] * tangent[2],
                n[0] * tangent[1] - n[1] * tangent[0],
            ];
            let handedness = if cross[0] * b[0] + cross[1] * b[1] + cross[2] * b[2] < 0.0 {
                -1.0
            } else {
                1.0
            };

            [tangent[0], tangent[1], tangent[2], handedness]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_skinned_model() {
        // RiggedSimple.glb 로딩 테스트
        let path = "assets/models/RiggedSimple.glb";
        if !std::path::Path::new(path).exists() {
            log::warn!("Test model not found: {}", path);
            return;
        }

        let model = load_gltf(path).expect("Failed to load RiggedSimple.glb");

        log::info!("=== RiggedSimple.glb Loading Test ===");
        log::debug!("Static meshes: {}", model.meshes.len());
        log::debug!("Skinned meshes: {}", model.skinned_meshes.len());
        log::debug!("Skins: {}", model.skins.len());
        log::debug!("Nodes: {}", model.nodes.len());

        // 스킨 정보 출력
        for (i, skin) in model.skins.iter().enumerate() {
            log::debug!("Skin {}: '{}' ({} joints)", i, skin.name, skin.joints.len());
            for (j, joint) in skin.joints.iter().enumerate() {
                log::debug!("Joint {}: '{}' (node {})", j, joint.name, joint.node_index);
            }
        }

        // 스킨드 메시 정보 출력
        for (i, sm) in model.skinned_meshes.iter().enumerate() {
            log::debug!("SkinnedMesh {}: {} verts, {} indices, skin {}",
                i, sm.vertices.len(), sm.indices.len(), sm.skin_index);

            // 첫 번째 vertex의 joints/weights 확인
            if let Some(v) = sm.vertices.first() {
                log::debug!("First vertex joints: {:?}", v.joints);
                log::debug!("First vertex weights: {:?}", v.weights);
            }
        }

        // 검증
        assert!(model.skinned_meshes.len() > 0, "Should have at least one skinned mesh");
        assert!(model.skins.len() > 0, "Should have at least one skin");
    }
}
