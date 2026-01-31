//! Asset Browser Panel - 프로젝트 에셋 탐색기
//!
//! 프로젝트의 에셋 폴더를 탐색하고 파일을 선택/열기

use std::any::Any;
use std::path::PathBuf;
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, PaintGeometry, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent};
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

    /// 타입에 따른 색상
    pub fn color(&self) -> Color {
        match self {
            AssetType::Folder => Color::rgba(0.714, 0.561, 0.333, 1.0),  // AccentFolder #B68F55
            AssetType::Scene => Color::rgba(0.3, 0.8, 0.4, 1.0),
            AssetType::Mesh => Color::rgba(0.4, 0.6, 0.9, 1.0),
            AssetType::Texture => Color::rgba(0.9, 0.5, 0.3, 1.0),
            AssetType::Material => Color::rgba(0.8, 0.3, 0.8, 1.0),
            AssetType::Script => Color::rgba(0.5, 0.9, 0.5, 1.0),
            AssetType::Audio => Color::rgba(0.3, 0.9, 0.9, 1.0),
            AssetType::Prefab => Color::rgba(0.6, 0.4, 0.9, 1.0),
            AssetType::UiLayout => Color::rgba(0.9, 0.6, 0.8, 1.0),
            AssetType::Unknown => Color::rgba(0.5, 0.5, 0.5, 1.0),
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
    last_click_time: f64,
    /// 마지막 클릭 인덱스
    last_click_index: Option<usize>,
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
            last_click_time: 0.0,
            last_click_index: None,
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

    /// 상수들
    const HEADER_HEIGHT: f32 = 28.0;
    const PATH_BAR_HEIGHT: f32 = 24.0;
    const GRID_ITEM_SIZE: f32 = 80.0;
    const GRID_ITEM_SPACING: f32 = 8.0;
    const LIST_ITEM_HEIGHT: f32 = 24.0;

    /// 그리드 모드에서 열 개수 계산
    fn grid_columns(&self, width: f32) -> usize {
        let content_width = width - 16.0;  // 좌우 패딩
        let item_total = Self::GRID_ITEM_SIZE + Self::GRID_ITEM_SPACING;
        (content_width / item_total).floor().max(1.0) as usize
    }

    /// 위치에서 항목 인덱스 찾기
    fn find_entry_at(&self, local_pos: Vec2, geometry: &Geometry) -> Option<usize> {
        let content_y = local_pos.y - Self::HEADER_HEIGHT - Self::PATH_BAR_HEIGHT;
        if content_y < 0.0 {
            return None;
        }

        let adjusted_y = content_y + self.scroll_offset;

        if self.grid_mode {
            let columns = self.grid_columns(geometry.local_size.x);
            let item_total = Self::GRID_ITEM_SIZE + Self::GRID_ITEM_SPACING;

            let col = ((local_pos.x - 8.0) / item_total).floor() as usize;
            let row = (adjusted_y / item_total).floor() as usize;

            if col < columns {
                let index = row * columns + col;
                if index < self.entries.len() {
                    return Some(index);
                }
            }
        } else {
            let index = (adjusted_y / Self::LIST_ITEM_HEIGHT).floor() as usize;
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

        // 배경
        let paint_geo = geometry.to_paint_geometry();
        draw_elements.add_box(
            current_layer,
            paint_geo,
            Color::rgba(0.141, 0.141, 0.141, 1.0),  // Panel #242424
        );
        current_layer += 1;

        // 헤더
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position,
                Vec2::new(geometry.local_size.x, Self::HEADER_HEIGHT),
                geometry.scale,
            ),
            Color::rgba(0.184, 0.184, 0.184, 1.0),  // Header #2F2F2F
        );
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(8.0, 7.0),
                Vec2::new(100.0, 14.0),
                geometry.scale,
            ),
            "Asset Browser".to_string(),
            Color::rgba(0.784, 0.784, 0.784, 1.0),  // ForegroundHeader #C8C8C8
            10.0,
        );

        // 뷰 모드 토글 버튼
        let mode_text = if self.grid_mode { "Grid" } else { "List" };
        draw_elements.add_box(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(geometry.local_size.x - 50.0, 4.0),
                Vec2::new(42.0, 20.0),
                geometry.scale,
            ),
            Color::rgba(0.220, 0.220, 0.220, 1.0),  // Dropdown #383838
        );
        draw_elements.add_text(
            current_layer + 2,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(geometry.local_size.x - 44.0, 7.0),
                Vec2::new(36.0, 14.0),
                geometry.scale,
            ),
            mode_text.to_string(),
            Color::rgba(0.753, 0.753, 0.753, 1.0),  // Foreground #C0C0C0
            10.0,
        );
        current_layer += 3;

        // 경로 바
        let path_y = Self::HEADER_HEIGHT;
        draw_elements.add_box(
            current_layer,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(0.0, path_y),
                Vec2::new(geometry.local_size.x, Self::PATH_BAR_HEIGHT),
                geometry.scale,
            ),
            Color::rgba(0.102, 0.102, 0.102, 1.0),  // Recessed #1A1A1A
        );

        // 경로 표시
        let relative_path = self.current_dir
            .strip_prefix(&self.root_dir)
            .unwrap_or(&self.current_dir);
        let path_str = format!("/ {}", relative_path.display());
        draw_elements.add_text(
            current_layer + 1,
            PaintGeometry::new(
                geometry.absolute_position + Vec2::new(8.0, path_y + 5.0),
                Vec2::new(geometry.local_size.x - 16.0, 14.0),
                geometry.scale,
            ),
            path_str,
            Color::rgba(0.376, 0.376, 0.376, 1.0),  // Faded #606060
            10.0,
        );
        current_layer += 2;

        // 컨텐츠 영역
        let content_y = Self::HEADER_HEIGHT + Self::PATH_BAR_HEIGHT;
        let content_height = geometry.local_size.y - content_y;

        if self.grid_mode {
            // 그리드 모드
            let columns = self.grid_columns(geometry.local_size.x);
            let item_total = Self::GRID_ITEM_SIZE + Self::GRID_ITEM_SPACING;

            for (i, entry) in self.entries.iter().enumerate() {
                let col = i % columns;
                let row = i / columns;

                let item_x = 8.0 + (col as f32) * item_total;
                let item_y = content_y + (row as f32) * item_total - self.scroll_offset;

                // 화면 밖이면 스킵
                if item_y + Self::GRID_ITEM_SIZE < content_y || item_y > geometry.local_size.y {
                    continue;
                }

                let is_selected = self.selected_index == Some(i);
                let is_hovered = self.hovered_index == Some(i);

                // 항목 배경
                let bg_color = if is_selected {
                    Color::rgba(0.0, 0.239, 0.502, 1.0)    // Select #003D80
                } else if is_hovered {
                    Color::rgba(0.220, 0.220, 0.220, 1.0)  // Hover2 #383838
                } else {
                    Color::rgba(0.102, 0.102, 0.102, 1.0)  // Recessed #1A1A1A
                };

                draw_elements.add_box(
                    current_layer,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(item_x, item_y),
                        Vec2::new(Self::GRID_ITEM_SIZE, Self::GRID_ITEM_SIZE),
                        geometry.scale,
                    ),
                    bg_color,
                );

                // 아이콘 (타입별 색상)
                let icon_size = 32.0;
                let icon_x = item_x + (Self::GRID_ITEM_SIZE - icon_size) * 0.5;
                let icon_y = item_y + 8.0;

                draw_elements.add_box(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(icon_x, icon_y),
                        Vec2::new(icon_size, icon_size),
                        geometry.scale,
                    ),
                    entry.asset_type.color(),
                );

                // 아이콘 텍스트
                draw_elements.add_text(
                    current_layer + 2,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(icon_x + 4.0, icon_y + 10.0),
                        Vec2::new(icon_size - 8.0, 12.0),
                        geometry.scale,
                    ),
                    entry.asset_type.icon().to_string(),
                    Color::rgba(0.753, 0.753, 0.753, 1.0),  // Foreground
                    9.0,
                );

                // 파일명
                draw_elements.add_text(
                    current_layer + 2,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(item_x + 2.0, item_y + Self::GRID_ITEM_SIZE - 18.0),
                        Vec2::new(Self::GRID_ITEM_SIZE - 4.0, 14.0),
                        geometry.scale,
                    ),
                    truncate_text(&entry.name, 12),
                    Color::rgba(0.753, 0.753, 0.753, 1.0),  // Foreground
                    9.0,
                );
            }
            current_layer += 3;
        } else {
            // 리스트 모드
            for (i, entry) in self.entries.iter().enumerate() {
                let item_y = content_y + (i as f32) * Self::LIST_ITEM_HEIGHT - self.scroll_offset;

                // 화면 밖이면 스킵
                if item_y + Self::LIST_ITEM_HEIGHT < content_y || item_y > geometry.local_size.y {
                    continue;
                }

                let is_selected = self.selected_index == Some(i);
                let is_hovered = self.hovered_index == Some(i);

                // 항목 배경
                let bg_color = if is_selected {
                    Color::rgba(0.0, 0.239, 0.502, 1.0)    // Select #003D80
                } else if is_hovered {
                    Color::rgba(0.220, 0.220, 0.220, 1.0)  // Hover2 #383838
                } else {
                    Color::TRANSPARENT
                };

                if bg_color.a > 0.0 {
                    draw_elements.add_box(
                        current_layer,
                        PaintGeometry::new(
                            geometry.absolute_position + Vec2::new(0.0, item_y),
                            Vec2::new(geometry.local_size.x, Self::LIST_ITEM_HEIGHT),
                            geometry.scale,
                        ),
                        bg_color,
                    );
                }

                // 아이콘
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(8.0, item_y + 5.0),
                        Vec2::new(24.0, 14.0),
                        geometry.scale,
                    ),
                    entry.asset_type.icon().to_string(),
                    entry.asset_type.color(),
                    10.0,
                );

                // 파일명
                draw_elements.add_text(
                    current_layer + 1,
                    PaintGeometry::new(
                        geometry.absolute_position + Vec2::new(36.0, item_y + 5.0),
                        Vec2::new(geometry.local_size.x - 44.0, 14.0),
                        geometry.scale,
                    ),
                    entry.name.clone(),
                    Color::rgba(0.753, 0.753, 0.753, 1.0),  // Foreground
                    10.0,
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
        if local_pos.y < Self::HEADER_HEIGHT {
            if local_pos.x > geometry.local_size.x - 50.0 {
                self.grid_mode = !self.grid_mode;
                return Reply::handled();
            }
        }

        // 항목 클릭
        if let Some(index) = self.find_entry_at(local_pos, geometry) {
            let now = event.screen_position.x as f64; // 임시 시간 대용
            let is_double_click = self.last_click_index == Some(index)
                && (now - self.last_click_time).abs() < 0.5;

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
            rows as f32 * (Self::GRID_ITEM_SIZE + Self::GRID_ITEM_SPACING)
        } else {
            self.entries.len() as f32 * Self::LIST_ITEM_HEIGHT
        };

        let content_height = geometry.local_size.y - Self::HEADER_HEIGHT - Self::PATH_BAR_HEIGHT;
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
