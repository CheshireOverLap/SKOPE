//! 도킹 탭 관리

use super::{TabId, TabRole};
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
    /// 닫기 요청 콜백 (true 반환 시 닫기 허용)
    pub on_close_requested: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    /// 탭 닫힌 후 콜백 (post-close notification)
    pub on_tab_closed: Option<Box<dyn Fn(TabId) + Send + Sync>>,
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
            on_close_requested: None,
            on_tab_closed: None,
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
            on_close_requested: None,
            on_tab_closed: None,
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
            on_close_requested: None,
            on_tab_closed: None,
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
            on_close_requested: None,
            on_tab_closed: None,
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
            on_close_requested: None,
            on_tab_closed: None,
        }
    }
}
