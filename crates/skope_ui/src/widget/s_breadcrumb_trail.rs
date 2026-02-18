//! SBreadcrumbTrail - 경로 네비게이션 위젯 (언리얼 Slate의 SBreadcrumbTrail)
//!
//! `Home > Folder > Sub` 형태의 경로 표시. 각 항목 클릭으로 해당 레벨로 이동.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 브레드크럼 항목
#[derive(Debug, Clone)]
pub struct BreadcrumbItem {
    pub id: String,
    pub label: String,
}

/// 브레드크럼 스타일
#[derive(Debug, Clone)]
pub struct BreadcrumbStyle {
    pub text_color: Color,
    pub hover_color: Color,
    pub current_color: Color,
    pub separator_color: Color,
    pub font_size: f32,
    pub height: f32,
    pub separator: String,
}

impl BreadcrumbStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            text_color: tc.accent,
            hover_color: tc.accent_hover,
            current_color: tc.text_primary,
            separator_color: tc.separator,
            font_size: theme.fonts.large,
            height: 24.0,
            separator: " > ".to_string(),
        }
    }
}

impl Default for BreadcrumbStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

/// 경로 네비게이션 위젯
pub struct SBreadcrumbTrail {
    items: Vec<BreadcrumbItem>,
    on_crumb_clicked: Option<Box<dyn Fn(usize, &str) + Send + Sync>>,
    hovered_index: Option<usize>,
    style: BreadcrumbStyle,
    /// 캐시: 각 항목의 (x_start, width)
    cached_item_rects: Vec<(f32, f32)>,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
}

impl SBreadcrumbTrail {
    pub fn new() -> SBreadcrumbTrailBuilder {
        SBreadcrumbTrailBuilder {
            items: Vec::new(),
            on_crumb_clicked: None,
            style: BreadcrumbStyle::default(),
        }
    }

    /// 경로 설정
    pub fn set_items(&mut self, items: Vec<BreadcrumbItem>) {
        self.items = items;
        self.cached_item_rects.clear();
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
    }

    /// 경로 추가
    pub fn push(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.items.push(BreadcrumbItem {
            id: id.into(),
            label: label.into(),
        });
        self.cached_item_rects.clear();
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
    }

    /// 항목 수
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn measure_text(text: &str, font_size: f32, font_scale: f32) -> f32 {
        if let Ok(m) = TextMeasurer::instance().read() {
            m.measure_width(text, font_size, FontFamily::UI, font_scale)
        } else {
            let scaled = font_size * font_scale;
            text.chars().count() as f32 * scaled * 0.5
        }
    }

    fn rebuild_rects(&mut self) {
        self.cached_item_rects.clear();
        let sep_w = Self::measure_text(&self.style.separator, self.style.font_size, 1.0);
        let mut x = 0.0f32;

        for (i, item) in self.items.iter().enumerate() {
            let w = Self::measure_text(&item.label, self.style.font_size, 1.0);
            self.cached_item_rects.push((x, w));
            x += w;
            if i + 1 < self.items.len() {
                x += sep_w;
            }
        }
    }
}

/// SBreadcrumbTrail 빌더
pub struct SBreadcrumbTrailBuilder {
    items: Vec<BreadcrumbItem>,
    on_crumb_clicked: Option<Box<dyn Fn(usize, &str) + Send + Sync>>,
    style: BreadcrumbStyle,
}

impl SBreadcrumbTrailBuilder {
    pub fn add_crumb(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.items.push(BreadcrumbItem {
            id: id.into(),
            label: label.into(),
        });
        self
    }

    pub fn on_crumb_clicked(mut self, f: impl Fn(usize, &str) + Send + Sync + 'static) -> Self {
        self.on_crumb_clicked = Some(Box::new(f));
        self
    }

    pub fn style(mut self, style: BreadcrumbStyle) -> Self {
        self.style = style;
        self
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.style.separator = sep.into();
        self
    }

    pub fn build(self) -> SBreadcrumbTrail {
        SBreadcrumbTrail {
            items: self.items,
            on_crumb_clicked: self.on_crumb_clicked,
            hovered_index: None,
            style: self.style,
            cached_item_rects: Vec::new(),
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
        }
    }
}

impl Widget for SBreadcrumbTrail {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let sep_w = Self::measure_text(&self.style.separator, self.style.font_size, 1.0);
        let mut width = 0.0f32;
        for (i, item) in self.items.iter().enumerate() {
            width += Self::measure_text(&item.label, self.style.font_size, 1.0);
            if i + 1 < self.items.len() {
                width += sep_w;
            }
        }
        Vec2::new(width, self.style.height)
    }

    fn type_name(&self) -> &'static str {
        "SBreadcrumbTrail"
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
        if self.items.is_empty() {
            return layer;
        }

        let pos = geometry.absolute_position;
        let text_y = pos.y + (self.style.height - self.style.font_size) * 0.5;
        let sep_w = Self::measure_text(&self.style.separator, self.style.font_size, 1.0);

        let mut x = pos.x;
        let last_idx = self.items.len() - 1;

        for (i, item) in self.items.iter().enumerate() {
            let w = Self::measure_text(&item.label, self.style.font_size, 1.0);
            let is_last = i == last_idx;

            let color = if is_last {
                self.style.current_color
            } else if self.hovered_index == Some(i) {
                self.style.hover_color
            } else {
                self.style.text_color
            };

            let text_geo = PaintGeometry::new(Vec2::new(x, text_y), Vec2::new(w, self.style.font_size), geometry.scale);
            draw_elements.add_text(layer, text_geo, item.label.clone(), color, self.style.font_size);
            x += w;

            // 구분자
            if !is_last {
                let sep_geo = PaintGeometry::new(Vec2::new(x, text_y), Vec2::new(sep_w, self.style.font_size), geometry.scale);
                draw_elements.add_text(layer, sep_geo, self.style.separator.clone(), self.style.separator_color, self.style.font_size);
                x += sep_w;
            }
        }

        layer + 1
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        self.rebuild_rects();
        let local = geometry.absolute_to_local(event.screen_position);
        let last_idx = self.items.len().saturating_sub(1);

        self.hovered_index = None;
        for (i, &(item_x, item_w)) in self.cached_item_rects.iter().enumerate() {
            if i == last_idx {
                break; // 마지막은 클릭 불가
            }
            if local.x >= item_x && local.x < item_x + item_w && local.y >= 0.0 && local.y < self.style.height {
                self.hovered_index = Some(i);
                break;
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        self.rebuild_rects();
        let local = geometry.absolute_to_local(event.screen_position);
        let last_idx = self.items.len().saturating_sub(1);

        for (i, &(item_x, item_w)) in self.cached_item_rects.iter().enumerate() {
            if i == last_idx {
                break;
            }
            if local.x >= item_x && local.x < item_x + item_w && local.y >= 0.0 && local.y < self.style.height {
                if let Some(ref cb) = self.on_crumb_clicked {
                    cb(i, &self.items[i].id);
                }
                return Reply::handled();
            }
        }

        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, _geometry: &Geometry, _event: &PointerEvent) -> Reply {
        Reply::unhandled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn widget_id(&self) -> u64 { self.id }

    fn dirty_flags(&self) -> InvalidateWidgetReason {
        self.dirty
    }

    fn invalidate(&mut self, reason: InvalidateWidgetReason) {
        self.dirty = self.dirty | reason;
    }

    fn clear_dirty(&mut self) {
        self.dirty = InvalidateWidgetReason::NONE;
    }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = BreadcrumbStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SBreadcrumbTrail {}
