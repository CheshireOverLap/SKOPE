//! SKOPE Asset Management Module
//!
//! glTF 로딩, ECS 변환, 프리미티브 메시 생성 등을 담당

pub mod loader;
pub mod gltf_importer;
pub mod primitives;
pub mod skinned_loader;

// Re-export main types for convenience
pub use loader::{scan_gltf_files, load_gltf_to_assets, load_all_assets};
pub use gltf_importer::{register_gltf_materials, spawn_gltf_model, spawn_gltf_model_with_offset, spawn_gltf_model_with_materials};
pub use primitives::{create_cube, create_plane, create_sphere, create_cylinder};
pub use skinned_loader::{
    load_skinned_model, spawn_skinned_model, has_skinned_meshes,
    get_animation_names, get_animation_count, get_animation_duration,
    SkinnedLoadContext, SkinnedLoadError,
};
