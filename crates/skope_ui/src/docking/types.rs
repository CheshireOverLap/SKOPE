//! 도킹 시스템 핵심 타입

use glam::Vec2;
use serde::{Serialize, Deserialize};

/// 노드 고유 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 탭 고유 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

impl TabId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 분할 방향
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDirection {
    /// 가로 분할 (좌우로 나눔)
    Horizontal,
    /// 세로 분할 (상하로 나눔)
    Vertical,
}

impl SplitDirection {
    /// 반대 방향
    pub fn opposite(&self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }

    /// 이 방향으로 분할할 때의 주 축
    pub fn main_axis(&self) -> Axis {
        match self {
            Self::Horizontal => Axis::X,
            Self::Vertical => Axis::Y,
        }
    }
}

/// 축
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
}

/// 도킹 위치
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockPosition {
    /// 탭으로 병합 (가운데)
    Center,
    /// 왼쪽에 분할 도킹
    Left,
    /// 오른쪽에 분할 도킹
    Right,
    /// 위쪽에 분할 도킹
    Top,
    /// 아래쪽에 분할 도킹
    Bottom,
}

impl DockPosition {
    /// 분할 방향 반환 (Center는 None)
    pub fn split_direction(&self) -> Option<SplitDirection> {
        match self {
            Self::Center => None,
            Self::Left | Self::Right => Some(SplitDirection::Horizontal),
            Self::Top | Self::Bottom => Some(SplitDirection::Vertical),
        }
    }

    /// 분할 시 새 노드가 첫 번째 자식인지
    pub fn is_first_child(&self) -> bool {
        matches!(self, Self::Left | Self::Top)
    }
}

/// 노드 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// 루트 영역 (OS 윈도우)
    Area,
    /// 분할자
    Splitter,
    /// 탭 스택
    TabStack,
}

/// 노드 레이아웃 정보
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeRect {
    /// 위치 (부모 기준)
    pub position: Vec2,
    /// 크기
    pub size: Vec2,
}

impl NodeRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            position: Vec2::new(x, y),
            size: Vec2::new(width, height),
        }
    }

    /// 절대 좌표로 변환
    pub fn to_absolute(&self, parent_pos: Vec2) -> Self {
        Self {
            position: parent_pos + self.position,
            size: self.size,
        }
    }

    /// 점이 이 영역 안에 있는지
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.position.x
            && point.x <= self.position.x + self.size.x
            && point.y >= self.position.y
            && point.y <= self.position.y + self.size.y
    }

    /// 중심점
    pub fn center(&self) -> Vec2 {
        self.position + self.size * 0.5
    }

    /// 왼쪽 절반
    pub fn left_half(&self) -> Self {
        Self {
            position: self.position,
            size: Vec2::new(self.size.x * 0.5, self.size.y),
        }
    }

    /// 오른쪽 절반
    pub fn right_half(&self) -> Self {
        Self {
            position: Vec2::new(self.position.x + self.size.x * 0.5, self.position.y),
            size: Vec2::new(self.size.x * 0.5, self.size.y),
        }
    }

    /// 위쪽 절반
    pub fn top_half(&self) -> Self {
        Self {
            position: self.position,
            size: Vec2::new(self.size.x, self.size.y * 0.5),
        }
    }

    /// 아래쪽 절반
    pub fn bottom_half(&self) -> Self {
        Self {
            position: Vec2::new(self.position.x, self.position.y + self.size.y * 0.5),
            size: Vec2::new(self.size.x, self.size.y * 0.5),
        }
    }

    /// 두 Rect 간 선형 보간 (MorphToShape 애니메이션용)
    pub fn lerp(&self, other: &NodeRect, t: f32) -> NodeRect {
        NodeRect {
            position: self.position.lerp(other.position, t),
            size: self.size.lerp(other.size, t),
        }
    }

    /// 영역이 0인지 (초기화 전 상태 감지용)
    pub fn is_zero(&self) -> bool {
        self.size.x <= 0.0 || self.size.y <= 0.0
    }
}

/// 탭 스택 스타일
#[derive(Debug, Clone)]
pub struct TabStackStyle {
    /// 탭 바 높이
    pub tab_bar_height: f32,
    /// 탭 최소 너비
    pub tab_min_width: f32,
    /// 탭 최대 너비
    pub tab_max_width: f32,
    /// 탭 간격
    pub tab_spacing: f32,
    /// 탭 패딩
    pub tab_padding: f32,
    /// 탭 오버랩 (크롬/언리얼 스타일)
    pub tab_overlap: f32,
}

impl Default for TabStackStyle {
    fn default() -> Self {
        Self {
            tab_bar_height: 28.0,
            tab_min_width: 60.0,
            tab_max_width: 200.0,
            tab_spacing: 2.0,
            tab_padding: 8.0,
            tab_overlap: 8.0,
        }
    }
}

/// 스플리터 스타일
#[derive(Debug, Clone)]
pub struct SplitterStyle {
    /// 분할선 두께
    pub thickness: f32,
    /// 드래그 히트 영역 (두께보다 넓게)
    pub hit_area: f32,
}

impl Default for SplitterStyle {
    fn default() -> Self {
        Self {
            thickness: 4.0,
            hit_area: 8.0,
        }
    }
}

/// 탭 역할 (언리얼 ETabRole)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TabRole {
    /// 타이틀바 MajorTab (LevelEditor, MaterialEditor 등)
    Major,
    /// MajorTab 내부 패널 (Viewport, Hierarchy 등)
    Panel,
    /// 어느 MajorTab에든 이동 가능한 패널 (Output Log, Console 등)
    Nomad,
    /// 다중 인스턴스 지원 (Blueprint Editor, Material Editor 등)
    Document,
}

impl Default for TabRole {
    fn default() -> Self {
        Self::Panel
    }
}

impl TabRole {
    /// 이 역할의 탭을 드래그할 수 있는지 (UE CanTabLeaveTabWell)
    ///
    /// Major 탭은 MajorTabBar에서 관리되므로 개별 드래그 불가.
    pub fn can_drag(&self) -> bool {
        !matches!(self, TabRole::Major)
    }

    /// 이 역할이 MajorTab 경계를 넘어 이동할 수 있는지 (UE CanDockInNode)
    ///
    /// Nomad/Document 탭은 어느 MajorTab에든 도킹 가능.
    /// Panel 탭은 원래 MajorTab 내에서만 이동 가능.
    pub fn can_cross_major_tab(&self) -> bool {
        matches!(self, TabRole::Nomad | TabRole::Document)
    }
}

/// 탭 영속성 (레이아웃 저장 시 포함 여부)
///
/// UE 참조: `TabManager::CanSaveLayout()`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TabPersistability {
    /// 레이아웃에 저장 가능 (기본)
    #[default]
    Saveable,
    /// 레이아웃에 저장하지 않음 (임시 탭)
    NotSaveable,
}

/// 활성 탭 변경 이벤트
///
/// UE의 `FOnActiveTabChanged` 델리게이트에 해당
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveTabChangedEvent {
    /// 이전 활성 탭 (None = 처음 활성화)
    pub old_tab: Option<TabId>,
    /// 새 활성 탭
    pub new_tab: TabId,
    /// 탭 스택 ID
    pub stack_id: NodeId,
}

/// 레이아웃 확장 영역
///
/// `LayoutExtender`가 위젯을 주입할 수 있는 영역
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutExtenderArea {
    Left,
    Right,
    Top,
    Bottom,
}

/// 레이아웃 확장자 (플러그인 주입)
///
/// UE의 `ILayoutExtender`에 해당.
/// 기존 레이아웃을 수정하지 않고 확장 위젯을 주입합니다.
pub trait LayoutExtender: Send + Sync {
    /// 확장자 이름 (디버깅용)
    fn name(&self) -> &str;

    /// 지정 영역에 위젯 제공 (None = 이 영역 사용 안함)
    fn extend_layout(&self, area: LayoutExtenderArea) -> Option<Box<dyn crate::widget::Widget>>;
}

/// 레이아웃 확장자 레지스트리
pub struct LayoutExtenderRegistry {
    extenders: Vec<Box<dyn LayoutExtender>>,
}

impl Default for LayoutExtenderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutExtenderRegistry {
    pub fn new() -> Self {
        Self { extenders: Vec::new() }
    }

    /// 확장자 등록
    pub fn register(&mut self, extender: Box<dyn LayoutExtender>) {
        self.extenders.push(extender);
    }

    /// 지정 영역의 모든 확장 위젯 수집
    pub fn collect_widgets(&self, area: LayoutExtenderArea) -> Vec<Box<dyn crate::widget::Widget>> {
        self.extenders.iter()
            .filter_map(|ext| ext.extend_layout(area))
            .collect()
    }

    /// 등록된 확장자 수
    pub fn len(&self) -> usize {
        self.extenders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.extenders.is_empty()
    }

    /// 모든 확장자 이름
    pub fn names(&self) -> Vec<&str> {
        self.extenders.iter().map(|e| e.name()).collect()
    }
}

/// 탭 컨텍스트 메뉴 액션
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabContextAction {
    /// 이 탭 닫기
    Close,
    /// 다른 탭 모두 닫기
    CloseOthers,
    /// 모든 탭 닫기
    CloseAll,
    /// 오른쪽 탭 모두 닫기
    CloseToRight,
    /// 사이드바로 이동
    MoveToSidebar,
}

impl TabContextAction {
    /// 메뉴 표시 텍스트
    pub fn label(&self) -> &'static str {
        match self {
            Self::Close => "Close",
            Self::CloseOthers => "Close Others",
            Self::CloseAll => "Close All",
            Self::CloseToRight => "Close to Right",
            Self::MoveToSidebar => "Move to Sidebar",
        }
    }

    /// 모든 액션 (메뉴 빌드용)
    pub fn all() -> &'static [TabContextAction] {
        &[
            TabContextAction::Close,
            TabContextAction::CloseOthers,
            TabContextAction::CloseAll,
            TabContextAction::CloseToRight,
            TabContextAction::MoveToSidebar,
        ]
    }
}

// WindowZone은 crate::core에서 재수출됨

/// 창 컨트롤 액션 (타이틀바 버튼) - 하위 호환용
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowControlAction {
    /// 창 최소화
    Minimize,
    /// 창 최대화/복원
    MaximizeRestore,
    /// 창 닫기
    Close,
    /// 타이틀바 드래그 시작 (창 이동)
    StartDrag,
    /// 타이틀바 더블클릭 (최대화/복원)
    DoubleClick,
}

/// 타이틀바 스타일
#[derive(Debug, Clone)]
pub struct TitleBarStyle {
    /// 타이틀바 높이 (탭 바 높이와 동일하게 사용)
    pub height: f32,
    /// 창 컨트롤 버튼 너비
    pub button_width: f32,
    /// 버튼 간격
    pub button_spacing: f32,
    /// 메뉴바 높이 (0이면 메뉴바 없음)
    pub menu_bar_height: f32,
    /// 툴바 높이 (0이면 툴바 없음)
    pub toolbar_height: f32,
    /// MajorTab 바 높이 (0이면 MajorTab 바 없음)
    pub major_tab_height: f32,
    /// 상태 바 높이 (0이면 상태 바 없음)
    pub status_bar_height: f32,
}

impl Default for TitleBarStyle {
    fn default() -> Self {
        Self {
            height: 28.0,
            button_width: 46.0,
            button_spacing: 0.0,
            menu_bar_height: 30.0,
            toolbar_height: 32.0,
            major_tab_height: 40.0,
            status_bar_height: 22.0,
        }
    }
}

impl TitleBarStyle {
    /// 총 헤더 높이 (메뉴바 + MajorTab바 + 툴바 + 탭바)
    pub fn total_header_height(&self) -> f32 {
        self.menu_bar_height + self.major_tab_height + self.toolbar_height + self.height
    }

    /// MajorTab 바 시작 Y 오프셋
    pub fn major_tab_y_offset(&self) -> f32 {
        self.menu_bar_height
    }

    /// 툴바 시작 Y 오프셋
    pub fn toolbar_y_offset(&self) -> f32 {
        self.menu_bar_height + self.major_tab_height
    }

    /// 탭 바 시작 Y 오프셋
    pub fn tab_bar_y_offset(&self) -> f32 {
        self.menu_bar_height + self.major_tab_height + self.toolbar_height
    }

    /// DPI 스케일 적용된 복사본 반환
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            height: self.height * scale,
            button_width: self.button_width * scale,
            button_spacing: self.button_spacing * scale,
            menu_bar_height: self.menu_bar_height * scale,
            toolbar_height: self.toolbar_height * scale,
            major_tab_height: self.major_tab_height * scale,
            status_bar_height: self.status_bar_height * scale,
        }
    }
}

impl TabStackStyle {
    /// DPI 스케일 적용된 복사본 반환
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            tab_bar_height: self.tab_bar_height * scale,
            tab_min_width: self.tab_min_width * scale,
            tab_max_width: self.tab_max_width * scale,
            tab_spacing: self.tab_spacing * scale,
            tab_padding: self.tab_padding * scale,
            tab_overlap: self.tab_overlap * scale,
        }
    }
}

impl SplitterStyle {
    /// DPI 스케일 적용된 복사본 반환
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            thickness: self.thickness * scale,
            hit_area: self.hit_area * scale,
        }
    }
}

// ============================================================================
// 자동저장 상태
// ============================================================================

/// 자동저장 상태 (dirty flag + 타이머)
pub struct AutoSaveState {
    /// 마지막 저장 시각
    pub last_save: std::time::Instant,
    /// 레이아웃 변경 여부
    pub dirty: bool,
    /// 저장 간격
    pub interval: std::time::Duration,
    /// 저장 디렉토리 (None이면 비활성)
    pub save_dir: Option<std::path::PathBuf>,
}

impl Default for AutoSaveState {
    fn default() -> Self {
        Self {
            last_save: std::time::Instant::now(),
            dirty: false,
            interval: std::time::Duration::from_secs(60),
            save_dir: None,
        }
    }
}

// ============================================================================
// 멀티캐스트 이벤트 시스템
// ============================================================================

/// 델리게이트 핸들 (구독 해제용)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DelegateHandle(u64);

/// 멀티캐스트 이벤트 델리게이트 (언리얼 FMulticastDelegate 스타일)
pub struct EventDelegate<T: Clone> {
    callbacks: Vec<(DelegateHandle, Box<dyn Fn(T) + Send + Sync>)>,
    next_id: u64,
}

impl<T: Clone> Default for EventDelegate<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Clone> EventDelegate<T> {
    pub fn new() -> Self {
        Self { callbacks: Vec::new(), next_id: 0 }
    }

    /// 콜백 추가 (핸들 반환 — 해제용)
    pub fn add<F: Fn(T) + Send + Sync + 'static>(&mut self, callback: F) -> DelegateHandle {
        let handle = DelegateHandle(self.next_id);
        self.next_id += 1;
        self.callbacks.push((handle, Box::new(callback)));
        handle
    }

    /// 콜백 해제
    pub fn remove(&mut self, handle: DelegateHandle) -> bool {
        let before = self.callbacks.len();
        self.callbacks.retain(|(h, _)| *h != handle);
        self.callbacks.len() < before
    }

    /// 모든 옵저버에 이벤트 브로드캐스트
    pub fn broadcast(&self, event: T) {
        for (_, callback) in &self.callbacks {
            callback(event.clone());
        }
    }

    /// 콜백 등록 여부
    pub fn is_empty(&self) -> bool {
        self.callbacks.is_empty()
    }

    /// 등록된 콜백 수
    pub fn len(&self) -> usize {
        self.callbacks.len()
    }

    /// 모든 콜백 제거
    pub fn clear(&mut self) {
        self.callbacks.clear();
    }
}

impl<T: Clone> std::fmt::Debug for EventDelegate<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EventDelegate({} callbacks)", self.callbacks.len())
    }
}

/// 탭 생명주기 이벤트들

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabOpeningEvent {
    pub tab_id: TabId,
    pub stack_id: NodeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabClosingEvent {
    pub tab_id: TabId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabClosedEvent {
    pub tab_id: TabId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabActivatedEvent {
    pub tab_id: TabId,
    pub stack_id: NodeId,
}
