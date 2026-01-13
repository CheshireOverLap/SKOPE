//! Environment settings for SKOPE Engine
//!
//! Ambient light, sky, and fog configuration.

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

/// 앰비언트 라이트 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbientLight {
    pub color: [f32; 3],
    pub intensity: f32,
}

impl Default for AmbientLight {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            intensity: 0.15,
        }
    }
}

/// 스카이 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkySettings {
    /// 그라데이션 색상
    Gradient {
        top: [f32; 3],
        bottom: [f32; 3],
    },
    /// HDRI 환경맵
    Hdri {
        path: String,
        intensity: f32,
    },
    /// 절차적 하늘
    Procedural {
        sun_size: f32,
        atmosphere: bool,
    },
    /// 단색
    SolidColor([f32; 3]),
}

impl Default for SkySettings {
    fn default() -> Self {
        Self::Gradient {
            top: [0.05, 0.15, 0.4],
            bottom: [0.15, 0.25, 0.45],
        }
    }
}

/// 안개 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FogSettings {
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
    pub density: f32,
}

impl Default for FogSettings {
    fn default() -> Self {
        Self {
            color: [0.5, 0.6, 0.7],
            start: 20.0,
            end: 100.0,
            density: 0.02,
        }
    }
}

/// 씬 환경 리소스
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub ambient: AmbientLight,
    pub sky: SkySettings,
    pub fog: Option<FogSettings>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            ambient: AmbientLight::default(),
            sky: SkySettings::default(),
            fog: None,
        }
    }
}

impl Environment {
    /// 밝은 실외 환경
    pub fn outdoor() -> Self {
        Self {
            ambient: AmbientLight {
                color: [0.6, 0.7, 1.0],
                intensity: 0.3,
            },
            sky: SkySettings::Procedural {
                sun_size: 0.05,
                atmosphere: true,
            },
            fog: Some(FogSettings {
                color: [0.7, 0.8, 0.9],
                start: 50.0,
                end: 200.0,
                density: 0.01,
            }),
        }
    }

    /// 어두운 실내 환경
    pub fn indoor() -> Self {
        Self {
            ambient: AmbientLight {
                color: [1.0, 0.9, 0.8],
                intensity: 0.1,
            },
            sky: SkySettings::SolidColor([0.02, 0.02, 0.02]),
            fog: None,
        }
    }

    /// 안개 추가
    pub fn with_fog(mut self, fog: FogSettings) -> Self {
        self.fog = Some(fog);
        self
    }
}
