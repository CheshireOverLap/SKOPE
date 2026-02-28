//! Asset Browser Panel - 프로젝트 에셋 탐색기
//!
//! UE5 SAssetView/SContentBrowser 대응 고급 시스템:
//! - 텍스트 검색 + 비동기 필터링 (FAssetTextFilter + ProcessItemsPendingFilter)
//! - 프론트엔드 필터 pill 바 (FFrontendFilter)
//! - 시간 예산 amortization (MaxSecondsPerFrame)
//! - 네비게이션 히스토리 (FHistoryManager)
//! - 멀티셀렉션 + 키보드 + 컨텍스트 메뉴
//! - 멀티 컬럼 정렬 + Column 뷰 (FAssetViewSortManager)

use std::any::Any;
use std::path::PathBuf;
use std::time::Instant;
use glam::Vec2;

use crate::core::{Geometry, Visibility, Color, SlateRect, InvalidateWidgetReason};
use crate::event::{Reply, PointerEvent, CharEvent, KeyEvent, KeyCode};
use crate::theme::EditorTheme;
use crate::widget::{Widget, PaintArgs, DrawElementList};

// ─────────────────────────────────────────────
// 타입 정의
// ─────────────────────────────────────────────

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

    /// 타입 라벨 (pill 표시용)
    fn label(&self) -> &'static str {
        match self {
            AssetType::Folder => "Folder",
            AssetType::Scene => "Scene",
            AssetType::Mesh => "Mesh",
            AssetType::Texture => "Tex",
            AssetType::Material => "Mat",
            AssetType::Script => "Script",
            AssetType::Audio => "Audio",
            AssetType::Prefab => "Prefab",
            AssetType::UiLayout => "UI",
            AssetType::Unknown => "Other",
        }
    }
}

/// 뷰 모드
#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Grid,
    List,
    Column,
}

/// 정렬 기준
#[derive(Clone, Copy, PartialEq)]
pub enum SortColumn {
    Name,
    Type,
    Size,
    Modified,
}

/// 정렬 방향
#[derive(Clone, Copy, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// 브레드크럼 세그먼트
struct BreadcrumbSegment {
    label: String,
    path: PathBuf,
}

/// 컨텍스트 메뉴
struct ContextMenu {
    position: Vec2,
    items: Vec<ContextMenuItem>,
}

/// 컨텍스트 메뉴 항목
struct ContextMenuItem {
    label: &'static str,
    action: AssetBrowserAction,
    enabled: bool,
}

/// 에셋 항목
#[derive(Debug, Clone)]
pub struct AssetEntry {
    pub name: String,
    pub path: PathBuf,
    pub asset_type: AssetType,
    pub is_directory: bool,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
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
    /// 뒤로
    GoBack,
    /// 앞으로
    GoForward,
    /// 여러 파일 선택 (멀티셀렉션)
    SelectMultiple(Vec<PathBuf>),
    /// Show in Explorer
    ShowInExplorer(PathBuf),
    /// Copy path to clipboard
    CopyPath(PathBuf),
}

// ─────────────────────────────────────────────
// 네비게이션 히스토리 (UE5 FHistoryManager)
// ─────────────────────────────────────────────

struct HistoryEntry {
    dir: PathBuf,
    scroll_offset: f32,
}

struct HistoryManager {
    entries: Vec<HistoryEntry>,
    current: i32,
    max_entries: usize,
}

impl HistoryManager {
    fn new() -> Self {
        Self { entries: Vec::new(), current: -1, max_entries: 300 }
    }

    fn push(&mut self, entry: HistoryEntry) {
        // forward 히스토리 제거
        if (self.current + 1) < self.entries.len() as i32 {
            self.entries.truncate((self.current + 1) as usize);
        }
        self.entries.push(entry);
        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }
        self.current = self.entries.len() as i32 - 1;
    }

    fn go_back(&mut self) -> Option<(PathBuf, f32)> {
        if self.current > 0 {
            self.current -= 1;
            let e = &self.entries[self.current as usize];
            Some((e.dir.clone(), e.scroll_offset))
        } else {
            None
        }
    }

    fn go_forward(&mut self) -> Option<(PathBuf, f32)> {
        if self.current + 1 < self.entries.len() as i32 {
            self.current += 1;
            let e = &self.entries[self.current as usize];
            Some((e.dir.clone(), e.scroll_offset))
        } else {
            None
        }
    }

    fn can_go_back(&self) -> bool { self.current > 0 }
    fn can_go_forward(&self) -> bool { self.current + 1 < self.entries.len() as i32 }
}

// ─────────────────────────────────────────────
// SAssetBrowser
// ─────────────────────────────────────────────

/// 필터 가능한 에셋 타입 목록 (pill 표시 순서)
const FILTER_TYPES: [AssetType; 9] = [
    AssetType::Scene, AssetType::Mesh, AssetType::Texture, AssetType::Material,
    AssetType::Script, AssetType::Audio, AssetType::Prefab, AssetType::UiLayout,
    AssetType::Unknown,
];

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
    /// 대기 중인 액션
    pending_action: Option<AssetBrowserAction>,
    /// 표시 상태
    visibility: Visibility,
    /// 호버된 항목 인덱스 (filtered_indices 기준)
    hovered_index: Option<usize>,
    /// 스크롤 오프셋
    scroll_offset: f32,
    /// 마지막 클릭 시간 (더블클릭 감지용)
    last_click_time: Instant,
    /// 마지막 클릭 인덱스 (filtered 기준)
    last_click_index: Option<usize>,
    /// 에디터 테마
    theme: EditorTheme,
    /// 로딩 중 표시
    is_loading: bool,

    // === Phase 1: 뷰 모드 + 멀티셀렉션 ===
    /// 뷰 모드 (Grid/List/Column)
    view_mode: ViewMode,
    /// 선택된 항목 인덱스들 (filtered_indices 기준)
    selected_indices: Vec<usize>,

    // === Phase 1: 필터 인프라 ===
    /// 필터 적용된 표시용 인덱스 (entries 인덱스 참조)
    filtered_indices: Vec<usize>,
    /// 필터 진행 상태 (amortization용)
    filter_progress: usize,
    /// 필터링 완료 여부
    filter_complete: bool,

    // === Phase 2: 검색 ===
    /// 검색 문자열
    search_query: String,
    /// 검색 커서 위치 (바이트 오프셋)
    search_cursor: usize,
    /// 검색 입력 포커스
    search_focused: bool,

    // === Phase 2: 타입 필터 ===
    /// 타입별 프론트엔드 필터 비트마스크 (0=필터없음=전체표시)
    asset_type_filters: u16,

    // === Phase 5: 정렬 ===
    sort_column: SortColumn,
    sort_direction: SortDirection,

    // === Phase 4: 멀티셀렉션 앵커 ===
    anchor_index: Option<usize>,

    // === Phase 3: 네비게이션 히스토리 ===
    history: HistoryManager,

    // === Phase 4: 컨텍스트 메뉴 ===
    context_menu: Option<ContextMenu>,

    // === Phase 3: 브레드크럼 ===
    breadcrumb_segments: Vec<BreadcrumbSegment>,
}

impl SAssetBrowser {
    pub fn new(root_dir: PathBuf) -> Self {
        let mut browser = Self {
            id: crate::widget::next_widget_id(),
            dirty: InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT,
            current_dir: root_dir.clone(),
            root_dir,
            entries: Vec::new(),
            pending_action: None,
            visibility: Visibility::Visible,
            hovered_index: None,
            scroll_offset: 0.0,
            last_click_time: Instant::now(),
            last_click_index: None,
            theme: EditorTheme::default(),
            is_loading: false,
            view_mode: ViewMode::Grid,
            selected_indices: Vec::new(),
            filtered_indices: Vec::new(),
            filter_progress: 0,
            filter_complete: true,
            search_query: String::new(),
            search_cursor: 0,
            search_focused: false,
            asset_type_filters: 0,
            sort_column: SortColumn::Name,
            sort_direction: SortDirection::Ascending,
            anchor_index: None,
            history: HistoryManager::new(),
            context_menu: None,
            breadcrumb_segments: Vec::new(),
        };
        browser.rebuild_breadcrumbs();
        browser
    }

    /// 디렉토리 내용 설정 (외부에서 파일시스템 읽어서 전달)
    pub fn set_entries(&mut self, entries: Vec<AssetEntry>) {
        self.entries = entries;
        self.scroll_offset = 0.0;
        self.apply_filters();
    }

    /// 항목 수
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// 현재 디렉토리 가져오기
    pub fn current_dir(&self) -> &PathBuf {
        &self.current_dir
    }

    /// 디렉토리 변경
    pub fn set_current_dir(&mut self, dir: PathBuf) {
        self.current_dir = dir;
        self.selected_indices.clear();
        self.anchor_index = None;
        self.scroll_offset = 0.0;
        self.rebuild_breadcrumbs();
    }

    /// 현재 상태를 히스토리에 저장
    pub fn push_history(&mut self) {
        self.history.push(HistoryEntry {
            dir: self.current_dir.clone(),
            scroll_offset: self.scroll_offset,
        });
    }

    /// 대기 중인 액션 가져오기 (큐 비움)
    pub fn take_action(&mut self) -> AssetBrowserAction {
        self.pending_action.take().unwrap_or(AssetBrowserAction::None)
    }

    /// 히스토리 뒤로 — 성공 시 true (디렉토리 변경됨)
    pub fn go_back(&mut self) -> bool {
        if let Some((dir, scroll)) = self.history.go_back() {
            self.current_dir = dir;
            self.scroll_offset = scroll;
            self.selected_indices.clear();
            self.anchor_index = None;
            self.rebuild_breadcrumbs();
            true
        } else {
            false
        }
    }

    /// 히스토리 앞으로 — 성공 시 true (디렉토리 변경됨)
    pub fn go_forward(&mut self) -> bool {
        if let Some((dir, scroll)) = self.history.go_forward() {
            self.current_dir = dir;
            self.scroll_offset = scroll;
            self.selected_indices.clear();
            self.anchor_index = None;
            self.rebuild_breadcrumbs();
            true
        } else {
            false
        }
    }

    /// 로딩 상태 설정
    pub fn set_loading(&mut self, loading: bool) {
        self.is_loading = loading;
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    // ── 레이아웃 헬퍼 ──

    fn header_height(&self) -> f32 { self.theme.spacing.panel_header_height - self.theme.spacing.gap }
    fn grid_item_size(&self) -> f32 { self.theme.spacing.grid_item_size }
    fn grid_item_spacing(&self) -> f32 { self.theme.spacing.content_padding }
    fn list_item_height(&self) -> f32 { self.theme.spacing.control_height }
    fn grid_icon_size(&self) -> f32 { self.theme.spacing.grid_icon_size }

    /// 컨텐츠 시작 Y 계산
    fn content_start_y(&self) -> f32 {
        let ts = &self.theme.spacing;
        let hdr_h = self.header_height();
        let search_h = ts.search_bar_height;
        let pill_h = ts.filter_pill_height + ts.gap;
        let bread_h = ts.breadcrumb_height;
        let col_hdr_h = if self.view_mode == ViewMode::Column { ts.column_header_height } else { 0.0 };
        hdr_h + search_h + pill_h + bread_h + col_hdr_h
    }

    fn grid_columns(&self, width: f32) -> usize {
        let pad = self.theme.spacing.content_padding;
        let content_width = width - pad * 2.0;
        let item_total = self.grid_item_size() + self.grid_item_spacing();
        (content_width / item_total).floor().max(1.0) as usize
    }

    // ── 필터링 파이프라인 (Phase 1-D) ──

    /// 필터 재시작
    fn apply_filters(&mut self) {
        self.filtered_indices.clear();
        self.filter_progress = 0;
        self.filter_complete = false;
        // 필터 변경으로 인덱스 체계가 바뀌므로 선택 초기화
        self.selected_indices.clear();
        self.anchor_index = None;
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    /// UE5 ProcessItemsPendingFilter — 시간 예산 amortization
    fn process_pending_filter(&mut self, budget_secs: f64) {
        let start = Instant::now();
        let query_lower = self.search_query.to_lowercase();

        while self.filter_progress < self.entries.len() {
            let entry = &self.entries[self.filter_progress];

            let pass_text = query_lower.is_empty()
                || entry.name.to_lowercase().contains(&query_lower);

            // 폴더는 항상 표시 (UE5 패턴: 폴더는 타입 필터 무시)
            let pass_frontend = entry.is_directory
                || self.asset_type_filters == 0
                || (self.asset_type_filters & (1 << entry.asset_type as u16)) != 0;

            if pass_text && pass_frontend {
                self.filtered_indices.push(self.filter_progress);
            }
            self.filter_progress += 1;

            if self.filter_progress % 128 == 0
                && start.elapsed().as_secs_f64() > budget_secs
            {
                break;
            }
        }

        if self.filter_progress >= self.entries.len() {
            self.filter_complete = true;
            self.sort_filtered();
        }
        self.dirty |= InvalidateWidgetReason::PAINT;
    }

    // ── 정렬 (Phase 5-A) ──

    fn sort_filtered(&mut self) {
        let entries = &self.entries;
        let col = self.sort_column;
        let dir = self.sort_direction;
        self.filtered_indices.sort_by(|&a, &b| {
            let ea = &entries[a];
            let eb = &entries[b];
            // 폴더 항상 우선
            let folder_ord = eb.is_directory.cmp(&ea.is_directory);
            if folder_ord != std::cmp::Ordering::Equal {
                return folder_ord;
            }
            let ord = match col {
                SortColumn::Name => ea.name.to_lowercase().cmp(&eb.name.to_lowercase()),
                SortColumn::Type => (ea.asset_type as u8).cmp(&(eb.asset_type as u8)),
                SortColumn::Size => ea.size.cmp(&eb.size),
                SortColumn::Modified => ea.modified.cmp(&eb.modified),
            };
            match dir {
                SortDirection::Ascending => ord,
                SortDirection::Descending => ord.reverse(),
            }
        });
    }

    // ── 브레드크럼 (Phase 3-B) ──

    fn rebuild_breadcrumbs(&mut self) {
        self.breadcrumb_segments.clear();
        let rel = self.current_dir.strip_prefix(&self.root_dir)
            .unwrap_or(&self.current_dir);
        let mut accum = self.root_dir.clone();

        // 루트 세그먼트
        self.breadcrumb_segments.push(BreadcrumbSegment {
            label: "assets".to_string(),
            path: self.root_dir.clone(),
        });

        for component in rel.components() {
            accum.push(component);
            self.breadcrumb_segments.push(BreadcrumbSegment {
                label: component.as_os_str().to_string_lossy().to_string(),
                path: accum.clone(),
            });
        }
    }

    // ── 위치 hit-test (filtered_indices 기반) ──

    fn find_filtered_entry_at(&self, local_pos: Vec2, geometry: &Geometry) -> Option<usize> {
        let pad = self.theme.spacing.content_padding;
        let content_y_start = self.content_start_y();
        let content_y = local_pos.y - content_y_start;
        if content_y < 0.0 {
            return None;
        }

        let adjusted_y = content_y + self.scroll_offset;
        let count = self.filtered_indices.len();

        match self.view_mode {
            ViewMode::Grid => {
                let columns = self.grid_columns(geometry.local_size.x);
                let item_total = self.grid_item_size() + self.grid_item_spacing();
                let col = ((local_pos.x - pad) / item_total).floor() as usize;
                let row = (adjusted_y / item_total).floor() as usize;
                if col < columns {
                    let fi = row * columns + col;
                    if fi < count { return Some(fi); }
                }
            }
            ViewMode::List => {
                let fi = (adjusted_y / self.list_item_height()).floor() as usize;
                if fi < count { return Some(fi); }
            }
            ViewMode::Column => {
                let fi = (adjusted_y / self.list_item_height()).floor() as usize;
                if fi < count { return Some(fi); }
            }
        }

        None
    }

    // ── 컨텍스트 메뉴 빌드 (Phase 4-C) ──

    fn build_context_menu_items(&self) -> Vec<ContextMenuItem> {
        let has_selection = !self.selected_indices.is_empty();
        let single_path = if self.selected_indices.len() == 1 {
            let fi = self.selected_indices[0];
            self.filtered_indices.get(fi).map(|&ei| self.entries[ei].path.clone())
        } else {
            None
        };
        let is_dir = single_path.as_ref().map(|p| p.is_dir()).unwrap_or(false);
        let sel_path = single_path.clone().unwrap_or_else(|| self.current_dir.clone());

        let mut items = Vec::new();

        // Open (폴더=NavigateTo, 파일=Open)
        if has_selection {
            if is_dir {
                items.push(ContextMenuItem {
                    label: "Open",
                    action: AssetBrowserAction::NavigateTo(sel_path.clone()),
                    enabled: true,
                });
            } else {
                items.push(ContextMenuItem {
                    label: "Open",
                    action: AssetBrowserAction::Open(sel_path.clone()),
                    enabled: true,
                });
            }
        }

        // Rename
        if let Some(ref p) = single_path {
            items.push(ContextMenuItem {
                label: "Rename (F2)",
                action: AssetBrowserAction::Rename(p.clone()),
                enabled: true,
            });
        }

        // Delete
        if has_selection {
            items.push(ContextMenuItem {
                label: "Delete",
                action: AssetBrowserAction::Delete(sel_path.clone()),
                enabled: true,
            });
        }

        // Copy Path
        items.push(ContextMenuItem {
            label: "Copy Path",
            action: AssetBrowserAction::CopyPath(sel_path.clone()),
            enabled: true,
        });

        // Show in Explorer
        items.push(ContextMenuItem {
            label: "Show in Explorer",
            action: AssetBrowserAction::ShowInExplorer(sel_path),
            enabled: true,
        });

        // New Folder
        items.push(ContextMenuItem {
            label: "New Folder",
            action: AssetBrowserAction::CreateFolder,
            enabled: true,
        });

        items
    }

    /// 컨텐츠 전체 높이 (스크롤 범위 계산용)
    fn total_content_height(&self, width: f32) -> f32 {
        let count = self.filtered_indices.len();
        match self.view_mode {
            ViewMode::Grid => {
                let columns = self.grid_columns(width);
                let rows = (count + columns - 1) / columns.max(1);
                rows as f32 * (self.grid_item_size() + self.grid_item_spacing())
            }
            ViewMode::List | ViewMode::Column => {
                count as f32 * self.list_item_height()
            }
        }
    }

    /// 선택된 항목이 화면에 보이도록 스크롤 조정
    fn ensure_visible(&mut self, fi: usize, geometry_width: f32, geometry_height: f32) {
        let content_y_start = self.content_start_y();
        let visible_h = geometry_height - content_y_start;

        let (item_top, item_bottom) = match self.view_mode {
            ViewMode::Grid => {
                let columns = self.grid_columns(geometry_width);
                let item_total = self.grid_item_size() + self.grid_item_spacing();
                let row = fi / columns.max(1);
                let top = row as f32 * item_total;
                (top, top + self.grid_item_size())
            }
            ViewMode::List | ViewMode::Column => {
                let top = fi as f32 * self.list_item_height();
                (top, top + self.list_item_height())
            }
        };

        if item_top < self.scroll_offset {
            self.scroll_offset = item_top;
        } else if item_bottom > self.scroll_offset + visible_h {
            self.scroll_offset = item_bottom - visible_h;
        }
    }

    /// 브레드크럼 hit-test (레이아웃 재계산)
    fn breadcrumb_hit_test(&self, local_x: f32) -> Option<PathBuf> {
        let ts = &self.theme.spacing;
        let tf = &self.theme.fonts;
        let pad = ts.content_padding;
        let nav_w = ts.nav_button_size;

        // [<] [>] 뒤의 위치
        let mut x = pad + nav_w * 2.0 + pad;

        for (i, seg) in self.breadcrumb_segments.iter().enumerate() {
            if i > 0 {
                // '>' 구분자 폭
                x += tf.large;
            }
            let label_w = seg.label.len() as f32 * tf.large * 0.55;
            if local_x >= x && local_x < x + label_w {
                return Some(seg.path.clone());
            }
            x += label_w;
        }
        None
    }

    /// 필터 pill hit-test
    fn pill_hit_test(&self, local_x: f32, pill_y: f32, local_pos_y: f32) -> Option<usize> {
        let ts = &self.theme.spacing;
        let tf = &self.theme.fonts;
        let pad = ts.content_padding;
        let pill_h = ts.filter_pill_height;

        if local_pos_y < pill_y || local_pos_y >= pill_y + pill_h {
            return None;
        }

        let mut x = pad;
        for (i, at) in FILTER_TYPES.iter().enumerate() {
            let label_w = at.label().len() as f32 * tf.large * 0.55 + pad * 2.0;
            if local_x >= x && local_x < x + label_w {
                return Some(i);
            }
            x += label_w + ts.filter_pill_spacing;
        }
        None
    }

    /// Column 헤더 hit-test
    fn column_header_hit_test(&self, local_x: f32, width: f32) -> Option<SortColumn> {
        // 컬럼 폭: Name=50%, Type=15%, Size=15%, Modified=20%
        let name_end = width * 0.50;
        let type_end = width * 0.65;
        let size_end = width * 0.80;

        if local_x < name_end { Some(SortColumn::Name) }
        else if local_x < type_end { Some(SortColumn::Type) }
        else if local_x < size_end { Some(SortColumn::Size) }
        else { Some(SortColumn::Modified) }
    }

    /// 사이즈 포맷
    fn format_size(size: u64) -> String {
        if size < 1024 { return format!("{} B", size); }
        let kb = size as f64 / 1024.0;
        if kb < 1024.0 { return format!("{:.1} KB", kb); }
        let mb = kb / 1024.0;
        if mb < 1024.0 { return format!("{:.1} MB", mb); }
        let gb = mb / 1024.0;
        format!("{:.1} GB", gb)
    }

    /// 수정 시간 포맷
    fn format_modified(time: Option<std::time::SystemTime>) -> String {
        match time {
            Some(t) => {
                let duration = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                let secs = duration.as_secs();
                // 간단 UTC 포맷: YYYY-MM-DD
                let days = secs / 86400;
                let y = 1970 + (days / 365); // 근사치
                let remaining = days % 365;
                let m = remaining / 30 + 1;
                let d = remaining % 30 + 1;
                format!("{:04}-{:02}-{:02}", y, m.min(12), d.min(31))
            }
            None => "-".to_string(),
        }
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

    // ── Tick (Phase 1-E) ──

    fn can_tick(&self) -> bool { !self.filter_complete }

    fn tick(&mut self, _delta_time: f32) {
        if !self.filter_complete {
            self.process_pending_filter(0.002); // 2ms 프레임 예산
        }
    }

    // ── Paint ──

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
        let grid_sz = self.grid_item_size();
        let grid_sp = self.grid_item_spacing();
        let list_h = self.list_item_height();
        let icon_sz = self.grid_icon_size();

        // ── 배경 ──
        draw_elements.add_box(current_layer, geometry.to_paint_geometry(), tc.panel_bg);
        current_layer += 1;

        // ── 헤더 ──
        draw_elements.add_box(
            current_layer,
            geometry.paint_at(geometry.absolute_position, Vec2::new(geometry.local_size.x, hdr_h)),
            tc.header_bg,
        );

        // [<] [>] 네비게이션 버튼
        let nav_sz = ts.nav_button_size;
        let nav_y = (hdr_h - nav_sz) * 0.5;
        let back_color = if self.history.can_go_back() { tc.text_primary } else { tc.text_muted };
        let fwd_color = if self.history.can_go_forward() { tc.text_primary } else { tc.text_muted };
        draw_elements.add_text(
            current_layer + 1,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(pad, nav_y),
                Vec2::new(nav_sz, nav_sz),
            ),
            "<".to_string(), back_color, tf.large,
        );
        draw_elements.add_text(
            current_layer + 1,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(pad + nav_sz, nav_y),
                Vec2::new(nav_sz, nav_sz),
            ),
            ">".to_string(), fwd_color, tf.large,
        );

        // 타이틀
        let title_x = pad + nav_sz * 2.0 + pad;
        draw_elements.add_text(
            current_layer + 1,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(title_x, (hdr_h - tf.large) * 0.5),
                Vec2::new(120.0, tf.large),
            ),
            "Asset Browser".to_string(),
            tc.sidebar_drawer_header_text,
            tf.large,
        );

        // 뷰 모드 토글
        let mode_text = match self.view_mode {
            ViewMode::Grid => "Grid",
            ViewMode::List => "List",
            ViewMode::Column => "Col",
        };
        let toggle_w = ts.toolbar_small_button_width + ts.content_padding;
        let toggle_h = ts.small_control_height;
        let toggle_x = geometry.local_size.x - toggle_w - ts.gap;
        let toggle_y = (hdr_h - toggle_h) * 0.5;
        draw_elements.add_box(
            current_layer + 1,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(toggle_x, toggle_y),
                Vec2::new(toggle_w, toggle_h),
            ),
            tc.border,
        );
        draw_elements.add_text(
            current_layer + 2,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(toggle_x + ts.gap, toggle_y + (toggle_h - tf.large) * 0.5),
                Vec2::new(toggle_w - ts.gap * 2.0, tf.large),
            ),
            mode_text.to_string(), tc.text_primary, tf.large,
        );
        current_layer += 3;

        // ── 검색 바 ──
        let search_y = hdr_h;
        let search_h = ts.search_bar_height;
        draw_elements.add_box(
            current_layer,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(0.0, search_y),
                Vec2::new(geometry.local_size.x, search_h),
            ),
            tc.search_bg,
        );
        // 포커스 시 테두리
        if self.search_focused {
            draw_elements.add_box(
                current_layer + 1,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(0.0, search_y),
                    Vec2::new(geometry.local_size.x, 1.0),
                ),
                tc.focus_border,
            );
            draw_elements.add_box(
                current_layer + 1,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(0.0, search_y + search_h - 1.0),
                    Vec2::new(geometry.local_size.x, 1.0),
                ),
                tc.focus_border,
            );
        }
        // 검색 텍스트/placeholder
        let search_text_y = search_y + (search_h - tf.large) * 0.5;
        if self.search_query.is_empty() {
            draw_elements.add_text(
                current_layer + 2,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pad, search_text_y),
                    Vec2::new(geometry.local_size.x - pad * 2.0, tf.large),
                ),
                "Search...".to_string(), tc.text_muted, tf.large,
            );
        } else {
            draw_elements.add_text(
                current_layer + 2,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pad, search_text_y),
                    Vec2::new(geometry.local_size.x - pad * 2.0, tf.large),
                ),
                self.search_query.clone(), tc.text_primary, tf.large,
            );
        }
        current_layer += 3;

        // ── 필터 pill 바 ──
        let pill_y = search_y + search_h;
        let pill_h = ts.filter_pill_height;
        let mut pill_x = pad;
        for at in &FILTER_TYPES {
            let active = self.asset_type_filters & (1 << *at as u16) != 0;
            let bg = if active { tc.filter_pill_active } else { tc.filter_pill_inactive };
            let label = at.label();
            let label_w = label.len() as f32 * tf.large * 0.55 + pad * 2.0;

            draw_elements.add_box(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pill_x, pill_y),
                    Vec2::new(label_w, pill_h),
                ),
                bg,
            );
            draw_elements.add_text(
                current_layer + 1,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pill_x + pad, pill_y + (pill_h - tf.large) * 0.5),
                    Vec2::new(label_w - pad * 2.0, tf.large),
                ),
                label.to_string(),
                if active { tc.text_bright } else { tc.text_secondary },
                tf.large,
            );
            pill_x += label_w + ts.filter_pill_spacing;
        }
        current_layer += 2;

        // ── 브레드크럼 ──
        let bread_y = pill_y + pill_h + ts.gap;
        let bread_h = ts.breadcrumb_height;
        draw_elements.add_box(
            current_layer,
            geometry.paint_at(
                geometry.absolute_position + Vec2::new(0.0, bread_y),
                Vec2::new(geometry.local_size.x, bread_h),
            ),
            tc.control_bg_hover,
        );

        let bread_text_y = bread_y + (bread_h - tf.large) * 0.5;
        let mut bx = pad + ts.nav_button_size * 2.0 + pad; // 네비 버튼 뒤
        for (i, seg) in self.breadcrumb_segments.iter().enumerate() {
            if i > 0 {
                // '>' 구분자
                draw_elements.add_text(
                    current_layer + 1,
                    geometry.paint_at(
                        geometry.absolute_position + Vec2::new(bx, bread_text_y),
                        Vec2::new(tf.large, tf.large),
                    ),
                    ">".to_string(), tc.breadcrumb_separator, tf.large,
                );
                bx += tf.large;
            }
            let lbl_w = seg.label.len() as f32 * tf.large * 0.55;
            let is_last = i == self.breadcrumb_segments.len() - 1;
            draw_elements.add_text(
                current_layer + 1,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(bx, bread_text_y),
                    Vec2::new(lbl_w, tf.large),
                ),
                seg.label.clone(),
                if is_last { tc.text_primary } else { tc.text_secondary },
                tf.large,
            );
            bx += lbl_w;
        }
        current_layer += 2;

        // ── Column 헤더 (Column 모드만) ──
        if self.view_mode == ViewMode::Column {
            let col_hdr_y = bread_y + bread_h;
            let col_hdr_h = ts.column_header_height;

            draw_elements.add_box(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(0.0, col_hdr_y),
                    Vec2::new(geometry.local_size.x, col_hdr_h),
                ),
                tc.column_header_bg,
            );

            let w = geometry.local_size.x;
            let col_text_y = col_hdr_y + (col_hdr_h - tf.large) * 0.5;

            // 컬럼 헤더 텍스트
            let cols: [(f32, f32, &str, SortColumn); 4] = [
                (0.0, w * 0.50, "Name", SortColumn::Name),
                (w * 0.50, w * 0.15, "Type", SortColumn::Type),
                (w * 0.65, w * 0.15, "Size", SortColumn::Size),
                (w * 0.80, w * 0.20, "Modified", SortColumn::Modified),
            ];
            for &(cx, cw, label, col) in &cols {
                let arrow = if self.sort_column == col {
                    match self.sort_direction {
                        SortDirection::Ascending => " ^",
                        SortDirection::Descending => " v",
                    }
                } else {
                    ""
                };
                draw_elements.add_text(
                    current_layer + 1,
                    geometry.paint_at(
                        geometry.absolute_position + Vec2::new(cx + pad, col_text_y),
                        Vec2::new(cw - pad * 2.0, tf.large),
                    ),
                    format!("{}{}", label, arrow), tc.text_secondary, tf.large,
                );
                // 구분선
                if cx > 0.0 {
                    draw_elements.add_box(
                        current_layer + 1,
                        geometry.paint_at(
                            geometry.absolute_position + Vec2::new(cx, col_hdr_y),
                            Vec2::new(1.0, col_hdr_h),
                        ),
                        tc.column_separator,
                    );
                }
            }
            current_layer += 2;
        }

        // ── 컨텐츠 영역 ──
        let content_y = self.content_start_y();

        if self.is_loading {
            draw_elements.add_text(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pad, content_y + pad),
                    Vec2::new(geometry.local_size.x - pad * 2.0, tf.large),
                ),
                "Loading...".to_string(), tc.text_muted, tf.large,
            );
            current_layer += 1;
        } else if self.filtered_indices.is_empty() && self.filter_complete {
            let msg = if self.entries.is_empty() { "Empty" } else { "No matches" };
            draw_elements.add_text(
                current_layer,
                geometry.paint_at(
                    geometry.absolute_position + Vec2::new(pad, content_y + pad),
                    Vec2::new(geometry.local_size.x - pad * 2.0, tf.large),
                ),
                msg.to_string(), tc.text_muted, tf.large,
            );
            current_layer += 1;
        } else {
            match self.view_mode {
                ViewMode::Grid => {
                    let columns = self.grid_columns(geometry.local_size.x);
                    let item_total = grid_sz + grid_sp;

                    for (fi, &entry_idx) in self.filtered_indices.iter().enumerate() {
                        let entry = &self.entries[entry_idx];
                        let col = fi % columns;
                        let row = fi / columns;
                        let item_x = pad + (col as f32) * item_total;
                        let item_y = content_y + (row as f32) * item_total - self.scroll_offset;

                        if item_y + grid_sz < content_y || item_y > geometry.local_size.y {
                            continue;
                        }

                        let is_selected = self.selected_indices.contains(&fi);
                        let is_hovered = self.hovered_index == Some(fi);
                        let bg_color = if is_selected {
                            tc.selection_bg
                        } else if is_hovered {
                            tc.hover_overlay
                        } else {
                            tc.control_bg_hover
                        };

                        draw_elements.add_box(
                            current_layer,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(item_x, item_y),
                                Vec2::new(grid_sz, grid_sz),
                            ),
                            bg_color,
                        );

                        // 아이콘
                        let icon_x = item_x + (grid_sz - icon_sz) * 0.5;
                        let icon_y = item_y + pad;
                        draw_elements.add_box(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(icon_x, icon_y),
                                Vec2::new(icon_sz, icon_sz),
                            ),
                            entry.asset_type.color(tc),
                        );
                        let icon_text_pad = ts.gap;
                        draw_elements.add_text(
                            current_layer + 2,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(icon_x + icon_text_pad, icon_y + (icon_sz - tf.large) * 0.5),
                                Vec2::new(icon_sz - icon_text_pad * 2.0, tf.large),
                            ),
                            entry.asset_type.icon().to_string(), tc.text_primary, tf.large,
                        );

                        // 파일명
                        draw_elements.add_text(
                            current_layer + 2,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(item_x + 2.0, item_y + grid_sz - tf.large - pad),
                                Vec2::new(grid_sz - ts.gap, tf.large + 2.0),
                            ),
                            truncate_text(&entry.name, 12), tc.text_primary, tf.large,
                        );
                    }
                    current_layer += 3;
                }
                ViewMode::List => {
                    let text_v_pad = (list_h - tf.large) * 0.5;
                    let icon_area_w = pad + tf.large * 2.5;

                    for (fi, &entry_idx) in self.filtered_indices.iter().enumerate() {
                        let entry = &self.entries[entry_idx];
                        let item_y = content_y + (fi as f32) * list_h - self.scroll_offset;

                        if item_y + list_h < content_y || item_y > geometry.local_size.y {
                            continue;
                        }

                        let is_selected = self.selected_indices.contains(&fi);
                        let is_hovered = self.hovered_index == Some(fi);
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
                                geometry.paint_at(
                                    geometry.absolute_position + Vec2::new(0.0, item_y),
                                    Vec2::new(geometry.local_size.x, list_h),
                                ),
                                bg_color,
                            );
                        }

                        draw_elements.add_text(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(pad, item_y + text_v_pad),
                                Vec2::new(icon_area_w - pad, tf.large),
                            ),
                            entry.asset_type.icon().to_string(),
                            entry.asset_type.color(tc), tf.large,
                        );

                        draw_elements.add_text(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(icon_area_w, item_y + text_v_pad),
                                Vec2::new(geometry.local_size.x - icon_area_w - pad, tf.large),
                            ),
                            entry.name.clone(), tc.text_primary, tf.large,
                        );
                    }
                    current_layer += 2;
                }
                ViewMode::Column => {
                    let text_v_pad = (list_h - tf.large) * 0.5;
                    let w = geometry.local_size.x;
                    let icon_area_w = pad + tf.large * 2.5;
                    let name_w = w * 0.50;

                    for (fi, &entry_idx) in self.filtered_indices.iter().enumerate() {
                        let entry = &self.entries[entry_idx];
                        let item_y = content_y + (fi as f32) * list_h - self.scroll_offset;

                        if item_y + list_h < content_y || item_y > geometry.local_size.y {
                            continue;
                        }

                        let is_selected = self.selected_indices.contains(&fi);
                        let is_hovered = self.hovered_index == Some(fi);
                        let bg_color = if is_selected {
                            tc.selection_bg
                        } else if is_hovered {
                            tc.hover_overlay
                        } else if fi % 2 == 1 {
                            tc.row_stripe_bg
                        } else {
                            Color::TRANSPARENT
                        };

                        if bg_color.a > 0.0 {
                            draw_elements.add_box(
                                current_layer,
                                geometry.paint_at(
                                    geometry.absolute_position + Vec2::new(0.0, item_y),
                                    Vec2::new(w, list_h),
                                ),
                                bg_color,
                            );
                        }

                        // Name 컬럼
                        draw_elements.add_text(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(pad, item_y + text_v_pad),
                                Vec2::new(icon_area_w - pad, tf.large),
                            ),
                            entry.asset_type.icon().to_string(),
                            entry.asset_type.color(tc), tf.large,
                        );
                        draw_elements.add_text(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(icon_area_w, item_y + text_v_pad),
                                Vec2::new(name_w - icon_area_w - pad, tf.large),
                            ),
                            entry.name.clone(), tc.text_primary, tf.large,
                        );

                        // Type 컬럼
                        let type_label = if entry.is_directory { "Folder" } else { entry.asset_type.label() };
                        draw_elements.add_text(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(w * 0.50 + pad, item_y + text_v_pad),
                                Vec2::new(w * 0.15 - pad * 2.0, tf.large),
                            ),
                            type_label.to_string(), tc.text_secondary, tf.large,
                        );

                        // Size 컬럼
                        let size_str = if entry.is_directory { "-".to_string() } else { Self::format_size(entry.size) };
                        draw_elements.add_text(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(w * 0.65 + pad, item_y + text_v_pad),
                                Vec2::new(w * 0.15 - pad * 2.0, tf.large),
                            ),
                            size_str, tc.text_secondary, tf.large,
                        );

                        // Modified 컬럼
                        draw_elements.add_text(
                            current_layer + 1,
                            geometry.paint_at(
                                geometry.absolute_position + Vec2::new(w * 0.80 + pad, item_y + text_v_pad),
                                Vec2::new(w * 0.20 - pad * 2.0, tf.large),
                            ),
                            Self::format_modified(entry.modified), tc.text_secondary, tf.large,
                        );

                        // 컬럼 구분선
                        for &col_x in &[w * 0.50, w * 0.65, w * 0.80] {
                            draw_elements.add_box(
                                current_layer + 1,
                                geometry.paint_at(
                                    geometry.absolute_position + Vec2::new(col_x, item_y),
                                    Vec2::new(1.0, list_h),
                                ),
                                tc.column_separator,
                            );
                        }
                    }
                    current_layer += 2;
                }
            }
        }

        // ── 컨텍스트 메뉴 (dropdown layer) ──
        if let Some(ref menu) = self.context_menu {
            let menu_base_layer = current_layer + 10;
            draw_elements.set_dropdown_layer(menu_base_layer);

            let menu_w = ts.context_menu_width;
            let item_h = ts.context_menu_item_height;
            let menu_h = menu.items.len() as f32 * item_h;

            // 메뉴 위치를 로컬 좌표로 변환
            let menu_local = geometry.absolute_to_local(menu.position);
            let menu_abs = geometry.absolute_position + menu_local;

            // 그림자
            draw_elements.add_box(
                menu_base_layer,
                geometry.paint_at(
                    menu_abs + Vec2::new(2.0, 2.0),
                    Vec2::new(menu_w, menu_h),
                ),
                tc.shadow,
            );

            // 배경
            draw_elements.add_box(
                menu_base_layer + 1,
                geometry.paint_at(menu_abs, Vec2::new(menu_w, menu_h)),
                tc.popup_bg,
            );

            // 항목
            for (i, item) in menu.items.iter().enumerate() {
                let iy = menu_local.y + i as f32 * item_h;
                let text_color = if item.enabled { tc.text_primary } else { tc.text_muted };

                draw_elements.add_text(
                    menu_base_layer + 3,
                    geometry.paint_at(
                        geometry.absolute_position + Vec2::new(menu_local.x + pad, iy + (item_h - tf.large) * 0.5),
                        Vec2::new(menu_w - pad * 2.0, tf.large),
                    ),
                    item.label.to_string(), text_color, tf.large,
                );
            }

            // 테두리
            draw_elements.add_box(
                menu_base_layer + 2,
                geometry.paint_at(menu_abs, Vec2::new(menu_w, 1.0)),
                tc.popup_border,
            );
            draw_elements.add_box(
                menu_base_layer + 2,
                geometry.paint_at(menu_abs + Vec2::new(0.0, menu_h - 1.0), Vec2::new(menu_w, 1.0)),
                tc.popup_border,
            );
            draw_elements.add_box(
                menu_base_layer + 2,
                geometry.paint_at(menu_abs, Vec2::new(1.0, menu_h)),
                tc.popup_border,
            );
            draw_elements.add_box(
                menu_base_layer + 2,
                geometry.paint_at(menu_abs + Vec2::new(menu_w - 1.0, 0.0), Vec2::new(1.0, menu_h)),
                tc.popup_border,
            );

            current_layer = menu_base_layer + 4;
        }

        debug_assert!(current_layer >= layer, "on_paint must return layer >= input");
        current_layer
    }

    // ── 입력 이벤트 ──

    fn on_mouse_move(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        let new_hover = self.find_filtered_entry_at(local_pos, geometry);
        if new_hover != self.hovered_index {
            self.hovered_index = new_hover;
            self.dirty |= InvalidateWidgetReason::PAINT;
        }
        Reply::unhandled()
    }

    fn on_mouse_leave(&mut self, _event: &PointerEvent) {
        self.hovered_index = None;
    }

    fn on_mouse_button_down(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let local_pos = geometry.absolute_to_local(event.screen_position);
        let ts = &self.theme.spacing;

        // === Z축 우선순위 1: 컨텍스트 메뉴 처리 ===
        if let Some(ref menu) = self.context_menu {
            let menu_local = geometry.absolute_to_local(menu.position);
            let menu_w = ts.context_menu_width;
            let menu_h = menu.items.len() as f32 * ts.context_menu_item_height;
            let menu_rect = SlateRect::new(
                menu_local.x, menu_local.y,
                menu_local.x + menu_w, menu_local.y + menu_h,
            );
            if menu_rect.contains(local_pos) {
                let idx = ((local_pos.y - menu_local.y) / ts.context_menu_item_height) as usize;
                if let Some(item) = menu.items.get(idx) {
                    if item.enabled {
                        self.pending_action = Some(item.action.clone());
                    }
                }
                self.context_menu = None;
                self.dirty |= InvalidateWidgetReason::PAINT;
                return Reply::handled();
            } else {
                self.context_menu = None;
                self.dirty |= InvalidateWidgetReason::PAINT;
                return Reply::handled();
            }
        }

        // 우클릭 → 컨텍스트 메뉴
        if event.is_right_button() {
            // 우클릭한 항목 선택
            if let Some(fi) = self.find_filtered_entry_at(local_pos, geometry) {
                if !self.selected_indices.contains(&fi) {
                    self.selected_indices = vec![fi];
                    self.anchor_index = Some(fi);
                }
            }
            let items = self.build_context_menu_items();
            self.context_menu = Some(ContextMenu {
                position: event.screen_position,
                items,
            });
            self.dirty |= InvalidateWidgetReason::PAINT;
            return Reply::handled();
        }

        // 좌클릭이 아니면 무시
        if !event.is_left_button() { return Reply::unhandled(); }

        let hdr_h = self.header_height();

        // 헤더 영역: 네비게이션 버튼 + 뷰 모드 토글
        if local_pos.y < hdr_h {
            let nav_sz = ts.nav_button_size;
            let pad = ts.content_padding;

            // [<] 버튼
            if local_pos.x >= pad && local_pos.x < pad + nav_sz {
                if self.history.can_go_back() {
                    self.pending_action = Some(AssetBrowserAction::GoBack);
                }
                return Reply::handled();
            }
            // [>] 버튼
            if local_pos.x >= pad + nav_sz && local_pos.x < pad + nav_sz * 2.0 {
                if self.history.can_go_forward() {
                    self.pending_action = Some(AssetBrowserAction::GoForward);
                }
                return Reply::handled();
            }

            // 뷰 모드 토글
            let toggle_w = ts.toolbar_small_button_width + ts.content_padding;
            if local_pos.x > geometry.local_size.x - toggle_w - ts.gap {
                self.view_mode = match self.view_mode {
                    ViewMode::Grid => ViewMode::List,
                    ViewMode::List => ViewMode::Column,
                    ViewMode::Column => ViewMode::Grid,
                };
                self.dirty |= InvalidateWidgetReason::PAINT;
                return Reply::handled();
            }
        }

        // 검색 바 클릭 → 포커스
        let search_y = hdr_h;
        let search_h = ts.search_bar_height;
        if local_pos.y >= search_y && local_pos.y < search_y + search_h {
            self.search_focused = true;
            self.dirty |= InvalidateWidgetReason::PAINT;
            return Reply::handled();
        } else if self.search_focused {
            self.search_focused = false;
            self.dirty |= InvalidateWidgetReason::PAINT;
        }

        // 필터 pill 클릭
        let pill_y = search_y + search_h;
        if let Some(pill_idx) = self.pill_hit_test(local_pos.x, pill_y, local_pos.y) {
            let at = FILTER_TYPES[pill_idx];
            self.asset_type_filters ^= 1 << at as u16;
            self.apply_filters();
            return Reply::handled();
        }

        // 브레드크럼 클릭
        let bread_y = pill_y + ts.filter_pill_height + ts.gap;
        let bread_h = ts.breadcrumb_height;
        if local_pos.y >= bread_y && local_pos.y < bread_y + bread_h {
            if let Some(path) = self.breadcrumb_hit_test(local_pos.x) {
                self.pending_action = Some(AssetBrowserAction::NavigateTo(path));
                return Reply::handled();
            }
        }

        // Column 헤더 클릭
        if self.view_mode == ViewMode::Column {
            let col_hdr_y = bread_y + bread_h;
            let col_hdr_h = ts.column_header_height;
            if local_pos.y >= col_hdr_y && local_pos.y < col_hdr_y + col_hdr_h {
                if let Some(col) = self.column_header_hit_test(local_pos.x, geometry.local_size.x) {
                    if self.sort_column == col {
                        self.sort_direction = match self.sort_direction {
                            SortDirection::Ascending => SortDirection::Descending,
                            SortDirection::Descending => SortDirection::Ascending,
                        };
                    } else {
                        self.sort_column = col;
                        self.sort_direction = SortDirection::Ascending;
                    }
                    self.sort_filtered();
                    self.dirty |= InvalidateWidgetReason::PAINT;
                    return Reply::handled();
                }
            }
        }

        // 항목 클릭 (멀티셀렉션)
        if let Some(fi) = self.find_filtered_entry_at(local_pos, geometry) {
            let now = Instant::now();
            let is_double_click = self.last_click_index == Some(fi)
                && now.duration_since(self.last_click_time).as_secs_f64() < 0.4;

            self.last_click_time = now;
            self.last_click_index = Some(fi);

            if is_double_click {
                if let Some(&entry_idx) = self.filtered_indices.get(fi) {
                    let entry = &self.entries[entry_idx];
                    if entry.is_directory {
                        self.pending_action = Some(AssetBrowserAction::NavigateTo(entry.path.clone()));
                    } else {
                        self.pending_action = Some(AssetBrowserAction::Open(entry.path.clone()));
                    }
                }
            } else if event.modifiers.ctrl {
                // Ctrl+클릭: 토글
                if let Some(pos) = self.selected_indices.iter().position(|&x| x == fi) {
                    self.selected_indices.remove(pos);
                } else {
                    self.selected_indices.push(fi);
                }
            } else if event.modifiers.shift && self.anchor_index.is_some() {
                // Shift+클릭: 범위 선택
                let anchor = self.anchor_index.unwrap();
                let (lo, hi) = if fi < anchor { (fi, anchor) } else { (anchor, fi) };
                self.selected_indices = (lo..=hi).collect();
            } else {
                // 단일 클릭
                self.selected_indices = vec![fi];
                self.anchor_index = Some(fi);
                if let Some(&entry_idx) = self.filtered_indices.get(fi) {
                    let entry = &self.entries[entry_idx];
                    self.pending_action = Some(AssetBrowserAction::Select(entry.path.clone()));
                }
            }

            self.dirty |= InvalidateWidgetReason::PAINT;
            return Reply::handled();
        }

        Reply::unhandled()
    }

    fn on_mouse_wheel(&mut self, geometry: &Geometry, event: &PointerEvent) -> Reply {
        let total_height = self.total_content_height(geometry.local_size.x);
        let content_height = geometry.local_size.y - self.content_start_y();
        let max_scroll = (total_height - content_height).max(0.0);

        self.scroll_offset = (self.scroll_offset - event.wheel_delta * 40.0)
            .max(0.0)
            .min(max_scroll);

        Reply::handled()
    }

    // ── 키보드 입력 (Phase 2-B, 4-B) ──

    fn on_key_char(&mut self, _geometry: &Geometry, event: &CharEvent) -> Reply {
        if !self.search_focused { return Reply::unhandled(); }

        match event.character {
            '\u{8}' => {
                // Backspace
                if self.search_cursor > 0 {
                    // 커서 이전 문자 삭제
                    let prev_char_start = self.search_query[..self.search_cursor]
                        .char_indices()
                        .last()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.search_query.remove(prev_char_start);
                    self.search_cursor = prev_char_start;
                    self.apply_filters();
                }
            }
            '\u{1b}' => {
                // ESC: 검색 포커스 해제
                self.search_focused = false;
            }
            c if !c.is_control() => {
                self.search_query.insert(self.search_cursor, c);
                self.search_cursor += c.len_utf8();
                self.apply_filters();
            }
            _ => return Reply::unhandled(),
        }
        self.dirty |= InvalidateWidgetReason::PAINT;
        Reply::handled()
    }

    fn on_key_down(&mut self, geometry: &Geometry, event: &KeyEvent) -> Reply {
        if self.search_focused { return Reply::unhandled(); }

        let count = self.filtered_indices.len();
        if count == 0 {
            // 검색 포커스 단축키만 처리
            if event.key == KeyCode::F && event.modifiers.ctrl {
                self.search_focused = true;
                self.dirty |= InvalidateWidgetReason::PAINT;
                return Reply::handled();
            }
            return Reply::unhandled();
        }

        match event.key {
            KeyCode::Up | KeyCode::Left => {
                let current = self.selected_indices.first().copied().unwrap_or(0);
                let next = if current > 0 { current - 1 } else { 0 };
                self.selected_indices = vec![next];
                self.anchor_index = Some(next);
                self.ensure_visible(next, geometry.local_size.x, geometry.local_size.y);
                self.dirty |= InvalidateWidgetReason::PAINT;
                Reply::handled()
            }
            KeyCode::Down | KeyCode::Right => {
                let current = self.selected_indices.first().copied().unwrap_or(0);
                let next = (current + 1).min(count - 1);
                self.selected_indices = vec![next];
                self.anchor_index = Some(next);
                self.ensure_visible(next, geometry.local_size.x, geometry.local_size.y);
                self.dirty |= InvalidateWidgetReason::PAINT;
                Reply::handled()
            }
            KeyCode::Enter => {
                if let Some(&fi) = self.selected_indices.first() {
                    if let Some(&entry_idx) = self.filtered_indices.get(fi) {
                        let entry = &self.entries[entry_idx];
                        if entry.is_directory {
                            self.pending_action = Some(AssetBrowserAction::NavigateTo(entry.path.clone()));
                        } else {
                            self.pending_action = Some(AssetBrowserAction::Open(entry.path.clone()));
                        }
                    }
                }
                Reply::handled()
            }
            KeyCode::Backspace => {
                self.pending_action = Some(AssetBrowserAction::GoBack);
                Reply::handled()
            }
            KeyCode::Delete => {
                if let Some(&fi) = self.selected_indices.first() {
                    if let Some(&entry_idx) = self.filtered_indices.get(fi) {
                        self.pending_action = Some(AssetBrowserAction::Delete(self.entries[entry_idx].path.clone()));
                    }
                }
                Reply::handled()
            }
            KeyCode::F2 => {
                if let Some(&fi) = self.selected_indices.first() {
                    if let Some(&entry_idx) = self.filtered_indices.get(fi) {
                        self.pending_action = Some(AssetBrowserAction::Rename(self.entries[entry_idx].path.clone()));
                    }
                }
                Reply::handled()
            }
            KeyCode::A if event.modifiers.ctrl => {
                // Ctrl+A: 전체 선택
                self.selected_indices = (0..count).collect();
                self.dirty |= InvalidateWidgetReason::PAINT;
                Reply::handled()
            }
            KeyCode::F if event.modifiers.ctrl => {
                // Ctrl+F: 검색 포커스
                self.search_focused = true;
                self.dirty |= InvalidateWidgetReason::PAINT;
                Reply::handled()
            }
            _ => Reply::unhandled(),
        }
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
