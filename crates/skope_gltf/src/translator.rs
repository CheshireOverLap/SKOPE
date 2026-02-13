//! glTF Translator — UE5.7 UInterchangeGLTFTranslator 대응
//!
//! glTF 파일을 파싱하여 GltfIntermediate(IR)로 변환
//! IO와 CPU 변환을 분리하여 병렬화 가능

use std::path::Path;

use crate::convert::{convert_vec3, convert_quat, convert_matrix};
use crate::intermediate::*;
use crate::types::*;

/// glTF 변환 에러
#[derive(Debug)]
pub enum GltfTranslateError {
    Io(std::io::Error),
    Gltf(gltf::Error),
    MissingPositions(String),
    Other(String),
}

impl std::fmt::Display for GltfTranslateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GltfTranslateError::Io(e) => write!(f, "IO error: {}", e),
            GltfTranslateError::Gltf(e) => write!(f, "glTF error: {}", e),
            GltfTranslateError::MissingPositions(name) => write!(f, "Missing positions in mesh: {}", name),
            GltfTranslateError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for GltfTranslateError {}

impl From<gltf::Error> for GltfTranslateError {
    fn from(e: gltf::Error) -> Self {
        GltfTranslateError::Gltf(e)
    }
}

/// glTF Translator
/// glTF 파일을 GltfIntermediate(IR)로 변환
pub struct GltfTranslator;

impl GltfTranslator {
    /// glTF 파일을 IR로 변환 (순차 버전)
    pub fn translate<P: AsRef<Path>>(path: P) -> Result<GltfIntermediate, GltfTranslateError> {
        let path = path.as_ref();
        let (document, buffers, images) = gltf::import(path)?;

        // 사용된 확장 목록 수집
        let extensions_used: Vec<String> = document.extensions_used()
            .map(|s| s.to_string())
            .collect();

        log::info!("[Translator] Extensions used: {:?}", extensions_used);

        // 1. 이미지 변환
        let ir_images = Self::translate_images(&document, images);

        // 2. 머티리얼 변환
        let ir_materials = Self::translate_materials(&document);

        // 3. 메시 변환
        let (ir_meshes, validation_reports) = Self::translate_meshes(&document, &buffers);

        // 4. 스킨 변환
        let skins = Self::translate_skins(&document, &buffers);

        // 5. 애니메이션 변환
        let animations = Self::translate_animations(&document, &buffers);

        // 6. 씬 노드 변환
        let (nodes, root_nodes) = Self::translate_scene(&document);

        // 7. 라이트 변환
        let lights = Self::translate_lights(&document);

        // 8. 카메라 변환
        let cameras = Self::translate_cameras(&document);

        log::info!(
            "[Translator] Translated: {} images, {} materials, {} meshes, {} skins, {} animations, {} nodes, {} lights, {} cameras",
            ir_images.len(), ir_materials.len(), ir_meshes.len(), skins.len(),
            animations.len(), nodes.len(), lights.len(), cameras.len()
        );

        Ok(GltfIntermediate {
            images: ir_images,
            materials: ir_materials,
            meshes: ir_meshes,
            skins,
            animations,
            nodes,
            root_nodes,
            lights,
            cameras,
            validation_reports,
            extensions_used,
        })
    }

    /// rayon 병렬 버전
    #[cfg(feature = "parallel")]
    pub fn translate_parallel<P: AsRef<Path>>(path: P) -> Result<GltfIntermediate, GltfTranslateError> {
        use rayon::prelude::*;

        let path = path.as_ref();
        let (document, buffers, images) = gltf::import(path)?;

        let extensions_used: Vec<String> = document.extensions_used()
            .map(|s| s.to_string())
            .collect();

        // IO 완료 후 CPU 작업 병렬화
        // 이미지 변환 (병렬)
        let ir_images: Vec<IntermediateImage> = images.into_par_iter()
            .enumerate()
            .map(|(i, image)| {
                Self::translate_single_image(&document, i, image)
            })
            .collect();

        // 나머지는 순차 (document 공유 참조 때문)
        let ir_materials = Self::translate_materials(&document);
        let (ir_meshes, validation_reports) = Self::translate_meshes(&document, &buffers);
        let skins = Self::translate_skins(&document, &buffers);
        let animations = Self::translate_animations(&document, &buffers);
        let (nodes, root_nodes) = Self::translate_scene(&document);
        let lights = Self::translate_lights(&document);
        let cameras = Self::translate_cameras(&document);

        Ok(GltfIntermediate {
            images: ir_images,
            materials: ir_materials,
            meshes: ir_meshes,
            skins,
            animations,
            nodes,
            root_nodes,
            lights,
            cameras,
            validation_reports,
            extensions_used,
        })
    }

    // ============ Image Translation ============

    fn translate_images(document: &gltf::Document, images: Vec<gltf::image::Data>) -> Vec<IntermediateImage> {
        images.into_iter().enumerate().map(|(i, image)| {
            Self::translate_single_image(document, i, image)
        }).collect()
    }

    fn translate_single_image(_document: &gltf::Document, _index: usize, image: gltf::image::Data) -> IntermediateImage {
        let source_format = match image.format {
            gltf::image::Format::R8 => SourceFormat::R8,
            gltf::image::Format::R8G8 => SourceFormat::Rg8,
            gltf::image::Format::R8G8B8 => SourceFormat::Rgb8,
            gltf::image::Format::R8G8B8A8 => SourceFormat::Rgba8,
            gltf::image::Format::R16 => SourceFormat::R16,
            gltf::image::Format::R16G16 => SourceFormat::Rg16,
            gltf::image::Format::R16G16B16 => SourceFormat::Rgb16,
            gltf::image::Format::R16G16B16A16 => SourceFormat::Rgba16,
            gltf::image::Format::R32G32B32FLOAT => SourceFormat::Rgb32Float,
            gltf::image::Format::R32G32B32A32FLOAT => SourceFormat::Rgba32Float,
        };

        log::info!("[Translator] Image {}x{}, format: {:?}", image.width, image.height, image.format);

        let rgba_data = match image.format {
            gltf::image::Format::R8 => {
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
                let pixel_count = (image.width * image.height) as usize;
                let mut rgba = vec![0u8; pixel_count * 4];
                for i in 0..pixel_count {
                    let src = i * 3;
                    let dst = i * 4;
                    rgba[dst] = image.pixels[src];
                    rgba[dst + 1] = image.pixels[src + 1];
                    rgba[dst + 2] = image.pixels[src + 2];
                    rgba[dst + 3] = 255;
                }
                rgba
            }
            gltf::image::Format::R8G8B8A8 => image.pixels,
            gltf::image::Format::R16 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(2) {
                    let val = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let gray = (val >> 8) as u8;
                    rgba.push(gray); rgba.push(gray); rgba.push(gray); rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R16G16 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(4) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    rgba.push((r >> 8) as u8); rgba.push((g >> 8) as u8); rgba.push(0); rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R16G16B16 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(6) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let b = u16::from_le_bytes([chunk[4], chunk[5]]);
                    rgba.push((r >> 8) as u8); rgba.push((g >> 8) as u8); rgba.push((b >> 8) as u8); rgba.push(255);
                }
                rgba
            }
            gltf::image::Format::R16G16B16A16 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(8) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let b = u16::from_le_bytes([chunk[4], chunk[5]]);
                    let a = u16::from_le_bytes([chunk[6], chunk[7]]);
                    rgba.push((r >> 8) as u8); rgba.push((g >> 8) as u8); rgba.push((b >> 8) as u8); rgba.push((a >> 8) as u8);
                }
                rgba
            }
            gltf::image::Format::R32G32B32FLOAT => {
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

        // 색공간은 나중에 머티리얼에서 결정됨 (기본 sRGB)
        IntermediateImage {
            data: rgba_data,
            width: image.width,
            height: image.height,
            color_space: ColorSpace::Srgb, // 기본값, 머티리얼 참조 시 갱신됨
            source_format,
        }
    }

    // ============ Material Translation ============

    fn translate_materials(document: &gltf::Document) -> Vec<IntermediateMaterial> {
        let mut materials: Vec<IntermediateMaterial> = document.materials().map(|material| {
            let pbr = material.pbr_metallic_roughness();

            // 기본 텍스처 참조 헬퍼
            let make_tex_ref = |info: Option<gltf::texture::Info>, cs: ColorSpace| -> Option<TextureRef> {
                info.map(|i| {
                    let tex = i.texture();
                    TextureRef {
                        image_index: tex.source().index(),
                        sampler: Self::translate_sampler(tex.sampler()),
                        transform: Self::translate_tex_transform(&i),
                        color_space: cs,
                    }
                })
            };

            let make_normal_tex_ref = |info: Option<gltf::material::NormalTexture>| -> Option<TextureRef> {
                info.map(|i| {
                    let tex = i.texture();
                    TextureRef {
                        image_index: tex.source().index(),
                        sampler: Self::translate_sampler(tex.sampler()),
                        transform: TextureTransform::default(), // normal 텍스처의 KHR_texture_transform
                        color_space: ColorSpace::Linear,
                    }
                })
            };

            let make_occlusion_tex_ref = |info: Option<gltf::material::OcclusionTexture>| -> Option<TextureRef> {
                info.map(|i| {
                    let tex = i.texture();
                    TextureRef {
                        image_index: tex.source().index(),
                        sampler: Self::translate_sampler(tex.sampler()),
                        transform: TextureTransform::default(),
                        color_space: ColorSpace::Linear,
                    }
                })
            };

            // 셰이딩 모델 결정
            let shading_model = if material.unlit() {
                GltfShadingModel::Unlit
            } else {
                GltfShadingModel::MetallicRoughness
            };

            // 알파 모드
            let alpha_mode = match material.alpha_mode() {
                gltf::material::AlphaMode::Opaque => AlphaMode::Opaque,
                gltf::material::AlphaMode::Mask => AlphaMode::Mask {
                    cutoff: material.alpha_cutoff().unwrap_or(0.5),
                },
                gltf::material::AlphaMode::Blend => AlphaMode::Blend,
            };

            // Normal scale
            let normal_scale = material.normal_texture()
                .map(|n| n.scale())
                .unwrap_or(1.0);

            // KHR_materials_emissive_strength
            let emissive_strength = material.emissive_strength().unwrap_or(1.0);

            let mut imat = IntermediateMaterial {
                name: material.name().unwrap_or("Unnamed").to_string(),
                shading_model,
                alpha_mode,
                double_sided: material.double_sided(),
                base_color_factor: pbr.base_color_factor(),
                base_color_texture: make_tex_ref(pbr.base_color_texture(), ColorSpace::Srgb),
                metallic_factor: pbr.metallic_factor(),
                roughness_factor: pbr.roughness_factor(),
                metallic_roughness_texture: make_tex_ref(pbr.metallic_roughness_texture(), ColorSpace::Linear),
                normal_texture: make_normal_tex_ref(material.normal_texture()),
                normal_scale,
                occlusion_texture: make_occlusion_tex_ref(material.occlusion_texture()),
                emissive_texture: make_tex_ref(material.emissive_texture(), ColorSpace::Srgb),
                emissive_factor: material.emissive_factor(),
                emissive_strength,
                clear_coat: None,
                sheen: None,
                transmission: None,
                volume: None,
                ior: material.ior(),
            };

            // KHR_materials_clearcoat
            // NOTE: material.clearcoat() API does not exist in gltf 1.4.1.
            // TODO: Parse via material.extension_value("KHR_materials_clearcoat") if needed.
            // imat.clear_coat remains None for now.

            // KHR_materials_sheen
            // NOTE: material.sheen() API does not exist in gltf 1.4.1.
            // TODO: Parse via material.extension_value("KHR_materials_sheen") if needed.
            // imat.sheen remains None for now.

            // KHR_materials_transmission
            if let Some(transmission) = material.transmission() {
                imat.transmission = Some(TransmissionData {
                    factor: transmission.transmission_factor(),
                    texture: transmission.transmission_texture().map(|i| TextureRef {
                        image_index: i.texture().source().index(),
                        sampler: Self::translate_sampler(i.texture().sampler()),
                        transform: TextureTransform::default(),
                        color_space: ColorSpace::Linear,
                    }),
                });
                if imat.shading_model == GltfShadingModel::MetallicRoughness {
                    imat.shading_model = GltfShadingModel::Transmission;
                }
            }

            // KHR_materials_volume
            if let Some(volume) = material.volume() {
                imat.volume = Some(VolumeData {
                    thickness_factor: volume.thickness_factor(),
                    attenuation_distance: volume.attenuation_distance(),
                    attenuation_color: volume.attenuation_color(),
                    thickness_texture: volume.thickness_texture().map(|i| TextureRef {
                        image_index: i.texture().source().index(),
                        sampler: Self::translate_sampler(i.texture().sampler()),
                        transform: TextureTransform::default(),
                        color_space: ColorSpace::Linear,
                    }),
                });
            }

            log::info!(
                "[Translator] Material '{}': model={:?}, alpha={:?}, double_sided={}, emissive_strength={}",
                imat.name, imat.shading_model, imat.alpha_mode, imat.double_sided, imat.emissive_strength
            );

            imat
        }).collect();

        // 머티리얼이 없으면 기본 머티리얼 추가
        if materials.is_empty() {
            materials.push(IntermediateMaterial::default());
        }

        materials
    }

    fn translate_sampler(sampler: gltf::texture::Sampler) -> SamplerDesc {
        let wrap_mode = |w: gltf::texture::WrappingMode| -> WrapMode {
            match w {
                gltf::texture::WrappingMode::Repeat => WrapMode::Repeat,
                gltf::texture::WrappingMode::ClampToEdge => WrapMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => WrapMode::MirroredRepeat,
            }
        };

        let filter_mode = |f: Option<gltf::texture::MagFilter>| -> FilterMode {
            match f {
                Some(gltf::texture::MagFilter::Nearest) => FilterMode::Nearest,
                Some(gltf::texture::MagFilter::Linear) | None => FilterMode::Linear,
            }
        };

        let min_filter = |f: Option<gltf::texture::MinFilter>| -> (FilterMode, FilterMode) {
            match f {
                Some(gltf::texture::MinFilter::Nearest) => (FilterMode::Nearest, FilterMode::Nearest),
                Some(gltf::texture::MinFilter::Linear) => (FilterMode::Linear, FilterMode::Nearest),
                Some(gltf::texture::MinFilter::NearestMipmapNearest) => (FilterMode::Nearest, FilterMode::Nearest),
                Some(gltf::texture::MinFilter::LinearMipmapNearest) => (FilterMode::Linear, FilterMode::Nearest),
                Some(gltf::texture::MinFilter::NearestMipmapLinear) => (FilterMode::Nearest, FilterMode::Linear),
                Some(gltf::texture::MinFilter::LinearMipmapLinear) | None => (FilterMode::Linear, FilterMode::Linear),
            }
        };

        let (min_f, mipmap_f) = min_filter(sampler.min_filter());

        SamplerDesc {
            wrap_u: wrap_mode(sampler.wrap_s()),
            wrap_v: wrap_mode(sampler.wrap_t()),
            mag_filter: filter_mode(sampler.mag_filter()),
            min_filter: min_f,
            mipmap_filter: mipmap_f,
        }
    }

    fn translate_tex_transform(info: &gltf::texture::Info) -> TextureTransform {
        if let Some(transform) = info.texture_transform() {
            TextureTransform {
                offset: transform.offset(),
                scale: transform.scale(),
                rotation: transform.rotation(),
                tex_coord: transform.tex_coord().unwrap_or(info.tex_coord()) as u32,
            }
        } else {
            TextureTransform {
                tex_coord: info.tex_coord() as u32,
                ..Default::default()
            }
        }
    }

    // ============ Mesh Translation ============

    fn translate_meshes(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> (Vec<Vec<IntermediatePrimitive>>, Vec<MeshValidationReport>) {
        let mut all_meshes = Vec::new();
        let mut all_reports = Vec::new();

        for mesh in document.meshes() {
            let mut primitives = Vec::new();

            for primitive in mesh.primitives() {
                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                // Position (필수)
                let positions: Vec<[f32; 3]> = match reader.read_positions() {
                    Some(iter) => iter.collect(),
                    None => {
                        log::warn!("[Translator] Missing positions in mesh '{}'", mesh.name().unwrap_or("unnamed"));
                        continue;
                    }
                };

                // Normal (optional)
                let normals: Option<Vec<[f32; 3]>> = reader.read_normals()
                    .map(|iter| iter.collect());

                // Tangent (optional)
                let tangents: Option<Vec<[f32; 4]>> = reader.read_tangents()
                    .map(|iter| iter.collect());

                // UV0
                let tex_coords_0: Option<Vec<[f32; 2]>> = reader.read_tex_coords(0)
                    .map(|iter| iter.into_f32().collect());

                // UV1
                let tex_coords_1: Option<Vec<[f32; 2]>> = reader.read_tex_coords(1)
                    .map(|iter| iter.into_f32().collect());

                // Vertex colors
                let colors_0: Option<Vec<[f32; 4]>> = reader.read_colors(0)
                    .map(|iter| iter.into_rgba_f32().collect());

                // Joints + Weights
                let joints: Option<Vec<[u16; 4]>> = reader.read_joints(0)
                    .map(|iter| iter.into_u16().collect());
                let weights: Option<Vec<[f32; 4]>> = reader.read_weights(0)
                    .map(|iter| iter.into_f32().collect());

                // Indices
                let indices: Option<Vec<u32>> = reader.read_indices()
                    .map(|iter| iter.into_u32().collect());

                // Morph targets
                let morph_targets = Self::parse_morph_targets(&primitive, buffers, positions.len());

                // Default morph weights
                let default_morph_weights = if morph_targets.is_some() {
                    Some(vec![0.0; morph_targets.as_ref().unwrap().targets.len()])
                } else {
                    None
                };

                let prim = IntermediatePrimitive {
                    positions,
                    normals,
                    tangents,
                    tex_coords_0,
                    tex_coords_1,
                    colors_0,
                    joints,
                    weights,
                    indices,
                    material_index: primitive.material().index(),
                    morph_targets,
                    default_morph_weights,
                };

                primitives.push(prim);
            }

            // 기본 빈 리포트 (validator에서 채움)
            all_reports.push(MeshValidationReport::default());
            all_meshes.push(primitives);
        }

        (all_meshes, all_reports)
    }

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

        for (i, morph_target) in morph_targets.iter().enumerate() {
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
                        convert_vec3([x, y, z])
                    })
                    .collect()
            } else {
                vec![[0.0, 0.0, 0.0]; vertex_count]
            };

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

            targets.push(MorphTarget {
                name: format!("Key_{}", i),
                position_deltas,
                normal_deltas,
                tangent_deltas,
            });
        }

        let default_weights = vec![0.0; targets.len()];
        log::info!("[Translator] Parsed {} morph targets", targets.len());

        Some(MorphTargetData::new(targets, default_weights))
    }

    // ============ Skin Translation ============

    fn translate_skins(document: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Vec<Skin> {
        document.skins().map(|skin| {
            let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));

            let inverse_bind_matrices: Vec<[[f32; 4]; 4]> = reader
                .read_inverse_bind_matrices()
                .map(|iter| iter.collect())
                .unwrap_or_else(|| {
                    vec![
                        [[1.0, 0.0, 0.0, 0.0],
                         [0.0, 1.0, 0.0, 0.0],
                         [0.0, 0.0, 1.0, 0.0],
                         [0.0, 0.0, 0.0, 1.0]];
                        skin.joints().count()
                    ]
                });

            let joints: Vec<Joint> = skin.joints()
                .zip(inverse_bind_matrices.iter())
                .map(|(joint_node, ibm)| Joint {
                    name: joint_node.name().unwrap_or("unnamed").to_string(),
                    node_index: joint_node.index(),
                    inverse_bind_matrix: convert_matrix(*ibm),
                })
                .collect();

            Skin {
                name: skin.name().unwrap_or("unnamed").to_string(),
                root_joint_index: skin.skeleton().map(|n| n.index()),
                joints,
            }
        }).collect()
    }

    // ============ Animation Translation ============

    fn translate_animations(document: &gltf::Document, buffers: &[gltf::buffer::Data]) -> Vec<Animation> {
        let mut animations = Vec::new();

        for anim in document.animations() {
            let mut channels = Vec::new();
            let mut max_time = 0.0f32;

            for channel in anim.channels() {
                let target = channel.target();
                let node_index = target.node().index();
                let sampler = channel.sampler();

                let interpolation = match sampler.interpolation() {
                    gltf::animation::Interpolation::Linear => Interpolation::Linear,
                    gltf::animation::Interpolation::Step => Interpolation::Step,
                    gltf::animation::Interpolation::CubicSpline => Interpolation::CubicSpline,
                };

                let property = match target.property() {
                    gltf::animation::Property::Translation => AnimationProperty::Translation,
                    gltf::animation::Property::Rotation => AnimationProperty::Rotation,
                    gltf::animation::Property::Scale => AnimationProperty::Scale,
                    gltf::animation::Property::MorphTargetWeights => AnimationProperty::MorphTargetWeights,
                };

                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));

                let times: Vec<f32> = reader.read_inputs()
                    .map(|iter| iter.collect())
                    .unwrap_or_default();

                if let Some(&t) = times.last() {
                    if t > max_time { max_time = t; }
                }

                let keyframes: Vec<Keyframe> = match property {
                    AnimationProperty::Translation => {
                        let outputs: Vec<[f32; 3]> = reader.read_outputs()
                            .map(|out| match out {
                                gltf::animation::util::ReadOutputs::Translations(iter) => iter.collect(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();

                        if interpolation == Interpolation::CubicSpline {
                            // CubicSpline: 3배수 triplet (in_tangent, value, out_tangent)
                            Self::parse_cubic_spline_vec3(&times, &outputs)
                        } else {
                            times.iter().zip(outputs.iter())
                                .map(|(&time, &value)| Keyframe {
                                    time,
                                    value: KeyframeValue::Vec3(convert_vec3(value)),
                                })
                                .collect()
                        }
                    }
                    AnimationProperty::Scale => {
                        let outputs: Vec<[f32; 3]> = reader.read_outputs()
                            .map(|out| match out {
                                gltf::animation::util::ReadOutputs::Scales(iter) => iter.collect(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();

                        if interpolation == Interpolation::CubicSpline {
                            Self::parse_cubic_spline_vec3_no_convert(&times, &outputs)
                        } else {
                            times.iter().zip(outputs.iter())
                                .map(|(&time, &value)| Keyframe {
                                    time,
                                    value: KeyframeValue::Vec3(value),
                                })
                                .collect()
                        }
                    }
                    AnimationProperty::Rotation => {
                        let outputs: Vec<[f32; 4]> = reader.read_outputs()
                            .map(|out| match out {
                                gltf::animation::util::ReadOutputs::Rotations(iter) => iter.into_f32().collect(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();

                        if interpolation == Interpolation::CubicSpline {
                            Self::parse_cubic_spline_quat(&times, &outputs)
                        } else {
                            times.iter().zip(outputs.iter())
                                .map(|(&time, &value)| Keyframe {
                                    time,
                                    value: KeyframeValue::Quat(convert_quat(value)),
                                })
                                .collect()
                        }
                    }
                    AnimationProperty::MorphTargetWeights => {
                        let outputs: Vec<f32> = reader.read_outputs()
                            .map(|out| match out {
                                gltf::animation::util::ReadOutputs::MorphTargetWeights(weights) => {
                                    weights.into_f32().collect()
                                }
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();

                        if outputs.is_empty() || times.is_empty() {
                            Vec::new()
                        } else {
                            let weight_count = outputs.len() / times.len();
                            times.iter().enumerate()
                                .map(|(i, &time)| {
                                    let start = i * weight_count;
                                    let end = start + weight_count;
                                    Keyframe {
                                        time,
                                        value: KeyframeValue::Weights(outputs[start..end].to_vec()),
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

        animations
    }

    /// CubicSpline Vec3 triplet 파싱 (with coordinate conversion)
    fn parse_cubic_spline_vec3(times: &[f32], outputs: &[[f32; 3]]) -> Vec<Keyframe> {
        // CubicSpline: 각 keyframe에 3개 값 (in_tangent, value, out_tangent)
        // 지금은 value만 추출하여 일반 Keyframe으로 저장
        // CubicSpline 보간은 animation.rs에서 CubicSplineKeyframe으로 처리
        let keyframe_count = times.len();
        if outputs.len() != keyframe_count * 3 {
            // Fallback: 일반 파싱
            return times.iter().zip(outputs.iter())
                .map(|(&time, &value)| Keyframe {
                    time,
                    value: KeyframeValue::Vec3(convert_vec3(value)),
                })
                .collect();
        }

        times.iter().enumerate()
            .map(|(i, &time)| {
                let _in_tangent = convert_vec3(outputs[i * 3]);
                let value = convert_vec3(outputs[i * 3 + 1]);
                let _out_tangent = convert_vec3(outputs[i * 3 + 2]);
                Keyframe {
                    time,
                    value: KeyframeValue::CubicSplineVec3 {
                        in_tangent: _in_tangent,
                        value,
                        out_tangent: _out_tangent,
                    },
                }
            })
            .collect()
    }

    /// CubicSpline Vec3 triplet 파싱 (without coordinate conversion, for Scale)
    fn parse_cubic_spline_vec3_no_convert(times: &[f32], outputs: &[[f32; 3]]) -> Vec<Keyframe> {
        let keyframe_count = times.len();
        if outputs.len() != keyframe_count * 3 {
            return times.iter().zip(outputs.iter())
                .map(|(&time, &value)| Keyframe { time, value: KeyframeValue::Vec3(value) })
                .collect();
        }

        times.iter().enumerate()
            .map(|(i, &time)| {
                Keyframe {
                    time,
                    value: KeyframeValue::CubicSplineVec3 {
                        in_tangent: outputs[i * 3],
                        value: outputs[i * 3 + 1],
                        out_tangent: outputs[i * 3 + 2],
                    },
                }
            })
            .collect()
    }

    /// CubicSpline Quaternion triplet 파싱
    fn parse_cubic_spline_quat(times: &[f32], outputs: &[[f32; 4]]) -> Vec<Keyframe> {
        let keyframe_count = times.len();
        if outputs.len() != keyframe_count * 3 {
            return times.iter().zip(outputs.iter())
                .map(|(&time, &value)| Keyframe { time, value: KeyframeValue::Quat(convert_quat(value)) })
                .collect();
        }

        times.iter().enumerate()
            .map(|(i, &time)| {
                Keyframe {
                    time,
                    value: KeyframeValue::CubicSplineQuat {
                        in_tangent: convert_quat(outputs[i * 3]),
                        value: convert_quat(outputs[i * 3 + 1]),
                        out_tangent: convert_quat(outputs[i * 3 + 2]),
                    },
                }
            })
            .collect()
    }

    // ============ Scene Translation ============

    fn translate_scene(document: &gltf::Document) -> (Vec<IntermediateSceneNode>, Vec<usize>) {
        let nodes: Vec<IntermediateSceneNode> = document.nodes().map(|node| {
            let (trans, rot, scale) = node.transform().decomposed();

            // 라이트 인덱스 (KHR_lights_punctual)
            let light_index = node.light().map(|l| l.index());

            // 카메라 인덱스
            let camera_index = node.camera().map(|c| c.index());

            IntermediateSceneNode {
                name: node.name().unwrap_or("Unnamed").to_string(),
                transform: Transform {
                    translation: convert_vec3(trans),
                    rotation: convert_quat(rot),
                    scale,
                },
                mesh_index: node.mesh().map(|m| m.index()),
                skin_index: node.skin().map(|s| s.index()),
                children: node.children().map(|c| c.index()).collect(),
                light_index,
                camera_index,
            }
        }).collect();

        let root_nodes: Vec<usize> = document
            .default_scene()
            .map(|scene| scene.nodes().map(|n| n.index()).collect())
            .unwrap_or_else(|| (0..nodes.len()).collect());

        (nodes, root_nodes)
    }

    // ============ Light Translation (KHR_lights_punctual) ============

    fn translate_lights(document: &gltf::Document) -> Vec<IntermediateLight> {
        document.lights().map(|lights| {
            lights.map(|light| {
                let light_type = match light.kind() {
                    gltf::khr_lights_punctual::Kind::Directional => LightType::Directional,
                    gltf::khr_lights_punctual::Kind::Point => LightType::Point,
                    gltf::khr_lights_punctual::Kind::Spot { inner_cone_angle: _, outer_cone_angle: _ } => LightType::Spot,
                };

                let (inner_cone, outer_cone) = match light.kind() {
                    gltf::khr_lights_punctual::Kind::Spot { inner_cone_angle, outer_cone_angle } => {
                        (Some(inner_cone_angle), Some(outer_cone_angle))
                    }
                    _ => (None, None),
                };

                IntermediateLight {
                    name: light.name().unwrap_or("unnamed").to_string(),
                    light_type,
                    color: light.color(),
                    intensity: light.intensity(),
                    range: light.range(),
                    inner_cone_angle: inner_cone,
                    outer_cone_angle: outer_cone,
                }
            }).collect()
        }).unwrap_or_default()
    }

    // ============ Camera Translation ============

    fn translate_cameras(document: &gltf::Document) -> Vec<IntermediateCamera> {
        document.cameras().map(|camera| {
            match camera.projection() {
                gltf::camera::Projection::Perspective(persp) => IntermediateCamera {
                    name: camera.name().unwrap_or("unnamed").to_string(),
                    projection_type: ProjectionType::Perspective,
                    fov: Some(persp.yfov()),
                    aspect_ratio: persp.aspect_ratio(),
                    near: persp.znear(),
                    far: persp.zfar(),
                    ortho_xmag: None,
                    ortho_ymag: None,
                },
                gltf::camera::Projection::Orthographic(ortho) => IntermediateCamera {
                    name: camera.name().unwrap_or("unnamed").to_string(),
                    projection_type: ProjectionType::Orthographic,
                    fov: None,
                    aspect_ratio: None,
                    near: ortho.znear(),
                    far: Some(ortho.zfar()),
                    ortho_xmag: Some(ortho.xmag()),
                    ortho_ymag: Some(ortho.ymag()),
                },
            }
        }).collect()
    }
}
