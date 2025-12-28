// glTF 로더 모듈

#![allow(dead_code)]

use std::path::Path;

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
}

/// 스켈레탈 메시 (스키닝 있음)
#[derive(Debug)]
pub struct SkinnedMesh {
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
    pub skin_index: usize,  // 이 메시가 사용하는 Skin
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
}

/// 키프레임 데이터
#[derive(Debug, Clone)]
pub struct Keyframe {
    pub time: f32,
    pub value: KeyframeValue,
}

/// 키프레임 값 (Translation/Scale은 Vec3, Rotation은 Quat)
#[derive(Debug, Clone)]
pub enum KeyframeValue {
    Vec3([f32; 3]),
    Quat([f32; 4]),  // (x, y, z, w)
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

/// 기본 정적 메시용 Vertex
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tangent: [f32; 4],     // xyz = tangent vector, w = handedness (±1)
    pub tex_coords: [f32; 2],
}

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

    // 1. 텍스처 로딩 (RGBA로 변환)
    for image in images {
        let channels = image.pixels.len() / (image.width * image.height) as usize;
        let rgba_data = if channels == 3 {
            // RGB → RGBA 변환
            let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
            for chunk in image.pixels.chunks(3) {
                rgba.push(chunk[0]); // R
                rgba.push(chunk[1]); // G
                rgba.push(chunk[2]); // B
                rgba.push(255);       // A (불투명)
            }
            rgba
        } else {
            // 이미 RGBA
            image.pixels
        };

        textures.push(TextureData {
            data: rgba_data,
            width: image.width,
            height: image.height,
        });
    }

    // 2. Materials 파싱
    for material in document.materials() {
        let pbr = material.pbr_metallic_roughness();

        let mat = Material {
            name: material.name().unwrap_or("Unnamed").to_string(),
            base_color_factor: pbr.base_color_factor(),
            base_color_texture: pbr.base_color_texture().map(|info| info.texture().index()),
            metallic_factor: pbr.metallic_factor(),
            roughness_factor: pbr.roughness_factor(),
            metallic_roughness_texture: pbr.metallic_roughness_texture().map(|info| info.texture().index()),
            normal_texture: material.normal_texture().map(|info| info.texture().index()),
            occlusion_texture: material.occlusion_texture().map(|info| info.texture().index()),
            emissive_texture: material.emissive_texture().map(|info| info.texture().index()),
            emissive_factor: material.emissive_factor(),
        };

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

        // Joints 수집
        let joints: Vec<Joint> = skin.joints()
            .zip(inverse_bind_matrices.iter())
            .map(|(joint_node, ibm)| Joint {
                name: joint_node.name().unwrap_or("unnamed").to_string(),
                node_index: joint_node.index(),
                inverse_bind_matrix: *ibm,
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

            // 스킨드 메시인지 확인 (JOINTS_0, WEIGHTS_0 존재 여부)
            let joints_opt = reader.read_joints(0);
            let weights_opt = reader.read_weights(0);

            if has_skin && joints_opt.is_some() && weights_opt.is_some() {
                // 스킨드 메시
                let joints: Vec<[u16; 4]> = joints_opt.unwrap().into_u16().collect();
                let weights: Vec<[f32; 4]> = weights_opt.unwrap().into_f32().collect();

                let skinned_vertices: Vec<SkinnedVertex> = positions
                    .iter()
                    .zip(normals.iter())
                    .zip(tangents.iter())
                    .zip(tex_coords.iter())
                    .zip(joints.iter())
                    .zip(weights.iter())
                    .map(|(((((pos, norm), tan), uv), jnt), wgt)| SkinnedVertex {
                        position: *pos,
                        normal: *norm,
                        tangent: *tan,
                        tex_coords: *uv,
                        joints: [jnt[0] as u32, jnt[1] as u32, jnt[2] as u32, jnt[3] as u32],
                        weights: *wgt,
                    })
                    .collect();

                let skin_index = mesh_to_skin[&mesh_idx];

                skinned_meshes.push(SkinnedMesh {
                    vertices: skinned_vertices,
                    indices,
                    material_index,
                    skin_index,
                });

                log::debug!("Loaded skinned mesh: {} ({} verts, {} joints)",
                    mesh.name().unwrap_or("unnamed"),
                    positions.len(),
                    skins[skin_index].joints.len());
            } else {
                // 정적 메시
                let vertices: Vec<Vertex> = positions
                    .iter()
                    .zip(normals.iter())
                    .zip(tangents.iter())
                    .zip(tex_coords.iter())
                    .map(|(((pos, norm), tan), uv)| Vertex {
                        position: *pos,
                        normal: *norm,
                        tangent: *tan,
                        tex_coords: *uv,
                    })
                    .collect();

                meshes.push(Mesh {
                    vertices,
                    indices,
                    material_index,
                });
            }
        }
    }

    // 5. Nodes 파싱 (Scene hierarchy)
    let mut nodes = Vec::new();

    for node in document.nodes() {
        let (trans, rot, scale) = node.transform().decomposed();

        let scene_node = SceneNode {
            name: node.name().unwrap_or("Unnamed").to_string(),
            transform: Transform {
                translation: trans,
                rotation: rot,
                scale,
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
                gltf::animation::Property::MorphTargetWeights => continue, // 모프 타겟은 스킵
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

            let keyframes: Vec<Keyframe> = match property {
                AnimationProperty::Translation | AnimationProperty::Scale => {
                    let outputs: Vec<[f32; 3]> = reader
                        .read_outputs()
                        .map(|out| match out {
                            gltf::animation::util::ReadOutputs::Translations(iter) => {
                                iter.collect()
                            }
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
                            value: KeyframeValue::Quat(value),
                        })
                        .collect()
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

