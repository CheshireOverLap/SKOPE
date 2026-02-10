//! SKOPE Shader System
//!
//! 셰이더 전처리, 로딩, 관리 시스템
//!
//! 현재 렌더러는 빌드 시점 임베딩 사용 - 런타임 ShaderManager 연동 예정

#![allow(dead_code)]
//!
//! # 사용법
//!
//! ```rust
//! use crate::shaders::ShaderManager;
//!
//! let mut manager = ShaderManager::new(device.clone(), "src/shaders");
//!
//! // 파일에서 로드 (#include 지원)
//! let shader = manager.load("material_eval", "passes/material_eval.wgsl")?;
//!
//! // 인라인 소스로 로드
//! let shader = manager.load_inline("simple", include_str!("simple.wgsl"))?;
//! ```
//!
//! # 셰이더 구조
//!
//! ```text
//! src/shaders/
//! ├── common/           # 공유 코드
//! │   ├── structs.wgsl  # 공통 구조체
//! │   ├── pbr.wgsl      # PBR 함수
//! │   └── lighting.wgsl # 조명 함수
//! │
//! ├── passes/           # 렌더 패스
//! │   ├── visibility.wgsl
//! │   └── material_eval.wgsl
//! │
//! └── editor/           # 에디터 전용
//!     ├── grid.wgsl
//!     └── gizmo.wgsl
//! ```
//!
//! # Include 문법
//!
//! ```wgsl
//! // 상대 경로 (현재 파일 기준)
//! #include "common/structs.wgsl"
//!
//! // 절대 경로 (base_path 기준)
//! #include <common/pbr.wgsl>
//! ```

mod preprocessor;
mod manager;
mod watcher;
mod hot_reload;
mod shader_id;
mod embedded;

pub use manager::ShaderManager;
pub use hot_reload::ShaderHotReload;
