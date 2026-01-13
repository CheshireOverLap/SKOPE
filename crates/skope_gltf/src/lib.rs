//! glTF/GLB 모델 로딩 크레이트
//!
//! 정적 메시, 스켈레탈 메시, 애니메이션, 머티리얼, 텍스처 로딩 지원
//!
//! ## 좌표계
//! - glTF 표준: Y-up, -Z forward (오른손 좌표계)
//! - SKOPE 엔진: Z-up, -Y forward (Blender와 동일)
//! - 로딩 시 자동 변환됨

#![allow(dead_code)]

pub mod types;
pub mod convert;
pub mod loader;

// Re-export all public types
pub use types::*;
pub use loader::load_gltf;
