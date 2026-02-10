//! MajorTabBar — 타이틀바에 위치하는 MajorTab 바
//!
//! 언리얼 SDockingTabStack(bShowingTitleBarArea=true) 대응

use glam::Vec2;
use crate::core::{Color, PaintGeometry, SlateBrush, WindowZone};
use crate::widget::{DrawElementList, ImageScaling};

/// MajorTab 바 스타일
#[derive(Debug, Clone)]
pub struct MajorTabBarStyle {
    pub height: f32,
    pub tab_max_width: f32,
    pub tab_min_width: f32,
    /// UE5 MajorTab TabPadding.Left
    pub tab_left_pad: f32,
    /// UE5 MajorTab TabPadding.Right
    pub tab_right_pad: f32,
    pub tab_spacing: f32,
    /// UE5 FDockTabStyle::IconSize (16x16)
    pub icon_size: f32,
    /// UE5 아이콘 슬롯 Padding(0,0,5,0)
    pub icon_right_margin: f32,
    /// UE5 close button Icon16x16
    pub close_size: f32,
    /// 배경 브러시
    pub background_brush: SlateBrush,
    /// 활성 탭 브러시
    pub active_brush: SlateBrush,
    /// 호버 탭 브러시
    pub hover_brush: SlateBrush,
    /// 비활성 탭 브러시
    pub inactive_brush: SlateBrush,
    pub text_color: Color,
    pub active_text_color: Color,
    /// 악센트 브러시 (활성 탭 하단 하이라이트)
    pub accent_brush: SlateBrush,
    /// 닫기 버튼 호버 브러시
    pub close_button_hovered: SlateBrush,
}

impl Default for MajorTabBarStyle {
    fn default() -> Self {
        let bg = Color::rgba(0.082, 0.082, 0.082, 1.0);       // Background #151515
        let active = Color::rgba(0.141, 0.141, 0.141, 1.0);   // Panel #242424 (ForegroundBrush)
        let hover = Color::rgba(0.141, 0.141, 0.141, 0.8);    // Panel #242424 @ 80% (HoveredBrush)
        let accent = Color::rgba(0.0, 0.439, 0.878, 1.0);     // Primary #0070E0
        Self {
            height: 30.0,          // 커스텀 MajorTab 높이
            tab_max_width: 210.0,  // UE5 MaxMajorTabSize.X = 210px
            tab_min_width: 100.0,
            tab_left_pad: 4.0,     // UE5 MajorTab TabPadding.Left = 4
            tab_right_pad: 10.0,   // UE5 MajorTab TabPadding.Right = 10
            tab_spacing: 2.0,      // UE5 OverlapWidth=-2.0 → 2px gap
            icon_size: 16.0,       // UE5 FDockTabStyle::IconSize = 16x16
            icon_right_margin: 5.0, // UE5 icon Padding(0,0,5,0)
            close_size: 16.0,      // UE5 close button Icon16x16
            background_brush: SlateBrush::Color(bg),
            active_brush: SlateBrush::Color(active),
            hover_brush: SlateBrush::Color(hover),
            inactive_brush: SlateBrush::None,
            text_color: Color::rgba(0.753, 0.753, 0.753, 1.0),        // Foreground #C0C0C0
            active_text_color: Color::rgba(1.0, 1.0, 1.0, 1.0),       // White #FFFFFF (UE5 ActiveForeground)
            accent_brush: SlateBrush::Color(accent),
            close_button_hovered: SlateBrush::Color(Color::rgba(0.8, 0.2, 0.2, 0.6)),
        }
    }
}

impl MajorTabBarStyle {
    /// 테마에서 색상 초기화
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_brush: SlateBrush::Color(tc.major_tab_bar_bg),
            active_brush: SlateBrush::Color(tc.major_tab_active_bg),
            hover_brush: SlateBrush::Color(tc.major_tab_hover_bg),
            inactive_brush: SlateBrush::Color(tc.major_tab_inactive_bg),
            text_color: tc.major_tab_inactive_text,
            active_text_color: tc.text_bright,  // UE5 ForegroundHover = #FFFFFF
            accent_brush: SlateBrush::Color(tc.major_tab_accent),
            ..Default::default()
        }
    }

    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            height: self.height * scale,
            tab_max_width: self.tab_max_width * scale,
            tab_min_width: self.tab_min_width * scale,
            tab_left_pad: self.tab_left_pad * scale,
            tab_right_pad: self.tab_right_pad * scale,
            tab_spacing: self.tab_spacing * scale,
            icon_size: self.icon_size * scale,
            icon_right_margin: self.icon_right_margin * scale,
            close_size: self.close_size * scale,
            ..self.clone()
        }
    }
}

/// MajorTab 바 위젯
pub struct MajorTabBar {
    pub style: MajorTabBarStyle,
    hovered_index: Option<usize>,
    /// 호버 중인 닫기 버튼 인덱스
    hovered_close: Option<usize>,
}

impl Default for MajorTabBar {
    fn default() -> Self {
        Self::new()
    }
}

impl MajorTabBar {
    pub fn new() -> Self {
        Self {
            style: MajorTabBarStyle::default(),
            hovered_index: None,
            hovered_close: None,
        }
    }

    /// 렌더링
    pub fn paint(
        &self,
        abs_x: f32,
        abs_y: f32,
        width: f32,
        active_index: usize,
        titles: &[(String, Option<String>, bool)], // (title, icon, closable)
        scale: f32,
        ui_scale: f32,
        draw_elements: &mut DrawElementList,
        layer: u32,
    ) -> u32 {
        let mut current_layer = layer;
        let style = self.style.scaled(ui_scale);

        // 배경
        let bg_geo = PaintGeometry::new(
            Vec2::new(abs_x, abs_y),
            Vec2::new(width, style.height),
            scale,
        );
        draw_elements.add_brush(current_layer, bg_geo, &style.background_brush);
        current_layer += 1;

        // 각 MajorTab 렌더링
        let mut x = abs_x + style.tab_left_pad;
        let font_size = 10.0;  // UE5 NormalText = 10pt
        let font_px = font_size * ui_scale;

        for (i, (title, icon, closable)) in titles.iter().enumerate() {
            let is_active = i == active_index;
            let is_hovered = self.hovered_index == Some(i);

            // 탭 너비 계산 — UE5: [left_pad][icon+margin][label][close][right_pad]
            let text_len = title.len() as f32 * font_px * 0.55;
            let icon_space = if icon.is_some() { style.icon_size + style.icon_right_margin } else { 0.0 };
            let close_space = if *closable { style.close_size } else { 0.0 };
            let tab_width = (text_len + icon_space + close_space + style.tab_left_pad + style.tab_right_pad)
                .clamp(style.tab_min_width, style.tab_max_width);

            // 탭 배경
            let tab_brush = if is_active {
                &style.active_brush
            } else if is_hovered {
                &style.hover_brush
            } else {
                &style.inactive_brush
            };

            let tab_y = abs_y + 4.0 * ui_scale; // 상단 여백
            let tab_height = style.height - 4.0 * ui_scale;

            let tab_geo = PaintGeometry::new(
                Vec2::new(x, tab_y),
                Vec2::new(tab_width, tab_height),
                scale,
            );
            draw_elements.add_brush(current_layer, tab_geo, tab_brush);

            // 활성 탭 하단 하이라이트 바
            if is_active {
                let accent_height = 2.0 * ui_scale;
                let accent_geo = PaintGeometry::new(
                    Vec2::new(x, abs_y + style.height - accent_height),
                    Vec2::new(tab_width, accent_height),
                    scale,
                );
                draw_elements.add_brush(current_layer + 1, accent_geo, &style.accent_brush);
            }

            // 아이콘 + 제목 — UE5: VAlign_Center, [Icon 16x16 + 5px gap][Label]
            let mut text_x = x + style.tab_left_pad;
            if let Some(icon_str) = icon {
                draw_elements.add_text(
                    current_layer + 2,
                    PaintGeometry::new(
                        Vec2::new(text_x, tab_y + (tab_height - style.icon_size) * 0.5),
                        Vec2::new(style.icon_size, style.icon_size),
                        scale,
                    ),
                    icon_str.clone(),
                    if is_active { style.active_text_color } else { style.text_color },
                    font_size,
                );
                text_x += style.icon_size + style.icon_right_margin;
            }

            let label_width = tab_width - style.tab_left_pad - style.tab_right_pad
                - if icon.is_some() { style.icon_size + style.icon_right_margin } else { 0.0 }
                - if *closable { style.close_size } else { 0.0 };
            draw_elements.add_text(
                current_layer + 2,
                PaintGeometry::new(
                    Vec2::new(text_x, tab_y + (tab_height - font_px) * 0.5),
                    Vec2::new(label_width.max(0.0), font_px),
                    scale,
                ),
                title.clone(),
                if is_active { style.active_text_color } else { style.text_color },
                font_size,
            );

            // 닫기 버튼 (closable인 경우) — UE5: Icon16x16, right pad 내
            if *closable && (is_active || is_hovered) {
                let close_x = x + tab_width - style.tab_right_pad - style.close_size;
                let close_y = tab_y + (tab_height - style.close_size) * 0.5;

                // 닫기 버튼 호버 하이라이트
                if self.hovered_close == Some(i) {
                    let pad = 2.0 * ui_scale;
                    let close_geo = PaintGeometry::new(
                        Vec2::new(close_x - pad, close_y - pad),
                        Vec2::new(style.close_size + pad * 2.0, style.close_size + pad * 2.0),
                        scale,
                    );
                    draw_elements.add_brush(current_layer + 3, close_geo, &style.close_button_hovered);
                }

                // 닫기 아이콘
                draw_elements.add_image(
                    current_layer + 4,
                    PaintGeometry::new(
                        Vec2::new(close_x, close_y),
                        Vec2::new(style.close_size, style.close_size),
                        scale,
                    ),
                    "titlebar/_Titlebar_x.png".to_string(),
                    Color::rgba(0.7, 0.7, 0.7, 1.0),
                    ImageScaling::Fit,
                );
            }

            x += tab_width + style.tab_spacing;
        }

        current_layer += 5;
        current_layer
    }

    /// 클릭 히트 테스트 — 반환: 탭 인덱스
    pub fn hit_test(
        &self,
        local_x: f32,
        local_y: f32,
        titles: &[(String, Option<String>, bool)],
        ui_scale: f32,
    ) -> Option<usize> {
        let style = self.style.scaled(ui_scale);
        if local_y < 0.0 || local_y > style.height {
            return None;
        }

        let font_px = 10.0 * ui_scale;
        let mut x = style.tab_left_pad;
        for (i, (title, icon, closable)) in titles.iter().enumerate() {
            let text_len = title.len() as f32 * font_px * 0.55;
            let icon_space = if icon.is_some() { style.icon_size + style.icon_right_margin } else { 0.0 };
            let close_space = if *closable { style.close_size } else { 0.0 };
            let tab_width = (text_len + icon_space + close_space + style.tab_left_pad + style.tab_right_pad)
                .clamp(style.tab_min_width, style.tab_max_width);

            if local_x >= x && local_x < x + tab_width {
                return Some(i);
            }
            x += tab_width + style.tab_spacing;
        }
        None
    }

    /// 닫기 버튼 히트 테스트 — 반환: 탭 인덱스
    pub fn hit_test_close(
        &self,
        local_x: f32,
        local_y: f32,
        titles: &[(String, Option<String>, bool)],
        ui_scale: f32,
    ) -> Option<usize> {
        let style = self.style.scaled(ui_scale);
        if local_y < 0.0 || local_y > style.height {
            return None;
        }

        let font_px = 10.0 * ui_scale;
        let tab_top = 4.0 * ui_scale;
        let tab_height = style.height - tab_top;

        let mut x = style.tab_left_pad;
        for (i, (title, icon, closable)) in titles.iter().enumerate() {
            let text_len = title.len() as f32 * font_px * 0.55;
            let icon_space = if icon.is_some() { style.icon_size + style.icon_right_margin } else { 0.0 };
            let close_space = if *closable { style.close_size } else { 0.0 };
            let tab_width = (text_len + icon_space + close_space + style.tab_left_pad + style.tab_right_pad)
                .clamp(style.tab_min_width, style.tab_max_width);

            if *closable && local_x >= x && local_x < x + tab_width {
                let close_x = x + tab_width - style.tab_right_pad - style.close_size;
                let close_y = tab_top + (tab_height - style.close_size) * 0.5;
                if local_x >= close_x && local_x <= close_x + style.close_size
                    && local_y >= close_y && local_y <= close_y + style.close_size
                {
                    return Some(i);
                }
            }
            x += tab_width + style.tab_spacing;
        }
        None
    }

    /// 호버 업데이트
    pub fn update_hover(
        &mut self,
        local_x: f32,
        local_y: f32,
        titles: &[(String, Option<String>, bool)],
        ui_scale: f32,
    ) {
        self.hovered_index = self.hit_test(local_x, local_y, titles, ui_scale);
        self.hovered_close = self.hit_test_close(local_x, local_y, titles, ui_scale);
    }

    /// Zone 반환 (MajorTab 바 내: ClientArea)
    pub fn get_zone_at(&self) -> WindowZone {
        WindowZone::ClientArea
    }
}
