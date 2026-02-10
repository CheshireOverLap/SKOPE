//! SKOPE Splash Screen Module
//!
//! 엔진 시작 시 로딩 화면을 표시하는 모듈

#![allow(clippy::too_many_arguments)]

mod loading;
mod renderer;
mod text_renderer;

pub use loading::InitStage;
pub use renderer::SplashRenderer;
