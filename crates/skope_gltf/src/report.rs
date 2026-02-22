//! Import Report — Analytics and extension detection for glTF imports
//!
//! Provides ImportReport with mesh/material/texture counts,
//! supported/unsupported extension detection, and validation summaries.

use crate::intermediate::MeshValidationReport;

/// 지원하는 glTF 확장 목록
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "KHR_lights_punctual",
    "KHR_materials_unlit",
    "KHR_materials_clearcoat",
    "KHR_materials_sheen",
    "KHR_materials_transmission",
    "KHR_texture_transform",
    "KHR_materials_volume",
    "KHR_materials_ior",
    "KHR_materials_pbrSpecularGlossiness",
    "MSFT_lod",
];

/// glTF 임포트 결과 리포트
#[derive(Debug, Clone)]
pub struct ImportReport {
    /// 원본 파일 경로
    pub file_path: String,
    /// 파일에서 사용된 확장 목록
    pub extensions_used: Vec<String>,
    /// 지원하지 않는 확장 목록
    pub extensions_unsupported: Vec<String>,
    /// 메시 수 (프리미티브 기준)
    pub mesh_count: usize,
    /// 머티리얼 수
    pub material_count: usize,
    /// 텍스처(이미지) 수
    pub texture_count: usize,
    /// 애니메이션 클립 수
    pub animation_count: usize,
    /// 스킨(스켈레톤) 수
    pub skin_count: usize,
    /// 메시 검증 리포트들
    pub validation_reports: Vec<MeshValidationReport>,
    /// 경고 메시지들
    pub warnings: Vec<String>,
}

impl ImportReport {
    /// 로그 출력
    pub fn log_summary(&self) {
        if !self.extensions_unsupported.is_empty() {
            log::warn!(
                "[ImportReport] Unsupported extensions: {:?}",
                self.extensions_unsupported
            );
        }

        log::info!(
            "[ImportReport] {} meshes, {} materials, {} textures, {} animations, {} skins",
            self.mesh_count,
            self.material_count,
            self.texture_count,
            self.animation_count,
            self.skin_count,
        );

        // 검증 경고 요약
        let auto_normals = self.validation_reports.iter().filter(|r| r.auto_generated_normals).count();
        let auto_tangents = self.validation_reports.iter().filter(|r| r.auto_generated_tangents).count();
        let degenerate: u32 = self.validation_reports.iter().map(|r| r.degenerate_triangles_removed).sum();

        if auto_normals > 0 || auto_tangents > 0 || degenerate > 0 {
            log::info!(
                "[ImportReport] Validation: {} auto-normals, {} auto-tangents, {} degenerate triangles removed",
                auto_normals, auto_tangents, degenerate,
            );
        }

        for warning in &self.warnings {
            log::warn!("[ImportReport] {}", warning);
        }
    }
}
