//! Brush System - 이미지/컬러 스타일링 (언리얼 Slate의 FSlateBrush)
//!
//! 위젯 배경, 테두리 등을 이미지 또는 색상으로 그리기 위한 시스템입니다.

use super::{Color, Margin};
use glam::Vec2;

// ============================================================================
// TextureId
// ============================================================================

/// 텍스처 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureId(pub u64);

impl TextureId {
    /// 유효하지 않은 텍스처 ID
    pub const INVALID: Self = Self(0);

    /// 유효한지 확인
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

impl Default for TextureId {
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// BrushDrawType
// ============================================================================

/// 브러시 그리기 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushDrawType {
    /// 이미지 그대로 그리기
    #[default]
    Image,
    /// 9-slice 박스 (테두리 유지하며 늘리기)
    Box,
    /// 테두리만 그리기
    Border,
    /// 둥근 박스
    RoundedBox,
    /// 그리지 않음
    NoDrawType,
}

// ============================================================================
// BrushTiling
// ============================================================================

/// 브러시 타일링 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushTiling {
    /// 타일링 없음 (늘리기)
    #[default]
    NoTile,
    /// 수평 타일링
    Horizontal,
    /// 수직 타일링
    Vertical,
    /// 양방향 타일링
    Both,
}

// ============================================================================
// BrushMirroring
// ============================================================================

/// 브러시 미러링 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrushMirroring {
    /// 미러링 없음
    #[default]
    NoMirror,
    /// 수평 미러링
    Horizontal,
    /// 수직 미러링
    Vertical,
    /// 양방향 미러링
    Both,
}

// ============================================================================
// SlateBrush
// ============================================================================

/// 슬레이트 브러시 (이미지/컬러 스타일링)
///
/// 언리얼 Slate의 `FSlateBrush`에 해당합니다.
#[derive(Clone, PartialEq)]
pub enum SlateBrush {
    /// 단색 채우기
    Color(Color),

    /// 이미지 브러시
    Image {
        /// 텍스처 ID
        texture_id: TextureId,
        /// 원본 이미지 크기
        image_size: Vec2,
        /// 틴트 색상
        tint: Color,
        /// 그리기 타입
        draw_type: BrushDrawType,
        /// 9-slice 마진 (Box 타입용)
        margin: Margin,
        /// 타일링 모드
        tiling: BrushTiling,
        /// 미러링 모드
        mirroring: BrushMirroring,
    },

    /// 둥근 사각형
    RoundedBox {
        /// 채우기 색상
        fill_color: Color,
        /// 테두리 색상
        outline_color: Color,
        /// 테두리 두께
        outline_width: f32,
        /// 코너 반경
        corner_radius: CornerRadius,
    },

    /// 그라데이션
    Gradient {
        /// 시작 색상
        start_color: Color,
        /// 끝 색상
        end_color: Color,
        /// 방향 (각도, 0 = 오른쪽, 90 = 아래)
        angle: f32,
    },

    /// 테두리만
    Outline {
        /// 테두리 색상
        color: Color,
        /// 두께
        width: f32,
        /// 코너 반경
        corner_radius: f32,
    },

    /// 그리지 않음
    None,
}

impl Default for SlateBrush {
    fn default() -> Self {
        Self::None
    }
}

impl SlateBrush {
    /// 단색 브러시
    pub fn color(color: Color) -> Self {
        Self::Color(color)
    }

    /// 이미지 브러시 빌더
    pub fn image(texture_id: TextureId) -> SlateBrushBuilder {
        SlateBrushBuilder::new(texture_id)
    }

    /// 둥근 사각형 브러시
    pub fn rounded(fill_color: Color, corner_radius: f32) -> Self {
        Self::RoundedBox {
            fill_color,
            outline_color: Color::TRANSPARENT,
            outline_width: 0.0,
            corner_radius: CornerRadius::all(corner_radius),
        }
    }

    /// 둥근 사각형 + 테두리
    pub fn rounded_with_outline(
        fill_color: Color,
        outline_color: Color,
        outline_width: f32,
        corner_radius: f32,
    ) -> Self {
        Self::RoundedBox {
            fill_color,
            outline_color,
            outline_width,
            corner_radius: CornerRadius::all(corner_radius),
        }
    }

    /// 수평 그라데이션
    pub fn gradient_horizontal(start: Color, end: Color) -> Self {
        Self::Gradient {
            start_color: start,
            end_color: end,
            angle: 0.0,
        }
    }

    /// 수직 그라데이션
    pub fn gradient_vertical(start: Color, end: Color) -> Self {
        Self::Gradient {
            start_color: start,
            end_color: end,
            angle: 90.0,
        }
    }

    /// 테두리만 그리기
    pub fn outline(color: Color, width: f32) -> Self {
        Self::Outline {
            color,
            width,
            corner_radius: 0.0,
        }
    }

    /// 둥근 테두리
    pub fn outline_rounded(color: Color, width: f32, corner_radius: f32) -> Self {
        Self::Outline {
            color,
            width,
            corner_radius,
        }
    }

    /// 투명 (그리지 않음)
    pub fn none() -> Self {
        Self::None
    }

    /// 모든 색상에 opacity 적용
    pub fn apply_opacity(&mut self, opacity: f32) {
        match self {
            Self::Color(c) => c.a *= opacity,
            Self::Image { tint, .. } => tint.a *= opacity,
            Self::RoundedBox { fill_color, outline_color, .. } => {
                fill_color.a *= opacity;
                outline_color.a *= opacity;
            }
            Self::Gradient { start_color, end_color, .. } => {
                start_color.a *= opacity;
                end_color.a *= opacity;
            }
            Self::Outline { color, .. } => color.a *= opacity,
            Self::None => {}
        }
    }

    /// 그릴 내용이 있는지
    pub fn has_draw_content(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// 틴트/색상 가져오기 (렌더링용)
    pub fn get_tint(&self) -> Color {
        match self {
            Self::Color(c) => *c,
            Self::Image { tint, .. } => *tint,
            Self::RoundedBox { fill_color, .. } => *fill_color,
            Self::Gradient { start_color, .. } => *start_color,
            Self::Outline { color, .. } => *color,
            Self::None => Color::TRANSPARENT,
        }
    }
}

// ============================================================================
// CornerRadius
// ============================================================================

/// 코너 반경 (각 모서리별)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadius {
    /// 좌상단
    pub top_left: f32,
    /// 우상단
    pub top_right: f32,
    /// 우하단
    pub bottom_right: f32,
    /// 좌하단
    pub bottom_left: f32,
}

impl CornerRadius {
    /// 모든 코너 동일 (uniform)
    pub fn uniform(radius: f32) -> Self {
        Self::all(radius)
    }

    /// 모든 코너 동일
    pub fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// 코너 없음
    pub fn zero() -> Self {
        Self::all(0.0)
    }

    /// 상단만
    pub fn top(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: 0.0,
            bottom_left: 0.0,
        }
    }

    /// 하단만
    pub fn bottom(radius: f32) -> Self {
        Self {
            top_left: 0.0,
            top_right: 0.0,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    /// 좌측만
    pub fn left(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: 0.0,
            bottom_right: 0.0,
            bottom_left: radius,
        }
    }

    /// 우측만
    pub fn right(radius: f32) -> Self {
        Self {
            top_left: 0.0,
            top_right: radius,
            bottom_right: radius,
            bottom_left: 0.0,
        }
    }

    /// 개별 설정
    pub fn each(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    /// 최대 반경
    pub fn max(&self) -> f32 {
        self.top_left
            .max(self.top_right)
            .max(self.bottom_right)
            .max(self.bottom_left)
    }

    /// 모두 0인지
    pub fn is_zero(&self) -> bool {
        self.top_left == 0.0
            && self.top_right == 0.0
            && self.bottom_right == 0.0
            && self.bottom_left == 0.0
    }

    /// 모두 같은 값인지
    pub fn is_uniform(&self) -> bool {
        self.top_left == self.top_right
            && self.top_right == self.bottom_right
            && self.bottom_right == self.bottom_left
    }
}

impl Default for CornerRadius {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<f32> for CornerRadius {
    fn from(radius: f32) -> Self {
        Self::all(radius)
    }
}

// ============================================================================
// SlateBrushBuilder
// ============================================================================

/// 이미지 브러시 빌더
pub struct SlateBrushBuilder {
    texture_id: TextureId,
    image_size: Vec2,
    tint: Color,
    draw_type: BrushDrawType,
    margin: Margin,
    tiling: BrushTiling,
    mirroring: BrushMirroring,
}

impl SlateBrushBuilder {
    /// 새 빌더
    pub fn new(texture_id: TextureId) -> Self {
        Self {
            texture_id,
            image_size: Vec2::new(32.0, 32.0),
            tint: Color::WHITE,
            draw_type: BrushDrawType::Image,
            margin: Margin::zero(),
            tiling: BrushTiling::NoTile,
            mirroring: BrushMirroring::NoMirror,
        }
    }

    /// 이미지 크기 설정
    pub fn size(mut self, size: Vec2) -> Self {
        self.image_size = size;
        self
    }

    /// 틴트 색상
    pub fn tint(mut self, color: Color) -> Self {
        self.tint = color;
        self
    }

    /// 9-slice 박스 모드
    pub fn as_box(mut self, margin: Margin) -> Self {
        self.draw_type = BrushDrawType::Box;
        self.margin = margin;
        self
    }

    /// 테두리 모드
    pub fn as_border(mut self, margin: Margin) -> Self {
        self.draw_type = BrushDrawType::Border;
        self.margin = margin;
        self
    }

    /// 타일링 설정
    pub fn tiling(mut self, tiling: BrushTiling) -> Self {
        self.tiling = tiling;
        self
    }

    /// 수평 타일링
    pub fn tile_horizontal(mut self) -> Self {
        self.tiling = BrushTiling::Horizontal;
        self
    }

    /// 수직 타일링
    pub fn tile_vertical(mut self) -> Self {
        self.tiling = BrushTiling::Vertical;
        self
    }

    /// 양방향 타일링
    pub fn tile_both(mut self) -> Self {
        self.tiling = BrushTiling::Both;
        self
    }

    /// 미러링 설정
    pub fn mirroring(mut self, mirroring: BrushMirroring) -> Self {
        self.mirroring = mirroring;
        self
    }

    /// 빌드
    pub fn build(self) -> SlateBrush {
        SlateBrush::Image {
            texture_id: self.texture_id,
            image_size: self.image_size,
            tint: self.tint,
            draw_type: self.draw_type,
            margin: self.margin,
            tiling: self.tiling,
            mirroring: self.mirroring,
        }
    }
}

// ============================================================================
// ImageType — 이미지 리소스 타입
// ============================================================================

/// 이미지 리소스 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageType {
    /// 일반 이미지
    #[default]
    FullColor,
    /// 선형(Linear) 이미지 (감마 보정 없음)
    Linear,
    /// SDF 이미지 (Signed Distance Field)
    Sdf,
    /// MSDF 이미지 (Multi-channel SDF)
    Msdf,
}

// ============================================================================
// RoundingType — 라운딩 보간 타입
// ============================================================================

/// 둥근 코너 보간 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoundingType {
    /// 고정 반경
    #[default]
    Fixed,
    /// 상대적 (크기 비율)
    HalfHeight,
}

// ============================================================================
// UVRegion — UV 영역 선택
// ============================================================================

/// UV 영역 (텍스처 내 서브 영역 선택)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UVRegion {
    /// U 최소값 (0.0~1.0)
    pub u_min: f32,
    /// V 최소값
    pub v_min: f32,
    /// U 최대값
    pub u_max: f32,
    /// V 최대값
    pub v_max: f32,
}

impl Default for UVRegion {
    fn default() -> Self {
        Self::FULL
    }
}

impl UVRegion {
    /// 전체 텍스처
    pub const FULL: Self = Self {
        u_min: 0.0,
        v_min: 0.0,
        u_max: 1.0,
        v_max: 1.0,
    };

    /// 새 UV 영역
    pub fn new(u_min: f32, v_min: f32, u_max: f32, v_max: f32) -> Self {
        Self { u_min, v_min, u_max, v_max }
    }

    /// 픽셀 좌표에서 UV로 변환
    pub fn from_pixels(x: f32, y: f32, w: f32, h: f32, tex_w: f32, tex_h: f32) -> Self {
        Self {
            u_min: x / tex_w,
            v_min: y / tex_h,
            u_max: (x + w) / tex_w,
            v_max: (y + h) / tex_h,
        }
    }

    /// 전체 텍스처 영역인지
    pub fn is_full(&self) -> bool {
        (self.u_min - 0.0).abs() < f32::EPSILON
            && (self.v_min - 0.0).abs() < f32::EPSILON
            && (self.u_max - 1.0).abs() < f32::EPSILON
            && (self.v_max - 1.0).abs() < f32::EPSILON
    }

    /// UV 크기
    pub fn size(&self) -> (f32, f32) {
        (self.u_max - self.u_min, self.v_max - self.v_min)
    }
}

// ============================================================================
// DynamicImageBrush — 동적 이미지 브러시
// ============================================================================

/// 동적 이미지 브러시 (UE5 FDynamicImageBrush에 해당)
///
/// 런타임에 텍스처를 교체할 수 있는 브러시입니다.
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicImageBrush {
    /// 현재 텍스처 ID
    pub texture_id: TextureId,
    /// 이미지 크기
    pub image_size: Vec2,
    /// 틴트 색상
    pub tint: Color,
    /// UV 영역
    pub uv_region: UVRegion,
    /// 이미지 타입
    pub image_type: ImageType,
}

impl DynamicImageBrush {
    /// 새 동적 이미지 브러시
    pub fn new(texture_id: TextureId, size: Vec2) -> Self {
        Self {
            texture_id,
            image_size: size,
            tint: Color::WHITE,
            uv_region: UVRegion::FULL,
            image_type: ImageType::FullColor,
        }
    }

    /// 틴트 설정
    pub fn with_tint(mut self, tint: Color) -> Self {
        self.tint = tint;
        self
    }

    /// UV 영역 설정
    pub fn with_uv_region(mut self, region: UVRegion) -> Self {
        self.uv_region = region;
        self
    }

    /// 텍스처 교체
    pub fn set_texture(&mut self, texture_id: TextureId, size: Vec2) {
        self.texture_id = texture_id;
        self.image_size = size;
    }

    /// SlateBrush로 변환
    pub fn to_brush(&self) -> SlateBrush {
        SlateBrush::Image {
            texture_id: self.texture_id,
            image_size: self.image_size,
            tint: self.tint,
            draw_type: BrushDrawType::Image,
            margin: Margin::zero(),
            tiling: BrushTiling::NoTile,
            mirroring: BrushMirroring::NoMirror,
        }
    }
}

// ============================================================================
// Debug implementations
// ============================================================================

impl std::fmt::Debug for SlateBrush {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Color(c) => f.debug_tuple("Color").field(c).finish(),
            Self::Image { texture_id, .. } => {
                f.debug_struct("Image").field("texture_id", texture_id).finish()
            }
            Self::RoundedBox { fill_color, corner_radius, .. } => f
                .debug_struct("RoundedBox")
                .field("fill_color", fill_color)
                .field("corner_radius", corner_radius)
                .finish(),
            Self::Gradient { start_color, end_color, angle } => f
                .debug_struct("Gradient")
                .field("start", start_color)
                .field("end", end_color)
                .field("angle", angle)
                .finish(),
            Self::Outline { color, width, .. } => f
                .debug_struct("Outline")
                .field("color", color)
                .field("width", width)
                .finish(),
            Self::None => write!(f, "None"),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_brush() {
        let brush = SlateBrush::color(Color::RED);
        assert!(brush.has_draw_content());
        assert_eq!(brush.get_tint(), Color::RED);
    }

    #[test]
    fn test_rounded_brush() {
        let brush = SlateBrush::rounded(Color::BLUE, 8.0);
        assert!(brush.has_draw_content());

        if let SlateBrush::RoundedBox { corner_radius, .. } = brush {
            assert!(corner_radius.is_uniform());
            assert_eq!(corner_radius.max(), 8.0);
        } else {
            panic!("Expected RoundedBox");
        }
    }

    #[test]
    fn test_image_builder() {
        let brush = SlateBrush::image(TextureId(1))
            .size(Vec2::new(64.0, 64.0))
            .tint(Color::rgba(1.0, 1.0, 1.0, 0.5))
            .as_box(Margin::uniform(4.0))
            .build();

        if let SlateBrush::Image { draw_type, margin, .. } = brush {
            assert_eq!(draw_type, BrushDrawType::Box);
            assert_eq!(margin.left, 4.0);
        } else {
            panic!("Expected Image");
        }
    }

    #[test]
    fn test_corner_radius() {
        let cr = CornerRadius::all(10.0);
        assert!(cr.is_uniform());
        assert_eq!(cr.max(), 10.0);

        let cr = CornerRadius::top(5.0);
        assert!(!cr.is_uniform());
        assert_eq!(cr.top_left, 5.0);
        assert_eq!(cr.bottom_left, 0.0);
    }
}
