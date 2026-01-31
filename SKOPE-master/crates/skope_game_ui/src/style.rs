// SKOPE UI - Styling System
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use crate::types::*;

/// 스타일 정의
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Style {
    /// 배경색
    #[serde(default)]
    pub background_color: Option<Color>,

    /// 배경 이미지
    #[serde(default)]
    pub background_image: Option<String>,

    /// 테두리 색상
    #[serde(default)]
    pub border_color: Option<Color>,

    /// 테두리 두께
    #[serde(default)]
    pub border_width: f32,

    /// 테두리 반경 (둥근 모서리)
    #[serde(default)]
    pub border_radius: f32,

    /// 불투명도 (0.0-1.0)
    #[serde(default = "default_opacity")]
    pub opacity: f32,

    /// 스케일
    #[serde(default = "default_scale")]
    pub scale: (f32, f32),

    /// 회전 (라디안)
    #[serde(default)]
    pub rotation: f32,

    /// 텍스트 색상
    #[serde(default)]
    pub text_color: Option<Color>,

    /// 폰트 크기
    #[serde(default)]
    pub font_size: Option<f32>,

    /// 폰트 패밀리
    #[serde(default)]
    pub font_family: Option<String>,

    /// 텍스트 정렬
    #[serde(default)]
    pub text_align: TextAlign,

    /// 줄 높이
    #[serde(default)]
    pub line_height: Option<f32>,

    /// 그림자
    #[serde(default)]
    pub shadow: Option<Shadow>,

    /// 틴트 색상 (이미지에 적용)
    #[serde(default)]
    pub tint: Option<Color>,

    /// 블러 효과
    #[serde(default)]
    pub blur: f32,
}

fn default_opacity() -> f32 {
    1.0
}

fn default_scale() -> (f32, f32) {
    (1.0, 1.0)
}

/// 텍스트 정렬
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// 그림자 설정
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub color: Color,
}

impl Default for Shadow {
    fn default() -> Self {
        Self {
            offset_x: 2.0,
            offset_y: 2.0,
            blur_radius: 4.0,
            color: Color::Rgba(0.0, 0.0, 0.0, 0.5),
        }
    }
}

/// 스타일 병합 (자식이 부모 스타일 상속)
impl Style {
    pub fn merge_with(&self, parent: &Style) -> Style {
        Style {
            background_color: self.background_color.or(parent.background_color),
            background_image: self.background_image.clone().or(parent.background_image.clone()),
            border_color: self.border_color.or(parent.border_color),
            border_width: if self.border_width != 0.0 { self.border_width } else { parent.border_width },
            border_radius: if self.border_radius != 0.0 { self.border_radius } else { parent.border_radius },
            opacity: self.opacity * parent.opacity,
            scale: (self.scale.0 * parent.scale.0, self.scale.1 * parent.scale.1),
            rotation: self.rotation + parent.rotation,
            text_color: self.text_color.or(parent.text_color),
            font_size: self.font_size.or(parent.font_size),
            font_family: self.font_family.clone().or(parent.font_family.clone()),
            text_align: self.text_align,
            line_height: self.line_height.or(parent.line_height),
            shadow: self.shadow.clone().or(parent.shadow.clone()),
            tint: self.tint.or(parent.tint),
            blur: if self.blur != 0.0 { self.blur } else { parent.blur },
        }
    }

    /// 두 스타일 사이 보간 (애니메이션용)
    pub fn lerp(&self, other: &Style, t: f32) -> Style {
        Style {
            background_color: lerp_color_opt(self.background_color, other.background_color, t),
            background_image: if t < 0.5 { self.background_image.clone() } else { other.background_image.clone() },
            border_color: lerp_color_opt(self.border_color, other.border_color, t),
            border_width: lerp_f32(self.border_width, other.border_width, t),
            border_radius: lerp_f32(self.border_radius, other.border_radius, t),
            opacity: lerp_f32(self.opacity, other.opacity, t),
            scale: (
                lerp_f32(self.scale.0, other.scale.0, t),
                lerp_f32(self.scale.1, other.scale.1, t),
            ),
            rotation: lerp_f32(self.rotation, other.rotation, t),
            text_color: lerp_color_opt(self.text_color, other.text_color, t),
            font_size: lerp_f32_opt(self.font_size, other.font_size, t),
            font_family: if t < 0.5 { self.font_family.clone() } else { other.font_family.clone() },
            text_align: if t < 0.5 { self.text_align } else { other.text_align },
            line_height: lerp_f32_opt(self.line_height, other.line_height, t),
            shadow: if t < 0.5 { self.shadow.clone() } else { other.shadow.clone() },
            tint: lerp_color_opt(self.tint, other.tint, t),
            blur: lerp_f32(self.blur, other.blur, t),
        }
    }
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_f32_opt(a: Option<f32>, b: Option<f32>, t: f32) -> Option<f32> {
    match (a, b) {
        (Some(a), Some(b)) => Some(lerp_f32(a, b, t)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn lerp_color_opt(a: Option<Color>, b: Option<Color>, t: f32) -> Option<Color> {
    match (a, b) {
        (Some(a), Some(b)) => Some(lerp_color(a, b, t)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let a_rgba = a.to_rgba();
    let b_rgba = b.to_rgba();
    Color::Rgba(
        lerp_f32(a_rgba[0], b_rgba[0], t),
        lerp_f32(a_rgba[1], b_rgba[1], t),
        lerp_f32(a_rgba[2], b_rgba[2], t),
        lerp_f32(a_rgba[3], b_rgba[3], t),
    )
}

/// 테마 시스템
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Theme {
    /// 테마 이름
    pub name: String,

    /// 색상 변수들
    #[serde(default)]
    pub colors: std::collections::HashMap<String, Color>,

    /// 폰트 변수들
    #[serde(default)]
    pub fonts: std::collections::HashMap<String, String>,

    /// 크기 변수들
    #[serde(default)]
    pub sizes: std::collections::HashMap<String, f32>,

    /// 스타일 프리셋들
    #[serde(default)]
    pub styles: std::collections::HashMap<String, Style>,
}

impl Theme {
    /// 색상 변수 가져오기
    pub fn get_color(&self, name: &str) -> Option<Color> {
        self.colors.get(name).copied()
    }

    /// 폰트 변수 가져오기
    pub fn get_font(&self, name: &str) -> Option<&String> {
        self.fonts.get(name)
    }

    /// 크기 변수 가져오기
    pub fn get_size(&self, name: &str) -> Option<f32> {
        self.sizes.get(name).copied()
    }

    /// 스타일 프리셋 가져오기
    pub fn get_style(&self, name: &str) -> Option<&Style> {
        self.styles.get(name)
    }
}
