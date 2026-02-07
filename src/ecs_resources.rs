// ECS Resources for SKOPE Engine
//
// This file re-exports common types from skope_core and defines
// application-specific types that depend on external crates.

#![allow(dead_code)]

use bevy_ecs::prelude::*;
use std::collections::HashMap;

// skope_gltf 크레이트 (gltf_loader alias)

// ============ Re-exports from skope_core ============
// Common resource types shared across the engine

pub use skope_core::{
    // GPU Resources
    GpuContext, RenderPipelineRes, SkinnedPipelineRes,
    // Asset Resources
    MeshGpuData, MeshAssets, MaterialGpuData, MaterialAssets,
    // Skinned Mesh Resources
    SkinnedMeshGpuData, SkinnedMeshAssets, SkinData, SkinAssets, UniformBuffer,
    // Input Resources
    KeyboardInput, MouseInput,
    // Time & Window
    Time, WindowSize,
    // Environment
    Environment,
    // Render Extracted Data
    ExtractedCamera, ExtractedMeshInstance, ExtractedSkinnedInstance,
    ExtractedLighting, RenderExtractedData, HairExtractedData,
};

// ============ Application-Specific Resources ============
// These types depend on crates not available in skope_core

/// Fox 스킨드 메시 전용 머티리얼 (임시)
#[derive(Resource)]
pub struct FoxMaterialRes {
    pub texture_bind_group: wgpu::BindGroup,
    pub material_bind_group: wgpu::BindGroup,
}

/// 독립 머티리얼 이름 → GPU 머티리얼 인덱스 매핑
#[derive(Resource, Default)]
pub struct StandaloneMaterialMap {
    /// 머티리얼 이름 → GPU 머티리얼 인덱스
    pub name_to_index: HashMap<String, u32>,
    /// 머티리얼 파일 경로 → GPU 머티리얼 인덱스
    pub path_to_index: HashMap<String, u32>,
}

impl StandaloneMaterialMap {
    /// 이름으로 GPU 머티리얼 인덱스 조회
    pub fn get_by_name(&self, name: &str) -> Option<u32> {
        self.name_to_index.get(name).copied()
    }

    /// 경로로 GPU 머티리얼 인덱스 조회
    pub fn get_by_path(&self, path: &str) -> Option<u32> {
        self.path_to_index.get(path).copied()
    }
}

// ============ Skinned Model Registry ============
// Depends on skope_gltf types

/// 스킨드 모델 머티리얼 바인드 그룹
pub struct SkinnedMaterialBindGroups {
    pub texture_bind_group: wgpu::BindGroup,
    pub material_bind_group: wgpu::BindGroup,
}

/// 스킨드 모델 데이터 (로드된 스켈레탈 모델 정보)
pub struct SkinnedModelData {
    pub name: String,
    /// GPU 메시 데이터 인덱스들 (SkinnedMeshAssets)
    pub mesh_indices: Vec<usize>,
    /// 스킨 인덱스 (SkinAssets)
    pub skin_index: usize,
    /// 애니메이션 클립들
    pub animations: Vec<skope_gltf::Animation>,
    /// 노드 계층 구조
    pub nodes: Vec<skope_gltf::SceneNode>,
    /// 스킨 데이터 (조인트 정보 포함)
    pub skin: skope_gltf::Skin,
    /// 머티리얼 바인드 그룹들
    pub material_bind_groups: Vec<SkinnedMaterialBindGroups>,
}

/// 스킨드 모델 레지스트리 (여러 스켈레탈 모델 관리)
#[derive(Resource, Default)]
pub struct SkinnedModelRegistry {
    pub models: HashMap<String, SkinnedModelData>,
}

impl SkinnedModelRegistry {
    /// 모델 등록
    pub fn register(&mut self, data: SkinnedModelData) {
        log::info!("[SkinnedModelRegistry] Registered '{}' with {} meshes, {} animations",
            data.name, data.mesh_indices.len(), data.animations.len());
        self.models.insert(data.name.clone(), data);
    }

    /// 모델 조회
    pub fn get(&self, name: &str) -> Option<&SkinnedModelData> {
        self.models.get(name)
    }

    /// 등록된 모델 이름 목록
    pub fn model_names(&self) -> Vec<&str> {
        self.models.keys().map(|s| s.as_str()).collect()
    }
}

// ============ Play State Resource ============

/// 게임 플레이 상태 (ECS에서 접근 가능)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayState {
    #[default]
    Edit,
    Playing,
    Paused,
}

impl PlayState {
    pub fn is_playing(&self) -> bool {
        matches!(self, PlayState::Playing)
    }

    pub fn is_paused(&self) -> bool {
        matches!(self, PlayState::Paused)
    }

    pub fn is_edit(&self) -> bool {
        matches!(self, PlayState::Edit)
    }

    pub fn is_running(&self) -> bool {
        // Playing 또는 Paused (Edit 아님)
        !self.is_edit()
    }
}

/// 게임 플레이 상태 리소스
#[derive(Resource, Default)]
pub struct GamePlayState {
    pub state: PlayState,
    pub previous_state: PlayState,
    pub step_requested: bool,
    /// 플레이어가 스폰되었는지 여부
    pub player_spawned: bool,
    /// 스폰된 플레이어 엔티티
    pub player_entity: Option<bevy_ecs::entity::Entity>,
}

impl GamePlayState {
    /// 게임 로직을 실행해야 하는지 (Playing 또는 Step 요청 시)
    pub fn should_run_gameplay(&self) -> bool {
        self.state.is_playing() || self.step_requested
    }

    /// Step 완료 후 플래그 리셋
    pub fn clear_step(&mut self) {
        self.step_requested = false;
    }

    /// 상태 전환 감지 (Edit → Playing)
    pub fn just_started_playing(&self) -> bool {
        self.previous_state.is_edit() && self.state.is_playing()
    }

    /// 상태 전환 감지 (Playing → Edit)
    pub fn just_stopped_playing(&self) -> bool {
        self.previous_state.is_playing() && self.state.is_edit()
    }

    /// 상태 업데이트 (이전 상태 저장)
    pub fn update_state(&mut self, new_state: PlayState) {
        self.previous_state = self.state;
        self.state = new_state;
    }
}

// ============ External Crate Wrappers ============
// Depends on skope_blitz and skope_fianchetto

/// Light Manager wrapper for ECS
#[derive(Resource)]
pub struct LightManagerRes {
    pub manager: skope_blitz::LightManager,
}

/// Hybrid Hair Renderer wrapper for ECS
#[derive(Resource)]
pub struct HairRendererRes {
    pub renderer: skope_fianchetto::HybridHairRenderer,
}

/// Outline Pipeline wrapper for ECS (Phase 14)
#[derive(Resource)]
pub struct OutlinePipelineRes {
    pub pipeline: skope_check::OutlinePipeline,
}
