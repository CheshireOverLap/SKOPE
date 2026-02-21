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

/// 라이트 컴포넌트 (UE5 DirectionalLightComponent 호환)
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Light {
    pub light_type: LightType,
    pub intensity: f32,
    #[serde(with = "crate::vec3_serde")]
    pub color: Vec3,
    pub range: f32,
    pub spot_angle: f32,
    pub cast_shadows: bool,
    // === UE5 DirectionalLight fields ===
    /// 색온도 (Kelvin), 기본 6500
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// true면 temperature에서 color 대체
    #[serde(default)]
    pub use_temperature: bool,
    /// 태양 각직경 (도), 기본 0.5357
    #[serde(default = "default_light_source_angle")]
    pub light_source_angle: f32,
    /// 소프트 소스 각도 (도), 기본 0.0
    #[serde(default)]
    pub light_source_soft_angle: f32,
    /// 대기에 영향 줄 태양인지, 기본 true
    #[serde(default = "default_true")]
    pub atmosphere_sun_light: bool,
    /// 대기 태양 인덱스 (0 or 1)
    #[serde(default)]
    pub atmosphere_sun_light_index: u32,
    /// 스페큘러 기여도, 기본 1.0
    #[serde(default = "default_one")]
    pub specular_scale: f32,
    /// 디퓨즈 기여도 (UE: Indirect Lighting Intensity), 기본 1.0
    #[serde(default = "default_one")]
    pub diffuse_scale: f32,
    /// 그림자 강도 0-1, 기본 1.0
    #[serde(default = "default_one")]
    pub shadow_amount: f32,
    /// UE5 캐스케이드 분포 지수, 기본 3.0
    #[serde(default = "default_cascade_distribution_exponent")]
    pub cascade_distribution_exponent: f32,
    /// 동적 그림자 최대 거리, 기본 200.0
    #[serde(default = "default_dynamic_shadow_distance")]
    pub dynamic_shadow_distance: f32,
    /// 캐스케이드 개수, 기본 4
    #[serde(default = "default_shadow_cascade_count")]
    pub shadow_cascade_count: u32,
}

fn default_temperature() -> f32 { 6500.0 }
fn default_light_source_angle() -> f32 { 0.5357 }
fn default_true() -> bool { true }
fn default_one() -> f32 { 1.0 }
fn default_cascade_distribution_exponent() -> f32 { 3.0 }
fn default_dynamic_shadow_distance() -> f32 { 200.0 }
fn default_shadow_cascade_count() -> u32 { 4 }

impl Light {
    pub fn point(intensity: f32, color: Vec3) -> Self {
        Self {
            light_type: LightType::Point,
            intensity,
            color,
            range: 10.0,
            spot_angle: 0.0,
            cast_shadows: true,
            temperature: 6500.0,
            use_temperature: false,
            light_source_angle: 0.0,
            light_source_soft_angle: 0.0,
            atmosphere_sun_light: false,
            atmosphere_sun_light_index: 0,
            specular_scale: 1.0,
            diffuse_scale: 1.0,
            shadow_amount: 1.0,
            cascade_distribution_exponent: 3.0,
            dynamic_shadow_distance: 200.0,
            shadow_cascade_count: 0,
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
            temperature: 6500.0,
            use_temperature: false,
            light_source_angle: 0.0,
            light_source_soft_angle: 0.0,
            atmosphere_sun_light: false,
            atmosphere_sun_light_index: 0,
            specular_scale: 1.0,
            diffuse_scale: 1.0,
            shadow_amount: 1.0,
            cascade_distribution_exponent: 3.0,
            dynamic_shadow_distance: 200.0,
            shadow_cascade_count: 0,
        }
    }

    /// UE5-style Sun/Directional Light with sensible defaults
    pub fn sun(intensity: f32, color: Vec3) -> Self {
        Self {
            light_type: LightType::Sun,
            intensity,
            color,
            range: f32::INFINITY,
            spot_angle: 0.0,
            cast_shadows: true,
            // UE5 defaults
            temperature: 6500.0,
            use_temperature: false,
            light_source_angle: 0.5357,
            light_source_soft_angle: 0.0,
            atmosphere_sun_light: true,
            atmosphere_sun_light_index: 0,
            specular_scale: 1.0,
            diffuse_scale: 1.0,
            shadow_amount: 1.0,
            cascade_distribution_exponent: 3.0,
            dynamic_shadow_distance: 200.0,
            shadow_cascade_count: 4,
        }
    }
}

/// SunPositionDriver — NOAA 태양 위치 기반 자동 방향 업데이트
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SunPositionDriver {
    /// 위도 (기본 37.5665 = 서울)
    #[serde(default = "default_latitude")]
    pub latitude: f64,
    /// 경도 (기본 126.978 = 서울)
    #[serde(default = "default_longitude")]
    pub longitude: f64,
    /// 타임존 UTC 오프셋 (기본 9.0 = KST)
    #[serde(default = "default_timezone")]
    pub timezone: f64,
    /// 시간 0.0~24.0
    #[serde(default = "default_time_of_day")]
    pub time_of_day: f32,
    /// 1~365
    #[serde(default = "default_day_of_year")]
    pub day_of_year: u32,
    /// 기본 2024
    #[serde(default = "default_year")]
    pub year: i32,
    /// true면 실시간 진행
    #[serde(default)]
    pub auto_advance: bool,
    /// 시간 배속 (1.0 = 실시간)
    #[serde(default = "default_one_f32")]
    pub time_speed: f32,
}

fn default_latitude() -> f64 { 37.5665 }
fn default_longitude() -> f64 { 126.978 }
fn default_timezone() -> f64 { 9.0 }
fn default_time_of_day() -> f32 { 12.0 }
fn default_day_of_year() -> u32 { 172 } // ~June 21 (summer solstice)
fn default_year() -> i32 { 2024 }
fn default_one_f32() -> f32 { 1.0 }

impl Default for SunPositionDriver {
    fn default() -> Self {
        Self {
            latitude: 37.5665,
            longitude: 126.978,
            timezone: 9.0,
            time_of_day: 12.0,
            day_of_year: 172,
            year: 2024,
            auto_advance: false,
            time_speed: 1.0,
        }
    }
}
