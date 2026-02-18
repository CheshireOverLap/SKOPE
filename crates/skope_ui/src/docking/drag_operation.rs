//! 통합 드래그 오퍼레이션 (UE FDockingDragOperation 스타일)
//!
//! 내부 드래그(같은 윈도우)와 외부 드래그(크로스 윈도우)를 통합하는 단일 구조체.
//! 언리얼 엔진의 FDockingDragOperation을 참고하여 설계.
//!
//! 이 모듈은 `app` 피처가 활성화될 때만 사용 가능합니다.

use glam::Vec2;

use super::{TabId, NodeId, DockPosition, TabRole, NodeRect, SidebarSide};
use crate::widget::Widget;

/// 윈도우 ID (winit::window::WindowId 래핑)
/// app 피처 없이도 타입 정의 가능하도록 분리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DragWindowId(pub u64);

impl DragWindowId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 통합 드래그 오퍼레이션 (UE FDockingDragOperation 스타일)
///
/// 드래그 시작부터 드롭까지의 모든 상태를 관리하는 단일 구조체.
/// SlateApp에서 소유하며, 위젯들은 이 구조체를 업데이트하는 이벤트를 발생시킴.
pub struct DockingDragOperation {
    // === 탭 정보 (TabBeingDragged) ===
    /// 드래그 중인 탭 ID
    pub tab_id: TabId,
    /// 탭 제목
    pub title: String,
    /// 탭 아이콘
    pub icon: Option<String>,
    /// 탭 콘텐츠 (드래그 중 임시 보관)
    pub content: Option<Box<dyn Widget>>,
    /// 탭 역할 (Major, Panel, Nomad, Document)
    pub role: TabRole,

    // === 원본 정보 (TabOwnerAreaOfOrigin) ===
    /// 원본 스택 ID
    pub source_stack_id: NodeId,
    /// 원본 윈도우 ID (None이면 메인 윈도우)
    pub source_window_id: Option<DragWindowId>,
    /// 원본 탭 rect (고스트 표시용)
    pub source_tab_rect: NodeRect,
    /// 원본 스택 크기 (플로팅 윈도우 기본 크기)
    pub source_size: Vec2,

    // === 드래그 상태 ===
    /// 드래그 시작 위치 (스크린 좌표)
    pub start_pos: Vec2,
    /// 현재 마우스 위치 (스크린 좌표)
    pub current_pos: Vec2,
    /// 탭 내 그랩 오프셋 (UE TabGrabOffsetFraction)
    pub grab_offset: Vec2,
    /// 드래그 임계값 초과 여부 (5px)
    pub is_threshold_exceeded: bool,

    // === 타겟 정보 (HoveredDockTarget) ===
    /// 현재 호버된 스택 ID
    pub target_stack_id: Option<NodeId>,
    /// 현재 호버된 윈도우 ID (None이면 메인 윈도우)
    pub target_window_id: Option<DragWindowId>,
    /// 나침반 도킹 위치 (Left, Right, Top, Bottom, Center)
    pub dock_position: Option<DockPosition>,
    /// 탭바 내 드롭 인덱스 (UE SDockingTabWell 스타일)
    pub drop_index: Option<usize>,
    /// 탭웰 호버 중 여부 (HoveredTabPanelPtr)
    pub is_in_tab_well: bool,
}

impl DockingDragOperation {
    /// 새 드래그 오퍼레이션 생성
    pub fn new(
        tab_id: TabId,
        title: String,
        icon: Option<String>,
        content: Box<dyn Widget>,
        role: TabRole,
        source_stack_id: NodeId,
        source_window_id: Option<DragWindowId>,
        source_tab_rect: NodeRect,
        source_size: Vec2,
        start_pos: Vec2,
        grab_offset: Vec2,
    ) -> Self {
        Self {
            tab_id,
            title,
            icon,
            content: Some(content),
            role,
            source_stack_id,
            source_window_id,
            source_tab_rect,
            source_size,
            start_pos,
            current_pos: start_pos,
            grab_offset,
            is_threshold_exceeded: false,
            target_stack_id: None,
            target_window_id: None,
            dock_position: None,
            drop_index: None,
            is_in_tab_well: false,
        }
    }

    /// 마우스 위치 업데이트 (UE OnDragged)
    pub fn update_position(&mut self, screen_pos: Vec2) {
        self.current_pos = screen_pos;

        // 임계값 체크 (5px)
        if !self.is_threshold_exceeded {
            let delta = screen_pos - self.start_pos;
            if delta.length() > 5.0 {
                self.is_threshold_exceeded = true;
            }
        }
    }

    /// 타겟 설정 (UE SetHoveredTarget)
    pub fn set_target(
        &mut self,
        stack_id: Option<NodeId>,
        window_id: Option<DragWindowId>,
        position: Option<DockPosition>,
    ) {
        self.target_stack_id = stack_id;
        self.target_window_id = window_id;
        self.dock_position = position;
    }

    /// 탭웰 진입 (UE OnTabWellEntered)
    pub fn enter_tab_well(&mut self, stack_id: NodeId, drop_index: usize) {
        self.is_in_tab_well = true;
        self.target_stack_id = Some(stack_id);
        self.drop_index = Some(drop_index);
        // 탭웰에서는 Center 도킹
        self.dock_position = Some(DockPosition::Center);
    }

    /// 탭웰 이탈 (UE OnTabWellLeft)
    pub fn leave_tab_well(&mut self) {
        self.is_in_tab_well = false;
        self.drop_index = None;
        // dock_position은 나침반 상태로 유지
    }

    /// 드롭 인덱스 업데이트 (탭웰 내 위치 변경)
    pub fn update_drop_index(&mut self, index: Option<usize>) {
        self.drop_index = index;
    }

    /// 콘텐츠 추출 (드롭 시 한 번만 호출)
    pub fn take_content(&mut self) -> Option<Box<dyn Widget>> {
        self.content.take()
    }

    /// 드래그 중인지 (임계값 초과)
    pub fn is_dragging(&self) -> bool {
        self.is_threshold_exceeded
    }

    /// 유효한 타겟이 있는지
    pub fn has_valid_target(&self) -> bool {
        self.target_stack_id.is_some() && self.dock_position.is_some()
    }

    /// 드래그 결과 계산 (UE OnDrop)
    pub fn finish(mut self) -> DragOperationResult {
        if !self.is_threshold_exceeded {
            return DragOperationResult::Cancelled {
                tab_id: self.tab_id,
                content: self.content.take(),
            };
        }

        let content = self.content.take();

        match (self.target_stack_id, self.dock_position) {
            // 타겟 스택 + 도킹 위치 있음
            (Some(target_stack_id), Some(position)) => {
                if self.is_in_tab_well && position == DockPosition::Center {
                    // 탭웰 내 드롭 → 탭 병합
                    if target_stack_id == self.source_stack_id {
                        // 같은 스택 → 순서 변경
                        DragOperationResult::ReorderInStack {
                            tab_id: self.tab_id,
                            stack_id: self.source_stack_id,
                            new_index: self.drop_index.unwrap_or(0),
                            content,
                        }
                    } else {
                        // 다른 스택 → 탭 이동
                        DragOperationResult::DockToStack {
                            tab_id: self.tab_id,
                            source_stack_id: self.source_stack_id,
                            target_stack_id,
                            target_window_id: self.target_window_id,
                            position,
                            insert_index: self.drop_index,
                            content,
                        }
                    }
                } else if position == DockPosition::Center {
                    // 나침반 중앙 (UE5: 동작 없음) → 플로팅
                    DragOperationResult::CreateFloatingWindow {
                        tab_id: self.tab_id,
                        title: self.title,
                        icon: self.icon,
                        position: self.current_pos - self.grab_offset,
                        size: self.source_size,
                        content,
                        role: self.role,
                    }
                } else {
                    // 나침반 4방향 → 분할 도킹
                    DragOperationResult::DockToStack {
                        tab_id: self.tab_id,
                        source_stack_id: self.source_stack_id,
                        target_stack_id,
                        target_window_id: self.target_window_id,
                        position,
                        insert_index: None, // 분할 도킹은 insert_index 없음
                        content,
                    }
                }
            }
            // 타겟 없음 → 플로팅 윈도우 생성 (UE DroppedOntoNothing)
            _ => DragOperationResult::CreateFloatingWindow {
                tab_id: self.tab_id,
                title: self.title,
                icon: self.icon,
                position: self.current_pos - self.grab_offset,
                size: self.source_size,
                content,
                role: self.role,
            },
        }
    }
}

/// 드래그 결과 (통합)
pub enum DragOperationResult {
    /// 취소됨 (임계값 미달 또는 ESC)
    Cancelled {
        tab_id: TabId,
        content: Option<Box<dyn Widget>>,
    },

    /// 스택에 도킹 (같은 윈도우 또는 다른 윈도우)
    DockToStack {
        tab_id: TabId,
        source_stack_id: NodeId,
        target_stack_id: NodeId,
        target_window_id: Option<DragWindowId>,
        position: DockPosition,
        insert_index: Option<usize>,
        content: Option<Box<dyn Widget>>,
    },

    /// 플로팅 윈도우 생성
    CreateFloatingWindow {
        tab_id: TabId,
        title: String,
        icon: Option<String>,
        position: Vec2,
        size: Vec2,
        content: Option<Box<dyn Widget>>,
        role: TabRole,
    },

    /// 같은 스택 내 순서 변경
    ReorderInStack {
        tab_id: TabId,
        stack_id: NodeId,
        new_index: usize,
        content: Option<Box<dyn Widget>>,
    },

    /// 사이드바에서 복원
    RestoreFromSidebar {
        tab_id: TabId,
        side: SidebarSide,
        target_stack_id: Option<NodeId>,
        position: Option<DockPosition>,
        content: Option<Box<dyn Widget>>,
    },
}

impl DragOperationResult {
    /// 성공적인 결과인지
    pub fn is_success(&self) -> bool {
        !matches!(self, Self::Cancelled { .. })
    }

    /// 콘텐츠 추출
    pub fn take_content(&mut self) -> Option<Box<dyn Widget>> {
        match self {
            Self::Cancelled { content, .. } => content.take(),
            Self::DockToStack { content, .. } => content.take(),
            Self::CreateFloatingWindow { content, .. } => content.take(),
            Self::ReorderInStack { content, .. } => content.take(),
            Self::RestoreFromSidebar { content, .. } => content.take(),
        }
    }
}

impl std::fmt::Debug for DragOperationResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancelled { tab_id, .. } => {
                f.debug_struct("Cancelled").field("tab_id", tab_id).finish()
            }
            Self::DockToStack { tab_id, source_stack_id, target_stack_id, position, insert_index, .. } => {
                f.debug_struct("DockToStack")
                    .field("tab_id", tab_id)
                    .field("source_stack_id", source_stack_id)
                    .field("target_stack_id", target_stack_id)
                    .field("position", position)
                    .field("insert_index", insert_index)
                    .finish()
            }
            Self::CreateFloatingWindow { tab_id, title, position, size, role, .. } => {
                f.debug_struct("CreateFloatingWindow")
                    .field("tab_id", tab_id)
                    .field("title", title)
                    .field("position", position)
                    .field("size", size)
                    .field("role", role)
                    .finish()
            }
            Self::ReorderInStack { tab_id, stack_id, new_index, .. } => {
                f.debug_struct("ReorderInStack")
                    .field("tab_id", tab_id)
                    .field("stack_id", stack_id)
                    .field("new_index", new_index)
                    .finish()
            }
            Self::RestoreFromSidebar { tab_id, side, target_stack_id, position, .. } => {
                f.debug_struct("RestoreFromSidebar")
                    .field("tab_id", tab_id)
                    .field("side", side)
                    .field("target_stack_id", target_stack_id)
                    .field("position", position)
                    .finish()
            }
        }
    }
}

/// 위젯에서 발생하는 드래그 이벤트 (SlateApp으로 전달)
#[derive(Debug)]
pub enum DragEvent {
    /// 드래그 시작 요청
    Started {
        tab_id: TabId,
        stack_id: NodeId,
        window_id: Option<DragWindowId>,
        local_pos: Vec2,
        grab_offset: Vec2,
    },

    /// 마우스 이동
    Moved {
        local_pos: Vec2,
    },

    /// 타겟 변경 (나침반 호버)
    TargetChanged {
        stack_id: Option<NodeId>,
        rect: Option<NodeRect>,
        content_rect: Option<NodeRect>,
    },

    /// 탭웰 진입
    TabWellEntered {
        stack_id: NodeId,
        drop_index: usize,
    },

    /// 탭웰 이탈
    TabWellLeft,

    /// 드롭 인덱스 변경
    DropIndexChanged {
        index: Option<usize>,
    },

    /// 드래그 종료 (마우스 업)
    Ended {
        local_pos: Vec2,
    },

    /// 드래그 취소 (ESC)
    Cancelled,
}

impl std::fmt::Debug for DockingDragOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DockingDragOperation")
            .field("tab_id", &self.tab_id)
            .field("title", &self.title)
            .field("role", &self.role)
            .field("source_stack_id", &self.source_stack_id)
            .field("source_window_id", &self.source_window_id)
            .field("is_threshold_exceeded", &self.is_threshold_exceeded)
            .field("target_stack_id", &self.target_stack_id)
            .field("target_window_id", &self.target_window_id)
            .field("dock_position", &self.dock_position)
            .field("drop_index", &self.drop_index)
            .field("is_in_tab_well", &self.is_in_tab_well)
            .finish()
    }
}
