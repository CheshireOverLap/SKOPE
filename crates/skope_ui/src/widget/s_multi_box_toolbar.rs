//! SMultiBoxToolbar — 커맨드 기반 툴바 위젯 (UE5 SMultiBoxWidget 툴바 모드)
//!
//! MultiBlockEntry 목록으로 수평 툴바를 렌더링합니다.
//! 각 엔트리는 아이콘+레이블 버튼, 구분선, 또는 섹션 헤더입니다.

use glam::Vec2;
use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::core::{
    Color, Geometry, InvalidateWidgetReason, PaintGeometry, SlateRect, Visibility,
    CornerRadius,
};
use crate::event::{PointerEvent, Reply};
use crate::framework::{CommandId, UICommandList, MultiBlockEntry, MultiBlockType};

use super::{DrawElementList, LeafWidget, PaintArgs, Widget};

// ============================================================================
// MultiBoxToolbarStyle
// ============================================================================

/// 툴바 스타일
#[derive(Debug, Clone)]
pub struct MultiBoxToolbarStyle {
    /// 툴바 높이
    pub height: f32,
    /// 버튼 패딩
    pub button_padding: f32,
    /// 아이콘 크기
    pub icon_size: f32,
    /// 구분선 너비
    pub separator_width: f32,
    /// 배경 색상
    pub background_color: Color,
    /// 호버 색상
    pub hover_color: Color,
    /// 누름 색상
    pub pressed_color: Color,
    /// 텍스트 색상
    pub text_color: Color,
    /// 비활성 텍스트 색상
    pub disabled_text_color: Color,
    /// 구분선 색상
    pub separator_color: Color,
    /// 섹션 간격
    pub section_spacing: f32,
    /// 버튼 코너 라디우스
    pub button_corner_radius: f32,
}

impl MultiBoxToolbarStyle {
    pub fn from_theme(theme: &crate::theme::EditorTheme) -> Self {
        let tc = &theme.colors;
        Self {
            height: 32.0,
            button_padding: 4.0,
            icon_size: 16.0,
            separator_width: 1.0,
            background_color: tc.toolbar_bg,
            hover_color: tc.control_bg_hover,
            pressed_color: tc.control_bg_pressed,
            text_color: tc.text_primary,
            disabled_text_color: tc.text_muted,
            separator_color: tc.separator,
            section_spacing: 8.0,
            button_corner_radius: theme.spacing.border_radius,
        }
    }
}

impl Default for MultiBoxToolbarStyle {
    fn default() -> Self {
        Self::from_theme(&crate::theme::EditorTheme::default())
    }
}

// ============================================================================
// ToolbarButtonState
// ============================================================================

/// 툴바 버튼 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolbarButtonState {
    Normal,
    Hovered,
    Pressed,
}

/// 레이아웃된 엔트리
struct LayoutEntry {
    /// X 오프셋
    x: f32,
    /// 너비
    width: f32,
    /// 원래 엔트리 인덱스
    index: usize,
    /// 블록 타입
    block_type: MultiBlockType,
}

// ============================================================================
// SMultiBoxToolbar
// ============================================================================

/// 커맨드 기반 툴바 위젯
pub struct SMultiBoxToolbar {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그
    dirty: InvalidateWidgetReason,
    /// 엔트리 목록
    entries: Vec<MultiBlockEntry>,
    /// 커맨드 리스트 (label/shortcut 조회용)
    command_list: Option<Arc<Mutex<UICommandList>>>,
    /// 스타일
    style: MultiBoxToolbarStyle,
    /// 버튼 상태 (엔트리별)
    button_states: Vec<ToolbarButtonState>,
    /// 가시성
    visibility: Visibility,
    /// 호버된 엔트리 인덱스
    hovered_index: Option<usize>,
    /// 커맨드 실행 콜백
    on_command: Option<Box<dyn Fn(CommandId) + Send + Sync>>,
}

impl SMultiBoxToolbar {
    /// 새 툴바 생성
    pub fn new() -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            entries: Vec::new(),
            command_list: None,
            style: MultiBoxToolbarStyle::default(),
            button_states: Vec::new(),
            visibility: Visibility::Visible,
            hovered_index: None,
            on_command: None,
        }
    }

    /// 엔트리 목록 설정
    pub fn with_entries(mut self, entries: Vec<MultiBlockEntry>) -> Self {
        let count = entries.len();
        self.entries = entries;
        self.button_states = vec![ToolbarButtonState::Normal; count];
        self
    }

    /// 커맨드 리스트 설정
    pub fn with_command_list(mut self, list: Arc<Mutex<UICommandList>>) -> Self {
        self.command_list = Some(list);
        self
    }

    /// 스타일 설정
    pub fn with_style(mut self, style: MultiBoxToolbarStyle) -> Self {
        self.style = style;
        self
    }

    /// 커맨드 실행 콜백 설정
    pub fn with_on_command(mut self, f: impl Fn(CommandId) + Send + Sync + 'static) -> Self {
        self.on_command = Some(Box::new(f));
        self
    }

    /// 엔트리 목록 접근
    pub fn entries(&self) -> &[MultiBlockEntry] {
        &self.entries
    }

    /// 엔트리 추가
    pub fn add_entry(&mut self, entry: MultiBlockEntry) {
        self.entries.push(entry);
        self.button_states.push(ToolbarButtonState::Normal);
        self.dirty = self.dirty | InvalidateWidgetReason::LAYOUT | InvalidateWidgetReason::PAINT;
    }

    /// 엔트리의 레이블 해석 (커맨드 → label)
    fn resolve_label(&self, entry: &MultiBlockEntry) -> String {
        if let Some(ref label) = entry.label {
            return label.clone();
        }
        if let (Some(cmd_id), Some(ref cmd_list)) = (entry.command_id, &self.command_list) {
            if let Ok(list) = cmd_list.lock() {
                if let Some((info, _)) = list.find_command(cmd_id) {
                    return info.label.to_string();
                }
            }
        }
        String::new()
    }

    /// 레이아웃 계산 (각 엔트리의 x, width)
    fn compute_layout(&self) -> Vec<LayoutEntry> {
        let style = &self.style;
        let mut layouts = Vec::new();
        let mut x = style.button_padding;

        for (i, entry) in self.entries.iter().enumerate() {
            match entry.block_type {
                MultiBlockType::Separator => {
                    layouts.push(LayoutEntry {
                        x,
                        width: style.separator_width + style.section_spacing * 2.0,
                        index: i,
                        block_type: entry.block_type,
                    });
                    x += style.separator_width + style.section_spacing * 2.0;
                }
                MultiBlockType::Heading => {
                    let label = self.resolve_label(entry);
                    let label_width = label.len() as f32 * 7.0; // 근사치
                    let width = label_width + style.button_padding * 2.0;
                    layouts.push(LayoutEntry {
                        x,
                        width,
                        index: i,
                        block_type: entry.block_type,
                    });
                    x += width + style.section_spacing;
                }
                _ => {
                    // 버튼 류
                    let label = self.resolve_label(entry);
                    let has_icon = entry.icon.is_some();
                    let icon_width = if has_icon { style.icon_size + style.button_padding } else { 0.0 };
                    let label_width = if label.is_empty() { 0.0 } else { label.len() as f32 * 7.0 };
                    let width = icon_width + label_width + style.button_padding * 2.0;

                    layouts.push(LayoutEntry {
                        x,
                        width: width.max(style.height), // 최소 높이만큼 너비
                        index: i,
                        block_type: entry.block_type,
                    });
                    x += width.max(style.height);
                }
            }
        }

        layouts
    }

    /// 마우스 위치 → 엔트리 인덱스 매핑
    fn hit_test_entry(&self, local_pos: Vec2) -> Option<usize> {
        let layouts = self.compute_layout();
        for layout in &layouts {
            if layout.block_type == MultiBlockType::Separator
                || layout.block_type == MultiBlockType::Heading
            {
                continue;
            }
            if local_pos.x >= layout.x && local_pos.x < layout.x + layout.width
                && local_pos.y >= 0.0 && local_pos.y < self.style.height
            {
                return Some(layout.index);
            }
        }
        None
    }

    /// 엔트리 실행
    fn execute_entry(&self, index: usize) {
        let entry = &self.entries[index];

        // 직접 콜백
        if let Some(ref on_execute) = entry.on_execute {
            on_execute();
            return;
        }

        // 커맨드 실행: on_command 콜백으로 위임
        if let Some(cmd_id) = entry.command_id {
            if let Some(ref callback) = self.on_command {
                callback(cmd_id);
            }
        }
    }
}

impl Widget for SMultiBoxToolbar {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        let layouts = self.compute_layout();
        let total_width = layouts.last()
            .map(|l| l.x + l.width + self.style.button_padding)
            .unwrap_or(0.0);
        Vec2::new(total_width, self.style.height)
    }

    fn type_name(&self) -> &'static str {
        "SMultiBoxToolbar"
    }

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        is_enabled: bool,
    ) -> u32 {
        let paint_geo = geometry.to_paint_geometry();
        let style = &self.style;
        let mut current_layer = layer;

        // 배경
        draw_elements.add_box(
            current_layer,
            paint_geo.clone(),
            style.background_color,
        );
        current_layer += 1;

        let layouts = self.compute_layout();

        for layout in &layouts {
            let entry = &self.entries[layout.index];
            let entry_x = paint_geo.position.x + layout.x;
            let entry_y = paint_geo.position.y;

            match layout.block_type {
                MultiBlockType::Separator => {
                    // 구분선 그리기
                    let sep_x = entry_x + style.section_spacing;
                    let sep_y = entry_y + 4.0;
                    let sep_height = style.height - 8.0;
                    let sep_geo = PaintGeometry::new(
                        Vec2::new(sep_x, sep_y),
                        Vec2::new(style.separator_width, sep_height),
                        paint_geo.scale,
                    );
                    draw_elements.add_box(
                        current_layer,
                        sep_geo,
                        style.separator_color,
                    );
                    current_layer += 1;
                }
                MultiBlockType::Heading => {
                    // 섹션 헤더 텍스트
                    let label = self.resolve_label(entry);
                    if !label.is_empty() {
                        let text_geo = PaintGeometry::new(
                            Vec2::new(entry_x + style.button_padding, entry_y + style.height * 0.25),
                            Vec2::new(layout.width, style.height * 0.5),
                            paint_geo.scale,
                        );
                        draw_elements.add_text(
                            current_layer,
                            text_geo,
                            label,
                            style.disabled_text_color,
                            10.0,
                        );
                        current_layer += 1;
                    }
                }
                _ => {
                    // 버튼 그리기
                    let state = self.button_states.get(layout.index)
                        .copied()
                        .unwrap_or(ToolbarButtonState::Normal);

                    let bg_color = match state {
                        ToolbarButtonState::Pressed => style.pressed_color,
                        ToolbarButtonState::Hovered => style.hover_color,
                        ToolbarButtonState::Normal => Color::TRANSPARENT,
                    };

                    // 버튼 배경
                    if bg_color.a > 0.0 {
                        let btn_geo = PaintGeometry::new(
                            Vec2::new(entry_x, entry_y + 2.0),
                            Vec2::new(layout.width, style.height - 4.0),
                            paint_geo.scale,
                        );
                        draw_elements.add_rounded_box(
                            current_layer,
                            btn_geo,
                            bg_color,
                            Color::TRANSPARENT,
                            0.0,
                            CornerRadius::uniform(style.button_corner_radius),
                        );
                        current_layer += 1;
                    }

                    // 아이콘 (유니코드 텍스트)
                    let mut content_x = entry_x + style.button_padding;
                    if let Some(ref icon) = entry.icon {
                        let icon_y = entry_y + (style.height - style.icon_size) * 0.5;
                        let icon_geo = PaintGeometry::new(
                            Vec2::new(content_x, icon_y),
                            Vec2::new(style.icon_size, style.icon_size),
                            paint_geo.scale,
                        );
                        let icon_color = if is_enabled {
                            style.text_color
                        } else {
                            style.disabled_text_color
                        };
                        draw_elements.add_text(
                            current_layer,
                            icon_geo,
                            icon.clone(),
                            icon_color,
                            style.icon_size,
                        );
                        content_x += style.icon_size + style.button_padding;
                        current_layer += 1;
                    }

                    // 레이블
                    let label = self.resolve_label(entry);
                    if !label.is_empty() {
                        let text_y = entry_y + (style.height - 12.0) * 0.5;
                        let text_geo = PaintGeometry::new(
                            Vec2::new(content_x, text_y),
                            Vec2::new(layout.width - (content_x - entry_x), 12.0),
                            paint_geo.scale,
                        );
                        let text_color = if is_enabled {
                            style.text_color
                        } else {
                            style.disabled_text_color
                        };
                        draw_elements.add_text(
                            current_layer,
                            text_geo,
                            label,
                            text_color,
                            11.0,
                        );
                        current_layer += 1;
                    }
                }
            }
        }

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = event.position() - geometry.absolute_position;
        let new_hovered = self.hit_test_entry(local_pos);

        if new_hovered != self.hovered_index {
            // 이전 호버 해제
            if let Some(old) = self.hovered_index {
                if let Some(state) = self.button_states.get_mut(old) {
                    if *state == ToolbarButtonState::Hovered {
                        *state = ToolbarButtonState::Normal;
                    }
                }
            }
            // 새 호버 설정
            if let Some(new) = new_hovered {
                if let Some(state) = self.button_states.get_mut(new) {
                    if *state == ToolbarButtonState::Normal {
                        *state = ToolbarButtonState::Hovered;
                    }
                }
            }
            self.hovered_index = new_hovered;
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        }

        Reply::unhandled()
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = event.position() - geometry.absolute_position;
        if let Some(idx) = self.hit_test_entry(local_pos) {
            if let Some(state) = self.button_states.get_mut(idx) {
                *state = ToolbarButtonState::Pressed;
            }
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return Reply::handled();
        }
        Reply::unhandled()
    }

    fn on_mouse_button_up(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = event.position() - geometry.absolute_position;

        // 누름 상태 해제
        for state in &mut self.button_states {
            if *state == ToolbarButtonState::Pressed {
                *state = ToolbarButtonState::Normal;
            }
        }

        // 클릭 실행
        if let Some(idx) = self.hit_test_entry(local_pos) {
            self.execute_entry(idx);
            if let Some(state) = self.button_states.get_mut(idx) {
                *state = ToolbarButtonState::Hovered;
            }
            self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
            return Reply::handled();
        }
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
        for state in &mut self.button_states {
            *state = ToolbarButtonState::Normal;
        }
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn is_enabled(&self) -> bool {
        true
    }

    fn set_enabled(&mut self, _enabled: bool) {}

    fn widget_id(&self) -> u64 {
        self.id
    }

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
        self.style = MultiBoxToolbarStyle::from_theme(theme);
        self.dirty = self.dirty | InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LeafWidget for SMultiBoxToolbar {}
