//! MajorTabBar — 타이틀바에 위치하는 MajorTab 바
//!
//! 언리얼 SDockingTabStack(bShowingTitleBarArea=true) 대응

use glam::Vec2;
use crate::core::{Color, PaintGeometry, SlateBrush, WindowZone};
use crate::widget::DrawElementList;

use super::{TabPillParams, measure_tab_text, paint_tab_pill};

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
    /// 탭 상하 여백 (pill 영역)
    pub v_padding: f32,
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
    /// 활성 탭 배경 fallback 색상
    pub active_fill_color: Color,
    /// 호버 탭 배경 fallback 색상
    pub hover_fill_color: Color,
    /// 악센트 브러시 (활성 탭 하단 하이라이트)
    pub accent_brush: SlateBrush,
    /// 닫기 버튼 호버 브러시
    pub close_button_hovered: SlateBrush,
    /// 닫기 아이콘 색상
    pub close_icon_color: Color,
    /// 탭 라벨 폰트 크기 (논리 단위)
    pub font_size: f32,
}

impl Default for MajorTabBarStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

impl MajorTabBarStyle {
    /// 테마에서 색상 + 크기 초기화
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        let ts = &theme.spacing;
        Self {
            height: ts.major_tab_height,
            tab_max_width: ts.major_tab_max_width,
            tab_min_width: ts.major_tab_min_width,
            tab_left_pad: ts.major_tab_left_pad,
            tab_right_pad: ts.major_tab_right_pad,
            tab_spacing: ts.major_tab_spacing,
            icon_size: ts.major_tab_icon_size,
            icon_right_margin: ts.major_tab_icon_margin,
            close_size: ts.major_tab_close_size,
            v_padding: ts.major_tab_v_padding,
            background_brush: SlateBrush::Color(tc.major_tab_bar_bg),
            active_brush: SlateBrush::Color(tc.major_tab_active_bg),
            hover_brush: SlateBrush::Color(tc.major_tab_hover_bg),
            inactive_brush: SlateBrush::Color(tc.major_tab_inactive_bg),
            text_color: tc.major_tab_inactive_text,
            active_text_color: tc.text_bright,
            active_fill_color: tc.major_tab_active_bg,
            hover_fill_color: tc.major_tab_hover_bg,
            accent_brush: SlateBrush::Color(tc.major_tab_accent),
            close_button_hovered: SlateBrush::Color(tc.danger),
            close_icon_color: tc.icon_tint,
            font_size: theme.fonts.large,
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
            v_padding: self.v_padding * scale,
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
        let font_size = style.font_size;

        for (i, (title, icon, closable)) in titles.iter().enumerate() {
            let is_active = i == active_index;
            let is_hovered = self.hovered_index == Some(i);

            let tab_width = Self::calc_tab_width(title, icon.as_deref(), *closable, &style, ui_scale);

            let tab_y = abs_y + style.v_padding;
            let tab_height = style.height - style.v_padding * 2.0;

            let tab_fill = if is_active {
                match &style.active_brush {
                    SlateBrush::Color(c) => *c,
                    _ => style.active_fill_color,
                }
            } else if is_hovered {
                match &style.hover_brush {
                    SlateBrush::Color(c) => *c,
                    _ => style.hover_fill_color,
                }
            } else {
                Color::TRANSPARENT
            };

            let _layers_used = paint_tab_pill(&TabPillParams {
                x, y: tab_y, width: tab_width, height: tab_height,
                title, icon: icon.as_deref(),
                show_close: *closable && (is_active || is_hovered),
                is_close_hovered: self.hovered_close == Some(i),
                bg_color: tab_fill,
                text_color: if is_active { style.active_text_color } else { style.text_color },
                icon_tint: if is_active { Color::WHITE } else { style.text_color },
                close_icon_color: style.close_icon_color,
                close_hover_brush: Some(&style.close_button_hovered),
                icon_size: style.icon_size, icon_margin: style.icon_right_margin,
                close_size: style.close_size, font_size,
                close_margin: style.tab_right_pad,
                scale, alpha: 1.0,
            }, draw_elements, current_layer);

            x += tab_width + style.tab_spacing;
        }

        current_layer += 5;
        current_layer
    }

    /// 탭 너비 계산 (paint · hit_test 공용)
    fn calc_tab_width(title: &str, icon: Option<&str>, closable: bool, style: &MajorTabBarStyle, ui_scale: f32) -> f32 {
        let text_len = measure_tab_text(title, style.font_size, ui_scale);
        let icon_space = if icon.is_some() { style.icon_size + style.icon_right_margin } else { 0.0 };
        let close_space = if closable { style.close_size } else { 0.0 };
        (text_len + icon_space + close_space + style.tab_left_pad + style.tab_right_pad)
            .clamp(style.tab_min_width, style.tab_max_width)
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

        let mut x = style.tab_left_pad;
        for (i, (title, icon, closable)) in titles.iter().enumerate() {
            let tab_width = Self::calc_tab_width(title, icon.as_deref(), *closable, &style, ui_scale);

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

        let tab_top = style.v_padding;
        let tab_height = style.height - style.v_padding * 2.0;

        let mut x = style.tab_left_pad;
        for (i, (title, icon, closable)) in titles.iter().enumerate() {
            let tab_width = Self::calc_tab_width(title, icon.as_deref(), *closable, &style, ui_scale);

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
