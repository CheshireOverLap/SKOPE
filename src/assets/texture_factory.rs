//! Texture Factory — bindless texture registration from Model data
//!
//! Wraps TextureArrayManager → bindless registration → handle mapping.
//! UE5.7 Texture Factory 패턴 참고.

use std::collections::HashMap;
use crate::gltf_loader::Model;
use crate::renderer::texture_array::TextureArrayManager;
use crate::renderer::material_eval::{MaterialEvalPipeline, BindlessHandleMaps};

/// glTF 텍스처 인덱스 → bindless slot 직접 매핑
#[derive(Debug, Clone, Default)]
pub struct TextureHandleMap {
    /// glTF texture index → bindless slot (albedo)
    pub albedo: HashMap<usize, u32>,
    /// glTF texture index → bindless slot (normal)
    pub normal: HashMap<usize, u32>,
    /// glTF texture index → bindless slot (metallic_roughness)
    pub metallic_roughness: HashMap<usize, u32>,
}

pub struct TextureFactory;

impl TextureFactory {
    /// Model의 텍스처 → D2Array → bindless 등록 → TextureHandleMap
    pub fn create_from_model(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        model: &Model,
        material_eval: &mut MaterialEvalPipeline,
    ) -> TextureHandleMap {
        if model.textures.is_empty() {
            log::info!("[TextureFactory] No textures in model, returning empty handle map");
            return TextureHandleMap::default();
        }

        // 1. TextureArrayManager 생성 (D2Array + 레이어 매핑)
        let tex_array = TextureArrayManager::from_gltf_textures(
            device, queue, &model.textures, &model.materials,
        );

        // 2. bindless 등록
        let views = tex_array.extract_bindless_views();
        let bindless_maps = material_eval.register_texture_array_views(device, views);

        // 3. 합성: glTF tex_idx → layer → bindless slot
        let handle_map = Self::compose_handle_map(&tex_array, &bindless_maps, &model.materials);

        log::info!(
            "[TextureFactory] Created handle map: albedo={}, normal={}, mr={}",
            handle_map.albedo.len(),
            handle_map.normal.len(),
            handle_map.metallic_roughness.len(),
        );

        handle_map
    }

    /// TextureArrayManager 레이어 매핑 + BindlessHandleMaps → TextureHandleMap 합성
    fn compose_handle_map(
        tex_array: &TextureArrayManager,
        bindless_maps: &BindlessHandleMaps,
        materials: &[crate::gltf_loader::Material],
    ) -> TextureHandleMap {
        let mut map = TextureHandleMap::default();

        for mat in materials {
            if let Some(idx) = mat.base_color_texture {
                if !map.albedo.contains_key(&idx) {
                    if let Some(layer) = tex_array.get_albedo_layer(idx) {
                        if let Some(&slot) = bindless_maps.albedo.get(&layer) {
                            map.albedo.insert(idx, slot);
                        }
                    }
                }
            }
            if let Some(idx) = mat.normal_texture {
                if !map.normal.contains_key(&idx) {
                    if let Some(layer) = tex_array.get_normal_layer(idx) {
                        if let Some(&slot) = bindless_maps.normal.get(&layer) {
                            map.normal.insert(idx, slot);
                        }
                    }
                }
            }
            if let Some(idx) = mat.metallic_roughness_texture {
                if !map.metallic_roughness.contains_key(&idx) {
                    if let Some(layer) = tex_array.get_mr_layer(idx) {
                        if let Some(&slot) = bindless_maps.metallic_roughness.get(&layer) {
                            map.metallic_roughness.insert(idx, slot);
                        }
                    }
                }
            }
        }

        map
    }
}
