//! glTF/GLB Model Loader
//!
//! Main loading function — delegates to GltfTranslator + MeshValidator
//! then converts GltfIntermediate to Model for backward compatibility.

use std::path::Path;

use crate::translator::GltfTranslator;
use crate::validator::MeshValidator;
use crate::types::*;

/// glTF/GLB 파일 로드 (하위 호환 API)
///
/// 내부적으로 GltfTranslator → MeshValidator → Model 변환 파이프라인 사용
pub fn load_gltf<P: AsRef<Path>>(path: P) -> Result<Model, Box<dyn std::error::Error>> {
    let path = path.as_ref();

    // 1. Translator: glTF → IR
    let mut intermediate = GltfTranslator::translate(path)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // 2. Validator: 각 프리미티브 검증 및 자동 수정
    let mut all_reports = Vec::new();
    for primitives in &mut intermediate.meshes {
        for prim in primitives.iter_mut() {
            let report = MeshValidator::validate_and_fix(prim);
            if report.auto_generated_normals || report.auto_generated_tangents
                || report.auto_generated_indices || report.degenerate_triangles_removed > 0
            {
                log::debug!(
                    "[Loader] Validation: normals={}, tangents={}, indices={}, degenerate_removed={}",
                    report.auto_generated_normals, report.auto_generated_tangents,
                    report.auto_generated_indices, report.degenerate_triangles_removed
                );
            }
            all_reports.push(report);
        }
    }
    intermediate.validation_reports = all_reports;

    // 3. IR → Model 변환
    let model = intermediate.to_model();

    log::info!(
        "Loaded {} meshes, {} skinned meshes, {} skins, {} animations, {} materials, {} textures, {} nodes ({} roots), {} lights, {} cameras",
        model.meshes.len(), model.skinned_meshes.len(), model.skins.len(),
        model.animations.len(), model.materials.len(), model.textures.len(),
        model.nodes.len(), model.root_nodes.len(), model.lights.len(), model.cameras.len()
    );

    Ok(model)
}

/// glTF/GLB 파일을 IR로 직접 로드 (새 API)
pub fn load_gltf_intermediate<P: AsRef<Path>>(path: P) -> Result<crate::intermediate::GltfIntermediate, Box<dyn std::error::Error>> {
    let path = path.as_ref();

    let mut intermediate = GltfTranslator::translate(path)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    // Validator 적용
    let mut all_reports = Vec::new();
    for primitives in &mut intermediate.meshes {
        for prim in primitives.iter_mut() {
            all_reports.push(MeshValidator::validate_and_fix(prim));
        }
    }
    intermediate.validation_reports = all_reports;

    Ok(intermediate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_skinned_model() {
        // RiggedSimple.glb 로딩 테스트
        let path = "assets/models/RiggedSimple.glb";
        if !std::path::Path::new(path).exists() {
            log::warn!("Test model not found: {}", path);
            return;
        }

        let model = load_gltf(path).expect("Failed to load RiggedSimple.glb");

        log::info!("=== RiggedSimple.glb Loading Test ===");
        log::debug!("Static meshes: {}", model.meshes.len());
        log::debug!("Skinned meshes: {}", model.skinned_meshes.len());
        log::debug!("Skins: {}", model.skins.len());
        log::debug!("Nodes: {}", model.nodes.len());

        for (i, skin) in model.skins.iter().enumerate() {
            log::debug!("Skin {}: '{}' ({} joints)", i, skin.name, skin.joints.len());
        }

        for (i, sm) in model.skinned_meshes.iter().enumerate() {
            log::debug!("SkinnedMesh {}: {} verts, {} indices, skin {}",
                i, sm.vertices.len(), sm.indices.len(), sm.skin_index);
        }

        assert!(model.skinned_meshes.len() > 0, "Should have at least one skinned mesh");
        assert!(model.skins.len() > 0, "Should have at least one skin");
    }
}
