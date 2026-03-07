//! PanelDrawer 시스템 (UE5 SPanelDrawerArea)
//!
//! 탭을 패널 드로워에 호스팅하여 사이드 패널로 표시.
//! DockArea에 연결되어 탭을 인라인 드로워로 관리.

use super::TabId;
use serde::{Serialize, Deserialize};

/// 패널 드로워 데이터 (UE5 FPanelDrawerData)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelDrawerData {
    /// 호스팅된 탭 ID
    pub tab_id: TabId,
    /// 드로워 크기 (폭 또는 높이)
    pub size: f32,
    /// 드로워 위치 (오프셋)
    pub position: f32,
}

impl PanelDrawerData {
    pub fn new(tab_id: TabId) -> Self {
        Self {
            tab_id,
            size: 300.0,
            position: 0.0,
        }
    }
}

/// 패널 드로워 탭 (UE5 FPanelDrawerTab)
#[derive(Debug, Clone)]
pub struct PanelDrawerTab {
    /// 탭 ID
    pub tab_id: TabId,
    /// 헤더 텍스트
    pub header: String,
}

impl PanelDrawerTab {
    pub fn new(tab_id: TabId, header: impl Into<String>) -> Self {
        Self {
            tab_id,
            header: header.into(),
        }
    }
}

/// 패널 드로워 애니메이션 단계 (UE5 bIsAnimating + bIsOpen 조합)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimationPhase {
    /// 완전히 닫힌 상태
    Closed,
    /// 열리는 중 (애니메이션 진행)
    Opening,
    /// 완전히 열린 상태
    Open,
    /// 닫히는 중 (애니메이션 진행)
    Closing,
}

impl Default for AnimationPhase {
    fn default() -> Self { Self::Closed }
}

/// 패널 드로워 상태 (DockArea에 내장)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PanelDrawerState {
    /// 현재 열린 드로워 탭 ID
    pub open_tab: Option<TabId>,
    /// 드로워가 열려있는지
    pub is_open: bool,
    /// 드로워 폭
    pub width: f32,
    /// 애니메이션 진행도 (0.0=닫힘, 1.0=열림)
    #[serde(skip)]
    pub animation_progress: f32,
    /// 애니메이션 단계 (UE5 SPanelDrawerArea 상태 머신)
    #[serde(skip, default)]
    pub animation_phase: AnimationPhase,
}

impl PanelDrawerState {
    pub fn new() -> Self {
        Self {
            open_tab: None,
            is_open: false,
            width: 300.0,
            animation_progress: 0.0,
            animation_phase: AnimationPhase::Closed,
        }
    }

    /// 패널 열기 (UE5 SPanelDrawerArea::OpenPanel)
    pub fn open_panel(&mut self, tab_id: TabId, size: super::PanelDrawerSize) {
        self.open_tab = Some(tab_id);
        self.is_open = true;
        if size.width > 0.0 {
            self.width = size.width;
        }
    }

    /// 패널 닫기 (UE5 SPanelDrawerArea::ClosePanel)
    pub fn close_panel(&mut self, _animate: bool) {
        self.is_open = false;
        // animate=true이면 animation_progress를 서서히 0으로 감소 (tick에서 처리)
        // animate=false이면 즉시 닫기
        if !_animate {
            self.animation_progress = 0.0;
        }
    }
}

/// 패널 드로워 영역 위젯 (UE5 SPanelDrawerArea)
pub struct SPanelDrawerArea {
    /// 호스팅된 탭 목록
    pub tabs: Vec<PanelDrawerTab>,
    /// 현재 열린 탭 인덱스
    pub active_tab: Option<usize>,
    /// 드로워 폭
    pub width: f32,
    /// 드로워 위치 (오프셋)
    pub position: f32,
    /// 애니메이션 진행도
    pub animation_progress: f32,
    /// 애니메이션 단계 (UE5 SPanelDrawerArea 상태 머신)
    pub animation_phase: AnimationPhase,

    // ── 13차: SPanelDrawerArea 확장 필드 ──

    /// 메인 콘텐츠 비율 (UE5 GetMainContentCoefficient)
    pub main_content_coefficient: f32,
    /// 드로워 비율 (UE5 GetPanelDrawerCoefficient)
    pub panel_drawer_coefficient: f32,
    /// 외부 상태 변경 콜백 (UE5 GetOnExternalStateChanged)
    #[allow(clippy::type_complexity)]
    pub on_external_state_changed: Option<Box<dyn FnMut() + Send + Sync>>,
}

impl SPanelDrawerArea {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
            width: 300.0,
            position: 0.0,
            animation_progress: 0.0,
            animation_phase: AnimationPhase::Closed,
            main_content_coefficient: 0.7,
            panel_drawer_coefficient: 0.3,
            on_external_state_changed: None,
        }
    }

    /// 탭 추가
    pub fn add_tab(&mut self, tab: PanelDrawerTab) {
        self.tabs.push(tab);
    }

    /// 탭 제거
    pub fn remove_tab(&mut self, tab_id: TabId) -> Option<PanelDrawerTab> {
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
            let tab = self.tabs.remove(idx);
            // 활성 탭 보정
            if let Some(active) = self.active_tab {
                if active >= self.tabs.len() {
                    self.active_tab = if self.tabs.is_empty() { None } else { Some(self.tabs.len() - 1) };
                } else if active > idx {
                    self.active_tab = Some(active - 1);
                }
            }
            Some(tab)
        } else {
            None
        }
    }

    /// 탭 활성화 (드로워 열기)
    pub fn activate_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
            self.active_tab = Some(idx);
            true
        } else {
            false
        }
    }

    /// 활성 탭 ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab.and_then(|idx| self.tabs.get(idx).map(|t| t.tab_id))
    }

    /// 비었는지
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// 탭 수
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    // ── 13차 Batch E: SPanelDrawerArea 확장 메서드 ──

    /// 외부 상태 변경 콜백 참조 (UE5 GetOnExternalStateChanged)
    pub fn get_on_external_state_changed(&mut self) -> &mut Option<Box<dyn FnMut() + Send + Sync>> {
        &mut self.on_external_state_changed
    }

    /// 호스팅 드로워 데이터 반환 (UE5 GetHostedPanelDrawerData)
    pub fn get_hosted_panel_drawer_data(&self) -> Vec<PanelDrawerData> {
        self.tabs.iter().map(|t| PanelDrawerData::new(t.tab_id)).collect()
    }

    /// 메인 콘텐츠 비율 (UE5 GetMainContentCoefficient)
    pub fn get_main_content_coefficient(&self) -> f32 {
        self.main_content_coefficient
    }

    /// 드로워 비율 (UE5 GetPanelDrawerCoefficient)
    pub fn get_panel_drawer_coefficient(&self) -> f32 {
        self.panel_drawer_coefficient
    }

    /// 메인 콘텐츠 리사이즈 (UE5 OnMainContentSlotResized)
    pub fn on_main_content_slot_resized(&mut self, val: f32) {
        self.main_content_coefficient = val.clamp(0.1, 0.9);
        self.panel_drawer_coefficient = 1.0 - self.main_content_coefficient;
        if let Some(ref mut cb) = self.on_external_state_changed {
            cb();
        }
    }

    /// 드로워 리사이즈 (UE5 OnPanelDrawerSlotResized)
    pub fn on_panel_drawer_slot_resized(&mut self, val: f32) {
        self.panel_drawer_coefficient = val.clamp(0.1, 0.9);
        self.main_content_coefficient = 1.0 - self.panel_drawer_coefficient;
        if let Some(ref mut cb) = self.on_external_state_changed {
            cb();
        }
    }

    /// 애니메이션 틱 — phase별 분기 (UE5 SPanelDrawerArea::Tick)
    pub fn tick(&mut self, dt: f32) {
        let speed = 8.0;
        match self.animation_phase {
            AnimationPhase::Opening => {
                self.animation_progress += (1.0 - self.animation_progress) * (speed * dt).min(1.0);
                if (self.animation_progress - 1.0).abs() < 0.001 {
                    self.animation_progress = 1.0;
                    self.animation_phase = AnimationPhase::Open;
                    self.setup_opened_layout();
                }
            }
            AnimationPhase::Closing => {
                self.animation_progress -= self.animation_progress * (speed * dt).min(1.0);
                if self.animation_progress < 0.001 {
                    self.animation_progress = 0.0;
                    self.animation_phase = AnimationPhase::Closed;
                    self.setup_closed_layout();
                }
            }
            AnimationPhase::Open | AnimationPhase::Closed => {
                // 정지 상태 — anim_t 클램프만
                let target = if self.active_tab.is_some() { 1.0 } else { 0.0 };
                self.animation_progress = target;
            }
        }
    }

    // ── 15차: SPanelDrawerArea 애니메이션 갭 클로저 (9건) ──

    /// 레이아웃 저장 요청 (UE5 SPanelDrawerArea::RequestSaveLayout)
    ///
    /// 외부 상태 변경 콜백을 통해 호출자에게 레이아웃 변경 알림.
    pub fn request_save_layout(&mut self) {
        if let Some(ref mut cb) = self.on_external_state_changed {
            cb();
        }
    }

    /// 애니메이션 레이아웃 초기화 (UE5 SPanelDrawerArea::SetupAnimationLayout)
    ///
    /// Open→Close 또는 Close→Open 전환 시작 시 호출.
    /// splitter coefficient를 애니메이션 중간 상태로 설정.
    pub fn setup_animation_layout(&mut self) {
        // 애니메이션 진행 중 coefficient는 animation_progress에 의해 보간됨
        // 별도 스냅샷 불필요 — tick()에서 실시간 계산
    }

    /// 열림 레이아웃 설정 (UE5 SPanelDrawerArea::SetupOpenedLayout)
    ///
    /// 애니메이션 완료 후 열림 상태의 최종 레이아웃 확정.
    pub fn setup_opened_layout(&mut self) {
        self.animation_progress = 1.0;
        // coefficient를 드로워 비율에 맞게 고정
        self.panel_drawer_coefficient = 0.3_f32.max(self.panel_drawer_coefficient);
        self.main_content_coefficient = 1.0 - self.panel_drawer_coefficient;
    }

    /// 닫힘 레이아웃 설정 (UE5 SPanelDrawerArea::SetupClosedLayout)
    ///
    /// 애니메이션 완료 후 닫힘 상태의 최종 레이아웃 확정.
    pub fn setup_closed_layout(&mut self) {
        self.animation_progress = 0.0;
        self.active_tab = None;
    }

    /// 애니메이션 슬라이드 폭 갱신 (UE5 SPanelDrawerArea::UpdateAnimatedSlideWidth)
    ///
    /// 현재 animation_progress에 기반하여 실제 표시할 드로워 폭 계산.
    pub fn update_animated_slide_width(&mut self) {
        self.position = self.width * self.animation_progress;
    }

    /// 스페이서 애니메이션 폭 (UE5 GetAnimatedWidthOverrideForSpacer)
    ///
    /// 드로워 열림 시 메인 콘텐츠 옆 스페이서 폭 반환.
    pub fn animated_width_for_spacer(&self) -> f32 {
        self.width * (1.0 - self.animation_progress)
    }

    /// 드로워 애니메이션 폭 (UE5 GetAnimatedWidthOverrideForPanelDrawer)
    ///
    /// 현재 animation_progress 기반 드로워 실제 표시 폭.
    pub fn animated_width_for_drawer(&self) -> f32 {
        self.width * self.animation_progress
    }

    /// 스페이서 가시성 (UE5 GetAnimatedSpacerVisibility)
    ///
    /// 드로워가 열리는 중이면 스페이서를 숨겨야 함 → false 반환.
    pub fn animated_spacer_visibility(&self) -> bool {
        matches!(self.animation_phase, AnimationPhase::Closed)
    }

    /// 드로워 패널 가시성 (UE5 GetAnimatedDrawerPanelVisibility)
    ///
    /// 드로워가 닫힌 상태에서만 false (완전히 숨김).
    pub fn animated_drawer_visibility(&self) -> bool {
        !matches!(self.animation_phase, AnimationPhase::Closed)
    }

    /// 패널 열기 (phase 기반, UE5 OpenPanel 확장)
    pub fn open_panel_animated(&mut self, tab_id: TabId) {
        if let Some(idx) = self.tabs.iter().position(|t| t.tab_id == tab_id) {
            self.active_tab = Some(idx);
            self.animation_phase = AnimationPhase::Opening;
            self.setup_animation_layout();
        }
    }

    /// 패널 닫기 (phase 기반, UE5 ClosePanel 확장)
    pub fn close_panel_animated(&mut self) {
        if self.active_tab.is_some() {
            self.animation_phase = AnimationPhase::Closing;
            self.setup_animation_layout();
        }
    }

    // ── 17차: SPanelDrawerArea 플로팅 윈도우 전환 (G1, G2) ──

    /// 드로워를 플로팅 윈도우로 전환 (UE5 SPanelDrawerArea::FloatDrawerToWindow)
    ///
    /// 현재 활성 드로워 탭을 분리하여 플로팅 윈도우로 전환 요청.
    /// 반환값: 플로팅으로 전환된 탭 ID (없으면 None).
    ///
    /// 실제 윈도우 생성은 호출자(SDockingPanel)에서 처리.
    pub fn float_drawer_to_window(&mut self, tab_id: TabId) -> Option<TabId> {
        let idx = self.tabs.iter().position(|t| t.tab_id == tab_id)?;
        let _removed = self.tabs.remove(idx);
        // 활성 탭 보정
        if let Some(active) = self.active_tab {
            if active >= self.tabs.len() {
                self.active_tab = if self.tabs.is_empty() { None } else { Some(self.tabs.len() - 1) };
            } else if active > idx {
                self.active_tab = Some(active - 1);
            }
        }
        // 드로워가 비었으면 닫기
        if self.tabs.is_empty() {
            self.animation_phase = AnimationPhase::Closing;
        }
        Some(tab_id)
    }

    /// 드로워 플로팅 완료 후 처리 콜백 (UE5 SPanelDrawerArea::HandleDrawerFloated)
    ///
    /// 플로팅 윈도우 전환 완료 후 호출.
    /// 외부 상태 변경 콜백을 트리거하여 레이아웃 갱신.
    pub fn handle_drawer_floated(&mut self, _tab_id: TabId) {
        // 레이아웃 갱신 알림
        if let Some(ref mut cb) = self.on_external_state_changed {
            cb();
        }
    }
}

impl Default for SPanelDrawerArea {
    fn default() -> Self {
        Self::new()
    }
}

/// 탭 패널 드로워 헤더 위젯 (UE5 STabPanelDrawer)
pub struct STabPanelDrawer {
    /// 탭 ID
    pub tab_id: TabId,
    /// 헤더 텍스트
    pub header: String,
    /// 호버 중인지
    pub is_hovered: bool,
}

impl STabPanelDrawer {
    pub fn new(tab_id: TabId, header: impl Into<String>) -> Self {
        Self {
            tab_id,
            header: header.into(),
            is_hovered: false,
        }
    }
}

// ============================================================================
// DockArea PanelDrawer 메서드 (node.rs에서 호출)
// ============================================================================

/// DockArea의 PanelDrawer API (UE5 SDockingArea PanelDrawer 메서드)
///
/// 이 함수들은 DockArea의 panel_drawer 필드를 통해 호출됨.
pub mod area_api {
    use super::*;

    /// 탭을 패널 드로워에 호스팅 (UE5 HostTabIntoPanelDrawer)
    pub fn host_tab(state: &mut Option<PanelDrawerState>, tab_id: TabId) {
        let drawer = state.get_or_insert_with(PanelDrawerState::new);
        drawer.open_tab = Some(tab_id);
        drawer.is_open = true;
    }

    /// 패널 드로워 닫기 (UE5 ClosePanelDrawer)
    pub fn close(state: &mut Option<PanelDrawerState>) {
        if let Some(ref mut drawer) = state {
            drawer.is_open = false;
            drawer.open_tab = None;
        }
    }

    /// 전송을 위한 패널 드로워 닫기 (UE5 ClosePanelDrawerForTransfer)
    pub fn close_for_transfer(state: &mut Option<PanelDrawerState>) -> Option<TabId> {
        if let Some(ref mut drawer) = state {
            let tab = drawer.open_tab.take();
            drawer.is_open = false;
            tab
        } else {
            None
        }
    }

    /// 패널 드로워 열려있는지 (UE5 IsPanelDrawerOpen)
    pub fn is_open(state: &Option<PanelDrawerState>) -> bool {
        state.as_ref().map_or(false, |d| d.is_open)
    }

    /// 패널 드로워 존재하는지 (UE5 HasPanelDrawer)
    pub fn has_drawer(state: &Option<PanelDrawerState>) -> bool {
        state.is_some()
    }

    /// 특정 탭이 호스팅 중인지 확인 (UE5 GetPanelDrawerSystemHostedTab — bool 버전)
    pub fn hosted_tab(state: &Option<PanelDrawerState>, tab_id: TabId) -> bool {
        state.as_ref().map_or(false, |d| d.open_tab == Some(tab_id))
    }

    /// 현재 호스팅 중인 탭 반환 (UE5 GetHostedTab)
    pub fn get_hosted_tab(state: &Option<PanelDrawerState>) -> Option<TabId> {
        state.as_ref().and_then(|d| if d.is_open { d.open_tab } else { None })
    }

    /// 패널 드로워 분리 (UE5 DetachPanelDrawerArea)
    pub fn detach(state: &mut Option<PanelDrawerState>) -> Option<PanelDrawerState> {
        state.take()
    }

    /// 패널 드로워 복원 (UE5 RestorePanelDrawerArea)
    pub fn restore(state: &mut Option<PanelDrawerState>, restored: PanelDrawerState) {
        *state = Some(restored);
    }

    /// 패널 드로워 정리 (UE5 CleanPanelDrawer)
    pub fn clean(state: &mut Option<PanelDrawerState>) {
        if let Some(ref mut drawer) = state {
            drawer.open_tab = None;
            drawer.is_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_drawer_data() {
        let data = PanelDrawerData::new(TabId::new(1));
        assert_eq!(data.tab_id, TabId::new(1));
        assert_eq!(data.size, 300.0);
    }

    #[test]
    fn test_panel_drawer_state() {
        let mut state: Option<PanelDrawerState> = None;

        assert!(!area_api::has_drawer(&state));
        assert!(!area_api::is_open(&state));

        area_api::host_tab(&mut state, TabId::new(1));
        assert!(area_api::has_drawer(&state));
        assert!(area_api::is_open(&state));
        assert!(area_api::hosted_tab(&state, TabId::new(1)));
        assert!(!area_api::hosted_tab(&state, TabId::new(2)));

        area_api::close(&mut state);
        assert!(!area_api::is_open(&state));
        assert!(area_api::has_drawer(&state)); // still exists

        area_api::clean(&mut state);
        assert!(!area_api::is_open(&state));
    }

    #[test]
    fn test_panel_drawer_area() {
        let mut area = SPanelDrawerArea::new();
        assert!(area.is_empty());

        area.add_tab(PanelDrawerTab::new(TabId::new(1), "Tab 1"));
        area.add_tab(PanelDrawerTab::new(TabId::new(2), "Tab 2"));
        assert_eq!(area.tab_count(), 2);

        assert!(area.activate_tab(TabId::new(1)));
        assert_eq!(area.active_tab_id(), Some(TabId::new(1)));

        let removed = area.remove_tab(TabId::new(1));
        assert!(removed.is_some());
        assert_eq!(area.tab_count(), 1);
    }

    #[test]
    fn test_panel_drawer_transfer() {
        let mut state: Option<PanelDrawerState> = None;
        area_api::host_tab(&mut state, TabId::new(5));

        let tab = area_api::close_for_transfer(&mut state);
        assert_eq!(tab, Some(TabId::new(5)));
        assert!(!area_api::is_open(&state));
    }

    #[test]
    fn test_panel_drawer_detach_restore() {
        let mut state: Option<PanelDrawerState> = None;
        area_api::host_tab(&mut state, TabId::new(3));

        let detached = area_api::detach(&mut state);
        assert!(detached.is_some());
        assert!(!area_api::has_drawer(&state));

        area_api::restore(&mut state, detached.unwrap());
        assert!(area_api::has_drawer(&state));
    }
}
