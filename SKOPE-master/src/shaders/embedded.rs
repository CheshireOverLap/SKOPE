//! 셰이더 임베딩 모듈
//!
//! build.rs에서 생성된 셰이더 소스를 포함합니다.
//! Release 빌드에서 파일 시스템 없이 셰이더를 로드할 수 있습니다.

use super::shader_id::ShaderId;

// build.rs에서 생성된 임베딩 파일 포함
include!(concat!(env!("OUT_DIR"), "/shaders_embedded.rs"));
