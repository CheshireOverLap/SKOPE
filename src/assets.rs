//! SKOPE Asset Management Module
//!
//! glTF 로딩, ECS 변환, 프리미티브 메시 생성 등을 담당

pub mod loader;
pub mod gltf_importer;
pub mod primitives;

// Re-export main types for convenience
pub use loader::load_all_assets;
pub use primitives::{create_cube, create_plane, create_sphere, create_cylinder, create_cone, create_arrow};
