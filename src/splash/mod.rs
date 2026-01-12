//! SKOPE Splash Screen Module
//!
//! 엔진 시작 시 로딩 화면을 표시하는 모듈

mod loading;
mod renderer;

pub use loading::{InitStage, InitContext};
pub use renderer::SplashRenderer;
