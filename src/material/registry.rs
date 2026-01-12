//! SKOPE Material Registry
//!
//! 로드된 머티리얼을 관리하고 GPU 인덱싱을 제공

use std::collections::HashMap;
use std::path::PathBuf;

use bevy_ecs::prelude::*;

use super::material_def::MaterialDef;
use crate::renderer::GpuMaterial;

/// 텍스처 배열 레이어 인덱스
#[derive(Debug, Clone, Default)]
pub struct MaterialTextureIndices {
    pub albedo_layer: i32,              // -1 = 텍스처 없음
    pub normal_layer: i32,
    pub metallic_roughness_layer: i32,
    pub emissive_layer: i32,
}

/// 로드된 머티리얼 엔트리
#[derive(Debug, Clone)]
pub struct MaterialEntry {
    /// 머티리얼 정의
    pub def: MaterialDef,
    /// GPU 배열 내 인덱스
    pub gpu_index: usize,
    /// 원본 파일 경로 (핫 리로드 및 저장용, None = glTF에서 로드됨)
    pub source_path: Option<PathBuf>,
    /// 텍스처 배열 레이어 인덱스들
    pub texture_indices: MaterialTextureIndices,
    /// GPU 업데이트 필요 플래그
    pub dirty: bool,
}

impl MaterialEntry {
    /// MaterialDef -> GpuMaterial 변환
    pub fn to_gpu_material(&self) -> GpuMaterial {
        GpuMaterial {
            base_color: self.def.base_color,
            metallic: self.def.metallic,
            roughness: self.def.roughness,
            emissive_strength: self.def.emissive_strength,
            normal_scale: self.def.normal_scale,
            albedo_tex_idx: self.texture_indices.albedo_layer,
            normal_tex_idx: self.texture_indices.normal_layer,
            metallic_roughness_tex_idx: self.texture_indices.metallic_roughness_layer,
            emissive_tex_idx: self.texture_indices.emissive_layer,
            uv_scale: self.def.uv_scale.unwrap_or([1.0, 1.0]),
            uv_mode: self.def.uv_mode,
            _pad: [0],
        }
    }

    /// RON 파일로 저장 가능한지 여부
    pub fn can_save(&self) -> bool {
        self.source_path.is_some()
    }
}

/// 머티리얼 레지스트리
///
/// 모든 머티리얼을 중앙에서 관리하고 GPU 인덱싱을 제공합니다.
/// ECS Resource로 사용됩니다.
#[derive(Resource)]
pub struct MaterialRegistry {
    /// 이름 -> 엔트리 매핑
    materials: HashMap<String, MaterialEntry>,
    /// GPU 인덱스 -> 이름 역매핑
    index_to_name: Vec<String>,
    /// 기본 머티리얼 인덱스
    default_index: usize,
    /// 다음 할당할 GPU 인덱스
    next_index: usize,
}

impl Default for MaterialRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MaterialRegistry {
    /// 새 레지스트리 생성
    pub fn new() -> Self {
        let mut registry = Self {
            materials: HashMap::new(),
            index_to_name: Vec::new(),
            default_index: 0,
            next_index: 0,
        };

        // 기본 머티리얼 등록
        let default_mat = MaterialDef::new("Default");
        registry.default_index = registry.register(default_mat, None);

        registry
    }

    /// 머티리얼 등록 (GPU 인덱스 할당)
    pub fn register(&mut self, def: MaterialDef, source_path: Option<PathBuf>) -> usize {
        // 이미 존재하면 업데이트
        if let Some(entry) = self.materials.get_mut(&def.name) {
            entry.def = def;
            entry.source_path = source_path;
            entry.dirty = true;
            return entry.gpu_index;
        }

        let gpu_index = self.next_index;
        self.next_index += 1;

        let name = def.name.clone();
        let entry = MaterialEntry {
            def,
            gpu_index,
            source_path,
            texture_indices: MaterialTextureIndices::default(),
            dirty: true,
        };

        self.materials.insert(name.clone(), entry);
        self.index_to_name.push(name);

        log::debug!("[MaterialRegistry] Registered '{}' at index {}",
            self.index_to_name.last().unwrap(), gpu_index);

        gpu_index
    }

    /// glTF에서 로드된 머티리얼 등록 (저장 불가)
    pub fn register_from_gltf(
        &mut self,
        name: String,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        texture_indices: MaterialTextureIndices,
    ) -> usize {
        let def = MaterialDef {
            name: name.clone(),
            base_color,
            metallic,
            roughness,
            emissive_strength: 0.0,
            normal_scale: 1.0,
            uv_scale: None,
            uv_mode: 0,
            textures: Default::default(),
        };

        // 이미 존재하면 기존 인덱스 반환
        if let Some(entry) = self.materials.get(&name) {
            return entry.gpu_index;
        }

        let gpu_index = self.next_index;
        self.next_index += 1;

        let entry = MaterialEntry {
            def,
            gpu_index,
            source_path: None, // glTF = 저장 불가
            texture_indices,
            dirty: true,
        };

        self.materials.insert(name.clone(), entry);
        self.index_to_name.push(name);

        gpu_index
    }

    /// 이름으로 머티리얼 조회
    pub fn get(&self, name: &str) -> Option<&MaterialEntry> {
        self.materials.get(name)
    }

    /// 이름으로 머티리얼 수정 가능 조회
    pub fn get_mut(&mut self, name: &str) -> Option<&mut MaterialEntry> {
        self.materials.get_mut(name)
    }

    /// GPU 인덱스로 조회
    pub fn get_by_index(&self, index: usize) -> Option<&MaterialEntry> {
        self.index_to_name
            .get(index)
            .and_then(|name| self.materials.get(name))
    }

    /// GPU 인덱스로 수정 가능 조회
    pub fn get_by_index_mut(&mut self, index: usize) -> Option<&mut MaterialEntry> {
        if let Some(name) = self.index_to_name.get(index).cloned() {
            self.materials.get_mut(&name)
        } else {
            None
        }
    }

    /// 기본 머티리얼 인덱스
    pub fn default_index(&self) -> usize {
        self.default_index
    }

    /// 더티 머티리얼들 수집 (GPU 업데이트용)
    pub fn collect_dirty(&mut self) -> Vec<(usize, GpuMaterial)> {
        let mut updates = Vec::new();

        for entry in self.materials.values_mut() {
            if entry.dirty {
                let gpu_mat = entry.to_gpu_material();
                updates.push((entry.gpu_index, gpu_mat));
                entry.dirty = false;
            }
        }

        updates
    }

    /// 모든 머티리얼을 GpuMaterial로 변환
    pub fn to_gpu_materials(&self) -> Vec<GpuMaterial> {
        let mut result = vec![GpuMaterial::default(); self.next_index];

        for entry in self.materials.values() {
            if entry.gpu_index < result.len() {
                result[entry.gpu_index] = entry.to_gpu_material();
            }
        }

        result
    }

    /// 전체 머티리얼 이름 목록
    pub fn list_names(&self) -> Vec<&str> {
        self.materials.keys().map(|s| s.as_str()).collect()
    }

    /// 총 머티리얼 수
    pub fn count(&self) -> usize {
        self.materials.len()
    }

    /// 모든 엔트리 순회 (불변)
    pub fn iter(&self) -> impl Iterator<Item = (&String, &MaterialEntry)> {
        self.materials.iter()
    }

    /// 모든 엔트리 순회 (가변)
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut MaterialEntry)> {
        self.materials.iter_mut()
    }

    /// 특정 머티리얼을 dirty로 표시
    pub fn mark_dirty(&mut self, name: &str) {
        if let Some(entry) = self.materials.get_mut(name) {
            entry.dirty = true;
        }
    }

    /// 모든 머티리얼을 dirty로 표시
    pub fn mark_all_dirty(&mut self) {
        for entry in self.materials.values_mut() {
            entry.dirty = true;
        }
    }
}
