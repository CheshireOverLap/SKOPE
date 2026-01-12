//! SKOPE Material Definition
//!
//! RON 직렬화 가능한 머티리얼 정의 구조체

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 머티리얼 정의 (RON 직렬화 가능)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaterialDef {
    /// 머티리얼 고유 이름
    pub name: String,

    /// 기본 색상 (RGBA, 0.0-1.0)
    #[serde(default = "default_base_color")]
    pub base_color: [f32; 4],

    /// 금속도 (0.0 = 비금속, 1.0 = 금속)
    #[serde(default)]
    pub metallic: f32,

    /// 거칠기 (0.0 = 매끄러움, 1.0 = 거침)
    #[serde(default = "default_roughness")]
    pub roughness: f32,

    /// 발광 강도
    #[serde(default)]
    pub emissive_strength: f32,

    /// 노멀 맵 스케일
    #[serde(default = "default_normal_scale")]
    pub normal_scale: f32,

    /// UV 스케일 (텍스처 타일링)
    /// uv_mode=0: None = [1.0, 1.0], Some([x, y]) = x/y배 타일링
    /// uv_mode=1: [x, y] = 텍스처가 커버하는 월드 단위 (미터)
    #[serde(default)]
    pub uv_scale: Option<[f32; 2]>,

    /// UV 모드
    /// 0 = mesh UV 사용 (기본)
    /// 1 = world XZ 사용 (프로토타이핑 그리드용)
    #[serde(default)]
    pub uv_mode: u32,

    /// 텍스처 경로들 (RON 파일 기준 상대 경로)
    #[serde(default)]
    pub textures: MaterialTextures,
}

/// 머티리얼 텍스처 경로
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MaterialTextures {
    /// 알베도(기본 색상) 텍스처
    #[serde(default)]
    pub albedo: Option<PathBuf>,

    /// 노멀 맵 텍스처
    #[serde(default)]
    pub normal: Option<PathBuf>,

    /// Metallic-Roughness 텍스처 (glTF 스타일: G=roughness, B=metallic)
    #[serde(default)]
    pub metallic_roughness: Option<PathBuf>,

    /// 발광 텍스처
    #[serde(default)]
    pub emissive: Option<PathBuf>,
}

// ============ Default 함수들 ============

fn default_base_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_roughness() -> f32 {
    0.5
}

fn default_normal_scale() -> f32 {
    1.0
}

impl Default for MaterialDef {
    fn default() -> Self {
        Self {
            name: "Untitled".to_string(),
            base_color: default_base_color(),
            metallic: 0.0,
            roughness: default_roughness(),
            emissive_strength: 0.0,
            normal_scale: default_normal_scale(),
            uv_scale: None,
            uv_mode: 0,
            textures: MaterialTextures::default(),
        }
    }
}

impl MaterialDef {
    /// 새 머티리얼 생성
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// 이름으로 기본 머티리얼 생성
    pub fn with_name(name: impl Into<String>) -> Self {
        Self::new(name)
    }

    /// 색상 설정 (빌더 패턴)
    pub fn with_base_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.base_color = [r, g, b, a];
        self
    }

    /// 금속도 설정
    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.metallic = metallic.clamp(0.0, 1.0);
        self
    }

    /// 거칠기 설정
    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness.clamp(0.0, 1.0);
        self
    }

    /// RON 문자열로 직렬화
    pub fn to_ron_string(&self) -> Result<String, ron::Error> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_material() {
        let mat = MaterialDef::default();
        assert_eq!(mat.name, "Untitled");
        assert_eq!(mat.base_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(mat.metallic, 0.0);
        assert_eq!(mat.roughness, 0.5);
    }

    #[test]
    fn test_builder_pattern() {
        let mat = MaterialDef::new("Metal")
            .with_base_color(0.8, 0.8, 0.8, 1.0)
            .with_metallic(1.0)
            .with_roughness(0.3);

        assert_eq!(mat.name, "Metal");
        assert_eq!(mat.metallic, 1.0);
        assert_eq!(mat.roughness, 0.3);
    }

    #[test]
    fn test_ron_serialization() {
        let mat = MaterialDef::new("TestMat");
        let ron_str = mat.to_ron_string().unwrap();
        assert!(ron_str.contains("TestMat"));
    }
}
