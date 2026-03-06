//! 도킹 탭 관리

use super::{TabId, TabRole, TabPersistability, TabActivationCause};
use super::docking_tab_stack::TabContextMenuItem;
use crate::core::Color;
use crate::framework::{CurveSequence, AnimationCurve, EasingFunction};
use crate::widget::Widget;
use std::collections::HashMap;

/// 텍스트 오버플로 정책 (UE5 ETextOverflowPolicy)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextOverflowPolicy {
    /// 말줄임표로 잘라냄
    Ellipsis,
    /// 클리핑으로 잘라냄
    Clip,
}

/// 탭 정보
pub struct DockTab {
    /// 고유 ID
    pub id: TabId,
    /// 탭 제목
    pub title: String,
    /// 탭 아이콘 (옵션)
    pub icon: Option<String>,
    /// 닫기 가능 여부
    pub closable: bool,
    /// 탭 콘텐츠 위젯
    pub content: Box<dyn Widget>,
    /// 탭 역할 (Major / Panel / Nomad / Document)
    pub role: TabRole,
    /// Document 탭용: 탭 타입 이름 (같은 타입의 여러 인스턴스 구분)
    pub tab_type: Option<String>,
    /// Document 인스턴스 ID (에셋 경로 등 — 동일 tab_type 내 재사용 검색용)
    pub instance_id: Option<String>,
    /// 레이아웃 영속성 (저장 가능 여부)
    pub persistability: TabPersistability,
    /// 닫기 요청 콜백 (true 반환 시 닫기 허용)
    pub on_close_requested: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    /// 탭 닫힌 후 콜백 (post-close notification)
    pub on_tab_closed: Option<Box<dyn Fn(TabId) + Send + Sync>>,
    /// 탭 활성화 콜백 (UE5 OnTabActivated — 뷰포트 렌더링 재개 등)
    pub on_tab_activated: Option<Box<dyn Fn(TabId, TabActivationCause) + Send + Sync>>,
    /// 탭별 색상 틴트 (UE TabColorScale)
    pub color_tint: Option<Color>,
    /// 스폰 애니메이션 (탭 생성 시 열리는 효과, UE SpawnAnimCurve)
    pub spawn_anim: CurveSequence,
    /// 스폰 애니메이션 활성 여부
    pub spawn_anim_playing: bool,
    /// 플래시 애니메이션 (주의 끌기, UE FlashTabCurve)
    pub flash_anim: CurveSequence,
    /// 탭웰 좌측 콘텐츠 슬롯 (UE ContentLeft)
    pub tab_well_content_left: Option<Box<dyn Widget>>,
    /// 탭웰 우측 콘텐츠 슬롯 (UE ContentRight)
    pub tab_well_content_right: Option<Box<dyn Widget>>,

    // ── H7: UE5 SDockTab 추가 필드 ──

    /// 탭 라벨 접미사 (UE5 TabLabelSuffix — 더티 마커 "*" 등)
    ///
    /// 기본 제목을 수정하지 않고 접미사를 별도 관리.
    /// `get_tab_label()` = `title + label_suffix`.
    pub label_suffix: String,
    /// 타이틀바 우측 커스텀 위젯 (UE5 TitleBarContentRight — Sequencer 녹화 버튼 등)
    pub title_bar_content_right: Option<Box<dyn Widget>>,
    /// 탭 잠금 여부 (UE5 bIsTabLocked — 잠금 시 닫기 불가)
    pub is_locked: bool,
    /// 시각적 탭 역할 (UE5 GetVisualTabRole — Nomad→Panel 변환 등)
    ///
    /// None이면 `role`을 그대로 사용.
    pub visual_tab_role: Option<TabRole>,
    /// 최종 활성화 시각 (UE5 LastActivationTime — MRU 탭 순서용)
    pub last_activation_time: f64,
    /// 마지막 활성화 원인 (UE5 ETabActivationCause)
    pub last_activation_cause: TabActivationCause,

    // ── S-06/S-09: UE5 SDockTab 추가 필드 ──

    /// 탭 이름 변경 콜백 (UE5 OnTabRenamed — 탭 리네임 시 외부 알림)
    pub on_tab_renamed: Option<Box<dyn Fn(TabId, &str) + Send + Sync>>,
    /// 시각 상태 영속화 콜백 (UE5 OnPersistVisualState — 레이아웃 저장 시 호출)
    pub on_persist_visual_state: Option<Box<dyn Fn(TabId) + Send + Sync>>,
    /// 자동 크기 조절 여부 (UE5 bShouldAutosize — SDockingTabStack에서 SizeToContent 용)
    pub should_autosize: bool,
    /// 탭 재배치 콜백 (UE5 OnTabRelocated — 탭이 다른 스택으로 이동 시 알림)
    pub on_tab_relocated: Option<Box<dyn Fn(TabId) + Send + Sync>>,
    /// 콘텐츠 영역 커스텀 패딩 (UE5 ContentAreaPadding — None이면 테마 기본값)
    pub content_padding: Option<f32>,
    /// 탭 이름 숨김 여부 (UE5 IsTabNameHidden — true이면 아이콘만 표시)
    pub is_tab_name_hidden: bool,

    // ── Batch 4: UE5 SDockTab 추가 필드 ──

    /// 읽기 전용 상태 (UE5 bIsReadOnly)
    pub is_read_only: bool,
    /// 메인 탭 여부 (true이면 닫기 불가)
    pub is_main_tab: bool,
    /// 소유 MajorTab 인덱스 (UE5 OwnerMajorTabIndex)
    pub owner_major_tab_index: Option<usize>,
    /// 드래그 호버 활성화 시간 (UE5 DragHoverActivationTime)
    pub drag_hover_activation_time: Option<f64>,

    // ── Batch 6: UE5 SDockTab 추가 필드 ──

    /// 오버플로 정책 (UE5 SetTabLabelOverflowPolicy)
    pub overflow_policy: Option<TextOverflowPolicy>,
    /// 아이콘 색상 틴트 (UE5 IconColor — per-icon tint, color_tint과 별도)
    pub icon_color: Option<Color>,
    /// 탭이 드래그 중인 DockArea ID (UE5 DraggedOverDockArea)
    pub dragged_over_dock_area: Option<u64>,
    /// DockArea 위로 드래그 시 콜백 (UE5 OnTabDraggedOverDockArea)
    pub on_tab_dragged_over_dock_area: Option<Box<dyn Fn() + Send + Sync>>,
    /// 컨텍스트 메뉴 확장 콜백 (UE5 OnExtendContextMenu)
    pub on_extend_context_menu: Option<Box<dyn Fn(&mut Vec<TabContextMenuItem>) + Send + Sync>>,
    /// 드로워 열림 콜백 (UE5 OnTabDrawerOpened)
    pub on_tab_drawer_opened: Option<Box<dyn Fn() + Send + Sync>>,
    /// 드로워 닫힘 콜백 (UE5 OnTabDrawerClosed)
    pub on_tab_drawer_closed: Option<Box<dyn Fn() + Send + Sync>>,
    /// 트리 ID (UE5 SetTabManager — 경량 역참조)
    pub tree_id: Option<u64>,
}

/// 스폰 애니메이션 CurveSequence 생성 (UE SpawnAnimCurve: 0→1, Linear, 0.15초)
/// UE5 원본: FCurveSequence(0, 0.15f) — 높이(Y) 스케일 0→1
fn make_spawn_anim() -> CurveSequence {
    let mut seq = CurveSequence::new();
    seq.add_curve(
        AnimationCurve::new(0.15)
            .with_easing(EasingFunction::Linear)
            .from_to(0.0, 1.0),
    );
    seq
}

/// 플래시 애니메이션 CurveSequence 생성 (UE FlashTabCurve: Linear, 1.0초)
/// 실제 플래시 값은 get_flash_value()에서 sin(2Hz)×fadeOut 공식으로 계산
fn make_flash_anim() -> CurveSequence {
    let mut seq = CurveSequence::new();
    seq.add_curve(
        AnimationCurve::new(1.0)
            .with_easing(EasingFunction::Linear)
            .from_to(0.0, 1.0),
    );
    seq
}

impl DockTab {
    pub fn new(id: TabId, title: impl Into<String>, content: Box<dyn Widget>) -> Self {
        let title = title.into();
        Self {
            id,
            tab_type: Some(title.clone()),
            instance_id: None,
            title,
            icon: None,
            closable: true,
            content,
            role: TabRole::Panel,
            persistability: TabPersistability::Saveable,
            on_close_requested: None,
            on_tab_closed: None,
            on_tab_activated: None,
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
            label_suffix: String::new(),
            title_bar_content_right: None,
            is_locked: false,
            visual_tab_role: None,
            last_activation_time: 0.0,
            last_activation_cause: TabActivationCause::SetDirectly,
            on_tab_renamed: None,
            on_persist_visual_state: None,
            should_autosize: false,
            on_tab_relocated: None,
            content_padding: None,
            is_tab_name_hidden: false,
            is_read_only: false,
            is_main_tab: false,
            owner_major_tab_index: None,
            drag_hover_activation_time: None,
            overflow_policy: None,
            icon_color: None,
            dragged_over_dock_area: None,
            on_tab_dragged_over_dock_area: None,
            on_extend_context_menu: None,
            on_tab_drawer_opened: None,
            on_tab_drawer_closed: None,
            tree_id: None,
        }
    }

    /// 역할 지정 탭 생성
    pub fn new_with_role(id: TabId, title: impl Into<String>, content: Box<dyn Widget>, role: TabRole) -> Self {
        let title = title.into();
        Self {
            id,
            tab_type: Some(title.clone()),
            instance_id: None,
            title,
            icon: None,
            closable: matches!(role, TabRole::Nomad | TabRole::Document),
            content,
            role,
            persistability: TabPersistability::Saveable,
            on_close_requested: None,
            on_tab_closed: None,
            on_tab_activated: None,
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
            label_suffix: String::new(),
            title_bar_content_right: None,
            is_locked: false,
            visual_tab_role: None,
            last_activation_time: 0.0,
            last_activation_cause: TabActivationCause::SetDirectly,
            on_tab_renamed: None,
            on_persist_visual_state: None,
            should_autosize: false,
            on_tab_relocated: None,
            content_padding: None,
            is_tab_name_hidden: false,
            is_read_only: false,
            is_main_tab: false,
            owner_major_tab_index: None,
            drag_hover_activation_time: None,
            overflow_policy: None,
            icon_color: None,
            dragged_over_dock_area: None,
            on_tab_dragged_over_dock_area: None,
            on_extend_context_menu: None,
            on_tab_drawer_opened: None,
            on_tab_drawer_closed: None,
            tree_id: None,
        }
    }

    /// MajorTab 생성
    pub fn new_major(id: TabId, title: impl Into<String>) -> Self {
        // MajorTab은 콘텐츠 위젯 불필요 (자체 DockTree를 소유)
        Self {
            id,
            title: title.into(),
            icon: None,
            closable: false,
            content: Box::new(crate::widget::SNullWidget::new()),
            role: TabRole::Major,
            tab_type: None,
            instance_id: None,
            persistability: TabPersistability::Saveable,
            on_close_requested: None,
            on_tab_closed: None,
            on_tab_activated: None,
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
            label_suffix: String::new(),
            title_bar_content_right: None,
            is_locked: false,
            visual_tab_role: None,
            last_activation_time: 0.0,
            last_activation_cause: TabActivationCause::SetDirectly,
            on_tab_renamed: None,
            on_persist_visual_state: None,
            should_autosize: false,
            on_tab_relocated: None,
            content_padding: None,
            is_tab_name_hidden: false,
            is_read_only: false,
            is_main_tab: false,
            owner_major_tab_index: None,
            drag_hover_activation_time: None,
            overflow_policy: None,
            icon_color: None,
            dragged_over_dock_area: None,
            on_tab_dragged_over_dock_area: None,
            on_extend_context_menu: None,
            on_tab_drawer_opened: None,
            on_tab_drawer_closed: None,
            tree_id: None,
        }
    }

    /// Document 탭 생성 (타입 이름 포함)
    pub fn new_document(id: TabId, title: impl Into<String>, content: Box<dyn Widget>, tab_type: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            icon: None,
            closable: true,
            content,
            role: TabRole::Document,
            tab_type: Some(tab_type.into()),
            instance_id: None,
            persistability: TabPersistability::Saveable,
            on_close_requested: None,
            on_tab_closed: None,
            on_tab_activated: None,
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
            label_suffix: String::new(),
            title_bar_content_right: None,
            is_locked: false,
            visual_tab_role: None,
            last_activation_time: 0.0,
            last_activation_cause: TabActivationCause::SetDirectly,
            on_tab_renamed: None,
            on_persist_visual_state: None,
            should_autosize: false,
            on_tab_relocated: None,
            content_padding: None,
            is_tab_name_hidden: false,
            is_read_only: false,
            is_main_tab: false,
            owner_major_tab_index: None,
            drag_hover_activation_time: None,
            overflow_policy: None,
            icon_color: None,
            dragged_over_dock_area: None,
            on_tab_dragged_over_dock_area: None,
            on_extend_context_menu: None,
            on_tab_drawer_opened: None,
            on_tab_drawer_closed: None,
            tree_id: None,
        }
    }

    /// 아이콘 설정
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 인스턴스 ID 설정 (Document 탭 재사용 검색용)
    pub fn with_instance_id(mut self, id: impl Into<String>) -> Self {
        self.instance_id = Some(id.into());
        self
    }

    /// 영속성 설정
    pub fn with_persistability(mut self, p: TabPersistability) -> Self {
        self.persistability = p;
        self
    }

    /// 레이아웃 저장 가능 여부
    pub fn should_save_layout(&self) -> bool {
        self.persistability == TabPersistability::Saveable
    }

    /// 닫기 불가능하게 설정
    pub fn not_closable(mut self) -> Self {
        self.closable = false;
        self
    }

    /// 닫기 요청 콜백 설정
    pub fn with_close_hook<F: Fn() -> bool + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.on_close_requested = Some(Box::new(f));
        self
    }

    /// 닫힌 후 콜백 설정
    pub fn with_closed_callback<F: Fn(TabId) + Send + Sync + 'static>(mut self, f: F) -> Self {
        self.on_tab_closed = Some(Box::new(f));
        self
    }

    // ============== 애니메이션 API (UE5 SDockTab 스타일) ==============

    /// 스폰 애니메이션 재생 (탭 생성/도킹 시)
    pub fn play_spawn_anim(&mut self, current_time: f64) {
        self.spawn_anim.play(current_time);
        self.spawn_anim_playing = true;
    }

    /// 플래시 애니메이션 재생 (주의 끌기, UE SDockTab::FlashTab)
    pub fn flash_tab(&mut self, current_time: f64) {
        self.flash_anim.play(current_time);
    }

    /// 애니메이션 틱 (매 프레임 호출)
    pub fn tick_animations(&mut self, _dt: f32, current_time: f64) {
        // 스폰 애니메이션 완료 체크
        if self.spawn_anim_playing && self.spawn_anim.is_at_end(current_time) {
            self.spawn_anim_playing = false;
        }
    }

    /// 스폰 애니메이션 기반 높이 스케일 (0.0→1.0)
    /// UE5 원본: Lerp((1,0), (1,1), t) → X=1 고정, Y=0→1
    pub fn get_animated_scale(&self, current_time: f64) -> f32 {
        if self.spawn_anim_playing {
            self.spawn_anim.get_curve_value(0, current_time)
        } else {
            1.0
        }
    }

    /// 현재 플래시 밝기 (UE SDockTab::GetFlashValue)
    /// sin(2Hz) × fadeOut, 1.0초 — UE5 원본 공식 그대로
    pub fn get_flash_value(&self, current_time: f64) -> f32 {
        if self.flash_anim.is_playing() {
            let lerp = self.flash_anim.get_lerp(current_time);
            let sin_rate = 2.0 * std::f32::consts::PI * 1.0 * 2.0; // 4π (2Hz × 1초)
            let sin_term = 0.5 * (f32::sin(lerp * sin_rate) + 1.0);
            let fade_term = 1.0 - lerp;
            sin_term * fade_term
        } else {
            0.0
        }
    }

    /// 애니메이션이 활성 상태인지 (스폰 또는 플래시)
    pub fn is_animating(&self) -> bool {
        self.spawn_anim_playing || self.flash_anim.is_playing()
    }

    // ── H7: UE5 SDockTab 추가 API ──

    /// 탭 라벨 전체 (제목 + 접미사, UE5 GetTabLabel)
    pub fn get_tab_label(&self) -> String {
        if self.label_suffix.is_empty() {
            self.title.clone()
        } else {
            format!("{}{}", self.title, self.label_suffix)
        }
    }

    /// 탭 라벨 접미사 설정 (UE5 SetTabLabelSuffix — 더티 마커 등)
    pub fn set_label_suffix(&mut self, suffix: impl Into<String>) {
        self.label_suffix = suffix.into();
    }

    /// 시각적 탭 역할 (UE5 GetVisualTabRole, T4: 컨텍스트 기반 변환)
    ///
    /// visual_tab_role이 설정되면 해당 역할, 아니면 원본 role 반환.
    /// Nomad 탭이 Major 탭웰에 들어갈 때 Panel로 표시하는 데 사용.
    ///
    /// UE5 3-point check:
    /// 1. DraggedOverDockingArea → GlobalTabManager → Major
    /// 2. Parent.GetDockArea().GetTabManager() → GlobalTabManager → Major
    /// 3. Unparented (dragging, no parent) → Major
    ///
    /// `is_in_global_tab_manager`: 호출자가 컨텍스트에 따라 제공.
    /// `is_unparented`: 부모 없이 플로팅 중인지.
    pub fn get_visual_tab_role(&self) -> TabRole {
        self.visual_tab_role.unwrap_or(self.role)
    }

    /// 컨텍스트 기반 시각적 탭 역할 (UE5 GetVisualTabRole — T4 확장)
    ///
    /// Nomad → Major 자동 변환 로직을 외부 컨텍스트로 판단.
    pub fn get_visual_tab_role_contextual(&self, is_in_global_tab_manager: bool, is_unparented: bool) -> TabRole {
        if let Some(override_role) = self.visual_tab_role {
            return override_role;
        }
        // UE5: Nomad 탭 → GlobalTabManager/unparented에서 Major 스타일
        if self.role == TabRole::Nomad {
            if is_in_global_tab_manager || is_unparented {
                return TabRole::Major;
            }
        }
        self.role
    }

    /// 탭 식별자 문자열 (UE5 FTabId 정규화)
    pub fn tab_identifier(&self) -> String {
        let tab_type = self.tab_type.as_deref().unwrap_or(&self.title);
        let instance = self.instance_id.as_deref().unwrap_or("0");
        format!("{}:{}", tab_type, instance)
    }

    /// 닫기 가능 여부 (UE5 CanCloseTab — is_locked / is_main_tab 고려)
    pub fn can_close(&self) -> bool {
        self.closable && !self.is_locked && !self.is_main_tab
    }

    /// 잠금 무시 닫기 가능 여부 (UE5 CanCloseTab(bIgnoreLockedTabs=true))
    ///
    /// "모두 닫기" 또는 에디터 종료 시 잠금 탭도 닫기 허용.
    /// `closable` 자체가 false이면 (MajorTab 등) 여전히 닫기 불가.
    pub fn can_close_ignoring_lock(&self) -> bool {
        self.closable
    }

    /// 닫기 요청 오케스트레이션 (UE5 SDockTab::RequestCloseTab)
    ///
    /// 1. persist_visual_state 호출 (저장 필요 시)
    /// 2. can_close() 확인
    /// 3. on_close_requested 콜백 호출 (있으면)
    /// 반환값: true이면 닫기 허용
    pub fn request_close(&mut self) -> bool {
        // 1. 시각 상태 영속화
        if let Some(ref cb) = self.on_persist_visual_state {
            cb(self.id);
        }
        // 2. 닫기 가능 여부
        if !self.can_close() {
            return false;
        }
        // 3. 닫기 요청 콜백
        if let Some(ref cb) = self.on_close_requested {
            return cb();
        }
        true
    }

    /// 콘텐츠 패딩 설정
    pub fn with_content_padding(mut self, padding: f32) -> Self {
        self.content_padding = Some(padding);
        self
    }

    /// 탭 활성화 시각 갱신 (UE5 UpdateActivationTime)
    pub fn update_activation_time(&mut self, time: f64, cause: TabActivationCause) {
        self.last_activation_time = time;
        self.last_activation_cause = cause;
    }

    // ── Batch 6: UE5 SDockTab 추가 API ──

    /// DockArea 위로 드래그 중인 영역 설정 (UE5 SetDraggedOverDockArea)
    pub fn set_dragged_over_dock_area(&mut self, area_id: Option<u64>) {
        self.dragged_over_dock_area = area_id;
        if area_id.is_some() {
            if let Some(ref cb) = self.on_tab_dragged_over_dock_area {
                cb();
            }
        }
    }

    /// 콘텐츠 설정 + 부모 알림 (UE5 SetContent → RefreshParentContent)
    ///
    /// 콘텐츠를 교체하고 변경 플래그를 설정.
    /// 호출자가 부모 SDockingTabStack에 변경을 알려야 함.
    pub fn set_content(&mut self, content: Box<dyn Widget>) -> bool {
        self.content = content;
        true // 변경됨
    }

    /// 기본 라벨 제공 (UE5 ProvideDefaultLabel — 사용자 설정 없을 때만 적용)
    pub fn provide_default_label(&mut self, label: impl Into<String>) {
        if self.title.is_empty() {
            self.title = label.into();
        }
    }

    /// 기본 아이콘 제공 (UE5 ProvideDefaultIcon — 사용자 설정 없을 때만 적용)
    pub fn provide_default_icon(&mut self, icon: impl Into<String>) {
        if self.icon.is_none() {
            self.icon = Some(icon.into());
        }
    }

    /// 탭 허용 여부 체크 (UE5 CheckTabAllowed — TabRegistry 위임)
    pub fn check_tab_allowed(&self, registry: &TabRegistry) -> bool {
        registry.is_tab_allowed(self)
    }

    /// 트리 ID 설정 (UE5 SetTabManager — 경량 역참조)
    pub fn set_tree_id(&mut self, tree_id: u64) {
        self.tree_id = Some(tree_id);
    }

    /// 실제 flash 로직 실행 (UE5 FlashTab — 애니메이션 재생)
    pub fn flash_tab_with_time(&mut self, current_time: f64) {
        self.flash_anim.play(current_time);
    }

    // ── Batch 10 (10차): UE5 SDockTab 접근자 메서드 ──

    /// 탭 겹침 너비 (UE5 GetOverlapWidth)
    ///
    /// 탭 바에서 인접 탭과의 겹침 픽셀 폭.
    /// 현재는 0.0 (gap-based 스타일). 오버랩 스타일 전환 시 조정.
    pub fn get_overlap_width(&self) -> f32 {
        0.0
    }

    /// 커스텀 툴팁 위젯 설정 (UE5 SetTabToolTipWidget)
    ///
    /// 현재는 title_bar_content_right를 재활용. 향후 전용 tooltip widget 필드 가능.
    pub fn set_tab_tooltip_widget(&mut self, widget: Box<dyn Widget>) {
        self.title_bar_content_right = Some(widget);
    }

    /// 닫기 요청 (UE5 RequestCloseTab — request_close 별칭)
    pub fn request_close_tab(&mut self) -> bool {
        self.request_close()
    }

    /// 주의 끌기 (UE5 DrawAttention → flash_tab 위임)
    pub fn draw_attention(&mut self, current_time: f64) {
        self.flash_tab(current_time);
    }

    /// 기본 라벨 반환 (UE5 ProvideDefaultLabel — title + suffix)
    pub fn get_default_label(&self) -> String {
        self.get_tab_label()
    }

    /// 기본 아이콘 반환 (UE5 ProvideDefaultIcon — icon 또는 None)
    pub fn get_default_icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// 레이아웃 식별자 (UE5 GetLayoutIdentifier — tab_type 반환)
    pub fn get_layout_identifier(&self) -> &str {
        self.tab_type.as_deref().unwrap_or(&self.title)
    }
}

/// 탭 레지스트리
///
/// 모든 탭을 ID로 관리
pub struct TabRegistry {
    tabs: HashMap<TabId, DockTab>,
    next_id: u64,
    /// 허용 탭 타입 화이트리스트 (None이면 전부 허용)
    pub allowed_tab_types: Option<std::collections::HashSet<String>>,
}

impl Default for TabRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TabRegistry {
    pub fn new() -> Self {
        Self {
            tabs: HashMap::new(),
            next_id: 1,
            allowed_tab_types: None,
        }
    }

    /// 새 탭 ID 생성
    pub fn next_tab_id(&mut self) -> TabId {
        let id = TabId::new(self.next_id);
        self.next_id += 1;
        id
    }

    /// 탭 등록
    pub fn register(&mut self, tab: DockTab) -> TabId {
        let id = tab.id;
        self.tabs.insert(id, tab);
        id
    }

    /// 탭 등록 (ID 자동 생성)
    pub fn register_new(&mut self, title: impl Into<String>, content: Box<dyn Widget>) -> TabId {
        let id = self.next_tab_id();
        let tab = DockTab::new(id, title, content);
        self.register(tab)
    }

    /// 탭 등록 (ID 자동 생성, 아이콘 포함)
    pub fn register_new_with_icon(&mut self, title: impl Into<String>, icon: impl Into<String>, content: Box<dyn Widget>) -> TabId {
        let id = self.next_tab_id();
        let tab = DockTab::new(id, title, content).with_icon(icon);
        self.register(tab)
    }

    /// 탭 등록 (기존 ID 사용 - 재도킹용)
    pub fn register_with_id(&mut self, id: TabId, title: impl Into<String>, content: Box<dyn Widget>) {
        let tab = DockTab::new(id, title, content);
        self.tabs.insert(id, tab);
        // next_id 업데이트 (충돌 방지)
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }
    }

    /// 탭 조회
    pub fn get(&self, id: TabId) -> Option<&DockTab> {
        self.tabs.get(&id)
    }

    /// 탭 조회 (mutable)
    pub fn get_mut(&mut self, id: TabId) -> Option<&mut DockTab> {
        self.tabs.get_mut(&id)
    }

    /// 탭 제거
    pub fn remove(&mut self, id: TabId) -> Option<DockTab> {
        self.tabs.remove(&id)
    }

    /// 탭 존재 여부
    pub fn contains(&self, id: TabId) -> bool {
        self.tabs.contains_key(&id)
    }

    /// 모든 탭 ID
    pub fn tab_ids(&self) -> impl Iterator<Item = TabId> + '_ {
        self.tabs.keys().copied()
    }

    /// 탭 개수
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    /// 비었는지
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// 탭 제목 조회
    pub fn get_title(&self, id: TabId) -> Option<String> {
        self.tabs.get(&id).map(|t| t.title.clone())
    }

    /// 제목으로 탭 ID 찾기
    pub fn find_by_title(&self, title: &str) -> Option<TabId> {
        self.tabs.iter()
            .find(|(_, tab)| tab.title == title)
            .map(|(id, _)| *id)
    }

    /// 탭 콘텐츠 조회
    pub fn get_content(&self, id: TabId) -> Option<&dyn Widget> {
        self.tabs.get(&id).map(|t| t.content.as_ref())
    }

    /// 탭 콘텐츠 조회 (mutable)
    pub fn get_content_mut(&mut self, id: TabId) -> Option<&mut dyn Widget> {
        self.tabs.get_mut(&id).map(|t| t.content.as_mut())
    }

    /// 모든 탭 제거 (레이아웃 복원용)
    pub fn clear(&mut self) {
        self.tabs.clear();
        // next_id는 리셋하지 않음 (ID 충돌 방지)
    }

    /// 모든 탭 제목 반환 (레이아웃 저장용)
    pub fn all_titles(&self) -> Vec<String> {
        self.tabs.values().map(|t| t.title.clone()).collect()
    }

    /// 모든 탭의 읽기 전용 상태 일괄 설정
    pub fn set_read_only_all(&mut self, val: bool) {
        for tab in self.tabs.values_mut() {
            tab.is_read_only = val;
        }
    }

    /// 탭 허용 여부 체크 (None이면 전부 허용, Some이면 whitelist)
    pub fn is_tab_allowed(&self, tab: &DockTab) -> bool {
        match &self.allowed_tab_types {
            None => true,
            Some(allowed) => {
                tab.tab_type.as_ref().map_or(true, |tt| allowed.contains(tt))
            }
        }
    }

    /// 식별자로 탭 검색 (UE5 FTabId 정규화)
    pub fn find_by_identifier(&self, ident: &str) -> Option<TabId> {
        self.tabs.iter()
            .find(|(_, tab)| tab.tab_identifier() == ident)
            .map(|(id, _)| *id)
    }
}

/// 탭 빌더
#[deprecated(note = "Use DockTab::new().with_*() builder pattern instead")]
pub struct TabBuilder {
    title: String,
    icon: Option<String>,
    closable: bool,
}

impl TabBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            icon: None,
            closable: true,
        }
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn not_closable(mut self) -> Self {
        self.closable = false;
        self
    }

    pub fn build(self, id: TabId, content: Box<dyn Widget>) -> DockTab {
        DockTab {
            id,
            title: self.title,
            icon: self.icon,
            closable: self.closable,
            content,
            role: TabRole::Panel,
            tab_type: None,
            instance_id: None,
            persistability: TabPersistability::Saveable,
            on_close_requested: None,
            on_tab_closed: None,
            on_tab_activated: None,
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
            label_suffix: String::new(),
            title_bar_content_right: None,
            is_locked: false,
            visual_tab_role: None,
            last_activation_time: 0.0,
            last_activation_cause: TabActivationCause::SetDirectly,
            on_tab_renamed: None,
            on_persist_visual_state: None,
            should_autosize: false,
            on_tab_relocated: None,
            content_padding: None,
            is_tab_name_hidden: false,
            is_read_only: false,
            is_main_tab: false,
            owner_major_tab_index: None,
            drag_hover_activation_time: None,
            overflow_policy: None,
            icon_color: None,
            dragged_over_dock_area: None,
            on_tab_dragged_over_dock_area: None,
            on_extend_context_menu: None,
            on_tab_drawer_opened: None,
            on_tab_drawer_closed: None,
            tree_id: None,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::NodeId;

    #[test]
    fn test_tab_persistability_default() {
        let tab = DockTab::new(TabId::new(1), "Test", Box::new(crate::widget::SNullWidget::new()));
        assert_eq!(tab.persistability, TabPersistability::Saveable);
        assert!(tab.should_save_layout());
    }

    #[test]
    fn test_tab_persistability_not_saveable() {
        let tab = DockTab::new(TabId::new(1), "Temp", Box::new(crate::widget::SNullWidget::new()))
            .with_persistability(TabPersistability::NotSaveable);
        assert!(!tab.should_save_layout());
    }

    #[test]
    fn test_active_tab_changed_event() {
        use super::super::{ActiveTabChangedEvent, EventDelegate};

        let mut delegate = EventDelegate::<ActiveTabChangedEvent>::new();
        let received = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        delegate.add(move |evt| {
            received_clone.lock().unwrap().push(evt);
        });

        delegate.broadcast(ActiveTabChangedEvent {
            old_tab: None,
            new_tab: TabId::new(5),
            stack_id: NodeId::new(10),
        });

        let events = received.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].new_tab, TabId::new(5));
        assert!(events[0].old_tab.is_none());
    }

    #[test]
    fn test_layout_extender_registry() {
        use super::super::{LayoutExtenderRegistry, LayoutExtenderArea, LayoutExtender};

        struct TestExtender;
        impl LayoutExtender for TestExtender {
            fn name(&self) -> &str { "TestExtender" }
            fn extend_layout(&self, area: LayoutExtenderArea) -> Option<Box<dyn crate::widget::Widget>> {
                match area {
                    LayoutExtenderArea::Left => Some(Box::new(crate::widget::SNullWidget::new())),
                    _ => None,
                }
            }
        }

        let mut registry = LayoutExtenderRegistry::new();
        assert!(registry.is_empty());

        registry.register(Box::new(TestExtender));
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.names(), vec!["TestExtender"]);

        let left_widgets = registry.collect_widgets(LayoutExtenderArea::Left);
        assert_eq!(left_widgets.len(), 1);

        let right_widgets = registry.collect_widgets(LayoutExtenderArea::Right);
        assert!(right_widgets.is_empty());
    }

    #[test]
    fn test_tab_registry_filter_saveable() {
        let mut registry = TabRegistry::new();

        let id1 = registry.next_tab_id();
        let tab1 = DockTab::new(id1, "Saveable", Box::new(crate::widget::SNullWidget::new()));
        registry.register(tab1);

        let id2 = registry.next_tab_id();
        let tab2 = DockTab::new(id2, "NotSaveable", Box::new(crate::widget::SNullWidget::new()))
            .with_persistability(TabPersistability::NotSaveable);
        registry.register(tab2);

        // 저장 가능한 탭만 필터링
        let saveable: Vec<TabId> = registry.tab_ids()
            .filter(|id| registry.get(*id).map_or(false, |t| t.should_save_layout()))
            .collect();

        assert_eq!(saveable.len(), 1);
        assert_eq!(saveable[0], id1);
    }
}
