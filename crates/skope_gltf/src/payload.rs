//! Payload Provider — Lazy loading support for glTF data
//!
//! Provides deferred data loading for large scenes.
//! The existing GltfTranslator remains as eager-loading option.
//! LazyGltfTranslator parses metadata first, data on demand.

use std::path::Path;

use crate::intermediate::*;
use crate::validator::MeshValidator;

/// Payload provider trait — abstracts data access for mesh/image payloads
#[allow(dead_code)]
pub trait PayloadProvider: Send + Sync {
    /// Get a specific primitive's data
    fn get_mesh_payload(
        &self,
        mesh_index: usize,
        primitive_index: usize,
    ) -> Result<IntermediatePrimitive, Box<dyn std::error::Error>>;

    /// Get a specific image's data
    fn get_image_payload(
        &self,
        image_index: usize,
    ) -> Result<IntermediateImage, Box<dyn std::error::Error>>;

    /// Total mesh count
    fn mesh_count(&self) -> usize;

    /// Primitives count for a given mesh
    fn primitive_count(&self, mesh_index: usize) -> usize;

    /// Total image count
    fn image_count(&self) -> usize;
}

/// Lazy glTF Translator — defers primitive/image data extraction until requested
pub struct LazyGltfTranslator {
    document: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    images: Vec<gltf::image::Data>,
}

impl LazyGltfTranslator {
    /// Open a glTF file, parse document + load buffers/images into memory
    /// but don't process primitives/images into intermediate form yet
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let (document, buffers, images) = gltf::import(path.as_ref())?;
        Ok(Self {
            document,
            buffers,
            images,
        })
    }

    /// Get the document for metadata access
    pub fn document(&self) -> &gltf::Document {
        &self.document
    }

    /// Get material count
    pub fn material_count(&self) -> usize {
        self.document.materials().len()
    }

    /// Get extensions used
    pub fn extensions_used(&self) -> Vec<String> {
        self.document
            .extensions_used()
            .map(|s| s.to_string())
            .collect()
    }
}

impl PayloadProvider for LazyGltfTranslator {
    fn get_mesh_payload(
        &self,
        mesh_index: usize,
        primitive_index: usize,
    ) -> Result<IntermediatePrimitive, Box<dyn std::error::Error>> {
        let mesh = self.document.meshes().nth(mesh_index)
            .ok_or_else(|| format!("Mesh index {} out of range", mesh_index))?;

        let primitive = mesh.primitives().nth(primitive_index)
            .ok_or_else(|| format!("Primitive index {} out of range for mesh {}", primitive_index, mesh_index))?;

        let reader = primitive.reader(|buffer| Some(&self.buffers[buffer.index()]));

        let positions: Vec<[f32; 3]> = reader
            .read_positions()
            .ok_or_else(|| format!("Missing positions in mesh {}", mesh_index))?
            .collect();

        let normals: Option<Vec<[f32; 3]>> = reader
            .read_normals()
            .map(|iter| iter.collect());

        let tangents: Option<Vec<[f32; 4]>> = reader
            .read_tangents()
            .map(|iter| iter.collect());

        let tex_coords_0: Option<Vec<[f32; 2]>> = reader
            .read_tex_coords(0)
            .map(|iter| iter.into_f32().collect());

        let tex_coords_1: Option<Vec<[f32; 2]>> = reader
            .read_tex_coords(1)
            .map(|iter| iter.into_f32().collect());

        let colors_0: Option<Vec<[f32; 4]>> = reader
            .read_colors(0)
            .map(|iter| iter.into_rgba_f32().collect());

        let joints: Option<Vec<[u16; 4]>> = reader
            .read_joints(0)
            .map(|iter| iter.into_u16().collect());

        let weights: Option<Vec<[f32; 4]>> = reader
            .read_weights(0)
            .map(|iter| iter.into_f32().collect());

        let indices: Option<Vec<u32>> = reader
            .read_indices()
            .map(|iter| iter.into_u32().collect());

        let material_index = primitive.material().index();

        let mut prim = IntermediatePrimitive {
            positions,
            normals,
            tangents,
            tex_coords_0,
            tex_coords_1,
            colors_0,
            joints,
            weights,
            indices,
            material_index,
            morph_targets: None,
            default_morph_weights: None,
        };

        // Apply validation (auto-gen normals/tangents/indices)
        let _report = MeshValidator::validate_and_fix(&mut prim);

        Ok(prim)
    }

    fn get_image_payload(
        &self,
        image_index: usize,
    ) -> Result<IntermediateImage, Box<dyn std::error::Error>> {
        let image = self.images.get(image_index)
            .ok_or_else(|| format!("Image index {} out of range", image_index))?;

        // Convert to RGBA8 (same logic as translator.rs)
        let (data, width, height, source_format) = match image.format {
            gltf::image::Format::R8 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for &gray in &image.pixels {
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(gray);
                    rgba.push(255);
                }
                (rgba, image.width, image.height, SourceFormat::R8)
            }
            gltf::image::Format::R8G8 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(2) {
                    rgba.push(chunk[0]);
                    rgba.push(chunk[1]);
                    rgba.push(0);
                    rgba.push(255);
                }
                (rgba, image.width, image.height, SourceFormat::Rg8)
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
                (rgba, image.width, image.height, SourceFormat::Rgb8)
            }
            gltf::image::Format::R8G8B8A8 => {
                (image.pixels.clone(), image.width, image.height, SourceFormat::Rgba8)
            }
            gltf::image::Format::R16 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(2) {
                    let val = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let gray = (val >> 8) as u8;
                    rgba.push(gray); rgba.push(gray); rgba.push(gray); rgba.push(255);
                }
                (rgba, image.width, image.height, SourceFormat::R16)
            }
            gltf::image::Format::R16G16 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(4) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    rgba.push((r >> 8) as u8); rgba.push((g >> 8) as u8); rgba.push(0); rgba.push(255);
                }
                (rgba, image.width, image.height, SourceFormat::Rg16)
            }
            gltf::image::Format::R16G16B16 => {
                let mut rgba = Vec::with_capacity((image.width * image.height * 4) as usize);
                for chunk in image.pixels.chunks(6) {
                    let r = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let g = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let b = u16::from_le_bytes([chunk[4], chunk[5]]);
                    rgba.push((r >> 8) as u8); rgba.push((g >> 8) as u8); rgba.push((b >> 8) as u8); rgba.push(255);
                }
                (rgba, image.width, image.height, SourceFormat::Rgb16)
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
                (rgba, image.width, image.height, SourceFormat::Rgba16)
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
                (rgba, image.width, image.height, SourceFormat::Rgb32Float)
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
                (rgba, image.width, image.height, SourceFormat::Rgba32Float)
            }
        };

        Ok(IntermediateImage {
            data,
            width,
            height,
            color_space: ColorSpace::Srgb, // default; caller should set based on usage
            source_format,
        })
    }

    fn mesh_count(&self) -> usize {
        self.document.meshes().len()
    }

    fn primitive_count(&self, mesh_index: usize) -> usize {
        self.document
            .meshes()
            .nth(mesh_index)
            .map(|m| m.primitives().len())
            .unwrap_or(0)
    }

    fn image_count(&self) -> usize {
        self.images.len()
    }
}
