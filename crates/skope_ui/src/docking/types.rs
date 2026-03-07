//! 도킹 시스템 핵심 타입

use glam::Vec2;
use serde::{Serialize, Deserialize};

use crate::core::WindowZone;

/// 노드 고유 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u64);

impl NodeId {
    /// Area-level 외곽 도킹 타겟 (스택이 아닌 전체 영역)
    pub const AREA_ROOT: NodeId = NodeId(u64::MAX);

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

/// UE5 ESplitterResizeMode — 스플리터 리사이즈 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitterResizeMode {
    /// 선택 슬롯 리사이즈, 부족한 공간은 다음 리사이즈 가능 슬롯에서 (UE5 FixedPosition)
    FixedPosition,
    /// 선택 슬롯 리사이즈, 부족한 공간은 마지막 리사이즈 가능 슬롯에서 (UE5 FixedSize)
    FixedSize,
    /// 선택 슬롯 리사이즈, 이후 리사이즈 가능 슬롯들에 균등 재분배 (UE5 Fill)
    Fill,
}

impl Default for SplitterResizeMode {
    fn default() -> Self {
        Self::FixedPosition
    }
}

/// UE5 SSplitter::ESizeRule — 자식 슬롯 크기 결정 방식
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SizeRule {
    /// DesiredSize 기반 자동 크기 (UE5 SizeToContent)
    ///
    /// 자식의 DesiredSize를 ResizableSpace에서 먼저 차감.
    /// 남은 공간이 FractionOfParent 자식에 분배됨.
    SizeToContent,
    /// 부모 공간 비율 기반 (UE5 FractionOfParent, 기본값)
    ///
    /// SizeCoefficient / CoefficientTotal 비율로 ResizableSpace 분배.
    FractionOfParent,
}

impl Default for SizeRule {
    fn default() -> Self {
        Self::FractionOfParent
    }
}

/// UE5 ECleanUpRetVal — 탭 스택 정리 결과
///
/// CleanUpNodes에서 반환하여 노드의 탭 상태를 구분.
/// HistoryTabsUnderNode → Collapsed (복원용 보존), NoTabsUnderNode → 제거.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanUpRetVal {
    /// 활성 탭이 있는 노드
    VisibleTabsUnderNode,
    /// 히스토리 탭만 있는 노드 (Collapsed로 보존, 추후 복원 가능)
    HistoryTabsUnderNode,
    /// 탭이 전혀 없는 노드 (제거 대상)
    NoTabsUnderNode,
}

impl CleanUpRetVal {
    /// UE5 MostResponsibility: Min(A, B) — 가장 "살아있는" 결과 선택 (Visible > History > NoTabs)
    pub fn most_responsibility(self, other: Self) -> Self {
        match (self, other) {
            (Self::VisibleTabsUnderNode, _) | (_, Self::VisibleTabsUnderNode) => Self::VisibleTabsUnderNode,
            (Self::HistoryTabsUnderNode, _) | (_, Self::HistoryTabsUnderNode) => Self::HistoryTabsUnderNode,
            _ => Self::NoTabsUnderNode,
        }
    }
}

/// UE5 ELayoutModification — 탭 제거 사유
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutModification {
    /// 탭이 드래그로 분리됨 (UE5 TabRemoval_DraggedOut)
    TabDraggedOut,
    /// 탭이 닫힘 (UE5 TabRemoval_Closed)
    TabClosed,
    /// 탭이 사이드바로 이동 (UE5 TabRemoval_Sidebar)
    TabMovedToSidebar,
    /// 제거 아님 / 기본값 (UE5 TabRemoval_None)
    None,
}

/// UE5 ETabState 비트 매칭 (L1: 비트필드 스타일 쿼리 지원)
///
/// TabState enum은 serde 호환을 위해 유지하되,
/// 비트 OR 결합 매칭을 위한 별도 마스크 타입 제공.
///
/// UE5 원본: OpenedTab=0x1, ClosedTab=0x2, SidebarTab=0x4, InvalidTab=0x8
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabStateMask(pub u8);

impl TabStateMask {
    pub const OPEN: TabStateMask = TabStateMask(0x1);
    pub const CLOSED: TabStateMask = TabStateMask(0x2);
    pub const SIDEBAR: TabStateMask = TabStateMask(0x4);
    pub const INVALID: TabStateMask = TabStateMask(0x8);
    /// 모든 상태 매칭
    pub const ANY: TabStateMask = TabStateMask(0x0F);
    /// 활성 + 닫힌 상태 (UE5 HasSiblingTab 쿼리)
    pub const OPEN_OR_CLOSED: TabStateMask = TabStateMask(0x1 | 0x2);

    /// 비트 OR 결합
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// TabState enum → 비트 마스크 변환
    pub fn from_state(state: crate::docking::layout::TabState) -> Self {
        match state {
            crate::docking::layout::TabState::Open => Self::OPEN,
            crate::docking::layout::TabState::Closed => Self::CLOSED,
            crate::docking::layout::TabState::Sidebar => Self::SIDEBAR,
            crate::docking::layout::TabState::Invalid => Self::INVALID,
        }
    }

    /// 마스크에 해당 상태가 포함되어 있는지
    pub fn matches(self, state: crate::docking::layout::TabState) -> bool {
        let bit = Self::from_state(state);
        (self.0 & bit.0) != 0
    }
}

/// UE5 FTabMatcher — 탭 쿼리 헬퍼 (L2)
///
/// TabId + 상태 마스크 + 와일드카드 모드를 캡슐화.
/// HasSiblingTab, FindTab 등 탭 스택 검색 쿼리에 사용.
#[derive(Debug, Clone, Copy)]
pub struct TabMatcher {
    /// 검색할 탭 ID (None = 와일드카드)
    pub tab_id: Option<TabId>,
    /// 매칭할 상태 마스크 (OR 결합)
    pub state_mask: TabStateMask,
}

impl TabMatcher {
    /// 특정 탭 ID + 상태 마스크로 매처 생성
    pub fn new(tab_id: TabId, state_mask: TabStateMask) -> Self {
        Self { tab_id: Some(tab_id), state_mask }
    }

    /// 와일드카드 (모든 탭 ID, 특정 상태만 매칭)
    pub fn any_tab(state_mask: TabStateMask) -> Self {
        Self { tab_id: None, state_mask }
    }

    /// 특정 탭 ID, 모든 상태
    pub fn by_id(tab_id: TabId) -> Self {
        Self { tab_id: Some(tab_id), state_mask: TabStateMask::ANY }
    }
}

/// UE5 ESidebarLocation — 사이드바 위치 (L3)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarLocation {
    /// 왼쪽 사이드바
    Left,
    /// 오른쪽 사이드바
    Right,
    /// 상단 사이드바
    Top,
    /// 하단 사이드바
    Bottom,
    /// 사이드바 아님
    None,
}

impl Default for SidebarLocation {
    fn default() -> Self {
        Self::None
    }
}

/// UE5 ETabActivationCause — 탭 활성화 원인
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabActivationCause {
    /// 사용자가 탭을 클릭 (UE5 UserClickedOnTab)
    UserClickedOnTab,
    /// 프로그래밍 방식으로 설정 (UE5 SetDirectly)
    SetDirectly,
}

/// UE5 EViaTabwell — 도킹 경로 구분 (탭웰 vs 타겟)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockingVia {
    /// 탭웰을 통한 도킹 (탭 바에 직접 드롭)
    TabWell,
    /// 나침반 타겟을 통한 도킹 (DockingCross 영역)
    Target,
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

/// 노드 종류 (UE5 SDockingNode::Type)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// 루트 영역 (OS 윈도우)
    Area,
    /// 분할자
    Splitter,
    /// 탭 스택
    TabStack,
    /// 레이아웃 재배치 중 공간 예약용 (UE5 PlaceholderNode)
    Placeholder,
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
    /// Major 탭 최대 너비 (Phase 4)
    pub major_tab_max_width: f32,
    /// 탭 간격
    pub tab_spacing: f32,
    /// 탭 패딩
    pub tab_padding: f32,
    /// 탭 오버랩 (크롬/언리얼 스타일)
    pub tab_overlap: f32,
    /// 탭 바 좌측 예약 영역 (로고 등)
    pub bar_left_reserve: f32,
    /// 탭 바 우측 예약 영역 (윈도우 버튼 등)
    pub bar_right_reserve: f32,
    /// UE5 MaxTabSizeNoNameWidth: 이름 숨기고 아이콘+닫기만 표시하는 너비 임계값
    pub tab_no_name_width: f32,
    /// UE5 MaxTabSizeNoNameCantCloseWidth: 아이콘만 표시하는 너비 임계값
    pub tab_no_name_no_close_width: f32,

    // ── 시각 피드백 ──
    /// 외부 드래그 고스트 탭 불투명도
    pub tab_ghost_opacity: f32,
    /// 로컬 리오더 드래그 탭 불투명도
    pub tab_drag_opacity: f32,
    /// 알림 플래시 블렌드 강도
    pub tab_flash_blend: f32,
    /// 비활성 탭 아이콘 불투명도
    pub inactive_icon_opacity: f32,
    /// 탭 구분선 높이 비율
    pub separator_height_ratio: f32,

    // ── 인터랙션 ──
    /// 탭웰 표시/숨김 애니메이션 속도
    pub well_anim_speed: f32,
    /// 드래그 호버 탭 활성화 지연(초)
    pub drag_hover_delay: f32,
    /// 로컬 리오더 시작 임계값(px)
    pub local_drag_threshold: f32,
    /// 크로스윈도우 드래그 탈출 임계값(px)
    pub drag_escape_threshold: f32,
    /// 탭웰 content-right 최소 폭
    pub well_min_slot_width: f32,
}

impl Default for TabStackStyle {
    fn default() -> Self {
        Self {
            tab_bar_height: 25.0,   // UE5 MaxMinorTabSize.Y = 25 (SDockTab::ComputeDesiredSize)
            tab_min_width: 60.0,
            tab_max_width: 160.0,   // UE5 MaxMinorTabSize.X = 160px
            major_tab_max_width: 210.0, // Phase 4: Major 탭은 더 넓게
            tab_spacing: 4.0,       // UE5.7 기준 4px gap
            tab_padding: 8.0,       // UE5.7 탭바 양 끝 여백 8px
            tab_overlap: 0.0,       // gap-based (not overlap)
            bar_left_reserve: 0.0,
            bar_right_reserve: 0.0,
            tab_no_name_width: 53.0,         // UE5 FDockingConstants::MaxTabSizeNoNameWidth
            tab_no_name_no_close_width: 32.0, // UE5 FDockingConstants::MaxTabSizeNoNameCantCloseWidth
            // 시각 피드백
            tab_ghost_opacity: 0.4,
            tab_drag_opacity: 0.85,
            tab_flash_blend: 0.4,
            inactive_icon_opacity: 0.7,
            separator_height_ratio: 0.65,
            // 인터랙션
            well_anim_speed: 8.0,
            drag_hover_delay: 0.75,
            local_drag_threshold: 5.0,
            drag_escape_threshold: 20.0,
            well_min_slot_width: 20.0,
        }
    }
}

/// 스플리터 스타일 (UE5 SSplitter 기본값 일치)
#[derive(Debug, Clone, Copy)]
pub struct SplitterStyle {
    /// 분할선 두께 (UE5 PhysicalSplitterHandleSize, 기본 5.0)
    pub thickness: f32,
    /// 드래그 히트 영역 (UE5 HitDetectionSplitterHandleSize, 기본 5.0)
    pub hit_area: f32,
    /// 분할 자식 최소 크기(px) (UE5 MinSplitterChildLength/MinimumSlotHeight, 기본 20.0)
    pub min_child_size: f32,
}

impl Default for SplitterStyle {
    fn default() -> Self {
        Self {
            thickness: 5.0,
            hit_area: 5.0,
            min_child_size: 20.0,
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

    /// UE5 CanDockInNode(EViaTabwell) — 도킹 경로별 허용 여부
    ///
    /// TabWell 경로: Major 탭의 탭웰에는 같은 Major 소속만 허용.
    /// Target 경로: Nomad/Document는 어디든 가능, Panel은 같은 Major만.
    pub fn can_dock_in_node(&self, via: DockingVia, same_major: bool) -> bool {
        match via {
            DockingVia::TabWell => {
                // Major 탭 바에는 도킹 불가, 같은 Major 내 Panel은 가능
                if matches!(self, TabRole::Major) { return false; }
                same_major || self.can_cross_major_tab()
            }
            DockingVia::Target => {
                same_major || self.can_cross_major_tab()
            }
        }
    }

    /// 역할 종류 수 (UE5 NumRoles — 배열 크기 결정용)
    pub const NUM_ROLES: usize = 4;
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

/// UE5 ETabReadOnlyBehavior — 읽기 전용 모드 동작
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOnlyBehavior {
    /// 읽기 전용 모드에서 탭 비활성화
    DisableTab,
    /// 읽기 전용 모드에서 탭 숨기기
    HideTab,
    /// 읽기 전용 모드에서 아무 동작 없음
    NoAction,
}

impl Default for ReadOnlyBehavior {
    fn default() -> Self {
        Self::NoAction
    }
}

/// UE5 SearchPreference — 문서 탭 검색 우선순위
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPreference {
    /// 기존 탭 우선 검색 (UE5 PreferLiveTab)
    PreferLiveTab,
    /// 새 탭 필수 (UE5 RequireClosedTab)
    RequireClosedTab,
}

impl Default for SearchPreference {
    fn default() -> Self {
        Self::PreferLiveTab
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
    /// 보더 드래그 리사이즈 시작 (UE5 SWindow border resize)
    StartResize(WindowZone),
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
    /// 우측 로고 너비 (0이면 로고 없음)
    pub logo_width: f32,
    /// 우측 로고 우측 마진 (윈도우 버튼과의 간격)
    pub logo_right_margin: f32,
}

impl Default for TitleBarStyle {
    fn default() -> Self {
        // ThemeSpacing::default()에서 파생 — 이중 정의 방지
        Self::from_theme(&crate::theme::ThemeSpacing::default())
    }
}

impl TitleBarStyle {
    /// EditorTheme의 ThemeSpacing에서 값을 읽어 TitleBarStyle 구성
    ///
    /// UE5.7 FSlateStyleSet → FAppStyle 패턴:
    /// 테마에 정의된 spacing 값이 레이아웃 결정에 직접 연결됨.
    pub fn from_theme(spacing: &crate::theme::ThemeSpacing) -> Self {
        Self {
            height: spacing.doc_tab_height,           // 탭 바 높이 = doc_tab_height (40)
            button_width: 46.0,                       // 윈도우 컨트롤 버튼 (OS 고정)
            button_spacing: 0.0,
            menu_bar_height: spacing.menu_bar_height, // 38
            toolbar_height: spacing.toolbar_height,   // 48
            major_tab_height: spacing.titlebar_height, // 38 (MajorTab = titlebar)
            status_bar_height: spacing.panel_header_height * 0.7, // 22.4 ≈ status bar
            logo_width: 45.0,
            logo_right_margin: 5.0,
        }
    }

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
            logo_width: self.logo_width * scale,
            logo_right_margin: self.logo_right_margin * scale,
        }
    }
}

impl TabStackStyle {
    /// 테마에서 크기 초기화
    pub fn from_theme(spacing: &crate::theme::ThemeSpacing) -> Self {
        Self {
            tab_bar_height: spacing.tab_bar_height,
            tab_min_width: spacing.tab_min_width,
            tab_max_width: spacing.tab_max_width,
            major_tab_max_width: spacing.major_tab_max_width,
            tab_spacing: spacing.tab_spacing,
            tab_padding: spacing.tab_h_padding,
            tab_overlap: 0.0,
            bar_left_reserve: 0.0,
            bar_right_reserve: 0.0,
            tab_no_name_width: 53.0,
            tab_no_name_no_close_width: 32.0,
            // 시각 피드백
            tab_ghost_opacity: spacing.tab_ghost_opacity,
            tab_drag_opacity: spacing.tab_drag_opacity,
            tab_flash_blend: spacing.tab_flash_blend,
            inactive_icon_opacity: spacing.tab_inactive_icon_opacity,
            separator_height_ratio: spacing.tab_separator_height_ratio,
            // 인터랙션
            well_anim_speed: spacing.tab_well_anim_speed,
            drag_hover_delay: spacing.drag_hover_activation_delay,
            local_drag_threshold: spacing.local_drag_threshold,
            drag_escape_threshold: spacing.drag_escape_threshold,
            well_min_slot_width: spacing.tab_well_min_slot_width,
        }
    }

    /// DPI 스케일 적용된 복사본 반환
    ///
    /// opacity/ratio 계열은 스케일링 안 함, 픽셀 계열만 `* scale`.
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            tab_bar_height: self.tab_bar_height * scale,
            tab_min_width: self.tab_min_width * scale,
            tab_max_width: self.tab_max_width * scale,
            major_tab_max_width: self.major_tab_max_width * scale,
            tab_spacing: self.tab_spacing * scale,
            tab_padding: self.tab_padding * scale,
            tab_overlap: self.tab_overlap * scale,
            bar_left_reserve: self.bar_left_reserve * scale,
            bar_right_reserve: self.bar_right_reserve * scale,
            tab_no_name_width: self.tab_no_name_width * scale,
            tab_no_name_no_close_width: self.tab_no_name_no_close_width * scale,
            // opacity/ratio — 스케일링 안 함
            tab_ghost_opacity: self.tab_ghost_opacity,
            tab_drag_opacity: self.tab_drag_opacity,
            tab_flash_blend: self.tab_flash_blend,
            inactive_icon_opacity: self.inactive_icon_opacity,
            separator_height_ratio: self.separator_height_ratio,
            // 인터랙션 — 시간/속도는 스케일링 안 함, 픽셀 임계값만 스케일
            well_anim_speed: self.well_anim_speed,
            drag_hover_delay: self.drag_hover_delay,
            local_drag_threshold: self.local_drag_threshold * scale,
            drag_escape_threshold: self.drag_escape_threshold * scale,
            well_min_slot_width: self.well_min_slot_width * scale,
        }
    }
}

impl SplitterStyle {
    /// 테마에서 크기 초기화
    pub fn from_theme(spacing: &crate::theme::ThemeSpacing) -> Self {
        Self {
            thickness: 5.0,
            hit_area: 5.0,
            min_child_size: spacing.splitter_min_child_size,
        }
    }

    /// DPI 스케일 적용된 복사본 반환
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            thickness: self.thickness * scale,
            hit_area: self.hit_area * scale,
            min_child_size: self.min_child_size * scale,
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

// ============================================================================
// 영속 탭 (Batch 3, 11차)
// ============================================================================

/// UE5 FTab — 열린/닫힌/사이드바 탭의 히스토리 정보
///
/// SDockingTabStack의 Tabs 배열에 대응.
/// 열린 탭과 닫힌 탭을 동일 목록에서 TabState로 구분.
#[derive(Debug, Clone)]
pub struct PersistentTab {
    /// 탭 타입 이름 (스포너 기반 복원용)
    pub tab_type: String,
    /// 현재 상태
    pub state: PersistentTabState,
    /// 사이드바 위치 (Sidebar 상태일 때만 유효)
    pub sidebar_location: Option<SidebarLocation>,
    /// 사이드바 크기 계수 (0.0~1.0)
    pub sidebar_size_coefficient: f32,
    /// 사이드바 핀 고정 여부
    pub pinned_in_sidebar: bool,
}

impl PersistentTab {
    pub fn new_open(tab_type: impl Into<String>) -> Self {
        Self {
            tab_type: tab_type.into(),
            state: PersistentTabState::Open,
            sidebar_location: None,
            sidebar_size_coefficient: 0.25,
            pinned_in_sidebar: false,
        }
    }

    pub fn new_closed(tab_type: impl Into<String>) -> Self {
        Self {
            tab_type: tab_type.into(),
            state: PersistentTabState::Closed,
            sidebar_location: None,
            sidebar_size_coefficient: 0.25,
            pinned_in_sidebar: false,
        }
    }
}

/// UE5 ETabState — 영속 탭 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistentTabState {
    /// 열려있음
    Open,
    /// 닫혀있음 (복원 가능)
    Closed,
    /// 사이드바에 있음
    Sidebar,
    /// 무효 (플러그인 미로드 등)
    Invalid,
}

/// 패널 드로워 크기 (UE5 SPanelDrawerArea 열기 시 크기 지정)
#[derive(Debug, Clone, Copy)]
pub struct PanelDrawerSize {
    /// 드로워 폭
    pub width: f32,
    /// 드로워 높이 (0이면 자동)
    pub height: f32,
}

impl Default for PanelDrawerSize {
    fn default() -> Self {
        Self { width: 300.0, height: 0.0 }
    }
}

/// UE5 FTabPermissionList — 탭 허용/거부 목록
///
/// allowed가 None이면 전부 허용(거부 목록만 체크).
/// allowed가 Some이면 화이트리스트 + 거부 목록 교차 체크.
pub struct TabPermissionList {
    /// 허용 탭 타입 (None이면 전부 허용)
    pub allowed: Option<std::collections::HashSet<String>>,
    /// 거부 탭 타입
    pub denied: std::collections::HashSet<String>,
}

impl Default for TabPermissionList {
    fn default() -> Self {
        Self::new()
    }
}

impl TabPermissionList {
    pub fn new() -> Self {
        Self {
            allowed: None,
            denied: std::collections::HashSet::new(),
        }
    }

    /// 탭 타입이 허용되는지
    pub fn is_allowed(&self, tab_type: &str) -> bool {
        if self.denied.contains(tab_type) {
            return false;
        }
        match &self.allowed {
            None => true,
            Some(set) => set.contains(tab_type),
        }
    }

    /// 허용 목록에 추가
    pub fn allow(&mut self, tab_type: &str) {
        let set = self.allowed.get_or_insert_with(std::collections::HashSet::new);
        set.insert(tab_type.to_string());
        self.denied.remove(tab_type);
    }

    /// 거부 목록에 추가
    pub fn deny(&mut self, tab_type: &str) {
        self.denied.insert(tab_type.to_string());
        if let Some(ref mut set) = self.allowed {
            set.remove(tab_type);
        }
    }

    /// 모든 목록 초기화
    pub fn clear(&mut self) {
        self.allowed = None;
        self.denied.clear();
    }
}

/// 닫을 탭 범위 (UE5 ETabsToClose)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabsToClose {
    /// Document 탭만
    Document,
    /// Document + Major 탭
    DocumentAndMajor,
    /// 모든 탭
    All,
}

impl TabsToClose {
    /// 주어진 역할의 탭을 이 범위에서 닫을 수 있는지
    pub fn matches_role(&self, role: TabRole) -> bool {
        match self {
            Self::Document => matches!(role, TabRole::Document),
            Self::DocumentAndMajor => matches!(role, TabRole::Document | TabRole::Major),
            Self::All => true,
        }
    }
}

/// 윈도우 크롬 요소 (UE5 EChromeElement)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromeElement {
    /// 윈도우 아이콘
    Icon,
    /// 윈도우 컨트롤 (최소/최대/닫기)
    Controls,
}

/// UE5 PanelDrawerStateEvent — 패널 드로워 상태 변경 이벤트
#[derive(Debug, Clone)]
pub struct PanelDrawerStateEvent {
    /// 드로워 열림 여부
    pub is_open: bool,
    /// 관련 탭 ID
    pub tab_id: Option<TabId>,
}
