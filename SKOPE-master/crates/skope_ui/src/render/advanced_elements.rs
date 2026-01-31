//! 고급 렌더링 엘리먼트 — ShapedText, Viewport, PostProcess 엘리먼트
//!
//! 텍스트 쉐이핑 결과, 3D 뷰포트 임베딩, 포스트 프로세스 패스 등
//! 고급 드로우 엘리먼트 타입을 정의합니다.

use crate::core::{Color, PaintGeometry};

/// 고급 드로우 엘리먼트 타입
#[derive(Debug, Clone)]
pub enum AdvancedDrawElement {
    /// 쉐이핑된 텍스트 엘리먼트
    ShapedText(ShapedTextElement),
    /// 뷰포트 엘리먼트 (외부 렌더 타겟)
    Viewport(ViewportElement),
    /// 포스트 프로세스 엘리먼트
    PostProcess(PostProcessElement),
    /// 커스텀 드로우 콜백 (ID 기반)
    CustomDraw(CustomDrawElement),
    /// 커스텀 버텍스 데이터
    CustomVerts(CustomVertsElement),
}

/// 쉐이핑된 텍스트 엘리먼트
#[derive(Debug, Clone)]
pub struct ShapedTextElement {
    pub geometry: PaintGeometry,
    pub layer: u32,
    /// 글리프 위치/인덱스 데이터
    pub glyph_entries: Vec<ShapedGlyphPosition>,
    pub color: Color,
    pub font_size: f32,
    pub outline_color: Option<Color>,
    pub outline_width: f32,
    pub shadow_offset: glam::Vec2,
    pub shadow_color: Color,
}

/// 개별 글리프 위치
#[derive(Debug, Clone)]
pub struct ShapedGlyphPosition {
    pub codepoint: u32,
    pub cluster: u32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub font_atlas_index: u32,
}

impl ShapedTextElement {
    pub fn new(geometry: PaintGeometry, layer: u32, font_size: f32, color: Color) -> Self {
        Self {
            geometry,
            layer,
            glyph_entries: Vec::new(),
            color,
            font_size,
            outline_color: None,
            outline_width: 0.0,
            shadow_offset: glam::Vec2::ZERO,
            shadow_color: Color::TRANSPARENT,
        }
    }

    pub fn add_glyph(&mut self, glyph: ShapedGlyphPosition) {
        self.glyph_entries.push(glyph);
    }

    pub fn glyph_count(&self) -> usize { self.glyph_entries.len() }

    pub fn has_outline(&self) -> bool { self.outline_width > 0.0 && self.outline_color.is_some() }
    pub fn has_shadow(&self) -> bool {
        self.shadow_offset.x.abs() > 0.001 || self.shadow_offset.y.abs() > 0.001
    }

    /// 총 너비 (모든 글리프의 x_advance 합)
    pub fn total_width(&self) -> f32 {
        self.glyph_entries.iter().map(|g| g.x_advance).sum()
    }
}

/// 뷰포트 엘리먼트 — 외부 렌더 타겟 표시
#[derive(Debug, Clone)]
pub struct ViewportElement {
    pub geometry: PaintGeometry,
    pub layer: u32,
    pub render_target_id: u64,
    pub clear_color: Option<Color>,
    pub gamma_correct: bool,
    pub allow_scaling: bool,
}

impl ViewportElement {
    pub fn new(geometry: PaintGeometry, layer: u32, render_target_id: u64) -> Self {
        Self {
            geometry,
            layer,
            render_target_id,
            clear_color: None,
            gamma_correct: true,
            allow_scaling: true,
        }
    }

    pub fn with_clear_color(mut self, color: Color) -> Self {
        self.clear_color = Some(color);
        self
    }
}

/// 포스트 프로세스 엘리먼트 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostProcessType {
    /// 가우시안 블러
    GaussianBlur,
    /// 배경 블러 (frosted glass)
    BackgroundBlur,
    /// 색 보정
    ColorCorrection,
    /// 비네팅
    Vignette,
    /// 커스텀 셰이더
    Custom(u32),
}

/// 포스트 프로세스 엘리먼트
#[derive(Debug, Clone)]
pub struct PostProcessElement {
    pub geometry: PaintGeometry,
    pub layer: u32,
    pub process_type: PostProcessType,
    pub parameters: PostProcessParams,
}

/// 포스트 프로세스 파라미터
#[derive(Debug, Clone)]
pub struct PostProcessParams {
    /// 블러 강도
    pub blur_strength: f32,
    /// 블러 커널 크기
    pub blur_kernel_size: u32,
    /// 색 보정 인자 (brightness, contrast, saturation)
    pub color_adjust: [f32; 3],
    /// 비네팅 강도
    pub vignette_intensity: f32,
    /// 틴트 컬러
    pub tint_color: Color,
    /// 불투명도
    pub opacity: f32,
}

impl Default for PostProcessParams {
    fn default() -> Self {
        Self {
            blur_strength: 0.0,
            blur_kernel_size: 5,
            color_adjust: [1.0, 1.0, 1.0],
            vignette_intensity: 0.0,
            tint_color: Color::WHITE,
            opacity: 1.0,
        }
    }
}

impl PostProcessParams {
    pub fn blur(strength: f32) -> Self {
        Self {
            blur_strength: strength,
            ..Default::default()
        }
    }

    pub fn background_blur(strength: f32, tint: Color) -> Self {
        Self {
            blur_strength: strength,
            tint_color: tint,
            ..Default::default()
        }
    }
}

impl PostProcessElement {
    pub fn gaussian_blur(geometry: PaintGeometry, layer: u32, strength: f32) -> Self {
        Self {
            geometry,
            layer,
            process_type: PostProcessType::GaussianBlur,
            parameters: PostProcessParams::blur(strength),
        }
    }

    pub fn background_blur(geometry: PaintGeometry, layer: u32, strength: f32, tint: Color) -> Self {
        Self {
            geometry,
            layer,
            process_type: PostProcessType::BackgroundBlur,
            parameters: PostProcessParams::background_blur(strength, tint),
        }
    }
}

/// 커스텀 드로우 엘리먼트
#[derive(Debug, Clone)]
pub struct CustomDrawElement {
    pub geometry: PaintGeometry,
    pub layer: u32,
    pub draw_callback_id: u64,
    pub user_data: u64,
}

/// 커스텀 버텍스 엘리먼트
#[derive(Debug, Clone)]
pub struct CustomVertsElement {
    pub layer: u32,
    pub vertices: Vec<CustomVertex>,
    pub indices: Vec<u32>,
    pub texture_id: Option<u32>,
}

/// 커스텀 버텍스
#[derive(Debug, Clone, Copy)]
pub struct CustomVertex {
    pub position: glam::Vec2,
    pub tex_coord: glam::Vec2,
    pub color: Color,
}

impl CustomVertsElement {
    pub fn new(layer: u32) -> Self {
        Self {
            layer,
            vertices: Vec::new(),
            indices: Vec::new(),
            texture_id: None,
        }
    }

    pub fn add_vertex(&mut self, pos: glam::Vec2, uv: glam::Vec2, color: Color) -> u32 {
        let idx = self.vertices.len() as u32;
        self.vertices.push(CustomVertex {
            position: pos,
            tex_coord: uv,
            color,
        });
        idx
    }

    pub fn add_triangle(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend_from_slice(&[a, b, c]);
    }

    pub fn vertex_count(&self) -> usize { self.vertices.len() }
    pub fn index_count(&self) -> usize { self.indices.len() }
    pub fn triangle_count(&self) -> usize { self.indices.len() / 3 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn dummy_geom() -> PaintGeometry {
        PaintGeometry::new(Vec2::ZERO, Vec2::new(100.0, 50.0), 1.0)
    }

    #[test]
    fn test_shaped_text_element() {
        let mut elem = ShapedTextElement::new(dummy_geom(), 0, 16.0, Color::WHITE);
        elem.add_glyph(ShapedGlyphPosition {
            codepoint: 'A' as u32,
            cluster: 0,
            x_offset: 0.0, y_offset: 0.0,
            x_advance: 10.0, y_advance: 0.0,
            font_atlas_index: 0,
        });
        elem.add_glyph(ShapedGlyphPosition {
            codepoint: 'B' as u32,
            cluster: 1,
            x_offset: 0.0, y_offset: 0.0,
            x_advance: 9.0, y_advance: 0.0,
            font_atlas_index: 0,
        });
        assert_eq!(elem.glyph_count(), 2);
        assert!((elem.total_width() - 19.0).abs() < 0.01);
    }

    #[test]
    fn test_shaped_text_outline_shadow() {
        let mut elem = ShapedTextElement::new(dummy_geom(), 0, 16.0, Color::WHITE);
        assert!(!elem.has_outline());
        assert!(!elem.has_shadow());
        elem.outline_color = Some(Color::BLACK);
        elem.outline_width = 1.0;
        assert!(elem.has_outline());
        elem.shadow_offset = Vec2::new(2.0, 2.0);
        assert!(elem.has_shadow());
    }

    #[test]
    fn test_viewport_element() {
        let elem = ViewportElement::new(dummy_geom(), 0, 42)
            .with_clear_color(Color::BLACK);
        assert_eq!(elem.render_target_id, 42);
        assert!(elem.clear_color.is_some());
    }

    #[test]
    fn test_post_process_gaussian() {
        let elem = PostProcessElement::gaussian_blur(dummy_geom(), 0, 5.0);
        assert_eq!(elem.process_type, PostProcessType::GaussianBlur);
        assert_eq!(elem.parameters.blur_strength, 5.0);
    }

    #[test]
    fn test_post_process_background_blur() {
        let elem = PostProcessElement::background_blur(
            dummy_geom(), 0, 10.0,
            Color::rgba(0.0, 0.0, 0.0, 0.3),
        );
        assert_eq!(elem.process_type, PostProcessType::BackgroundBlur);
    }

    #[test]
    fn test_custom_verts_element() {
        let mut elem = CustomVertsElement::new(0);
        let v0 = elem.add_vertex(Vec2::new(0.0, 0.0), Vec2::ZERO, Color::WHITE);
        let v1 = elem.add_vertex(Vec2::new(100.0, 0.0), Vec2::new(1.0, 0.0), Color::WHITE);
        let v2 = elem.add_vertex(Vec2::new(50.0, 100.0), Vec2::new(0.5, 1.0), Color::WHITE);
        elem.add_triangle(v0, v1, v2);
        assert_eq!(elem.vertex_count(), 3);
        assert_eq!(elem.triangle_count(), 1);
    }

    #[test]
    fn test_advanced_draw_element_enum() {
        let elem = AdvancedDrawElement::ShapedText(
            ShapedTextElement::new(dummy_geom(), 0, 16.0, Color::WHITE),
        );
        match elem {
            AdvancedDrawElement::ShapedText(st) => assert_eq!(st.font_size, 16.0),
            _ => panic!("wrong variant"),
        }
    }
}
