//! 드래그 상태 관리
//!
//! 탭 드래그 & 드롭 로직

use super::{NodeId, TabId, DockPosition, NodeRect, DockingCompass, SidebarSide};
use glam::Vec2;

/// 드래그 작업 종류
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragOperation {
    /// 아무것도 안 함
    None,
    /// 탭 드래그
    DragTab {
        /// 드래그 중인 탭 ID
        tab_id: TabId,
        /// 원래 속했던 스택 ID
        source_stack_id: NodeId,
    },
    /// 스플리터 드래그 (크기 조절)
    DragSplitter {
        /// 스플리터 노드 ID
        splitter_id: NodeId,
        /// 조절 중인 자식 인덱스
        child_index: usize,
    },
    /// 사이드바 탭 드래그 (도킹 영역으로 복원)
    DragSidebarTab {
        /// 드래그 중인 탭 ID
        tab_id: TabId,
        /// 원래 사이드바 위치
        side: SidebarSide,
    },
}

/// 드래그 상태
pub struct DragState {
    /// 현재 드래그 작업
    pub operation: DragOperation,
    /// 드래그 시작 위치
    pub start_pos: Vec2,
    /// 현재 마우스 위치
    pub current_pos: Vec2,
    /// 드래그 임계값 (픽셀)
    pub drag_threshold: f32,
    /// 드래그가 활성화되었는지 (임계값 초과)
    pub is_dragging: bool,
    /// 도킹 나침반
    pub compass: DockingCompass,
    /// 현재 타겟 스택 ID
    pub target_stack_id: Option<NodeId>,
    /// 현재 도킹 위치
    pub dock_position: Option<DockPosition>,
    /// 탭 바 내 드롭 인덱스 (언리얼 ComputeChildDropIndex)
    pub drop_index: Option<usize>,
}

impl Default for DragState {
    fn default() -> Self {
        Self::new()
    }
}

impl DragState {
    pub fn new() -> Self {
        Self {
            operation: DragOperation::None,
            start_pos: Vec2::ZERO,
            current_pos: Vec2::ZERO,
            drag_threshold: 5.0,
            is_dragging: false,
            compass: DockingCompass::new(),
            target_stack_id: None,
            dock_position: None,
            drop_index: None,
        }
    }

    /// 드래그 중인지
    pub fn is_active(&self) -> bool {
        !matches!(self.operation, DragOperation::None)
    }

    /// 탭 드래그 시작
    pub fn start_tab_drag(&mut self, tab_id: TabId, source_stack_id: NodeId, pos: Vec2) {
        self.operation = DragOperation::DragTab {
            tab_id,
            source_stack_id,
        };
        self.start_pos = pos;
        self.current_pos = pos;
        self.is_dragging = false;
        self.target_stack_id = None;
        self.dock_position = None;
    }

    /// 스플리터 드래그 시작
    pub fn start_splitter_drag(&mut self, splitter_id: NodeId, child_index: usize, pos: Vec2) {
        self.operation = DragOperation::DragSplitter {
            splitter_id,
            child_index,
        };
        self.start_pos = pos;
        self.current_pos = pos;
        self.is_dragging = true; // 스플리터는 즉시 드래그 시작
    }

    /// 사이드바 탭 드래그 시작
    pub fn start_sidebar_tab_drag(&mut self, tab_id: TabId, side: SidebarSide, pos: Vec2) {
        self.operation = DragOperation::DragSidebarTab { tab_id, side };
        self.start_pos = pos;
        self.current_pos = pos;
        self.is_dragging = false;
        self.target_stack_id = None;
        self.dock_position = None;
    }

    /// 마우스 이동 업데이트
    pub fn update(&mut self, pos: Vec2) {
        self.current_pos = pos;

        // 드래그 임계값 체크
        if !self.is_dragging {
            let delta = pos - self.start_pos;
            if delta.length() > self.drag_threshold {
                self.is_dragging = true;
            }
        }

        // 나침반 업데이트
        if matches!(self.operation, DragOperation::DragTab { .. } | DragOperation::DragSidebarTab { .. }) {
            if self.compass.is_visible() {
                if let Some(button) = self.compass.update_hover(pos) {
                    self.dock_position = Some(button.to_dock_position());
                } else {
                    self.dock_position = None;
                }
            }
        }
    }

    /// 타겟 스택 설정
    pub fn set_target(&mut self, stack_id: Option<NodeId>, stack_rect: Option<NodeRect>) {
        self.target_stack_id = stack_id;

        if let Some(rect) = stack_rect {
            self.compass.show(rect);
        } else {
            self.compass.hide();
        }
    }

    /// 드롭 인덱스 설정 (탭 순서 변경용)
    pub fn set_drop_index(&mut self, index: Option<usize>) {
        self.drop_index = index;
    }

    /// 나침반 모핑 애니메이션 틱 (MorphToShape)
    pub fn tick_compass(&mut self, dt: f32) {
        self.compass.tick(dt);
    }

    /// 나침반 호버 상태 업데이트
    pub fn update_compass_hover(&mut self, pos: Vec2) {
        if matches!(self.operation, DragOperation::DragTab { .. } | DragOperation::DragSidebarTab { .. }) {
            if self.compass.is_visible() {
                if let Some(button) = self.compass.update_hover(pos) {
                    self.dock_position = Some(button.to_dock_position());
                } else {
                    self.dock_position = None;
                }
            }
        }
    }

    /// 드래그 취소
    pub fn cancel(&mut self) {
        self.operation = DragOperation::None;
        self.is_dragging = false;
        self.compass.hide();
        self.target_stack_id = None;
        self.dock_position = None;
        self.drop_index = None;
    }

    /// 드래그 종료 (드롭)
    pub fn finish(&mut self) -> DragResult {
        let result = match self.operation {
            DragOperation::None => DragResult::Cancelled,
            DragOperation::DragTab { tab_id, source_stack_id } => {
                if !self.is_dragging {
                    DragResult::Cancelled
                } else if let Some(target_id) = self.target_stack_id {
                    // 같은 스택 + drop_index 있음 → 탭 순서 변경 (언리얼 SDockingTabWell 스타일)
                    if target_id == source_stack_id {
                        if let Some(new_index) = self.drop_index {
                            DragResult::ReorderTab {
                                tab_id,
                                stack_id: source_stack_id,
                                new_index,
                            }
                        } else if let Some(position) = self.dock_position {
                            // 나침반으로 도킹 (같은 스택이지만 분할)
                            DragResult::DockTab {
                                tab_id,
                                source_stack_id,
                                target_stack_id: target_id,
                                position,
                            }
                        } else {
                            // 같은 스택, 나침반 없음 → 취소 (원래 위치로)
                            DragResult::Cancelled
                        }
                    } else if let Some(position) = self.dock_position {
                        // 다른 스택으로 도킹
                        DragResult::DockTab {
                            tab_id,
                            source_stack_id,
                            target_stack_id: target_id,
                            position,
                        }
                    } else {
                        // 다른 스택이지만 나침반 없음 → Center 도킹
                        DragResult::DockTab {
                            tab_id,
                            source_stack_id,
                            target_stack_id: target_id,
                            position: DockPosition::Center,
                        }
                    }
                } else {
                    // 타겟 없이 드롭 - 플로팅 윈도우 생성
                    DragResult::FloatTab {
                        tab_id,
                        source_stack_id,
                        position: self.current_pos,
                    }
                }
            }
            DragOperation::DragSplitter { splitter_id, child_index } => {
                let delta = self.current_pos - self.start_pos;
                DragResult::ResizeSplitter {
                    splitter_id,
                    child_index,
                    delta,
                }
            }
            DragOperation::DragSidebarTab { tab_id, side } => {
                if !self.is_dragging {
                    DragResult::Cancelled
                } else {
                    DragResult::RestoreFromSidebar {
                        tab_id,
                        side,
                        target_stack_id: self.target_stack_id,
                        position: self.dock_position,
                    }
                }
            }
        };

        self.cancel();
        result
    }

    /// 드래그 델타
    pub fn delta(&self) -> Vec2 {
        self.current_pos - self.start_pos
    }

    /// 드래그 중인 탭 ID
    pub fn dragging_tab(&self) -> Option<TabId> {
        match self.operation {
            DragOperation::DragTab { tab_id, .. } => Some(tab_id),
            DragOperation::DragSidebarTab { tab_id, .. } => Some(tab_id),
            _ => None,
        }
    }

    /// 드래그 중인 스플리터
    pub fn dragging_splitter(&self) -> Option<(NodeId, usize)> {
        match self.operation {
            DragOperation::DragSplitter { splitter_id, child_index } => Some((splitter_id, child_index)),
            _ => None,
        }
    }
}

/// 크로스 윈도우 드래그 드롭 이벤트
///
/// slate_app의 DockingDragOperation과 도킹 패널 간 이벤트 추상화.
/// 멀티 윈도우 간 탭 이동 시 발생하는 이벤트를 통합.
#[derive(Debug, Clone)]
pub enum DragDropEvent {
    /// 드래그 시작
    DragStarted {
        /// 드래그 중인 탭 ID
        tab_id: TabId,
        /// 스크린 좌표
        screen_pos: Vec2,
    },
    /// 드래그 중인 탭이 타겟 위를 지나가는 중
    DragOver {
        /// 드래그 중인 탭 ID
        tab_id: TabId,
        /// 타겟 스택 ID
        target_stack_id: Option<NodeId>,
        /// 마우스 위치 (로컬 좌표)
        local_pos: Vec2,
        /// 도킹 위치 (나침반에서 결정)
        dock_position: Option<DockPosition>,
    },
    /// 탭이 타겟에 드롭됨
    Drop {
        /// 드래그 중인 탭 ID
        tab_id: TabId,
        /// 타겟 스택 ID
        target_stack_id: Option<NodeId>,
        /// 도킹 위치
        dock_position: DockPosition,
    },
    /// 드래그가 타겟 영역을 벗어남
    DragLeave {
        /// 탭 ID
        tab_id: TabId,
    },
    /// 드래그 취소 (ESC 키 또는 원래 위치로 복귀)
    DragCancel {
        /// 탭 ID
        tab_id: TabId,
    },
}

/// 드래그 결과
#[derive(Debug, Clone)]
pub enum DragResult {
    /// 취소됨
    Cancelled,
    /// 탭 도킹
    DockTab {
        tab_id: TabId,
        source_stack_id: NodeId,
        target_stack_id: NodeId,
        position: DockPosition,
    },
    /// 탭 플로팅 (새 윈도우)
    FloatTab {
        tab_id: TabId,
        source_stack_id: NodeId,
        position: Vec2,
    },
    /// 스플리터 크기 조절
    ResizeSplitter {
        splitter_id: NodeId,
        child_index: usize,
        delta: Vec2,
    },
    /// 탭 순서 변경 (같은 스택 내, 언리얼 SDockingTabWell 스타일)
    ReorderTab {
        tab_id: TabId,
        stack_id: NodeId,
        new_index: usize,
    },
    /// 사이드바에서 도킹 영역으로 복원
    RestoreFromSidebar {
        tab_id: TabId,
        side: SidebarSide,
        target_stack_id: Option<NodeId>,
        position: Option<DockPosition>,
    },
}

impl DragResult {
    /// 성공적인 결과인지
    pub fn is_success(&self) -> bool {
        !matches!(self, Self::Cancelled)
    }
}
