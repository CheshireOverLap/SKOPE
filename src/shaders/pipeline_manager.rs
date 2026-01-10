//! Pipeline Manager
//!
//! 셰이더 핫리로드 시 파이프라인 자동 리빌드 관리
//!
//! # 사용법
//!
//! ```rust,ignore
//! // PipelineManager 생성
//! let mut pm = PipelineManager::new(shader_manager);
//!
//! // 파이프라인-셰이더 의존성 등록
//! pm.register_dependency(PipelineId::Bloom, ShaderId::BloomThreshold);
//! pm.register_dependency(PipelineId::Bloom, ShaderId::BloomDownsample);
//!
//! // 리빌드 콜백 등록
//! pm.on_rebuild(PipelineId::Bloom, |shaders, device| {
//!     rebuild_bloom_pipeline(shaders, device);
//! });
//!
//! // 매 프레임 업데이트
//! pm.update(&device);
//! ```

use std::collections::{HashMap, HashSet};
use wgpu::Device;

use super::shader_id::ShaderId;
use super::manager::ShaderManager;

/// 파이프라인 식별자
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum PipelineId {
    // === G-Buffer ===
    GBuffer,
    MaterialEval,
    SkinnedMesh,
    DebugDraw,

    // === Lighting ===
    ShadowDepth,
    ClusterCull,
    CharacterLighting,

    // === Post Processing ===
    Bloom,
    Tonemapping,
    ColorGrading,
    Taa,
    Dof,
    MotionBlur,
    Ssao,
    FilmEffects,
    SssBlur,

    // === Effects ===
    Particle,
    Flipbook,
    Vat,

    // === Editor ===
    Gizmo,
    Grid,
    EditorUi,
    Outline,

    // === Hair ===
    Hair,

    // === Magic ===
    MagicCircle,

    // === UI ===
    GameUi,
}

/// 리빌드 콜백 타입
pub type RebuildCallback = Box<dyn FnMut(&mut ShaderManager, &Device) + Send>;

/// 파이프라인 매니저
///
/// 셰이더 변경 시 관련 파이프라인을 자동으로 리빌드합니다.
pub struct PipelineManager {
    /// 셰이더 매니저
    shader_manager: ShaderManager,

    /// 셰이더 → 파이프라인 의존성 맵
    shader_to_pipelines: HashMap<ShaderId, HashSet<PipelineId>>,

    /// 리빌드 콜백
    rebuild_callbacks: HashMap<PipelineId, RebuildCallback>,

    /// 리빌드 대기 중인 파이프라인
    pending_rebuilds: HashSet<PipelineId>,
}

impl PipelineManager {
    /// 새 파이프라인 매니저 생성
    pub fn new(shader_manager: ShaderManager) -> Self {
        Self {
            shader_manager,
            shader_to_pipelines: HashMap::new(),
            rebuild_callbacks: HashMap::new(),
            pending_rebuilds: HashSet::new(),
        }
    }

    /// 셰이더 매니저 접근
    pub fn shaders(&mut self) -> &mut ShaderManager {
        &mut self.shader_manager
    }

    /// 셰이더 매니저 (읽기 전용)
    pub fn shaders_ref(&self) -> &ShaderManager {
        &self.shader_manager
    }

    /// 파이프라인-셰이더 의존성 등록
    pub fn register_dependency(&mut self, pipeline: PipelineId, shader: ShaderId) {
        self.shader_to_pipelines
            .entry(shader)
            .or_insert_with(HashSet::new)
            .insert(pipeline);

        log::debug!("[PipelineManager] Registered: {:?} depends on {:?}", pipeline, shader);
    }

    /// 여러 셰이더에 대한 의존성 등록
    pub fn register_dependencies(&mut self, pipeline: PipelineId, shaders: &[ShaderId]) {
        for shader in shaders {
            self.register_dependency(pipeline, *shader);
        }
    }

    /// 리빌드 콜백 등록
    pub fn on_rebuild<F>(&mut self, pipeline: PipelineId, callback: F)
    where
        F: FnMut(&mut ShaderManager, &Device) + Send + 'static,
    {
        self.rebuild_callbacks.insert(pipeline, Box::new(callback));
        log::debug!("[PipelineManager] Registered rebuild callback for {:?}", pipeline);
    }

    /// 매 프레임 업데이트 - 핫리로드 및 파이프라인 리빌드
    pub fn update(&mut self, device: &Device) {
        // 셰이더 자동 리로드
        let reloaded = self.shader_manager.auto_reload();

        // 리로드된 셰이더에 의존하는 파이프라인 찾기
        for shader_id in reloaded {
            if let Some(pipeline_ids) = self.shader_to_pipelines.get(&shader_id) {
                for pid in pipeline_ids {
                    self.pending_rebuilds.insert(*pid);
                }
            }
        }

        // 대기 중인 파이프라인 리빌드
        self.flush_pending_rebuilds(device);
    }

    /// 대기 중인 파이프라인 리빌드 실행
    fn flush_pending_rebuilds(&mut self, device: &Device) {
        if self.pending_rebuilds.is_empty() {
            return;
        }

        let pending: Vec<PipelineId> = self.pending_rebuilds.drain().collect();

        for pid in pending {
            if let Some(callback) = self.rebuild_callbacks.get_mut(&pid) {
                log::info!("[PipelineManager] Rebuilding pipeline: {:?}", pid);
                callback(&mut self.shader_manager, device);
            } else {
                log::warn!("[PipelineManager] No rebuild callback for {:?}", pid);
            }
        }
    }

    /// 특정 파이프라인 강제 리빌드
    pub fn force_rebuild(&mut self, pipeline: PipelineId, device: &Device) {
        if let Some(callback) = self.rebuild_callbacks.get_mut(&pipeline) {
            log::info!("[PipelineManager] Force rebuilding: {:?}", pipeline);
            callback(&mut self.shader_manager, device);
        }
    }

    /// 모든 파이프라인 강제 리빌드
    pub fn force_rebuild_all(&mut self, device: &Device) {
        log::info!("[PipelineManager] Force rebuilding all pipelines...");

        // 모든 셰이더 리로드
        let _reloaded = self.shader_manager.force_reload_all();

        // 모든 등록된 파이프라인 리빌드
        let all_pipelines: Vec<PipelineId> = self.rebuild_callbacks.keys().cloned().collect();
        for pid in all_pipelines {
            if let Some(callback) = self.rebuild_callbacks.get_mut(&pid) {
                log::info!("[PipelineManager] Rebuilding: {:?}", pid);
                callback(&mut self.shader_manager, device);
            }
        }
    }

    /// 등록된 의존성 개수
    pub fn dependency_count(&self) -> usize {
        self.shader_to_pipelines.values().map(|v| v.len()).sum()
    }

    /// 등록된 콜백 개수
    pub fn callback_count(&self) -> usize {
        self.rebuild_callbacks.len()
    }
}

/// 기본 파이프라인-셰이더 매핑 설정
///
/// PipelineManager에 일반적인 의존성을 등록합니다.
pub fn setup_default_dependencies(pm: &mut PipelineManager) {
    // G-Buffer
    pm.register_dependencies(PipelineId::GBuffer, &[ShaderId::Visibility, ShaderId::Shader]);
    pm.register_dependency(PipelineId::MaterialEval, ShaderId::MaterialEval);
    pm.register_dependency(PipelineId::SkinnedMesh, ShaderId::SkinnedMesh);
    pm.register_dependency(PipelineId::DebugDraw, ShaderId::DebugDraw);

    // Lighting
    pm.register_dependency(PipelineId::ShadowDepth, ShaderId::ShadowDepth);
    pm.register_dependency(PipelineId::ClusterCull, ShaderId::ClusterCull);
    pm.register_dependency(PipelineId::CharacterLighting, ShaderId::CharacterLighting);

    // Post Processing
    pm.register_dependencies(PipelineId::Bloom, &[
        ShaderId::BloomThreshold,
        ShaderId::BloomDownsample,
        ShaderId::BloomUpsample,
    ]);
    pm.register_dependency(PipelineId::Tonemapping, ShaderId::Tonemapping);
    pm.register_dependency(PipelineId::ColorGrading, ShaderId::ColorGrading);
    pm.register_dependency(PipelineId::Taa, ShaderId::Taa);
    pm.register_dependency(PipelineId::Dof, ShaderId::Dof);
    pm.register_dependency(PipelineId::MotionBlur, ShaderId::MotionBlur);
    pm.register_dependency(PipelineId::Ssao, ShaderId::Ssao);
    pm.register_dependency(PipelineId::FilmEffects, ShaderId::FilmEffects);
    pm.register_dependency(PipelineId::SssBlur, ShaderId::SssBlur);

    // Effects
    pm.register_dependencies(PipelineId::Particle, &[
        ShaderId::Particle,
        ShaderId::ParticleUpdate,
        ShaderId::ParticleSpawn,
        ShaderId::ParticleRender,
    ]);
    pm.register_dependency(PipelineId::Flipbook, ShaderId::Flipbook);
    pm.register_dependency(PipelineId::Vat, ShaderId::Vat);

    // Editor
    pm.register_dependency(PipelineId::Gizmo, ShaderId::Gizmo);
    pm.register_dependency(PipelineId::Grid, ShaderId::Grid);
    pm.register_dependencies(PipelineId::EditorUi, &[ShaderId::EditorUi, ShaderId::EditorUiFont]);
    pm.register_dependencies(PipelineId::Outline, &[
        ShaderId::OutlineHull,
        ShaderId::OutlineEdgeDetect,
        ShaderId::OutlineComposite,
    ]);

    // Hair
    pm.register_dependencies(PipelineId::Hair, &[
        ShaderId::HairCard,
        ShaderId::HairComposite,
        ShaderId::HairFlyaway,
        ShaderId::HairStrandRasterize,
        ShaderId::HairStrandSpawn,
    ]);

    // Magic
    pm.register_dependencies(PipelineId::MagicCircle, &[ShaderId::MagicCircle, ShaderId::SdfPrimitives]);

    // UI
    pm.register_dependencies(PipelineId::GameUi, &[ShaderId::GameUi, ShaderId::GameText]);

    log::info!(
        "[PipelineManager] Setup complete: {} dependencies",
        pm.dependency_count()
    );
}
