//! SHeaderRow - 테이블 컬럼 헤더 (언리얼 Slate의 SHeaderRow)
//!
//! 리스트뷰/테이블 상단에 컬럼 이름과 정렬 표시를 제공합니다.

use glam::Vec2;
use std::any::Any;

use crate::core::{
    Color, FontFamily, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
};
use crate::event::{PointerEvent, Reply};
use crate::render::text_renderer::TextMeasurer;

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

/// 컬럼 너비 모드
#[derive(Debug, Clone, Copy)]
pub enum ColumnWidth {
    /// 고정 픽셀
    Fixed(f32),
    /// 남은 공간 비율 (가중치)
    Fill(f32),
    /// 내용에 맞춤
    Auto,
}

/// 정렬 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    None,
    Ascending,
    Descending,
}

impl SortMode {
    pub fn next(self) -> Self {
        match self {
            SortMode::None => SortMode::Ascending,
            SortMode::Ascending => SortMode::Descending,
            SortMode::Descending => SortMode::None,
        }
    }
}

/// 헤더 컬럼 정의
pub struct HeaderColumn {
    pub id: String,
    pub label: String,
    pub width: ColumnWidth,
    pub sortable: bool,
    pub sort_mode: SortMode,
}

/// 헤더 행 스타일
#[derive(Debug, Clone)]
pub struct HeaderRowStyle {
    pub background_color: Color,
    pub hover_color: Color,
    pub border_color: Color,
    pub text_color: Color,
    pub sort_arrow_color: Color,
    pub font_size: f32,
    pub height: f32,
    pub padding: f32,
}

impl HeaderRowStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            background_color: tc.header_bg,
            hover_color: tc.control_bg_hover,
            border_color: tc.separator,
            text_color: tc.text_primary,
            sort_arrow_color: tc.text_secondary,
            font_size: theme.fonts.normal,
            height: 24.0,
            padding: 6.0,
        }
    }
}

impl Default for HeaderRowStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

/// 테이블 컬럼 헤더 위젯
pub struct SHeaderRow {
    columns: Vec<HeaderColumn>,
    on_sort_changed: Option<Box<dyn Fn(&str, SortMode) + Send + Sync>>,
    hovered_column: Option<usize>,
    style: HeaderRowStyle,
    /// 캐시된 컬럼 x좌표 + 폭
    cached_rects: Vec<(f32, f32)>,
    visibility: Visibility,
    enabled: bool,
    /// 위젯 고유 ID
    id: u64,
    dirty: InvalidateWidgetReason,
}

impl SHeaderRow {
    pub fn new() -> SHeaderRowBuilder {
        SHeaderRowBuilder {
            columns: Vec::new(),
            on_sort_changed: None,
            style: HeaderRowStyle::default(),
        }
    }

    pub fn columns(&self) -> &[HeaderColumn] {
        &self.columns
    }

    /// 컬럼 크기 계산
    fn compute_column_rects(&mut self, total_width: f32) {
        self.cached_rects.clear();

        let mut fixed_used = 0.0f32;
        let mut fill_total = 0.0f32;
        let mut auto_widths: Vec<f32> = Vec::new();

        for col in &self.columns {
            match col.width {
                ColumnWidth::Fixed(w) => {
                    fixed_used += w;
                    auto_widths.push(0.0);
                }
                ColumnWidth::Fill(w) => {
                    fill_total += w;
                    auto_widths.push(0.0);
                }
                ColumnWidth::Auto => {
                    let w = Self::measure_label_width(&col.label, self.style.font_size, 1.0) + self.style.padding * 2.0 + 16.0;
                    fixed_used += w;
                    auto_widths.push(w);
                }
            }
        }

        let remaining = (total_width - fixed_used).max(0.0);
        let mut x = 0.0f32;

        for (i, col) in self.columns.iter().enumerate() {
            let w = match col.width {
                ColumnWidth::Fixed(w) => w,
                ColumnWidth::Fill(weight) => {
                    if fill_total > 0.0 { remaining * weight / fill_total } else { 0.0 }
                }
                ColumnWidth::Auto => auto_widths[i],
            };
            self.cached_rects.push((x, w));
            x += w;
        }
    }

    /// 테마 적용
    pub fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.style = HeaderRowStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
    }

    fn measure_label_width(text: &str, font_size: f32, font_scale: f32) -> f32 {
        if let Ok(m) = TextMeasurer::instance().read() {
            m.measure_width(text, font_size, FontFamily::UI, font_scale)
        } else {
            let scaled = font_size * font_scale;
            text.len() as f32 * scaled * 0.5
        }
    }
}

/// SHeaderRow 빌더
pub struct SHeaderRowBuilder {
    columns: Vec<HeaderColumn>,
    on_sort_changed: Option<Box<dyn Fn(&str, SortMode) + Send + Sync>>,
    style: HeaderRowStyle,
}

impl SHeaderRowBuilder {
    pub fn column(mut self, id: impl Into<String>, label: impl Into<String>, width: ColumnWidth) -> Self {
        self.columns.push(HeaderColumn {
            id: id.into(),
            label: label.into(),
            width,
            sortable: true,
            sort_mode: SortMode::None,
        });
        self
    }

    pub fn column_non_sortable(mut self, id: impl Into<String>, label: impl Into<String>, width: ColumnWidth) -> Self {
        self.columns.push(HeaderColumn {
            id: id.into(),
            label: label.into(),
            width,
            sortable: false,
            sort_mode: SortMode::None,
        });
        self
    }

    pub fn on_sort_changed(mut self, f: impl Fn(&str, SortMode) + Send + Sync + 'static) -> Self {
        self.on_sort_changed = Some(Box::new(f));
        self
    }

    pub fn style(mut self, style: HeaderRowStyle) -> Self {
        self.style = style;
        self
    }

    pub fn build(self) -> SHeaderRow {
        SHeaderRow {
            columns: self.columns,
            on_sort_changed: self.on_sort_changed,
            hovered_column: None,
            style: self.style,
            cached_rects: Vec::new(),
            visibility: Visibility::Visible,
            enabled: true,
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT,
        }
    }
}

impl Widget for SHeaderRow {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(0.0, self.style.height)
    }

    fn type_name(&self) -> &'static str {
        "SHeaderRow"
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
        let pos = geometry.absolute_position;
        let size = geometry.local_size;
        let mut current_layer = layer;

        // 배경
        let bg_geo = PaintGeometry::new(pos, Vec2::new(size.x, self.style.height), geometry.scale);
        draw_elements.add_box(current_layer, bg_geo, self.style.background_color);
        current_layer += 1;

        // 컬럼 헤더
        for (i, col) in self.columns.iter().enumerate() {
            if i >= self.cached_rects.len() {
                break;
            }
            let (col_x, col_w) = self.cached_rects[i];

            // 호버 배경
            if self.hovered_column == Some(i) {
                let hover_geo = PaintGeometry::new(
                    Vec2::new(pos.x + col_x, pos.y),
                    Vec2::new(col_w, self.style.height),
                    geometry.scale,
                );
                draw_elements.add_box(current_layer, hover_geo, self.style.hover_color);
            }

            // 라벨
            let text_x = pos.x + col_x + self.style.padding;
            let text_y = pos.y + (self.style.height - self.style.font_size) * 0.5;
            let text_geo = PaintGeometry::new(
                Vec2::new(text_x, text_y),
                Vec2::new(col_w - self.style.padding * 2.0, self.style.font_size),
                geometry.scale,
            );
            draw_elements.add_text(current_layer + 1, text_geo, col.label.clone(), self.style.text_color, self.style.font_size);

            // 정렬 화살표
            if col.sort_mode != SortMode::None {
                let arrow_text = match col.sort_mode {
                    SortMode::Ascending => "▲",
                    SortMode::Descending => "▼",
                    SortMode::None => "",
                };
                let arrow_x = pos.x + col_x + col_w - self.style.padding - 10.0;
                let arrow_geo = PaintGeometry::new(
                    Vec2::new(arrow_x, text_y),
                    Vec2::new(10.0, self.style.font_size),
                    geometry.scale,
                );
                draw_elements.add_text(current_layer + 1, arrow_geo, arrow_text.to_string(), self.style.sort_arrow_color, self.style.font_size);
            }

            // 컬럼 구분선
            if i + 1 < self.columns.len() {
                let sep_x = pos.x + col_x + col_w - 0.5;
                let sep_geo = PaintGeometry::new(
                    Vec2::new(sep_x, pos.y + 4.0),
                    Vec2::new(1.0, self.style.height - 8.0),
                    geometry.scale,
                );
                draw_elements.add_box(current_layer + 1, sep_geo, self.style.border_color);
            }
        }
        current_layer += 2;

        // 하단 보더
        let border_geo = PaintGeometry::new(
            Vec2::new(pos.x, pos.y + self.style.height - 1.0),
            Vec2::new(size.x, 1.0),
            geometry.scale,
        );
        draw_elements.add_box(current_layer, border_geo, self.style.border_color);
        current_layer += 1;

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local = geometry.absolute_to_local(event.screen_position);
        self.compute_column_rects(geometry.local_size.x);

        self.hovered_column = None;
        for (i, &(x, w)) in self.cached_rects.iter().enumerate() {
            if local.x >= x && local.x < x + w && local.y >= 0.0 && local.y < self.style.height {
                self.hovered_column = Some(i);
                break;
            }
        }
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_column = None;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        self.compute_column_rects(geometry.local_size.x);
        let local = geometry.absolute_to_local(event.screen_position);

        for (i, &(x, w)) in self.cached_rects.iter().enumerate() {
            if local.x >= x && local.x < x + w && local.y >= 0.0 && local.y < self.style.height {
                if self.columns[i].sortable {
                    let new_mode = self.columns[i].sort_mode.next();
                    // 다른 컬럼 정렬 리셋
                    for (j, col) in self.columns.iter_mut().enumerate() {
                        if j != i {
                            col.sort_mode = SortMode::None;
                        }
                    }
                    self.columns[i].sort_mode = new_mode;
                    if let Some(ref cb) = self.on_sort_changed {
                        cb(&self.columns[i].id, new_mode);
                    }
                    return Reply::handled();
                }
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

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SHeaderRow {}
