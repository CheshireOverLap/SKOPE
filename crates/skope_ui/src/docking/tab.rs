//! 도킹 탭 관리

use super::TabId;
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
}

impl DockTab {
    pub fn new(id: TabId, title: impl Into<String>, content: Box<dyn Widget>) -> Self {
        Self {
            id,
            title: title.into(),
            icon: None,
            closable: true,
            content,
        }
    }

    /// 아이콘 설정
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 닫기 불가능하게 설정
    pub fn not_closable(mut self) -> Self {
        self.closable = false;
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
    pub fn get_title(&self, id: TabId) -> Option<&str> {
        self.tabs.get(&id).map(|t| t.title.as_str())
    }

    /// 탭 콘텐츠 조회
    pub fn get_content(&self, id: TabId) -> Option<&dyn Widget> {
        self.tabs.get(&id).map(|t| t.content.as_ref())
    }

    /// 탭 콘텐츠 조회 (mutable)
    pub fn get_content_mut(&mut self, id: TabId) -> Option<&mut dyn Widget> {
        self.tabs.get_mut(&id).map(|t| t.content.as_mut())
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
        }
    }
}
