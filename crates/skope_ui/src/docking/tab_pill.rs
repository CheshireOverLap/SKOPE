//! 탭 pill 공통 렌더링 — UE5 SDockTab::OnPaint 대응
//!
//! MajorTabBar와 SDockingTabStack에서 공유하는 탭 pill 렌더링 로직.
//! SDockTabPill 위젯: UE5 SDockTab에 대응하는 개별 탭 헤더 위젯.

use glam::Vec2;
use std::any::Any;
use crate::core::{
    Color, CornerRadius, FontFamily, Geometry, InvalidateWidgetReason,
    PaintGeometry, SlateBrush, SlateRect, Visibility,
};
use crate::event::{CursorIcon, PointerEvent, Reply};
use crate::framework::DockTabStyle;
use crate::render::text_renderer::TextMeasurer;
use crate::theme::EditorTheme;
use crate::widget::{DesiredSizeCache, DrawElementList, ImageScaling, PaintArgs, Widget};

use super::{TabId, TabRole, TabStackStyle};

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
    /// UE5 IsTabNameHidden: 탭 이름 숨김 (공간 부족 시 아이콘만 표시)
    pub hide_title: bool,
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
        scale, alpha, hide_title,
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
    // UE5 GetChildSizeToUse: IsTabNameHidden()이면 이름 숨기고 아이콘+닫기만
    let icon_space = if icon.is_some() { icon_size + icon_margin } else { 0.0 };
    let close_reserve = if *show_close { close_size + close_margin } else { 0.0 };

    if *hide_title {
        // UE5 IsTabNameHidden: 아이콘만 중앙 배치 (텍스트 생략)
        let content_width = if icon.is_some() { *icon_size } else { 0.0 };
        let content_x = x + (width - content_width - close_reserve) * 0.5;
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
        }
    } else {
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
    }

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
            "_Titlebar_x.png".to_string(),
            close_color,
            ImageScaling::Fit,
        );
    }

    // 반환: 사용한 레이어 수 (pill 1 + content 1 + close hover 1 + close icon 1 + 여유 1)
    5
}

// ============================================================================
// SDockTabPill — 개별 탭 헤더 위젯 (UE5 SDockTab 대응)
// ============================================================================

/// SDockTabPill 액션 (부모 TabWell로 전달)
#[derive(Debug, Clone)]
pub enum TabPillAction {
    /// 탭 클릭 → 활성화
    Activate,
    /// 닫기 버튼 클릭
    Close,
    /// 드래그 감지 (press position)
    DragDetected(Vec2),
}

/// 개별 탭 헤더 위젯 — UE5 SDockTab에 대응
///
/// 수동 on_paint (paint_tab_pill 호출) + 자체 호버/클릭 상태 관리.
/// 부모(SDockingTabWell)가 arrange_children으로 위치/크기 결정 후 on_paint 호출.
pub struct SDockTabPill {
    id: u64,
    dirty: InvalidateWidgetReason,
    desired_size_cache: DesiredSizeCache,

    // ── 탭 데이터 (외부에서 동기화) ──
    pub tab_id: TabId,
    pub title: String,
    pub icon: Option<String>,
    pub role: TabRole,
    pub closable: bool,
    pub color_tint: Option<Color>,

    // ── 위젯 상태 (부모 TabWell이 관리) ──
    pub is_hovered: bool,
    pub is_close_hovered: bool,
    pub is_foreground: bool,
    pub collapse_level: u8,
    pub alpha: f32,

    // ── 애니메이션 (부모 tick에서 갱신) ──
    pub spawn_anim_scale: f32,
    pub flash_value: f32,

    // ── 스타일 참조 ──
    pub tab_style: DockTabStyle,
    pub stack_style: TabStackStyle,
    pub theme: EditorTheme,

    // ── 액션 큐 ──
    pub pending_actions: Vec<TabPillAction>,
}

impl SDockTabPill {
    pub fn new(
        tab_id: TabId,
        title: String,
        icon: Option<String>,
        role: TabRole,
        closable: bool,
        tab_style: DockTabStyle,
        stack_style: TabStackStyle,
        theme: EditorTheme,
    ) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT,
            desired_size_cache: DesiredSizeCache::new(),
            tab_id,
            title,
            icon,
            role,
            closable,
            color_tint: None,
            is_hovered: false,
            is_close_hovered: false,
            is_foreground: false,
            collapse_level: 0,
            alpha: 1.0,
            spawn_anim_scale: 1.0,
            flash_value: 0.0,
            tab_style,
            stack_style,
            theme,
            pending_actions: Vec::new(),
        }
    }

    /// 부모에서 상태 동기화
    pub fn set_foreground(&mut self, fg: bool) { self.is_foreground = fg; }
    pub fn set_collapse_level(&mut self, level: u8) { self.collapse_level = level; }
    pub fn set_alpha(&mut self, alpha: f32) { self.alpha = alpha; }
    pub fn set_spawn_scale(&mut self, scale: f32) { self.spawn_anim_scale = scale; }
    pub fn set_flash_value(&mut self, v: f32) { self.flash_value = v; }

    /// 닫기 버튼 로컬 히트테스트 (물리 좌표)
    ///
    /// `local`: pill 좌상단 기준 로컬 좌표 (물리)
    /// `abs_size`: pill 물리 크기
    /// `scale`: DPI 스케일
    pub fn hit_test_close(&self, local: Vec2, abs_size: Vec2, scale: f32) -> bool {
        if !self.closable || self.collapse_level >= 2 { return false; }
        if !self.is_foreground && !self.is_hovered { return false; }
        let close_size = self.theme.spacing.tab_close_size * scale;
        let close_margin = self.theme.spacing.tab_close_margin * scale;
        let close_x = abs_size.x - close_size - close_margin;
        let close_y = (abs_size.y - close_size) / 2.0;
        local.x >= close_x && local.x <= close_x + close_size
            && local.y >= close_y && local.y <= close_y + close_size
    }

    /// 대기 액션 소비
    pub fn drain_actions(&mut self) -> Vec<TabPillAction> {
        std::mem::take(&mut self.pending_actions)
    }

    /// 닫기 버튼 표시 조건 (UE5 HandleIsCloseButtonVisible + GetChildSizeToUse)
    fn should_show_close(&self) -> bool {
        self.closable && self.collapse_level < 2
            && (self.is_foreground || self.is_hovered || self.is_close_hovered)
    }

    /// 배경색 결정 (active/hovered/normal + color_tint + flash)
    fn compute_bg_color(&self, scale: f32) -> Color {
        let brush = if self.is_foreground {
            &self.tab_style.active_brush
        } else if self.is_hovered {
            &self.tab_style.hovered_brush
        } else {
            &self.tab_style.normal_brush
        };
        let base = brush.get_tint();
        let mut c = Color::rgba(base.r, base.g, base.b, base.a * self.alpha);
        if let Some(tint) = self.color_tint {
            c = Color::rgba(c.r * tint.r, c.g * tint.g, c.b * tint.b, c.a);
        }
        if self.flash_value > 0.01 {
            let flash = self.tab_style.flash_color;
            let style = self.stack_style.scaled(scale);
            let blend = style.tab_flash_blend;
            c = Color::rgba(
                c.r + (flash.r - c.r) * self.flash_value * blend,
                c.g + (flash.g - c.g) * self.flash_value * blend,
                c.b + (flash.b - c.b) * self.flash_value * blend,
                c.a,
            );
        }
        c
    }

    /// 텍스트 색상 결정
    fn compute_text_color(&self) -> Color {
        let base = if self.is_foreground {
            self.tab_style.active_foreground_color
        } else if self.is_hovered {
            self.tab_style.hovered_foreground_color
        } else {
            self.tab_style.normal_foreground_color
        };
        Color::rgba(base.r, base.g, base.b, base.a * self.alpha)
    }

    /// 아이콘 틴트 결정
    fn compute_icon_tint(&self, scale: f32) -> Color {
        let style = self.stack_style.scaled(scale);
        let opacity = if self.is_foreground || self.is_hovered { 1.0 } else { style.inactive_icon_opacity };
        Color::rgba(
            self.theme.colors.icon_tint.r,
            self.theme.colors.icon_tint.g,
            self.theme.colors.icon_tint.b,
            opacity * self.alpha,
        )
    }
}

impl Widget for SDockTabPill {
    fn type_name(&self) -> &'static str { "SDockTabPill" }
    fn widget_id(&self) -> u64 { self.id }
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }

    fn dirty_flags(&self) -> InvalidateWidgetReason { self.dirty }
    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }
    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn cache_desired_size(&mut self, layout_scale: f32) {
        let size = self.compute_desired_size(layout_scale);
        self.desired_size_cache.cache(size, layout_scale);
    }

    fn get_cached_desired_size(&self) -> Option<Vec2> {
        self.desired_size_cache.get()
    }

    fn compute_desired_size(&self, _scale: f32) -> Vec2 {
        // 논리 크기 (부모 TabWell의 arrange_children이 실제 크기 결정)
        Vec2::new(self.stack_style.tab_max_width, self.stack_style.tab_bar_height)
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let abs_pos = geometry.absolute_position;
        let abs_size = geometry.absolute_size();
        let scale = geometry.scale;

        let bg_color = self.compute_bg_color(scale);
        let text_color = self.compute_text_color();
        let icon_tint = self.compute_icon_tint(scale);

        let show_close = self.should_show_close();
        let close_icon_color = if self.is_close_hovered {
            self.tab_style.active_foreground_color
        } else {
            self.tab_style.normal_foreground_color
        };

        paint_tab_pill(&TabPillParams {
            x: abs_pos.x,
            y: abs_pos.y,
            width: abs_size.x,
            height: abs_size.y,
            title: &self.title,
            icon: self.icon.as_deref(),
            show_close,
            is_close_hovered: self.is_close_hovered,
            bg_color,
            text_color,
            icon_tint,
            close_icon_color,
            close_hover_brush: Some(&self.tab_style.close_button_hovered),
            icon_size: self.theme.spacing.tab_icon_size * scale,
            icon_margin: self.theme.spacing.tab_icon_margin * scale,
            close_size: self.theme.spacing.tab_close_size * scale,
            font_size: self.theme.fonts.large,
            close_margin: self.theme.spacing.tab_close_margin * scale,
            scale,
            alpha: self.alpha,
            hide_title: self.collapse_level >= 1,
        }, draw_elements, layer);

        layer + 5
    }

    fn on_mouse_enter(&mut self, _geometry: &Geometry, _event: &PointerEvent) {
        self.is_hovered = true;
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.is_hovered = false;
        self.is_close_hovered = false;
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = event.position() - geometry.absolute_position;
        let abs_size = geometry.absolute_size();
        self.is_close_hovered = self.hit_test_close(local, abs_size, geometry.scale);
        self.dirty |= InvalidateWidgetReason::PAINT;
        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if event.is_left_button() {
            let local = event.position() - geometry.absolute_position;
            let abs_size = geometry.absolute_size();
            if self.closable && self.hit_test_close(local, abs_size, geometry.scale) {
                self.pending_actions.push(TabPillAction::Close);
                return Reply::handled();
            }
            self.pending_actions.push(TabPillAction::Activate);
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn get_cursor(&self) -> Option<CursorIcon> {
        if self.is_close_hovered {
            Some(CursorIcon::Pointer)
        } else {
            None
        }
    }

    fn get_visibility(&self) -> Visibility { Visibility::Visible }
    fn is_enabled(&self) -> bool { true }

    fn set_theme(&mut self, theme: &EditorTheme) {
        self.theme = theme.clone();
        self.tab_style = DockTabStyle::from_theme(theme);
        self.dirty |= InvalidateWidgetReason::PAINT;
    }
}

unsafe impl Send for SDockTabPill {}
unsafe impl Sync for SDockTabPill {}
