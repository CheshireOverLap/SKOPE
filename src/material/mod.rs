//! SKOPE Material Instance System
//!
//! RON 기반 머티리얼 정의, 런타임 편집, GPU 동기화
//!
//! # 개요
//!
//! 이 모듈은 머티리얼을 파일로 정의하고 Inspector에서 편집할 수 있게 합니다.
//!
//! # 사용법
//!
//! ```rust,ignore
//! // MaterialRegistry 생성 및 로드
//! let mut registry = MaterialRegistry::new();
//! let loader = MaterialLoader::new("game/assets/materials");
//! loader.load_directory(&mut registry)?;
//!
//! // Inspector에서 편집 후 GPU 동기화
//! sync_materials_to_gpu(&mut registry, &pipeline, &queue);
//! ```
//!
//! # 파일 형식
//!
//! ```ron
//! // example.mat.ron
//! (
//!     name: "MetalPlate",
//!     base_color: (0.8, 0.8, 0.85, 1.0),
//!     metallic: 1.0,
//!     roughness: 0.3,
//!     textures: (
//!         albedo: Some("textures/metal_albedo.png"),
//!         normal: Some("textures/metal_normal.png"),
//!     ),
//! )
//! ```

pub mod material_def;
pub mod registry;
pub mod loader;
pub mod gpu_sync;

#[cfg(debug_assertions)]
pub mod hot_reload;

// Re-exports
pub use registry::{MaterialRegistry, MaterialTextureIndices};
pub use loader::MaterialLoader;
pub use gpu_sync::sync_materials_to_gpu;

#[cfg(debug_assertions)]
pub use hot_reload::MaterialHotReload;
