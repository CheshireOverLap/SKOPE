//! Lighting Components
//!
//! 라이트 관련 컴포넌트

use bevy_ecs::prelude::*;
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
    #[serde(with = "vec3_serde")]
    pub color: Vec3,
    pub range: f32,           // Point/Spot 전용
    pub spot_angle: f32,      // Spot 전용
    pub cast_shadows: bool,
}

// Vec3 직렬화 헬퍼
mod vec3_serde {
    use glam::Vec3;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(v: &Vec3, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        [v.x, v.y, v.z].serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec3, D::Error>
    where D: Deserializer<'de> {
        let arr: [f32; 3] = Deserialize::deserialize(deserializer)?;
        Ok(Vec3::new(arr[0], arr[1], arr[2]))
    }
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
