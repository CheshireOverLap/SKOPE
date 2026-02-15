//! Asset Browser Panel - 프로젝트 에셋 탐색기
//!
//! 프로젝트의 에셋 폴더를 탐색하고 파일을 선택/열기

use std::any::Any;
use std::path::PathBuf;
use std::time::Instant;
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
use crate::theme::EditorTheme;
use crate::widget::{Widget, PaintArgs, DrawElementList};

/// 에셋 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetType {
    Folder,
    Scene,
    Mesh,
    Texture,
    Material,
    Script,
    Audio,
    Prefab,
    UiLayout,
    Unknown,
}

impl AssetType {
    /// 확장자로부터 에셋 타입 추론
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "skscene" | "scene" => AssetType::Scene,
            "gltf" | "glb" | "obj" | "fbx" => AssetType::Mesh,
            "png" | "jpg" | "jpeg" | "tga" | "bmp" | "ktx2" => AssetType::Texture,
            "skmat" | "material" => AssetType::Material,
            "rs" | "lua" | "rhai" => AssetType::Script,
            "wav" | "mp3" | "ogg" | "flac" => AssetType::Audio,
            "skprefab" | "prefab" => AssetType::Prefab,
            "skui" | "ui" => AssetType::UiLayout,
            _ => AssetType::Unknown,
        }
    }

    /// 타입에 따른 색상 (테마 참조)
    pub fn color(&self, tc: &crate::theme::ThemeColors) -> Color {
        match self {
            AssetType::Folder => tc.asset_folder,
            AssetType::Scene => tc.asset_scene,
            AssetType::Mesh => tc.asset_mesh,
            AssetType::Texture => tc.asset_texture,
            AssetType::Material => tc.asset_material,
            AssetType::Script => tc.asset_script,
            AssetType::Audio => tc.asset_audio,
            AssetType::Prefab => tc.asset_prefab,
            AssetType::UiLayout => tc.asset_ui_layout,
            AssetType::Unknown => tc.asset_unknown,
        }
    }

    /// 타입 아이콘 (텍스트)
    pub fn icon(&self) -> &'static str {
        match self {
            AssetType::Folder => "[D]",
            AssetType::Scene => "[S]",
            AssetType::Mesh => "[M]",
            AssetType::Texture => "[T]",
            AssetType::Material => "[Mt]",
            AssetType::Script => "[Sc]",
            AssetType::Audio => "[A]",
            AssetType::Prefab => "[P]",
            AssetType::UiLayout => "[UI]",
            AssetType::Unknown => "[?]",
        }
    }
}

/// 에셋 항목
#[derive(Debug, Clone)]
pub struct AssetEntry {
    pub name: String,
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub is_directory: bool,
}

/// Asset Browser 액션
#[derive(Debug, Clone)]
pub enum AssetBrowserAction {
    None,
    /// 디렉토리 이동
    NavigateTo(PathBuf),
    /// 파일 선택
    Select(PathBuf),
    /// 파일 열기 (더블클릭)
    Open(PathBuf),
    /// 드래그 시작
    StartDrag(PathBuf, AssetType),
    /// 새 폴더 생성
    CreateFolder,
    /// 새 에셋 생성
    CreateAsset(AssetType),
    /// 삭제
    Delete(PathBuf),
    /// 이름 변경
    Rename(PathBuf),
}

/// Asset Browser 패널 위젯
pub struct SAssetBrowser {
    /// 위젯 고유 ID
    id: u64,
    /// Dirty 플래그 (언리얼 EInvalidateWidgetReason)
    dirty: InvalidateWidgetReason,
    /// 현재 디렉토리
    current_dir: PathBuf,
    /// 루트 디렉토리 (프로젝트 assets 폴더)
    root_dir: PathBuf,
    /// 현재 디렉토리의 항목들
    entries: Vec<AssetEntry>,
    /// 선택된 항목 인덱스
    selected_index: Option<usize>,
    /// 대기 중인 액션
    pending_action: Option<AssetBrowserAction>,
    /// 표시 상태
    visibility: Visibility,
    /// 호버된 항목 인덱스
    hovered_index: Option<usize>,
    /// 스크롤 오프셋
    scroll_offset: f32,
    /// 뷰 모드 (그리드/리스트)
    grid_mode: bool,
    /// 마지막 클릭 시간 (더블클릭 감지용)
    last_click_time: Instant,
    /// 마지막 클릭 인덱스
    last_click_index: Option<usize>,
    /// 에디터 테마
    theme: EditorTheme,
}

impl SAssetBrowser {
    pub fn new(root_dir: PathBuf) -> Self {
        Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            current_dir: root_dir.clone(),
            root_dir,
            entries: Vec::new(),
            selected_index: None,
            pending_action: None,
            visibility: Visibility::Visible,
            hovered_index: None,
            scroll_offset: 0.0,
            grid_mode: true,  // 기본 그리드 모드
            last_click_time: Instant::now(),
            last_click_index: None,
            theme: EditorTheme::default(),
        }
    }

    /// 디렉토리 내용 설정 (외부에서 파일시스템 읽어서 전달)
    pub fn set_entries(&mut self, entries: Vec<AssetEntry>) {
        self.entries = entries;
        self.scroll_offset = 0.0;
    }

    /// 현재 디렉토리 가져오기
    pub fn current_dir(&self) -> &PathBuf {
        &self.current_dir
    }

    /// 디렉토리 변경
    pub fn set_current_dir(&mut self, dir: PathBuf) {
        self.current_dir = dir;
        self.selected_index = None;
        self.scroll_offset = 0.0;
    }

    /// 대기 중인 액션 가져오기 (큐 비움)
    pub fn take_action(&mut self) -> AssetBrowserAction {
        self.pending_action.take().unwrap_or(AssetBrowserAction::None)
    }

    /// 뷰 모드 토글
    pub fn toggle_view_mode(&mut self) {
        self.grid_mode = !self.grid_mode;
    }

    /// 헤더 높이 (테마 기반)
    fn header_height(&self) -> f32 { self.theme.spacing.panel_header_height - self.theme.spacing.gap }
    /// 경로 바 높이 (테마 기반)
    fn path_bar_height(&self) -> f32 { self.theme.spacing.control_height }
    /// 그리드 아이템 크기 (테마 기반)
    fn grid_item_size(&self) -> f32 { self.theme.spacing.grid_item_size }
    /// 그리드 아이템 간격 (테마 기반)
    fn grid_item_spacing(&self) -> f32 { self.theme.spacing.content_padding }
    /// 리스트 행 높이 (테마 기반)
    fn list_item_height(&self) -> f32 { self.theme.spacing.control_height }
    /// 그리드 아이콘 크기 (테마 기반)
    fn grid_icon_size(&self) -> f32 { self.theme.spacing.grid_icon_size }

    /// 그리드 모드에서 열 개수 계산
    fn grid_columns(&self, width: f32) -> usize {
        let pad = self.theme.spacing.content_padding;
        let content_width = width - pad * 2.0;
        let item_total = self.grid_item_size() + self.grid_item_spacing();
        (content_width / item_total).floor().max(1.0) as usize
    }

    /// 위치에서 항목 인덱스 찾기
    fn find_entry_at(&self, local_pos: Vec2, geometry: &Geometry) -> Option<usize> {
        let pad = self.theme.spacing.content_padding;
        let content_y = local_pos.y - self.header_height() - self.path_bar_height();
        if content_y < 0.0 {
            return None;
        }

        let adjusted_y = content_y + self.scroll_offset;

        if self.grid_mode {
            let columns = self.grid_columns(geometry.local_size.x);
            let item_total = self.grid_item_size() + self.grid_item_spacing();

            let col = ((local_pos.x - pad) / item_total).floor() as usize;
            let row = (adjusted_y / item_total).floor() as usize;

            if col < columns {
                let index = row * columns + col;
                if index < self.entries.len() {
                    return Some(index);
                }
            }
        } else {
            let index = (adjusted_y / self.list_item_height()).floor() as usize;
            if index < self.entries.len() {
                return Some(index);
            }
        }

        None
    }
}

impl Default for SAssetBrowser {
    fn default() -> Self {
        Self::new(PathBuf::from("assets"))
    }
}

impl Widget for SAssetBrowser {
    fn compute_desired_size(&self, _layout_scale: f32) -> Vec2 {
        Vec2::new(300.0, f32::INFINITY)
    }

    fn type_name(&self) -> &'static str {
        "SAssetBrowser"
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

    fn on_paint(
        &self,
        _args: &PaintArgs,
        geometry: &Geometry,
        _culling_rect: &SlateRect,
        draw_elements: &mut DrawElementList,
        layer: u32,
        _is_enabled: bool,
    ) -> u32 {
        let mut current_layer = layer;

        let tc = &self.theme.colors;
        let ts = &self.theme.spacing;
        let tf = &self.theme.fonts;
        let pad = ts.content_padding;
        let hdr_h = self.header_height();
        let path_h = self.path_bar_height();
        let grid_sz = self.grid_item_size();
        let grid_sp = self.grid_item_spacing();
        let list_h = self.list_item_height();
        let icon_sz = self.grid_icon_size();

        // 배경
        draw_elements.add_box(current_layer, geometry.to_paint_geometry(), tc.panel_bg);
        current_layer += 1;

        // 헤더
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(geometry.absolute_position, Vec2::new(geometry.local_size.x, hdr_h), geometry.scale),
            tc.header_bg,
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(pad, (hdr_h - tf.medium) * 0.5),
                Vec2::new(120.0, tf.medium),
                geometry.scale,
            ),
            "Asset Browser".to_string(),
            tc.sidebar_drawer_header_text,
            tf.normal,
        );

        // 뷰 모드 토글 버튼
        let mode_text = if self.grid_mode { "Grid" } else { "List" };
        let toggle_w = ts.toolbar_small_button_width + ts.content_padding;
        let toggle_h = ts.small_control_height;
        let toggle_x = geometry.local_size.x - toggle_w - ts.gap;
        let toggle_y = (hdr_h - toggle_h) * 0.5;
        draw_elements.add_box(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(toggle_x, toggle_y),
                Vec2::new(toggle_w, toggle_h),
                geometry.scale,
            ),
            tc.border,
        );
        draw_elements.add_text(
            current_layer + 2,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(toggle_x + ts.gap, toggle_y + (toggle_h - tf.normal) * 0.5),
                Vec2::new(toggle_w - ts.gap * 2.0, tf.normal),
                geometry.scale,
            ),
            mode_text.to_string(),
            tc.text_primary,
            tf.normal,
        );
        current_layer += 3;

        // 경로 바
        let path_y = hdr_h;
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(0.0, path_y),
                Vec2::new(geometry.local_size.x, path_h),
                geometry.scale,
            ),
            tc.control_bg_hover,
        );

        let relative_path = self.current_dir.strip_prefix(&self.root_dir).unwrap_or(&self.current_dir);
        let path_str = format!("/ {}", relative_path.display());
        let path_text_y = path_y + (path_h - tf.normal) * 0.5;
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(pad, path_text_y),
                Vec2::new(geometry.local_size.x - pad * 2.0, tf.normal),
                geometry.scale,
            ),
            path_str,
            tc.text_secondary,
            tf.normal,
        );
        current_layer += 2;

        // 컨텐츠 영역
        let content_y = hdr_h + path_h;

        if self.grid_mode {
            let columns = self.grid_columns(geometry.local_size.x);
            let item_total = grid_sz + grid_sp;

            for (i, entry) in self.entries.iter().enumerate() {
                let col = i % columns;
                let row = i / columns;
                let item_x = pad + (col as f32) * item_total;
                let item_y = content_y + (row as f32) * item_total - self.scroll_offset;

                if item_y + grid_sz < content_y || item_y > geometry.local_size.y {
                    continue;
                }

                let is_selected = self.selected_index == Some(i);
                let is_hovered = self.hovered_index == Some(i);
                let bg_color = if is_selected {
                    tc.selection_bg
                } else if is_hovered {
                    tc.hover_overlay
                } else {
                    tc.control_bg_hover
                };

                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(item_x, item_y),
                        Vec2::new(grid_sz, grid_sz),
                        geometry.scale,
                    ),
                    bg_color,
                );

                // 아이콘 (타입별 색상)
                let icon_x = item_x + (grid_sz - icon_sz) * 0.5;
                let icon_y = item_y + pad;
                draw_elements.add_box(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(icon_x, icon_y),
                        Vec2::new(icon_sz, icon_sz),
                        geometry.scale,
                    ),
                    entry.asset_type.color(tc),
                );

                // 아이콘 텍스트
                let icon_text_pad = ts.gap;
                draw_elements.add_text(
                    current_layer + 2,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(icon_x + icon_text_pad, icon_y + (icon_sz - tf.small) * 0.5),
                        Vec2::new(icon_sz - icon_text_pad * 2.0, tf.small),
                        geometry.scale,
                    ),
                    entry.asset_type.icon().to_string(),
                    tc.text_primary,
                    tf.small,
                );

                // 파일명
                draw_elements.add_text(
                    current_layer + 2,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(item_x + 2.0, item_y + grid_sz - tf.small - pad),
                        Vec2::new(grid_sz - ts.gap, tf.small + 2.0),
                        geometry.scale,
                    ),
                    truncate_text(&entry.name, 12),
                    tc.text_primary,
                    tf.small,
                );
            }
            current_layer += 3;
        } else {
            // 리스트 모드
            let text_v_pad = (list_h - tf.normal) * 0.5;
            let icon_area_w = pad + tf.normal * 2.5;

            for (i, entry) in self.entries.iter().enumerate() {
                let item_y = content_y + (i as f32) * list_h - self.scroll_offset;

                if item_y + list_h < content_y || item_y > geometry.local_size.y {
                    continue;
                }

                let is_selected = self.selected_index == Some(i);
                let is_hovered = self.hovered_index == Some(i);
                let bg_color = if is_selected {
                    tc.selection_bg
                } else if is_hovered {
                    tc.hover_overlay
                } else {
                    Color::TRANSPARENT
                };

                if bg_color.a > 0.0 {
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(
                            geometry.absolute_position + Vec2::new(0.0, item_y),
                            Vec2::new(geometry.local_size.x, list_h),
                            geometry.scale,
                        ),
                        bg_color,
                    );
                }

                // 아이콘
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(pad, item_y + text_v_pad),
                        Vec2::new(icon_area_w - pad, tf.normal),
                        geometry.scale,
                    ),
                    entry.asset_type.icon().to_string(),
                    entry.asset_type.color(tc),
                    tf.normal,
                );

                // 파일명
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(icon_area_w, item_y + text_v_pad),
                        Vec2::new(geometry.local_size.x - icon_area_w - pad, tf.normal),
                        geometry.scale,
                    ),
                    entry.name.clone(),
                    tc.text_primary,
                    tf.normal,
                );
            }
            current_layer += 2;
        }

        current_layer
    }

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        self.hovered_index = self.find_entry_at(local_pos, geometry);
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        if !event.is_left_button() {
            return Reply::unhandled();
        }

        let local_pos = geometry.absolute_to_local(event.screen_position);

        // 뷰 모드 토글 버튼 클릭 확인
        let hdr_h = self.header_height();
        if local_pos.y < hdr_h {
            let toggle_w = self.theme.spacing.toolbar_small_button_width + self.theme.spacing.content_padding;
            if local_pos.x > geometry.local_size.x - toggle_w - self.theme.spacing.gap {
                self.grid_mode = !self.grid_mode;
                return Reply::handled();
            }
        }

        // 항목 클릭
        if let Some(index) = self.find_entry_at(local_pos, geometry) {
            let now = Instant::now();
            let is_double_click = self.last_click_index == Some(index)
                && now.duration_since(self.last_click_time).as_secs_f64() < 0.4;

            self.last_click_time = now;
            self.last_click_index = Some(index);

            if is_double_click {
                // 더블클릭
                let entry = &self.entries[index];
                if entry.is_directory {
                    self.pending_action = Some(AssetBrowserAction::NavigateTo(entry.path.clone()));
                } else {
                    self.pending_action = Some(AssetBrowserAction::Open(entry.path.clone()));
                }
            } else {
                // 싱글클릭 - 선택
                self.selected_index = Some(index);
                let entry = &self.entries[index];
                self.pending_action = Some(AssetBrowserAction::Select(entry.path.clone()));
            }

            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_wheel(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let total_height = if self.grid_mode {
            let columns = self.grid_columns(geometry.local_size.x);
            let rows = (self.entries.len() + columns - 1) / columns;
            rows as f32 * (self.grid_item_size() + self.grid_item_spacing())
        } else {
            self.entries.len() as f32 * self.list_item_height()
        };

        let content_height = geometry.local_size.y - self.header_height() - self.path_bar_height();
        let max_scroll = (total_height - content_height).max(0.0);

        self.scroll_offset = (self.scroll_offset - event.wheel_delta * 40.0)
            .max(0.0)
            .min(max_scroll);

        Reply::handled()
    }

    fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    fn set_visibility(&mut self, visibility: Visibility) {
        self.visibility = visibility;
    }

    fn set_theme(&mut self, theme: &crate::theme::EditorTheme) {
        self.theme = theme.clone();
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// 텍스트 자르기 (긴 이름 처리)
fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars - 2).collect();
        format!("{}...", truncated)
    }
}
