//! 도킹 탭 관리

use super::{TabId, TabRole, TabPersistability};
use crate::core::Color;
use crate::framework::{CurveSequence, AnimationCurve, EasingFunction};
use crate::widget::Widget;
use std::collections::HashMap;

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
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
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
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
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
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
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
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
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
}

/// 탭 레지스트리
///
/// 모든 탭을 ID로 관리
pub struct TabRegistry {
    tabs: HashMap<TabId, DockTab>,
    next_id: u64,
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
}

/// 탭 빌더
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
            color_tint: None,
            spawn_anim: make_spawn_anim(),
            spawn_anim_playing: false,
            flash_anim: make_flash_anim(),
            tab_well_content_left: None,
            tab_well_content_right: None,
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
