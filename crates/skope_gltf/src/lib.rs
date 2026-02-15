//! glTF/GLB 모델 로딩 크레이트
//!
//! 정적 메시, 스켈레탈 메시, 애니메이션, 머티리얼, 텍스처 로딩 지원
//!
//! ## 아키텍처
//! - Translator: glTF 파싱 → GltfIntermediate(IR) 변환
//! - Validator: IR 메시 검증 + 자동 수정 (노멀/탄젠트/인덱스 생성)
//! - Loader: IR → Model 변환 (하위 호환 레이어)
//!
//! ## 좌표계
//! - glTF 표준: Y-up, -Z forward (오른손 좌표계)
//! - SKOPE 엔진: Z-up, -Y forward (Blender와 동일)
//! - 로딩 시 자동 변환됨

#![allow(dead_code)]

pub mod types;
pub mod convert;
pub mod loader;
pub mod intermediate;
pub mod translator;
pub mod validator;
pub mod report;
pub mod payload;

// Re-export all public types
pub use types::*;
pub use loader::load_gltf;
pub use intermediate::GltfIntermediate;
pub use translator::{GltfTranslator, GltfTranslateError};
pub use validator::MeshValidator;
pub use report::ImportReport;
pub use payload::{PayloadProvider, LazyGltfTranslator};
