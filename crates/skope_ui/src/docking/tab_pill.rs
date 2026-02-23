//! 탭 pill 공통 렌더링 — UE5 SDockTab::OnPaint 대응
//!
//! MajorTabBar와 SDockingTabStack에서 공유하는 탭 pill 렌더링 로직.

use glam::Vec2;
use crate::core::{Color, CornerRadius, FontFamily, PaintGeometry, SlateBrush};
use crate::render::text_renderer::TextMeasurer;
use crate::widget::{DrawElementList, ImageScaling};

/// 개별 탭 pill 렌더링 파라미터 (UE5 SDockTab::OnPaint 대응)
pub struct TabPillParams<'a> {
    // 위치/크기
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    // 콘텐츠
    pub title: &'a str,
    pub icon: Option<&'a str>,
    // 상태
    pub show_close: bool,
    pub is_close_hovered: bool,
    // 색상 — 호출자가 alpha를 미리 적용
    pub bg_color: Color,
    pub text_color: Color,
    pub icon_tint: Color,
    pub close_icon_color: Color,
    pub close_hover_brush: Option<&'a SlateBrush>,
    // 크기 파라미터
    pub icon_size: f32,
    pub icon_margin: f32,
    pub close_size: f32,
    pub font_size: f32,
    pub close_margin: f32,
    // 렌더링
    pub scale: f32,
    /// 내부 생성 색상에 적용할 alpha (닫기 버튼 등)
    pub alpha: f32,
}

/// 텍스트 너비 측정 (TextMeasurer 사용, 폴백: 고정 비율)
///
/// MajorTabBar·SDockingTabStack 공통.
pub fn measure_tab_text(text: &str, font_size: f32, scale: f32) -> f32 {
    if let Ok(m) = TextMeasurer::instance().read() {
        m.measure_width(text, font_size, FontFamily::UI, scale)
    } else {
        text.chars().count() as f32 * font_size * scale * 0.5
    }
}

/// 개별 탭 pill 렌더링 (공용)
///
/// 반환: 사용한 레이어 수 (호출자가 layer + 반환값으로 다음 레이어 산정)
pub fn paint_tab_pill(
    params: &TabPillParams,
    draw_elements: &mut DrawElementList,
    layer: u32,
) -> u32 {
    let TabPillParams {
        x, y, width, height,
        title, icon,
        show_close, is_close_hovered,
        bg_color, text_color, icon_tint, close_icon_color, close_hover_brush,
        icon_size, icon_margin, close_size, font_size, close_margin,
        scale, alpha,
    } = params;

    // 1. pill 배경 (rounded rect, radius = height/2)
    if bg_color.a > 0.001 {
        let pill_radius = height * 0.5;
        let tab_geo = PaintGeometry::new(
            Vec2::new(*x, *y),
            Vec2::new(*width, *height),
            *scale,
        );
        draw_elements.add_rounded_box(
            layer,
            tab_geo,
            *bg_color,
            Color::TRANSPARENT,
            0.0,
            CornerRadius::uniform(pill_radius),
        );
    }

    // 2. 아이콘 + 텍스트 — pill 전체 기준 가운데 정렬 (닫기 버튼 무시)
    let icon_space = if icon.is_some() { icon_size + icon_margin } else { 0.0 };

    // 텍스트 말줄임 — 사용 가능한 최대 텍스트 너비 계산
    let max_text_width = width - icon_space - close_size - close_margin;
    let font_px = font_size * scale;
    let max_chars = (max_text_width / (6.0 * scale)).max(1.0) as usize;
    let char_count = title.chars().count();
    let display_title = if char_count > max_chars && max_chars > 3 {
        let truncated: String = title.chars().take(max_chars - 3).collect();
        format!("{}...", truncated)
    } else {
        title.to_string()
    };

    let text_w = measure_tab_text(&display_title, *font_size, *scale);
    let content_width = icon_space + text_w;
    let mut content_x = x + (width - content_width) * 0.5;

    if let Some(icon_path) = icon {
        draw_elements.add_image(
            layer + 1,
            PaintGeometry::new(
                Vec2::new(content_x, y + (height - icon_size) * 0.5),
                Vec2::new(*icon_size, *icon_size),
                *scale,
            ),
            icon_path.to_string(),
            *icon_tint,
            ImageScaling::Fit,
        );
        content_x += icon_size + icon_margin;
    }

    draw_elements.add_text(
        layer + 1,
        PaintGeometry::new(
            Vec2::new(content_x, y + (height - font_px) * 0.5),
            Vec2::new(max_text_width.max(0.0), font_px),
            *scale,
        ),
        display_title,
        *text_color,
        *font_size,
    );

    // 3. 닫기 버튼 (show_close=true일 때만)
    if *show_close {
        let close_x = x + width - close_margin - close_size;
        let close_y = y + (height - close_size) * 0.5;

        // 호버 하이라이트
        if *is_close_hovered {
            if let Some(brush) = close_hover_brush {
                let close_geo = PaintGeometry::new(
                    Vec2::new(close_x, close_y),
                    Vec2::new(*close_size, *close_size),
                    *scale,
                );
                draw_elements.add_brush(layer + 2, close_geo, brush);
            }
        }

        // 닫기 아이콘
        let close_color = Color::rgba(
            close_icon_color.r,
            close_icon_color.g,
            close_icon_color.b,
            close_icon_color.a * alpha,
        );
        draw_elements.add_image(
            layer + 3,
            PaintGeometry::new(
                Vec2::new(close_x + 1.0, close_y),
                Vec2::new(*close_size, *close_size),
                *scale,
            ),
            "titlebar/_Titlebar_x.png".to_string(),
            close_color,
            ImageScaling::Fit,
        );
    }

    // 반환: 사용한 레이어 수 (pill 1 + content 1 + close hover 1 + close icon 1 + 여유 1)
    5
}
