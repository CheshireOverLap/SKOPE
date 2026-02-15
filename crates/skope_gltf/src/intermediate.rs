//! Intermediate Representation (IR) for glTF data
//!
//! UE5.7 Interchange Node에 대응하는 중간 표현 타입.
//! Translator가 glTF를 파싱하여 이 IR로 변환하고,
//! Factory가 IR을 GPU 리소스로 변환한다.

use crate::types::*;

// ============ Color Space & Texture ============

/// 텍스처 색공간 (sRGB vs Linear)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    /// sRGB 색공간 (Base Color, Emissive 등)
    Srgb,
    /// 선형 색공간 (Normal, MetallicRoughness, Occlusion 등)
    Linear,
}

/// 알파 모드 (glTF spec 2.0)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AlphaMode {
    /// 완전 불투명 (기본값)
    Opaque,
    /// 알파 마스크 (cutoff 이하 픽셀 discard)
    Mask { cutoff: f32 },
    /// 알파 블렌딩
    Blend,
}

impl Default for AlphaMode {
    fn default() -> Self {
        AlphaMode::Opaque
    }
}

/// 텍스처 래핑 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WrapMode {
    Repeat,
    ClampToEdge,
    MirroredRepeat,
}

impl Default for WrapMode {
    fn default() -> Self {
        WrapMode::Repeat
    }
}

/// 텍스처 필터 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FilterMode {
    Nearest,
    Linear,
}

impl Default for FilterMode {
    fn default() -> Self {
        FilterMode::Linear
    }
}

/// 샘플러 설정
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    pub wrap_u: WrapMode,
    pub wrap_v: WrapMode,
    pub mag_filter: FilterMode,
    pub min_filter: FilterMode,
    pub mipmap_filter: FilterMode,
}

impl Default for SamplerDesc {
    fn default() -> Self {
        Self {
            wrap_u: WrapMode::Repeat,
            wrap_v: WrapMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
        }
    }
}

/// KHR_texture_transform 데이터
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureTransform {
    pub offset: [f32; 2],
    pub scale: [f32; 2],
    pub rotation: f32,
    pub tex_coord: u32,
}

impl Default for TextureTransform {
    fn default() -> Self {
        Self {
            offset: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation: 0.0,
            tex_coord: 0,
        }
    }
}

/// 텍스처 참조 (이미지 인덱스 + 샘플러 + 트랜스폼 + 색공간)
#[derive(Debug, Clone)]
pub struct TextureRef {
    pub image_index: usize,
    pub sampler: SamplerDesc,
    pub transform: TextureTransform,
    pub color_space: ColorSpace,
}

// ============ Image ============

/// glTF에서 디코딩된 이미지의 원본 포맷
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    R8,
    Rg8,
    Rgb8,
    Rgba8,
    R16,
    Rg16,
    Rgb16,
    Rgba16,
    Rgb32Float,
    Rgba32Float,
}

/// 중간 이미지 표현 (RGBA8로 변환된 상태)
#[derive(Debug, Clone)]
pub struct IntermediateImage {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub color_space: ColorSpace,
    pub source_format: SourceFormat,
}

// ============ Shading Model ============

/// glTF 셰이딩 모델 (UE5.7 6종 + SpecularGlossiness)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GltfShadingModel {
    /// 기본 PBR Metallic-Roughness (SKOPE ID=0)
    MetallicRoughness,
    /// KHR_materials_unlit (SKOPE ID=6)
    Unlit,
    /// KHR_materials_clearcoat (기존 clear_coat 필드 사용)
    ClearCoat,
    /// KHR_materials_sheen (SKOPE ID=7)
    Sheen,
    /// KHR_materials_transmission (SKOPE ID=8)
    Transmission,
    /// KHR_materials_pbrSpecularGlossiness
    SpecularGlossiness,
    /// KHR_materials_volume
    Volume,
}

impl Default for GltfShadingModel {
    fn default() -> Self {
        GltfShadingModel::MetallicRoughness
    }
}

impl GltfShadingModel {
    /// SKOPE 셰이딩 모델 ID로 변환
    pub fn to_skope_id(&self) -> u32 {
        match self {
            GltfShadingModel::MetallicRoughness => 0,
            GltfShadingModel::Unlit => 6,
            GltfShadingModel::ClearCoat => 0, // StandardPBR + clear_coat 파라미터
            GltfShadingModel::Sheen => 7,
            GltfShadingModel::Transmission => 8,
            GltfShadingModel::SpecularGlossiness => 0, // StandardPBR로 매핑
            GltfShadingModel::Volume => 8,              // Transmission과 동일 처리
        }
    }
}

// ============ Material Extension Data ============

/// KHR_materials_clearcoat 데이터
#[derive(Debug, Clone)]
pub struct ClearCoatData {
    pub factor: f32,
    pub roughness_factor: f32,
    pub texture: Option<TextureRef>,
    pub roughness_texture: Option<TextureRef>,
    pub normal_texture: Option<TextureRef>,
}

/// KHR_materials_sheen 데이터
#[derive(Debug, Clone)]
pub struct SheenData {
    pub color_factor: [f32; 3],
    pub roughness_factor: f32,
    pub color_texture: Option<TextureRef>,
    pub roughness_texture: Option<TextureRef>,
}

/// KHR_materials_transmission 데이터
#[derive(Debug, Clone)]
pub struct TransmissionData {
    pub factor: f32,
    pub texture: Option<TextureRef>,
}

/// KHR_materials_volume 데이터
#[derive(Debug, Clone)]
pub struct VolumeData {
    pub thickness_factor: f32,
    pub attenuation_distance: f32,
    pub attenuation_color: [f32; 3],
    pub thickness_texture: Option<TextureRef>,
}

// ============ Material ============

/// UE5.7 IntermediateMaterial 대응 — 중간 머티리얼 표현
#[derive(Debug, Clone)]
pub struct IntermediateMaterial {
    pub name: String,
    pub shading_model: GltfShadingModel,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,

    // PBR Metallic-Roughness 기본 속성
    pub base_color_factor: [f32; 4],
    pub base_color_texture: Option<TextureRef>,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub metallic_roughness_texture: Option<TextureRef>,

    // Additional Maps
    pub normal_texture: Option<TextureRef>,
    pub normal_scale: f32,
    pub occlusion_texture: Option<TextureRef>,
    pub emissive_texture: Option<TextureRef>,
    pub emissive_factor: [f32; 3],
    pub emissive_strength: f32,

    // Extension data
    pub clear_coat: Option<ClearCoatData>,
    pub sheen: Option<SheenData>,
    pub transmission: Option<TransmissionData>,
    pub volume: Option<VolumeData>,
    pub ior: Option<f32>,
}

impl Default for IntermediateMaterial {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            shading_model: GltfShadingModel::MetallicRoughness,
            alpha_mode: AlphaMode::Opaque,
            double_sided: false,
            base_color_factor: [1.0, 1.0, 1.0, 1.0],
            base_color_texture: None,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            occlusion_texture: None,
            emissive_texture: None,
            emissive_factor: [0.0, 0.0, 0.0],
            emissive_strength: 1.0,
            clear_coat: None,
            sheen: None,
            transmission: None,
            volume: None,
            ior: None,
        }
    }
}

// ============ Mesh ============

/// 중간 프리미티브 표현 (검증/수정 가능한 형태)
#[derive(Debug, Clone)]
pub struct IntermediatePrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub tangents: Option<Vec<[f32; 4]>>,
    pub tex_coords_0: Option<Vec<[f32; 2]>>,
    pub tex_coords_1: Option<Vec<[f32; 2]>>,
    pub colors_0: Option<Vec<[f32; 4]>>,
    pub joints: Option<Vec<[u16; 4]>>,
    pub weights: Option<Vec<[f32; 4]>>,
    pub indices: Option<Vec<u32>>,
    pub material_index: Option<usize>,
    pub morph_targets: Option<MorphTargetData>,
    pub default_morph_weights: Option<Vec<f32>>,
}

// ============ Light ============

/// 라이트 유형 (KHR_lights_punctual)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    Directional,
    Point,
    Spot,
}

/// 중간 라이트 표현 (KHR_lights_punctual)
#[derive(Debug, Clone)]
pub struct IntermediateLight {
    pub name: String,
    pub light_type: LightType,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: Option<f32>,
    pub inner_cone_angle: Option<f32>,
    pub outer_cone_angle: Option<f32>,
}

// ============ Camera ============

/// 카메라 투영 유형
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionType {
    Perspective,
    Orthographic,
}

/// 중간 카메라 표현
#[derive(Debug, Clone)]
pub struct IntermediateCamera {
    pub name: String,
    pub projection_type: ProjectionType,
    pub fov: Option<f32>,
    pub aspect_ratio: Option<f32>,
    pub near: f32,
    pub far: Option<f32>,
    pub ortho_xmag: Option<f32>,
    pub ortho_ymag: Option<f32>,
}

// ============ Validation Report ============

/// 메시 검증 리포트
#[derive(Debug, Clone, Default)]
pub struct MeshValidationReport {
    pub auto_generated_normals: bool,
    pub auto_generated_tangents: bool,
    pub auto_generated_indices: bool,
    pub degenerate_triangles_removed: u32,
    pub warnings: Vec<String>,
}

// ============ Scene Node (Extended) ============

/// 확장된 씬 노드 (라이트/카메라 인덱스 포함)
#[derive(Debug, Clone)]
pub struct IntermediateSceneNode {
    pub name: String,
    pub transform: Transform,
    pub mesh_index: Option<usize>,
    pub skin_index: Option<usize>,
    pub children: Vec<usize>,
    pub light_index: Option<usize>,
    pub camera_index: Option<usize>,
    /// MSFT_lod: LOD mesh indices [lod1, lod2, ...] (LOD0 = mesh_index)
    pub lod_mesh_indices: Option<Vec<usize>>,
}

// ============ Top-Level IR ============

/// 최상위 glTF 중간 표현
/// Translator가 생성하고, Factory/Loader가 소비
#[derive(Debug)]
pub struct GltfIntermediate {
    pub images: Vec<IntermediateImage>,
    pub materials: Vec<IntermediateMaterial>,
    pub meshes: Vec<Vec<IntermediatePrimitive>>,  // mesh[i] = primitives
    pub skins: Vec<Skin>,
    pub animations: Vec<Animation>,
    pub nodes: Vec<IntermediateSceneNode>,
    pub root_nodes: Vec<usize>,
    pub lights: Vec<IntermediateLight>,
    pub cameras: Vec<IntermediateCamera>,
    pub validation_reports: Vec<MeshValidationReport>,
    pub extensions_used: Vec<String>,
}

impl GltfIntermediate {
    /// 기존 Model 타입으로 변환 (하위 호환)
    pub fn to_model(&self) -> Model {
        use crate::convert::{convert_vec3, convert_tangent};

        let mut meshes = Vec::new();
        let mut skinned_meshes = Vec::new();

        // 노드별 스킨 매핑
        let mut mesh_to_skin: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for node in &self.nodes {
            if let (Some(mesh_idx), Some(skin_idx)) = (node.mesh_index, node.skin_index) {
                mesh_to_skin.insert(mesh_idx, skin_idx);
            }
        }

        for (mesh_idx, primitives) in self.meshes.iter().enumerate() {
            let has_skin = mesh_to_skin.contains_key(&mesh_idx);

            for prim in primitives {
                let positions = &prim.positions;
                let normals = prim.normals.as_ref()
                    .cloned()
                    .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);
                let tangents = prim.tangents.as_ref()
                    .cloned()
                    .unwrap_or_else(|| vec![[1.0, 0.0, 0.0, 1.0]; positions.len()]);
                let tex_coords_0 = prim.tex_coords_0.as_ref()
                    .cloned()
                    .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);
                let tex_coords_1 = prim.tex_coords_1.clone();
                let colors_0 = prim.colors_0.clone();
                let indices = prim.indices.as_ref()
                    .cloned()
                    .unwrap_or_else(|| (0..positions.len() as u32).collect());

                if has_skin {
                    if let (Some(ref joints_data), Some(ref weights_data)) = (&prim.joints, &prim.weights) {
                        let skinned_vertices: Vec<SkinnedVertex> = positions.iter()
                            .zip(normals.iter())
                            .zip(tangents.iter())
                            .zip(tex_coords_0.iter())
                            .zip(joints_data.iter())
                            .zip(weights_data.iter())
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
                        skinned_meshes.push(SkinnedMesh {
                            vertices: skinned_vertices,
                            indices,
                            material_index: prim.material_index,
                            skin_index,
                            morph_targets: prim.morph_targets.clone(),
                        });
                        continue;
                    }
                }

                // 정적 메시
                let vertices: Vec<Vertex> = positions.iter()
                    .zip(normals.iter())
                    .zip(tangents.iter())
                    .zip(tex_coords_0.iter())
                    .enumerate()
                    .map(|(i, (((pos, norm), tan), uv))| {
                        let uv1 = tex_coords_1.as_ref().map(|uvs| uvs[i]).unwrap_or([0.0, 0.0]);
                        let color = colors_0.as_ref().map(|cs| cs[i]).unwrap_or([1.0, 1.0, 1.0, 1.0]);
                        Vertex {
                            position: convert_vec3(*pos),
                            _pad1: 0.0,
                            normal: convert_vec3(*norm),
                            _pad2: 0.0,
                            tangent: convert_tangent(*tan),
                            tex_coords: *uv,
                            tex_coords_1: uv1,
                            color,
                        }
                    })
                    .collect();

                meshes.push(Mesh {
                    vertices,
                    indices,
                    material_index: prim.material_index,
                    morph_targets: prim.morph_targets.clone(),
                });
            }
        }

        // 머티리얼을 기존 Material 타입으로 변환
        let materials: Vec<Material> = self.materials.iter().map(|imat| {
            Material {
                name: imat.name.clone(),
                base_color_factor: imat.base_color_factor,
                base_color_texture: imat.base_color_texture.as_ref().map(|t| t.image_index),
                metallic_factor: imat.metallic_factor,
                roughness_factor: imat.roughness_factor,
                metallic_roughness_texture: imat.metallic_roughness_texture.as_ref().map(|t| t.image_index),
                normal_texture: imat.normal_texture.as_ref().map(|t| t.image_index),
                occlusion_texture: imat.occlusion_texture.as_ref().map(|t| t.image_index),
                emissive_texture: imat.emissive_texture.as_ref().map(|t| t.image_index),
                emissive_factor: imat.emissive_factor,
                // 새 필드들
                alpha_mode: match imat.alpha_mode {
                    AlphaMode::Opaque => 0,
                    AlphaMode::Mask { .. } => 1,
                    AlphaMode::Blend => 2,
                },
                alpha_cutoff: match imat.alpha_mode {
                    AlphaMode::Mask { cutoff } => cutoff,
                    _ => 0.5,
                },
                double_sided: imat.double_sided,
                shading_model: imat.shading_model.to_skope_id(),
                emissive_strength: imat.emissive_strength,
                clear_coat: imat.clear_coat.as_ref().map(|cc| cc.factor).unwrap_or(0.0),
                clear_coat_roughness: imat.clear_coat.as_ref().map(|cc| cc.roughness_factor).unwrap_or(0.1),
            }
        }).collect();

        // 텍스처 변환
        let textures: Vec<TextureData> = self.images.iter().map(|img| {
            TextureData {
                data: img.data.clone(),
                width: img.width,
                height: img.height,
            }
        }).collect();

        // 노드 변환
        let nodes: Vec<SceneNode> = self.nodes.iter().map(|n| {
            SceneNode {
                name: n.name.clone(),
                transform: n.transform.clone(),
                mesh_index: n.mesh_index,
                skin_index: n.skin_index,
                children: n.children.clone(),
                light_index: n.light_index,
                camera_index: n.camera_index,
            }
        }).collect();

        Model {
            meshes,
            skinned_meshes,
            skins: self.skins.clone(),
            animations: self.animations.clone(),
            materials,
            textures,
            nodes,
            root_nodes: self.root_nodes.clone(),
            lights: self.lights.iter().map(|l| {
                crate::types::Light {
                    name: l.name.clone(),
                    light_type: match l.light_type {
                        LightType::Directional => crate::types::LightKind::Directional,
                        LightType::Point => crate::types::LightKind::Point,
                        LightType::Spot => crate::types::LightKind::Spot {
                            inner_cone_angle: l.inner_cone_angle.unwrap_or(0.0),
                            outer_cone_angle: l.outer_cone_angle.unwrap_or(std::f32::consts::FRAC_PI_4),
                        },
                    },
                    color: l.color,
                    intensity: l.intensity,
                    range: l.range,
                }
            }).collect(),
            cameras: self.cameras.iter().map(|c| {
                crate::types::Camera {
                    name: c.name.clone(),
                    projection: match c.projection_type {
                        ProjectionType::Perspective => crate::types::Projection::Perspective {
                            fov: c.fov.unwrap_or(std::f32::consts::FRAC_PI_4),
                            aspect_ratio: c.aspect_ratio,
                            near: c.near,
                            far: c.far,
                        },
                        ProjectionType::Orthographic => crate::types::Projection::Orthographic {
                            xmag: c.ortho_xmag.unwrap_or(1.0),
                            ymag: c.ortho_ymag.unwrap_or(1.0),
                            near: c.near,
                            far: c.far.unwrap_or(1000.0),
                        },
                    },
                }
            }).collect(),
        }
    }

    /// ImportReport 생성 (통계 + 확장 감지)
    pub fn to_report(&self) -> crate::report::ImportReport {
        let extensions_unsupported: Vec<String> = self.extensions_used
            .iter()
            .filter(|ext| !crate::report::SUPPORTED_EXTENSIONS.contains(&ext.as_str()))
            .cloned()
            .collect();

        let mut warnings = Vec::new();

        // 프리미티브 총 수 계산
        let mesh_count: usize = self.meshes.iter().map(|m| m.len()).sum();

        // 검증 경고 수집
        for report in &self.validation_reports {
            for w in &report.warnings {
                warnings.push(w.clone());
            }
        }

        crate::report::ImportReport {
            file_path: String::new(), // 호출자가 설정
            extensions_used: self.extensions_used.clone(),
            extensions_unsupported,
            mesh_count,
            material_count: self.materials.len(),
            texture_count: self.images.len(),
            animation_count: self.animations.len(),
            skin_count: self.skins.len(),
            validation_reports: self.validation_reports.clone(),
            warnings,
        }
    }
}
