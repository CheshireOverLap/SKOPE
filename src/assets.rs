//! SKOPE Asset Management Module
//!
//! glTF 로딩, ECS 변환, 프리미티브 메시 생성 등을 담당

pub mod loader;
pub mod gltf_importer;
pub mod primitives;
pub mod skinned_loader;

// Re-export main types for convenience
#[allow(unused_imports)]
pub use loader::{load_gltf_to_assets, load_all_assets};
#[allow(unused_imports)]
pub use gltf_importer::{spawn_gltf_model, spawn_gltf_model_with_offset};
#[allow(unused_imports)]
pub use primitives::{create_cube, create_plane, create_sphere, create_cylinder, create_cone, create_arrow};
#[allow(unused_imports)]
pub use skinned_loader::{
    load_skinned_model, spawn_skinned_model, has_skinned_meshes,
    SkinnedLoadContext,
};
