//! Light Components for SKOPE Engine

use skope_ecs::prelude::*;
use glam::Vec3;

/// 라이트 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LightType {
    Point,
    Spot,
    Sun,
    Area,
}

/// 라이트 컴포넌트
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Light {
    pub light_type: LightType,
    pub intensity: f32,
    #[serde(with = "crate::vec3_serde")]
    pub color: Vec3,
    pub range: f32,
    pub spot_angle: f32,
    pub cast_shadows: bool,
}

impl Light {
    pub fn point(intensity: f32, color: Vec3) -> Self {
        Self {
            light_type: LightType::Point,
            intensity,
            color,
            range: 10.0,
            spot_angle: 0.0,
            cast_shadows: true,
        }
    }

    pub fn spot(intensity: f32, color: Vec3, angle: f32) -> Self {
        Self {
            light_type: LightType::Spot,
            intensity,
            color,
            range: 15.0,
            spot_angle: angle,
            cast_shadows: true,
        }
    }

    pub fn sun(intensity: f32, color: Vec3) -> Self {
        Self {
            light_type: LightType::Sun,
            intensity,
            color,
            range: f32::INFINITY,
            spot_angle: 0.0,
            cast_shadows: true,
        }
    }
}
