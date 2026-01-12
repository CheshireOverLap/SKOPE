//! Post Processing Components
//!
//! 포스트 프로세싱 관련 컴포넌트

use bevy_ecs::prelude::*;

/// 톤매핑 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Tonemapping {
    #[default]
    Aces,
    Reinhard,
    Filmic,
    None,
}

/// 블룸 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BloomSettings {
    pub intensity: f32,
    pub threshold: f32,
    pub knee: f32,
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            threshold: 1.0,
            knee: 0.5,
        }
    }
}

/// 아웃라인 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutlineSettings {
    pub color: [f32; 3],
    pub strength: f32,
}

impl Default for OutlineSettings {
    fn default() -> Self {
        Self {
            color: [0.02, 0.01, 0.01],
            strength: 0.7,
        }
    }
}

/// 포스트 프로세스 설정 컴포넌트 (카메라에 붙임)
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostProcess {
    pub exposure: f32,
    pub gamma: f32,
    pub tonemapping: Tonemapping,
    pub bloom: Option<BloomSettings>,
    pub outline: Option<OutlineSettings>,
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for PostProcess {
    fn default() -> Self {
        Self {
            exposure: 1.5,
            gamma: 2.2,
            tonemapping: Tonemapping::Aces,
            bloom: None,
            outline: Some(OutlineSettings::default()),
            saturation: 1.0,
            contrast: 1.0,
        }
    }
}

impl PostProcess {
    /// 기본 설정 (블룸 + 아웃라인)
    pub fn with_bloom(mut self) -> Self {
        self.bloom = Some(BloomSettings::default());
        self
    }

    /// 노출값 설정
    pub fn with_exposure(mut self, exposure: f32) -> Self {
        self.exposure = exposure;
        self
    }

    /// 아웃라인 비활성화
    pub fn without_outline(mut self) -> Self {
        self.outline = None;
        self
    }
}
