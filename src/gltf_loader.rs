// glTF 로더 모듈

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
    pub children: Vec<usize>,  // 자식 노드 인덱스
}

#[derive(Debug)]
pub struct Model {
    pub meshes: Vec<Mesh>,
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

#[derive(Debug)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub material_index: Option<usize>,
}

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
    let mut materials = Vec::new();
    let mut textures = Vec::new();

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

    // 3. Meshes 로딩
    for mesh in document.meshes() {
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
                println!("No tangents in glTF, calculating...");
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

            // Vertex 조합
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

            // Indices 읽기
            let indices = reader
                .read_indices()
                .ok_or("Missing indices")?
                .into_u32()
                .collect::<Vec<u32>>();

            // Material index
            let material_index = primitive.material().index();

            meshes.push(Mesh {
                vertices,
                indices,
                material_index,
            });
        }
    }

    // 4. Nodes 파싱 (Scene hierarchy)
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
            children: node.children().map(|c| c.index()).collect(),
        };

        nodes.push(scene_node);
    }

    // 5. Root nodes 찾기 (Scene에 직접 속한 노드들)
    let root_nodes: Vec<usize> = document
        .default_scene()
        .map(|scene| scene.nodes().map(|n| n.index()).collect())
        .unwrap_or_else(|| (0..nodes.len()).collect());  // 기본: 모든 노드

    println!("Loaded {} meshes, {} materials, {} textures, {} nodes ({} roots)",
             meshes.len(), materials.len(), textures.len(), nodes.len(), root_nodes.len());

    Ok(Model { meshes, materials, textures, nodes, root_nodes })
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

