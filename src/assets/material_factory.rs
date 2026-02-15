//! Material Factory — MaterialRegistry registration from Model data
//!
//! Extracts material registration logic from gltf_importer.rs.
//! UE5.7 Material Instance Factory 패턴 참고.

#![allow(dead_code)]

use crate::gltf_loader::Model;
use crate::material::{MaterialRegistry, MaterialTextureIndices};
use crate::material::variant::MaterialVariantKey;
use crate::renderer::material_eval::types::{GpuMaterial, INVALID_TEXTURE_HANDLE};
use super::texture_factory::TextureHandleMap;
use skope_gltf::intermediate::{GltfShadingModel, AlphaMode};

/// 머티리얼 GPU 리소스 생성 팩토리
pub struct MaterialFactory;

impl MaterialFactory {
    /// Model의 머티리얼 → MaterialRegistry 등록
    ///
    /// Returns: glTF material index → Registry gpu_index 매핑
    pub fn create_from_model(
        registry: &mut MaterialRegistry,
        model: &Model,
        model_name: &str,
    ) -> Vec<usize> {
        let mut index_map = Vec::with_capacity(model.materials.len());

        for (i, mat) in model.materials.iter().enumerate() {
            let unique_name = if mat.name.is_empty() || mat.name == "Unnamed" {
                format!("{}_{}", model_name, i)
            } else {
                format!("{}_{}", model_name, mat.name)
            };

            let texture_indices = Self::build_texture_indices(mat);

            let variant = MaterialVariantKey::new(
                shading_model_from_u32(mat.shading_model),
                alpha_mode_from_u32(mat.alpha_mode, mat.alpha_cutoff),
                mat.double_sided,
            );
            log::debug!("[MaterialFactory] {} → variant: {}", unique_name, variant.identifier());

            let gpu_index = registry.register_from_gltf_extended(
                unique_name,
                mat.base_color_factor,
                mat.metallic_factor,
                mat.roughness_factor,
                texture_indices,
                mat.alpha_mode,
                mat.alpha_cutoff,
                mat.double_sided,
                mat.shading_model,
                mat.emissive_strength,
                mat.clear_coat,
                mat.clear_coat_roughness,
            );

            index_map.push(gpu_index);
        }

        log::info!(
            "[MaterialFactory] Registered {} materials from '{}'",
            index_map.len(),
            model_name
        );

        index_map
    }

    /// Model의 머티리얼 → Vec<GpuMaterial> (bindless 핸들 포함)
    ///
    /// handle_map: TextureHandleMap (glTF tex idx → bindless slot 직접 매핑)
    /// state.rs Phase 10.3 GpuMaterial 생성 패턴과 동일
    pub fn create_gpu_materials(
        model: &Model,
        handle_map: &TextureHandleMap,
    ) -> Vec<GpuMaterial> {
        let gpu_materials: Vec<GpuMaterial> = model.materials.iter().map(|mat| {
            let albedo_handle = mat.base_color_texture
                .and_then(|idx| handle_map.albedo.get(&idx).copied())
                .unwrap_or(INVALID_TEXTURE_HANDLE);

            let normal_handle = mat.normal_texture
                .and_then(|idx| handle_map.normal.get(&idx).copied())
                .unwrap_or(INVALID_TEXTURE_HANDLE);

            let mr_handle = mat.metallic_roughness_texture
                .and_then(|idx| handle_map.metallic_roughness.get(&idx).copied())
                .unwrap_or(INVALID_TEXTURE_HANDLE);

            log::debug!(
                "[MaterialFactory] GpuMaterial '{}': albedo={}, normal={}, mr={}, base_color={:?}",
                mat.name, albedo_handle, normal_handle, mr_handle, mat.base_color_factor
            );

            GpuMaterial {
                base_color: mat.base_color_factor,
                metallic: mat.metallic_factor,
                roughness: mat.roughness_factor,
                emissive_strength: mat.emissive_factor.iter().fold(0.0f32, |acc, &x| acc.max(x)),
                normal_scale: 1.0,
                albedo_tex_handle: albedo_handle,
                normal_tex_handle: normal_handle,
                metallic_roughness_tex_handle: mr_handle,
                emissive_tex_handle: INVALID_TEXTURE_HANDLE,
                shading_model: mat.shading_model,
                alpha_mode: mat.alpha_mode,
                alpha_cutoff: mat.alpha_cutoff,
                clear_coat: mat.clear_coat,
                clear_coat_roughness: mat.clear_coat_roughness,
                flags: if mat.double_sided { 1 } else { 0 },
                ..Default::default()
            }
        }).collect();

        log::info!(
            "[MaterialFactory] Created {} GpuMaterials with bindless handles",
            gpu_materials.len()
        );

        gpu_materials
    }

    /// Material → MaterialTextureIndices 변환
    fn build_texture_indices(mat: &crate::gltf_loader::Material) -> MaterialTextureIndices {
        MaterialTextureIndices {
            albedo_layer: mat.base_color_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
            normal_layer: mat.normal_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
            metallic_roughness_layer: mat.metallic_roughness_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
            emissive_layer: mat.emissive_texture.map(|idx| idx as u32).unwrap_or(INVALID_TEXTURE_HANDLE),
        }
    }
}

fn shading_model_from_u32(id: u32) -> GltfShadingModel {
    match id {
        6 => GltfShadingModel::Unlit,
        7 => GltfShadingModel::Sheen,
        8 => GltfShadingModel::Transmission,
        _ => GltfShadingModel::MetallicRoughness,
    }
}

fn alpha_mode_from_u32(mode: u32, cutoff: f32) -> AlphaMode {
    match mode {
        1 => AlphaMode::Mask { cutoff },
        2 => AlphaMode::Blend,
        _ => AlphaMode::Opaque,
    }
}
