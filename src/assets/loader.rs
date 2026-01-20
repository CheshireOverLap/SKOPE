// Asset Loader - Scans and loads assets from assets/ folder
// Phase 9: glTF auto-loading support

use std::path::{Path, PathBuf};
use std::fs;
use wgpu::util::DeviceExt;

use crate::gltf_loader;
use crate::ecs_resources::{MeshAssets, MeshGpuData, MaterialAssets};
use crate::renderer::GpuVertex;

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

/// Load a glTF file and register its meshes to MeshAssets
/// Returns the number of meshes loaded
pub fn load_gltf_to_assets(
    gltf_path: &Path,
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    mesh_assets: &mut MeshAssets,
    _material_assets: &mut MaterialAssets,  // TODO: material 등록
) -> Result<usize, Box<dyn std::error::Error>> {
    let path_str = gltf_path.to_string_lossy().to_string();
    log::info!("[AssetLoader] Loading glTF: {}", path_str);

    // Load glTF model
    let model = gltf_loader::load_gltf(&path_str)?;

    // Get the file name (without extension) for mesh naming
    let file_stem = gltf_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    let mut loaded_count = 0;

    for (mesh_idx, mesh) in model.meshes.iter().enumerate() {
        // Convert to GpuVertex (64-byte stride) for V-Buffer and Shadow compatibility
        let gpu_vertices: Vec<GpuVertex> = mesh.vertices
            .iter()
            .map(GpuVertex::from_vertex)
            .collect();

        // Create GPU buffers
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Vertex Buffer {}", file_stem, mesh_idx)),
            contents: bytemuck::cast_slice(&gpu_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::STORAGE,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Index Buffer {}", file_stem, mesh_idx)),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let gpu_mesh = MeshGpuData {
            vertex_buffer,
            index_buffer,
            num_indices: mesh.indices.len() as u32,
        };

        // Register with name: "filename" or "filename_mesh0" if multiple meshes
        let mesh_name = if model.meshes.len() == 1 {
            file_stem.to_string()
        } else {
            format!("{}_{}", file_stem, mesh_idx)
        };

        // Skip if already registered (avoid duplicates with main.rs explicit loading)
        if mesh_assets.get_index(&mesh_name).is_some() {
            log::debug!("[AssetLoader] Skipping '{}' (already registered)", mesh_name);
            continue;
        }

        // Use default material (index 0) since asset_loader doesn't create GPU materials
        mesh_assets.register_with_material(&mesh_name, gpu_mesh, 0);
        loaded_count += 1;
    }

    // Also register with full relative path (e.g., "models/chair.gltf")
    // This allows .skope files to reference by path
    if let Some(relative_path) = get_relative_asset_path(gltf_path) {
        if model.meshes.len() == 1 {
            // Add alias with path
            if let Some(idx) = mesh_assets.get_index(file_stem) {
                mesh_assets.name_to_index.insert(relative_path, idx);
            }
        }
    }

    log::info!("[AssetLoader] Loaded {} meshes from {}", loaded_count, file_stem);
    Ok(loaded_count)
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

/// Load all glTF files from assets folder
pub fn load_all_assets(
    assets_path: &Path,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    mesh_assets: &mut MeshAssets,
    material_assets: &mut MaterialAssets,
) -> usize {
    let gltf_files = scan_gltf_files(assets_path);
    let mut total_loaded = 0;

    for gltf_path in gltf_files {
        match load_gltf_to_assets(&gltf_path, device, queue, mesh_assets, material_assets) {
            Ok(count) => total_loaded += count,
            Err(e) => {
                log::info!("[AssetLoader] Failed to load {:?}: {}", gltf_path, e);
            }
        }
    }

    log::info!("[AssetLoader] Total meshes loaded from assets/: {}", total_loaded);
    total_loaded
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
