//! Sidebar (AutoHide) 시스템
//!
//! 탭을 사이드바로 이동하면 아이콘 버튼으로 축소.
//! 클릭하면 서랍처럼 펼침, 외부 클릭 시 닫힘.

use serde::{Serialize, Deserialize};
use super::TabId;

/// 사이드바 컨텍스트 메뉴 항목 (UE5 OnGetTabDrawerContextMenuWidget)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarContextMenuItem {
    /// 탭 고정
    Pin,
    /// 탭 고정 해제
    Unpin,
    /// 탭 웰로 복원 (사이드바에서 독 영역으로)
    RestoreToTabWell,
    /// 탭 닫기
    Close,
}

/// 사이드바 위치
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SidebarSide {
    Left,
    Right,
}

/// 사이드바에 들어간 탭 항목
#[derive(Debug, Clone)]
pub struct SidebarTabEntry {
    /// 원래 탭 ID
    pub tab_id: TabId,
    /// 탭 타입명 (스포너 복원용)
    pub tab_type_name: String,
    /// 표시 이름
    pub display_name: String,
    /// 아이콘
    pub icon: Option<String>,
    /// 핀 고정 여부 — UE5.7 STabSidebar pinned tab
    /// 핀 고정된 탭은 포커스를 잃어도 서랍이 닫히지 않음
    pub pinned: bool,
    /// 도킹 상태 (UE5 ESidebarDrawerState::bIsDocked)
    pub is_docked: bool,
}

/// 사이드바 패널
pub struct SidebarPanel {
    /// 좌/우 위치
    pub location: SidebarSide,
    /// 사이드바에 있는 탭들
    pub tabs: Vec<SidebarTabEntry>,
    /// 현재 펼쳐진 탭 인덱스 (None이면 모두 닫힘)
    pub expanded: Option<usize>,
    /// 사이드바 버튼 영역 폭 (기본 32px)
    pub width: f32,
    /// 서랍 펼침 폭 (기본 280px)
    pub drawer_width: f32,
    /// 호버 중인 버튼 인덱스
    pub hovered_index: Option<usize>,
    /// 애니메이션 진행도 (0.0 = 닫힘, 1.0 = 열림)
    pub animation_progress: f32,
    /// 애니메이션 목표 (열림=true, 닫힘=false)
    pub animation_target_open: bool,

    // ---- UE5.7 추가 필드 ----

    /// 서랍 최소 폭 — UE5.7 MinDrawerSize
    pub min_drawer_width: f32,
    /// 서랍 최대 폭 — UE5.7 MaxDrawerSize
    pub max_drawer_width: f32,
    /// 서랍 폭을 부모 대비 비율로 저장 (0.0~1.0) — UE5.7 drawer size percentage
    pub drawer_size_coefficient: f32,
    /// 리사이즈 핸들 드래그 중
    pub resizing: bool,
    /// 포커스 자동 닫기 활성화 — UE5.7 focus-loss auto-dismiss
    pub auto_close_on_focus_loss: bool,
    /// 닫히는 중인 드로워들 (인덱스, 애니메이션 진행도) — UE5 OpenedDrawers
    pub closing_drawers: Vec<(usize, f32)>,
    /// 애니메이션 지속 시간 (초) — UE5 FCurveSequence 0.15s
    pub animation_duration: f32,
    /// 애니메이션 경과 시간 (초)
    pub animation_elapsed: f32,

    // ---- Batch 9: UE5 STabSidebar 추가 필드 ----

    /// 오프셋 (상단 패딩, UE5 SetOffset)
    pub offset: f32,
    /// 다음 프레임에 열 드로워 (UE5 OpenDrawerNextFrame)
    pub open_drawer_next_frame: Option<TabId>,
    /// 드로워 닫힘 콜백 (UE5 OnDrawerClosed)
    pub on_drawer_closed: Option<Box<dyn Fn(TabId) + Send + Sync>>,
    /// 드로워 크기 변경 콜백 (UE5 OnTargetDrawerSizeChanged)
    pub on_drawer_size_changed: Option<Box<dyn Fn(f32) + Send + Sync>>,

    // ---- Batch 10: UE5 SSidebar 5.7 추가 필드 ----

    /// 자동 도킹 임계값 (UE5 AutoDockThresholdSize)
    pub auto_dock_threshold: f32,
    /// 모두 도킹 시 숨기기 (UE5 bHideWhenAllDocked)
    pub hide_when_all_docked: bool,
}

impl SidebarPanel {
    pub fn new(location: SidebarSide) -> Self {
        Self {
            location,
            tabs: Vec::new(),
            expanded: None,
            width: 32.0,
            drawer_width: 280.0,
            hovered_index: None,
            animation_progress: 0.0,
            animation_target_open: false,
            min_drawer_width: 150.0,
            max_drawer_width: 600.0,
            drawer_size_coefficient: 0.0,
            resizing: false,
            auto_close_on_focus_loss: true,
            closing_drawers: Vec::new(),
            animation_duration: 0.15,
            animation_elapsed: 0.0,
            offset: 0.0,
            open_drawer_next_frame: None,
            on_drawer_closed: None,
            on_drawer_size_changed: None,
            auto_dock_threshold: 200.0,
            hide_when_all_docked: false,
        }
    }

    /// 탭 추가
    pub fn add_tab(&mut self, entry: SidebarTabEntry) {
        self.tabs.push(entry);
    }

    /// 탭 제거 (TabId로)
    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<SidebarTabEntry> {
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
            // expanded 인덱스 조정
            if let Some(exp) = self.expanded {
                if exp == idx {
                    self.expanded = None;
                } else if exp > idx {
                    self.expanded = Some(exp - 1);
                }
            }
            Some(self.tabs.remove(idx))
        } else {
            None
        }
    }

    /// 서랍 토글 (열려있으면 닫고, 닫혀있으면 열기)
    pub fn toggle(&mut self, index: usize) {
        if self.expanded == Some(index) {
            // 현재 열린 드로워를 closing_drawers로 이동
            self.closing_drawers.push((index, self.animation_progress));
            self.expanded = None;
            self.animation_target_open = false;
        } else if index < self.tabs.len() {
            // 기존 열린 드로워가 있으면 closing_drawers로 이동
            if let Some(prev_idx) = self.expanded {
                self.closing_drawers.push((prev_idx, self.animation_progress));
            }
            self.expanded = Some(index);
            self.animation_elapsed = 0.0;
            self.animation_target_open = true;
        }
    }

    /// 서랍 닫기
    pub fn close(&mut self) {
        if let Some(idx) = self.expanded {
            self.closing_drawers.push((idx, self.animation_progress));
        }
        self.expanded = None;
        self.animation_target_open = false;
    }

    /// 애니메이션 틱 (매 프레임 호출, dt_seconds)
    pub fn tick_animation(&mut self, dt: f32) {
        // 메인 드로워 애니메이션 (QuadOut)
        if self.animation_target_open {
            self.animation_elapsed = (self.animation_elapsed + dt).min(self.animation_duration);
        } else {
            self.animation_elapsed = (self.animation_elapsed - dt).max(0.0);
        }
        let t = if self.animation_duration > 0.0 {
            (self.animation_elapsed / self.animation_duration).clamp(0.0, 1.0)
        } else {
            if self.animation_target_open { 1.0 } else { 0.0 }
        };
        // QuadOut easing: 1 - (1-t)^2
        self.animation_progress = 1.0 - (1.0 - t) * (1.0 - t);

        // closing_drawers 애니메이션 업데이트
        self.closing_drawers.retain_mut(|(_, progress)| {
            *progress -= dt * (1.0 / self.animation_duration.max(0.01));
            *progress > 0.01
        });
    }

    /// 현재 애니메이션 적용된 서랍 폭
    pub fn animated_drawer_width(&self) -> f32 {
        self.drawer_width * self.animation_progress
    }

    /// 탭이 있는지
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// 탭 수
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// 비었는지
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// 실제 차지하는 폭 (탭이 없으면 0)
    pub fn total_width(&self) -> f32 {
        if self.tabs.is_empty() { 0.0 } else { self.width }
    }

    /// 서랍이 열려있는지
    pub fn is_expanded(&self) -> bool {
        self.expanded.is_some()
    }

    /// 현재 펼쳐진 탭 ID
    pub fn expanded_tab_id(&self) -> Option<TabId> {
        self.expanded.and_then(|idx| self.tabs.get(idx).map(|t| t.tab_id))
    }

    /// TabId로 탭 검색
    pub fn find_by_tab_id(&self, tab_id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.tab_id == tab_id)
    }

    // ---- UE5.7 추가 메서드 ----

    /// 탭 핀 토글 — UE5.7 핀 상호배제 + 자동 소환
    pub fn toggle_pin(&mut self, tab_id: TabId) {
        let mut target_pinned = false;
        // 1. 토글 대상 찾기
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.tab_id == tab_id) {
            tab.pinned = !tab.pinned;
            target_pinned = tab.pinned;
        }
        // 2. 핀 설정 시 다른 모든 탭 핀 해제 (상호배제)
        if target_pinned {
            for tab in &mut self.tabs {
                if tab.tab_id != tab_id {
                    tab.pinned = false;
                }
            }
            // 3. 핀 설정 시 해당 탭 드로워 자동 열기
            if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
                if self.expanded != Some(idx) {
                    if let Some(prev) = self.expanded {
                        self.closing_drawers.push((prev, self.animation_progress));
                    }
                    self.expanded = Some(idx);
                    self.animation_elapsed = 0.0;
                    self.animation_target_open = true;
                }
            }
        }
    }

    /// 핀 탭 자동 소환 — 열린 드로워 없으면 첫 번째 핀 탭 열기
    pub fn summon_pinned_tab(&mut self) {
        if self.expanded.is_some() { return; }
        if let Some(idx) = self.tabs.iter().position(|t| t.pinned) {
            self.expanded = Some(idx);
            self.animation_elapsed = 0.0;
            self.animation_target_open = true;
        }
    }

    /// 현재 펼쳐진 탭이 핀 고정되어 있는지
    pub fn is_expanded_pinned(&self) -> bool {
        self.expanded
            .and_then(|idx| self.tabs.get(idx))
            .map_or(false, |t| t.pinned)
    }

    /// 포커스 잃었을 때 호출 — UE5.7 focus-loss auto-dismiss
    ///
    /// 핀 고정 탭이 아닌 경우에만 닫힙니다.
    /// Called from SlateApp global focus change handler
    #[allow(dead_code)]
    pub fn on_focus_lost(&mut self) {
        if self.auto_close_on_focus_loss && !self.is_expanded_pinned() {
            self.close();
        }
    }

    /// 서랍 폭 리사이즈 (min/max 적용)
    pub fn resize_drawer(&mut self, new_width: f32) {
        self.drawer_width = new_width.clamp(self.min_drawer_width, self.max_drawer_width);
    }

    /// 서랍 폭을 부모 대비 비율로 계산하여 저장
    /// Called when sidebar drawer is resized — saves ratio for layout persistence
    #[allow(dead_code)]
    pub fn save_size_coefficient(&mut self, parent_width: f32) {
        if parent_width > 0.0 {
            self.drawer_size_coefficient = self.drawer_width / parent_width;
        }
    }

    /// 저장된 비율로 서랍 폭 복원
    /// Called on layout restore or window resize to recalculate drawer width from saved ratio
    #[allow(dead_code)]
    pub fn restore_from_coefficient(&mut self, parent_width: f32) {
        if self.drawer_size_coefficient > 0.0 {
            self.drawer_width = (parent_width * self.drawer_size_coefficient)
                .clamp(self.min_drawer_width, self.max_drawer_width);
        }
    }

    /// 리사이즈 핸들 시작
    pub fn begin_resize(&mut self) {
        self.resizing = true;
    }

    /// 리사이즈 핸들 종료
    pub fn end_resize(&mut self) {
        self.resizing = false;
    }

    // ── Batch 9: UE5 STabSidebar 추가 메서드 ──

    /// 활성 탭 변경 시 호출 (UE5 OnActiveTabChanged)
    pub fn on_active_tab_changed(&mut self, tab_id: TabId) {
        // 사이드바에 해당 탭이 있으면 자동 열기
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
            self.expanded = Some(idx);
            self.animation_target_open = true;
            self.animation_elapsed = 0.0;
        }
    }

    /// 다음 프레임에 드로워 열기 (UE5 OpenDrawerNextFrame)
    pub fn open_drawer_next_frame(&mut self, tab_id: TabId) {
        self.open_drawer_next_frame = Some(tab_id);
    }

    /// 틱에서 지연 열기 처리
    pub fn process_deferred_open(&mut self) {
        if let Some(tab_id) = self.open_drawer_next_frame.take() {
            if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
                self.expanded = Some(idx);
                self.animation_target_open = true;
                self.animation_elapsed = 0.0;
            }
        }
    }

    /// 드로워 크기 동적 계산 (UE5 compute drawer size)
    pub fn compute_drawer_size(&self, available_width: f32) -> f32 {
        if self.drawer_size_coefficient > 0.0 {
            (available_width * self.drawer_size_coefficient)
                .clamp(self.min_drawer_width, self.max_drawer_width)
        } else {
            self.drawer_width.clamp(self.min_drawer_width, self.max_drawer_width)
        }
    }

    /// 탭 복원 (사이드바→독, UE5 RestoreTab)
    ///
    /// 사이드바에서 탭을 제거하고 반환.
    /// expanded 인덱스 조정은 `remove_tab()`에서 이미 처리
    /// (해당 탭이 expanded였으면 expanded=None, 이후 인덱스이면 -1).
    pub fn restore_tab(&mut self, tab_id: TabId) -> Option<SidebarTabEntry> {
        self.remove_tab(tab_id)
    }

    /// 탭 닫기 (UE5 OnCloseTab)
    pub fn close_tab(&mut self, tab_id: TabId) -> Option<SidebarTabEntry> {
        let entry = self.remove_tab(tab_id)?;
        if let Some(ref cb) = self.on_drawer_closed {
            cb(tab_id);
        }
        Some(entry)
    }

    /// 상단 오프셋 설정 (UE5 SetOffset)
    pub fn set_offset(&mut self, offset: f32) {
        self.offset = offset;
    }

    /// 드로워 크기 변경 알림 (UE5 OnTargetDrawerSizeChanged)
    pub fn notify_drawer_size_changed(&self, new_size: f32) {
        if let Some(ref cb) = self.on_drawer_size_changed {
            cb(new_size);
        }
    }

    // ── Batch 10: UE5 SSidebar 5.7 추가 메서드 ──

    /// 드로워 도킹 상태 설정 (UE5 SetDrawerDocked)
    pub fn set_drawer_docked(&mut self, tab_id: TabId, docked: bool) {
        if let Some(entry) = self.tabs.iter_mut().find(|t| t.tab_id == tab_id) {
            entry.is_docked = docked;
        }
    }

    /// 드로워 등록 (UE5 RegisterDrawer)
    pub fn register_drawer(&mut self, entry: SidebarTabEntry) {
        if !self.tabs.iter().any(|t| t.tab_id == entry.tab_id) {
            self.tabs.push(entry);
        }
    }

    /// 드로워 등록 해제 (UE5 UnregisterDrawer)
    pub fn unregister_drawer(&mut self, tab_type: &str) -> Option<SidebarTabEntry> {
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_type_name == tab_type) {
            Some(self.tabs.remove(idx))
        } else {
            None
        }
    }

    /// 드로워 포함 여부 (UE5 ContainsDrawer)
    pub fn contains_drawer(&self, tab_type: &str) -> bool {
        self.tabs.iter().any(|t| t.tab_type_name == tab_type)
    }

    /// 드로워 수 (UE5 GetDrawerCount)
    pub fn drawer_count(&self) -> usize {
        self.tabs.len()
    }

    /// 사이드바 표시 여부 (hide_when_all_docked 체크)
    pub fn should_show(&self) -> bool {
        if self.tabs.is_empty() {
            return false;
        }
        if self.hide_when_all_docked && self.tabs.iter().all(|t| t.is_docked) {
            return false;
        }
        true
    }

    // ── 12차 Batch 8: STabSidebar 완성 ──

    /// 전체 탭 ID 목록 (UE5 GetAllTabIds)
    pub fn get_all_tab_ids(&self) -> Vec<super::TabId> {
        self.tabs.iter().map(|t| t.tab_id).collect()
    }

    /// 전체 탭 참조 (UE5 GetAllTabs)
    pub fn get_all_tabs(&self) -> &[SidebarTabEntry] {
        &self.tabs
    }

    // ── 13차 Batch E: STabSidebar 확장 ──

    /// 탭 포함 여부 (UE5 ContainsTab)
    pub fn contains_tab_by_id(&self, tab_id: super::TabId) -> bool {
        self.tabs.iter().any(|t| t.tab_id == tab_id)
    }

    /// 열린 드로워 검색 (UE5 FindOpenedDrawer)
    pub fn find_opened_drawer(&self, tab_id: super::TabId) -> Option<usize> {
        if let Some(idx) = self.expanded {
            if self.tabs.get(idx).map_or(false, |t| t.tab_id == tab_id) {
                return Some(idx);
            }
        }
        None
    }

    /// 포그라운드 탭 반환 (UE5 GetForegroundTab)
    pub fn get_foreground_tab(&self) -> Option<super::TabId> {
        self.expanded.and_then(|idx| self.tabs.get(idx).map(|t| t.tab_id))
    }

    /// 첫 번째 고정 탭 (UE5 FindFirstPinnedTab)
    pub fn find_first_pinned_tab(&self) -> Option<super::TabId> {
        self.tabs.iter().find(|t| t.pinned).map(|t| t.tab_id)
    }

    /// 드로워 외관 갱신 (UE5 UpdateDrawerAppearance)
    pub fn update_drawer_appearance(&mut self) {
        // 모든 탭이 도킹 상태이고 hide_when_all_docked이면 닫기
        if self.hide_when_all_docked && self.tabs.iter().all(|t| t.is_docked) {
            self.close();
        }
    }

    /// 전체 드로워 제거 (UE5 RemoveAllDrawers)
    pub fn remove_all_drawers(&mut self) {
        self.tabs.clear();
        self.expanded = None;
        self.animation_target_open = false;
        self.closing_drawers.clear();
    }

    /// 탭 드로워 컨텍스트 메뉴 항목 (UE5 OnGetTabDrawerContextMenuWidget)
    ///
    /// 사이드바 탭 우클릭 시 표시할 메뉴 항목 목록 반환.
    pub fn get_context_menu_items(&self, tab_id: super::TabId) -> Vec<SidebarContextMenuItem> {
        let mut items = Vec::new();
        if let Some(entry) = self.tabs.iter().find(|t| t.tab_id == tab_id) {
            // Pin/Unpin 토글
            if entry.pinned {
                items.push(SidebarContextMenuItem::Unpin);
            } else {
                items.push(SidebarContextMenuItem::Pin);
            }
            // 탭 웰로 복원
            items.push(SidebarContextMenuItem::RestoreToTabWell);
            // 닫기
            items.push(SidebarContextMenuItem::Close);
        }
        items
    }

    /// 드로워 열기 시도 (UE5 TryOpenSidebarDrawer)
    ///
    /// 해당 탭이 사이드바에 있으면 열고 true 반환, 없으면 false.
    pub fn try_open_sidebar_drawer(&mut self, tab_id: super::TabId) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
            // 이미 열려있으면 무시
            if self.expanded == Some(idx) {
                return true;
            }
            // 기존 열린 드로워 닫기
            if let Some(prev) = self.expanded {
                self.closing_drawers.push((prev, self.animation_progress));
            }
            self.expanded = Some(idx);
            self.animation_elapsed = 0.0;
            self.animation_target_open = true;
            true
        } else {
            false
        }
    }
}

/// 사이드바 상태 직렬화 (UE5 FSidebarState)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarState {
    /// 사이드바 위치
    pub location: SidebarSide,
    /// 탭 타입 이름 목록
    pub tab_types: Vec<String>,
    /// 각 탭의 도킹 상태
    pub docked_states: Vec<bool>,
    /// 드로워 크기 계수
    pub drawer_size_coefficient: f32,
}

impl SidebarPanel {
    /// 현재 상태 캡처 (UE5 FSidebarState::GetState)
    pub fn get_state(&self) -> SidebarState {
        SidebarState {
            location: self.location,
            tab_types: self.tabs.iter().map(|t| t.tab_type_name.clone()).collect(),
            docked_states: self.tabs.iter().map(|t| t.is_docked).collect(),
            drawer_size_coefficient: self.drawer_size_coefficient,
        }
    }

    /// 상태 복원 (UE5 FSidebarState::RestoreState)
    pub fn restore_state(&mut self, state: &SidebarState) {
        self.drawer_size_coefficient = state.drawer_size_coefficient;
        // 탭 도킹 상태 복원
        for (i, tab_type) in state.tab_types.iter().enumerate() {
            if let Some(entry) = self.tabs.iter_mut().find(|t| &t.tab_type_name == tab_type) {
                entry.is_docked = state.docked_states.get(i).copied().unwrap_or(false);
            }
        }
    }
}
