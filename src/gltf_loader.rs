// glTF 로더 모듈

use std::path::Path;

#[derive(Debug)]
pub struct Model {
    pub meshes: Vec<Mesh>,
    pub textures: Vec<TextureData>,
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
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
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
                // UV 좌표
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() * 2) as wgpu::BufferAddress,
                    shader_location: 2,
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
    let mut textures = Vec::new();

    // 텍스처 로딩 (RGBA로 변환)
    for image in images {
        // gltf 이미지를 RGBA로 변환
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

        let tex_data = TextureData {
            data: rgba_data,
            width: image.width,
            height: image.height,
        };
        textures.push(tex_data);
    }

    // 텍스처가 없으면 체커보드 생성
    if textures.is_empty() {
        println!("No textures found in glTF, using checkerboard");
        let checker_size = 512;
        let checker_data = create_checkerboard_texture(checker_size);
        textures.push(TextureData {
            data: checker_data,
            width: checker_size,
            height: checker_size,
        });
    }

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
                .unwrap_or_else(|| vec![[0.0, 0.0, 1.0]; positions.len()]);

            // UV 읽기 (없으면 기본값)
            let tex_coords = reader
                .read_tex_coords(0)
                .map(|iter| iter.into_f32().collect::<Vec<[f32; 2]>>())
                .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

            // Vertex 조합
            let vertices: Vec<Vertex> = positions
                .iter()
                .zip(normals.iter())
                .zip(tex_coords.iter())
                .map(|((pos, norm), uv)| Vertex {
                    position: *pos,
                    normal: *norm,
                    tex_coords: *uv,
                })
                .collect();

            // Indices 읽기
            let indices = reader
                .read_indices()
                .ok_or("Missing indices")?
                .into_u32()
                .collect::<Vec<u32>>();

            meshes.push(Mesh { vertices, indices });
        }
    }

    Ok(Model { meshes, textures })
}

fn create_checkerboard_texture(size: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((size * size * 4) as usize);

    for y in 0..size {
        for x in 0..size {
            let checker_size = size / 8;
            let is_white = ((x / checker_size) + (y / checker_size)) % 2 == 0;

            let color = if is_white {
                [255u8, 255, 255, 255]
            } else {
                [0u8, 0, 0, 255]
            };

            data.extend_from_slice(&color);
        }
    }

    data
}
