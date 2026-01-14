//! SKOPE Splash Screen Module
//!
//! 엔진 시작 시 로딩 화면을 표시하는 모듈

#![allow(clippy::too_many_arguments)]

mod loading;
mod renderer;
mod text_renderer;

#[allow(unused_imports)]
pub use loading::{InitStage, InitContext};
pub use renderer::SplashRenderer;
pub use text_renderer::SplashTextRenderer;
