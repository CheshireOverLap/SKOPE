// Asset Loader - Scans and loads assets from assets/ folder
// Phase 9: glTF auto-loading + texture/material/ECS pipeline

use std::path::{Path, PathBuf};
use std::fs;

use crate::gltf_loader;
use crate::ecs_resources::MeshAssets;
use crate::renderer::material_eval::MaterialEvalPipeline;
use crate::renderer::material_eval::types::GpuMaterial;

use super::mesh_factory::MeshFactory;
use super::texture_factory::TextureFactory;
use super::material_factory::MaterialFactory;

/// 개별 모델 임포트 결과
#[allow(dead_code)]
pub struct ImportedModel {
    pub name: String,
    pub model: crate::gltf_loader::Model,
    pub mesh_indices: Vec<usize>,       // MeshAssets 인덱스들
    pub gpu_materials: Vec<GpuMaterial>, // 이 모델의 GpuMaterial들
}

/// 전체 임포트 결과
pub struct AssetImportResult {
    pub models: Vec<ImportedModel>,
    pub total_meshes: usize,
    pub total_materials: usize,
}

/// Scan assets folder for glTF files and return paths
pub fn scan_gltf_files(assets_path: &Path) -> Vec<PathBuf> {
    let mut gltf_files = Vec::new();

    if !assets_path.exists() {
        log::info!("[AssetLoader] Assets folder not found: {:?}", assets_path);
        return gltf_files;
    }

    // Recursively scan for .gltf and .glb files
    scan_directory_recursive(assets_path, &mut gltf_files);

    log::info!("[AssetLoader] Found {} glTF files in {:?}", gltf_files.len(), assets_path);
    for path in &gltf_files {
        log::debug!("  - {:?}", path);
    }

    gltf_files
}

fn scan_directory_recursive(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_directory_recursive(&path, files);
            } else if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if ext_lower == "gltf" || ext_lower == "glb" {
                    files.push(path);
                }
            }
        }
    }
}

/// Get relative path from assets folder
fn get_relative_asset_path(full_path: &Path) -> Option<String> {
    // Find "assets" in path and get everything after it
    let components: Vec<_> = full_path.components().collect();
    for (i, comp) in components.iter().enumerate() {
        if let std::path::Component::Normal(os_str) = comp {
            if os_str.to_string_lossy() == "assets" {
                let relative: PathBuf = components[i+1..].iter().collect();
                return Some(relative.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Load all glTF files from assets folder — full pipeline
///
/// Phase 1: rayon 병렬 파싱 (CPU-bound IO + 디코딩 + IR→Model 변환)
/// Phase 2: 순차 GPU 업로드 — 메시 + 텍스처(bindless) + 머티리얼(GpuMaterial)
pub fn load_all_assets(
    assets_path: &Path,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh_assets: &mut MeshAssets,
    material_eval: &mut MaterialEvalPipeline,
) -> AssetImportResult {
    let gltf_files = scan_gltf_files(assets_path);

    if gltf_files.is_empty() {
        return AssetImportResult {
            models: Vec::new(),
            total_meshes: 0,
            total_materials: 0,
        };
    }

    let start = std::time::Instant::now();

    // Phase 1: rayon 병렬 파싱 (CPU-bound)
    use rayon::prelude::*;

    struct ParsedFile {
        file_stem: String,
        model: crate::gltf_loader::Model,
        report: crate::gltf_loader::ImportReport,
        relative_path: Option<String>,
    }

    let parsed: Vec<ParsedFile> = gltf_files
        .into_par_iter()
        .filter_map(|gltf_path| {
            let path_str = gltf_path.to_string_lossy().to_string();
            match gltf_loader::loader::load_gltf_intermediate(&path_str) {
                Ok(intermediate) => {
                    let report = intermediate.to_report();
                    let model = intermediate.to_model();
                    let file_stem = gltf_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let relative_path = get_relative_asset_path(&gltf_path);
                    Some(ParsedFile { file_stem, model, report, relative_path })
                }
                Err(e) => {
                    log::info!("[AssetLoader] Failed to parse {:?}: {}", gltf_path, e);
                    None
                }
            }
        })
        .collect();

    let parse_elapsed = start.elapsed();

    // Phase 2: GPU 업로드 + 텍스처 + 머티리얼 (순차 — wgpu::Device 접근)
    let mut imported_models = Vec::new();
    let mut total_meshes = 0usize;
    let mut total_materials = 0usize;

    for pf in parsed {
        pf.report.log_summary();

        // ── 1. Skinned-only 모델 early-out ──
        // UE5.7: FNode::EType::MeshSkinned → SkeletalMeshFactory 별도 경로
        if pf.model.meshes.is_empty() {
            log::info!(
                "[AssetLoader] '{}' has no static meshes ({} skinned, {} skins) — skipping static pipeline",
                pf.file_stem, pf.model.skinned_meshes.len(), pf.model.skins.len()
            );
            continue;
        }

        // ── 2. Mesh → GPU upload + MeshAssets 등록 ──
        let (mesh_indices, newly_registered) = MeshFactory::create_from_model(
            device, &pf.model, mesh_assets, &pf.file_stem,
        );

        // relative_path 등록 (기존 로직 유지)
        if let Some(ref relative_path) = pf.relative_path {
            if pf.model.meshes.len() == 1 {
                if let Some(idx) = mesh_assets.get_index(&pf.file_stem) {
                    mesh_assets.name_to_index.insert(relative_path.clone(), idx);
                }
            }
        }

        // ── 3. 중복 모델 skip (모든 메시가 이미 등록됨) ──
        // DamagedHelmet: Phase 10.3에서 이미 로드 → newly_registered==0
        if newly_registered == 0 {
            log::info!(
                "[AssetLoader] '{}' all {} meshes already registered — skipping texture/material/spawn",
                pf.file_stem, mesh_indices.len()
            );
            continue;
        }

        // ── 4. Texture → bindless 등록 ──
        let handle_map = TextureFactory::create_from_model(
            device, queue, &pf.model, material_eval,
        );

        // ── 5. Material → GpuMaterial 생성 (bindless 핸들 포함) ──
        let gpu_materials = MaterialFactory::create_gpu_materials(&pf.model, &handle_map);

        total_meshes += newly_registered;  // 새로 등록된 것만 카운트
        total_materials += gpu_materials.len();

        imported_models.push(ImportedModel {
            name: pf.file_stem,
            model: pf.model,
            mesh_indices,
            gpu_materials,
        });
    }

    log::info!(
        "[AssetLoader] Loaded {} files, {} meshes, {} materials (parse: {:.1}ms, total: {:.1}ms)",
        imported_models.len(),
        total_meshes,
        total_materials,
        parse_elapsed.as_secs_f64() * 1000.0,
        start.elapsed().as_secs_f64() * 1000.0,
    );

    AssetImportResult {
        models: imported_models,
        total_meshes,
        total_materials,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_gltf_files() {
        // This test requires the assets folder to exist
        let assets_path = Path::new("assets");
        let files = scan_gltf_files(assets_path);
        log::info!("Found {} glTF files", files.len());
    }
}
