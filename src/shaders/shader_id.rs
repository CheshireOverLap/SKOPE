//! Shader ID 열거형
//!
//! 모든 셰이더에 대한 타입-안전 식별자

use std::path::PathBuf;
use crate::paths;

/// 셰이더 식별자 (타입-안전)
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum ShaderId {
    // === GBuffer ===
    Visibility,
    MaterialEval,
    SkinnedMesh,
    Shader,  // 기본 셰이더
    DebugDraw,

    // === Lighting ===
    ShadowDepth,
    ShadowSampling,
    IblPrefilter,
    ClusterCull,
    CharacterLighting,

    // === Post Processing ===
    BloomThreshold,
    BloomDownsample,
    BloomUpsample,
    Tonemapping,
    ColorGrading,
    Taa,
    Dof,
    MotionBlur,
    Ssao,
    FilmEffects,

    // === Compute ===
    HistogramCompute,
    HistogramAverage,
    SssBlur,

    // === Effects ===
    Particle,
    ParticleUpdate,
    ParticleSpawn,
    ParticleRender,
    Flipbook,
    Vat,

    // === Editor ===
    Gizmo,
    Grid,
    EditorUi,
    EditorUiFont,
    // === Magic ===
    MagicCircle,
    SdfPrimitives,

    // === UI ===
    GameUi,
    GameText,

    // === Common (include용) ===
    CommonConstants,
    CommonMath,
    CommonPbr,
    CommonShadow,
    CommonStructs,
}

impl ShaderId {
    /// 셰이더 파일의 상대 경로 (engine_assets/shaders/ 기준)
    pub fn relative_path(&self) -> &'static str {
        match self {
            // GBuffer
            Self::Visibility => "gbuffer/visibility.wgsl",
            Self::MaterialEval => "gbuffer/material_eval.wgsl",
            Self::SkinnedMesh => "gbuffer/skinned_mesh.wgsl",
            Self::Shader => "gbuffer/shader.wgsl",
            Self::DebugDraw => "gbuffer/debug_draw.wgsl",

            // Lighting
            Self::ShadowDepth => "lighting/shadow_depth.wgsl",
            Self::ShadowSampling => "lighting/shadow_sampling.wgsl",
            Self::IblPrefilter => "lighting/ibl_prefilter.wgsl",
            Self::ClusterCull => "lighting/cluster_cull.wgsl",
            Self::CharacterLighting => "lighting/character_lighting.wgsl",

            // Post
            Self::BloomThreshold => "post/bloom_threshold.wgsl",
            Self::BloomDownsample => "post/bloom_downsample.wgsl",
            Self::BloomUpsample => "post/bloom_upsample.wgsl",
            Self::Tonemapping => "post/tonemapping.wgsl",
            Self::ColorGrading => "post/color_grading.wgsl",
            Self::Taa => "post/taa.wgsl",
            Self::Dof => "post/dof.wgsl",
            Self::MotionBlur => "post/motion_blur.wgsl",
            Self::Ssao => "post/ssao.wgsl",
            Self::FilmEffects => "post/film_effects.wgsl",

            // Compute
            Self::HistogramCompute => "compute/histogram_compute.wgsl",
            Self::HistogramAverage => "compute/histogram_average.wgsl",
            Self::SssBlur => "compute/sss_blur.wgsl",

            // Effects
            Self::Particle => "effects/particle.wgsl",
            Self::ParticleUpdate => "effects/particle_update.wgsl",
            Self::ParticleSpawn => "effects/particle_spawn.wgsl",
            Self::ParticleRender => "effects/gpu_particle_render.wgsl",
            Self::Flipbook => "effects/flipbook.wgsl",
            Self::Vat => "effects/vat.wgsl",

            // Editor
            Self::Gizmo => "editor/gizmo.wgsl",
            Self::Grid => "editor/grid.wgsl",
            Self::EditorUi => "editor/ui.wgsl",
            Self::EditorUiFont => "editor/ui_font.wgsl",
            // Magic
            Self::MagicCircle => "magic/magic_circle.wgsl",
            Self::SdfPrimitives => "magic/sdf_primitives.wgsl",

            // UI
            Self::GameUi => "ui/ui_shader.wgsl",
            Self::GameText => "ui/text_shader.wgsl",

            // Common
            Self::CommonConstants => "common/constants.wgsl",
            Self::CommonMath => "common/math.wgsl",
            Self::CommonPbr => "common/pbr.wgsl",
            Self::CommonShadow => "common/shadow.wgsl",
            Self::CommonStructs => "common/structs.wgsl",
        }
    }

    /// 전체 파일 경로
    pub fn to_path(self) -> PathBuf {
        PathBuf::from(paths::engine::SHADERS).join(self.relative_path())
    }

    /// 셰이더 이름 (디버깅/로깅용)
    pub fn name(&self) -> &'static str {
        match self {
            Self::Visibility => "Visibility",
            Self::MaterialEval => "MaterialEval",
            Self::SkinnedMesh => "SkinnedMesh",
            Self::Shader => "Shader",
            Self::DebugDraw => "DebugDraw",
            Self::ShadowDepth => "ShadowDepth",
            Self::ShadowSampling => "ShadowSampling",
            Self::IblPrefilter => "IblPrefilter",
            Self::ClusterCull => "ClusterCull",
            Self::CharacterLighting => "CharacterLighting",
            Self::BloomThreshold => "BloomThreshold",
            Self::BloomDownsample => "BloomDownsample",
            Self::BloomUpsample => "BloomUpsample",
            Self::Tonemapping => "Tonemapping",
            Self::ColorGrading => "ColorGrading",
            Self::Taa => "TAA",
            Self::Dof => "DoF",
            Self::MotionBlur => "MotionBlur",
            Self::Ssao => "SSAO",
            Self::FilmEffects => "FilmEffects",
            Self::HistogramCompute => "HistogramCompute",
            Self::HistogramAverage => "HistogramAverage",
            Self::SssBlur => "SSSBlur",
            Self::Particle => "Particle",
            Self::ParticleUpdate => "ParticleUpdate",
            Self::ParticleSpawn => "ParticleSpawn",
            Self::ParticleRender => "ParticleRender",
            Self::Flipbook => "Flipbook",
            Self::Vat => "VAT",
            Self::Gizmo => "Gizmo",
            Self::Grid => "Grid",
            Self::EditorUi => "EditorUI",
            Self::EditorUiFont => "EditorUIFont",
            Self::MagicCircle => "MagicCircle",
            Self::SdfPrimitives => "SDFPrimitives",
            Self::GameUi => "GameUI",
            Self::GameText => "GameText",
            Self::CommonConstants => "CommonConstants",
            Self::CommonMath => "CommonMath",
            Self::CommonPbr => "CommonPBR",
            Self::CommonShadow => "CommonShadow",
            Self::CommonStructs => "CommonStructs",
        }
    }

    /// 모든 셰이더 ID 목록
    pub fn all() -> &'static [ShaderId] {
        &[
            // GBuffer
            Self::Visibility, Self::MaterialEval, Self::SkinnedMesh,
            Self::Shader, Self::DebugDraw,
            // Lighting
            Self::ShadowDepth, Self::ShadowSampling, Self::IblPrefilter,
            Self::ClusterCull, Self::CharacterLighting,
            // Post
            Self::BloomThreshold, Self::BloomDownsample, Self::BloomUpsample,
            Self::Tonemapping, Self::ColorGrading, Self::Taa, Self::Dof,
            Self::MotionBlur, Self::Ssao, Self::FilmEffects,
            // Compute
            Self::HistogramCompute, Self::HistogramAverage, Self::SssBlur,
            // Effects
            Self::Particle, Self::ParticleUpdate, Self::ParticleSpawn,
            Self::ParticleRender, Self::Flipbook, Self::Vat,
            // Editor
            Self::Gizmo, Self::Grid, Self::EditorUi, Self::EditorUiFont,
            // Magic
            Self::MagicCircle, Self::SdfPrimitives,
            // UI
            Self::GameUi, Self::GameText,
            // Common
            Self::CommonConstants, Self::CommonMath, Self::CommonPbr,
            Self::CommonShadow, Self::CommonStructs,
        ]
    }

    /// 카테고리별 셰이더 목록
    pub fn by_category(category: ShaderCategory) -> &'static [ShaderId] {
        match category {
            ShaderCategory::GBuffer => &[
                Self::Visibility, Self::MaterialEval, Self::SkinnedMesh,
                Self::Shader, Self::DebugDraw,
            ],
            ShaderCategory::Lighting => &[
                Self::ShadowDepth, Self::ShadowSampling, Self::IblPrefilter,
                Self::ClusterCull, Self::CharacterLighting,
            ],
            ShaderCategory::Post => &[
                Self::BloomThreshold, Self::BloomDownsample, Self::BloomUpsample,
                Self::Tonemapping, Self::ColorGrading, Self::Taa, Self::Dof,
                Self::MotionBlur, Self::Ssao, Self::FilmEffects,
            ],
            ShaderCategory::Compute => &[
                Self::HistogramCompute, Self::HistogramAverage, Self::SssBlur,
            ],
            ShaderCategory::Effects => &[
                Self::Particle, Self::ParticleUpdate, Self::ParticleSpawn,
                Self::ParticleRender, Self::Flipbook, Self::Vat,
            ],
            ShaderCategory::Editor => &[
                Self::Gizmo, Self::Grid, Self::EditorUi, Self::EditorUiFont,
            ],
            ShaderCategory::Magic => &[
                Self::MagicCircle, Self::SdfPrimitives,
            ],
            ShaderCategory::Ui => &[
                Self::GameUi, Self::GameText,
            ],
            ShaderCategory::Common => &[
                Self::CommonConstants, Self::CommonMath, Self::CommonPbr,
                Self::CommonShadow, Self::CommonStructs,
            ],
        }
    }

    /// 파일 경로에서 ShaderId 역매핑
    pub fn from_path(path: &std::path::Path) -> Option<ShaderId> {
        let path_str = path.to_string_lossy();
        for id in Self::all() {
            if path_str.ends_with(id.relative_path()) {
                return Some(*id);
            }
        }
        None
    }
}

/// 셰이더 카테고리
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderCategory {
    GBuffer,
    Lighting,
    Post,
    Compute,
    Effects,
    Editor,
    Magic,
    Ui,
    Common,
}
