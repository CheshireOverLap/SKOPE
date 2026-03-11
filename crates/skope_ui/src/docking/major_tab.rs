//! MajorTab — 독립 도킹 레이아웃을 소유하는 최상위 탭
//!
//! 언리얼 FTabManager에 대응. 각 MajorTab이 자체 DockTree + TabRegistry를 소유.

use super::{DockTree, TabRegistry, TabId, DockPosition, TabSpawnerRegistry, SidebarPanel, SidebarSide, SidebarTabEntry, SDockingArea};
use crate::widget::Widget;


/// MajorTab (언리얼 FTabManager 대응)
///
/// 각 MajorTab은 독립적인 도킹 레이아웃을 소유한다.
/// 예: "Level Editor", "Material Editor", "Blueprint Editor"
pub struct MajorTab {
    /// 표시 제목
    pub title: String,
    /// 아이콘 (옵션)
    pub icon: Option<String>,
    /// 닫기 가능 여부
    pub closable: bool,
    /// 내부 도킹 트리 (데이터/직렬화용)
    pub tree: DockTree,
    /// 내부 탭 레지스트리 (build_widget_tree 전 소유, 빌드 후 비워짐)
    pub tabs: TabRegistry,
    /// 라이브 위젯 트리 (SDockingSplitter/SDockingTabStack 소유)
    pub dock_area: Option<SDockingArea>,
    /// 로컬 탭 스포너 (이 MajorTab 전용)
    pub spawners: TabSpawnerRegistry,
    /// 왼쪽 사이드바
    pub left_sidebar: SidebarPanel,
    /// 오른쪽 사이드바
    pub right_sidebar: SidebarPanel,
    /// 표시 여부 (UE5 FTabManager::ShowAllWindows/HideWindows)
    pub visible: bool,
}

impl MajorTab {
    pub fn new(title: impl Into<String>) -> Self {
        let title = title.into();
        Self {
            tree: DockTree::new(&title),
            tabs: TabRegistry::new(),
            dock_area: None,
            spawners: TabSpawnerRegistry::new(),
            left_sidebar: SidebarPanel::new(SidebarSide::Left),
            right_sidebar: SidebarPanel::new(SidebarSide::Right),
            title,
            icon: None,
            closable: false,
            visible: true,
        }
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn with_closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }

    /// 내부에 PanelTab 추가
    pub fn add_tab(&mut self, title: impl Into<String>, content: Box<dyn Widget>) -> TabId {
        let tab_id = self.tabs.register_new(title, content);
        self.tree.add_tab(tab_id);
        tab_id
    }

    /// 내부에 PanelTab 추가 (아이콘 포함)
    pub fn add_tab_with_icon(&mut self, title: impl Into<String>, icon: impl Into<String>, content: Box<dyn Widget>) -> TabId {
        let tab_id = self.tabs.register_new_with_icon(title, icon, content);
        self.tree.add_tab(tab_id);
        tab_id
    }

    /// 역할 지정하여 탭 추가 (NomadTab, DocumentTab 등)
    pub fn add_tab_with_role(
        &mut self,
        title: impl Into<String>,
        content: Box<dyn Widget>,
        role: super::TabRole,
    ) -> TabId {
        let id = self.tabs.next_tab_id();
        let tab = super::DockTab::new_with_role(id, title, content, role);
        self.tabs.register(tab);
        self.tree.add_tab(id);
        id
    }

    /// Document 탭 추가 (타입 이름 포함)
    pub fn add_document_tab(
        &mut self,
        title: impl Into<String>,
        content: Box<dyn Widget>,
        tab_type: impl Into<String>,
    ) -> TabId {
        let id = self.tabs.next_tab_id();
        let tab = super::DockTab::new_document(id, title, content, tab_type);
        self.tabs.register(tab);
        self.tree.add_tab(id);
        id
    }

    /// 내부 도킹 (tab_title의 탭을 target_title이 있는 스택의 position에 도킹)
    pub fn dock_tab_by_title(&mut self, tab_title: &str, target_title: &str, position: DockPosition) -> bool {
        let tab_id = match self.tabs.find_by_title(tab_title) {
            Some(id) => id,
            None => return false,
        };
        let target_tab_id = match self.tabs.find_by_title(target_title) {
            Some(id) => id,
            None => return false,
        };
        let target_stack = match self.tree.find_tab_stack_containing(target_tab_id) {
            Some(id) => id,
            None => return false,
        };
        self.tree.dock_tab(tab_id, target_stack, position)
    }

    /// 탭 타입명으로 탭 호출 (있으면 활성화, 없으면 스포너로 생성)
    ///
    /// M1: UE5 TryInvokeTab 5단계 중 로컬 MajorTab 범위:
    /// Step 1: live 탭 검색 + 활성화
    /// Step 1.5: history_tabs에서 reopen_tab (향후 tab_type 정보 포함 시 활성화)
    /// Step 2: 로컬 스포너에서 새 탭 생성
    ///
    /// 반환: 활성화/생성된 탭 ID, 실패 시 None
    pub fn invoke_tab(&mut self, tab_type_name: &str) -> Option<TabId> {
        // Step 1: 이미 열린 탭 중 같은 tab_type이 있으면 활성화
        let existing = self.tabs.tab_ids()
            .find(|&id| {
                self.tabs.get(id)
                    .and_then(|t| t.tab_type.as_deref())
                    .map(|tt| tt == tab_type_name)
                    .unwrap_or(false)
            });

        if let Some(tab_id) = existing {
            // 해당 탭이 속한 스택에서 활성화
            if let Some(stack_id) = self.tree.find_tab_stack_containing(tab_id) {
                if let Some(stack) = self.tree.find_tab_stack_mut(stack_id) {
                    if let Some(idx) = stack.tabs.iter().position(|&id| id == tab_id) {
                        stack.active_tab = idx;
                    }
                }
            }
            log::info!("[TabSpawner] Activated existing tab '{}' (id={})", tab_type_name, tab_id.0);
            return Some(tab_id);
        }

        // Step 1.5: history_tabs에서 reopen 시도 (UE5 OpenPersistentTab)
        // history_tabs는 TabId만 보존하므로 tab_type 매칭 불가.
        // 향후 history에 tab_type 메타데이터 추가 시 아래 패턴으로 활성화:
        //   for &hist_id in self.tree.history_tabs() {
        //       if tab_type matches → self.tree.reopen_tab(hist_id); return Some(hist_id);
        //   }

        // Step 2: 스포너에서 팩토리로 새 탭 생성
        let entry = self.spawners.get(tab_type_name)?;
        let content = (entry.factory)();
        let role = entry.role;
        let icon = entry.icon.clone();
        let singleton = entry.singleton;

        let id = self.tabs.next_tab_id();
        let mut tab = super::DockTab::new_with_role(id, tab_type_name, content, role);
        tab.tab_type = Some(tab_type_name.to_string());
        tab.icon = icon;
        self.tabs.register(tab);
        self.tree.add_tab(id);

        log::info!("[TabSpawner] Created new tab '{}' (id={}, singleton={})", tab_type_name, id.0, singleton);
        Some(id)
    }

    /// 탭을 사이드바로 이동 (DockTree에서 제거 → 사이드바에 추가)
    pub fn move_tab_to_sidebar(&mut self, tab_id: TabId, side: SidebarSide) -> bool {
        // 탭 정보 조회
        let (display_name, tab_type_name, icon) = match self.tabs.get(tab_id) {
            Some(tab) => (
                tab.title.clone(),
                tab.tab_type.clone().unwrap_or_else(|| tab.title.clone()),
                tab.icon.clone(),
            ),
            None => return false,
        };

        // DockTree에서 탭 제거 (콘텐츠는 TabRegistry에 유지)
        self.tree.remove_tab(tab_id);

        // 사이드바에 추가
        let entry = SidebarTabEntry {
            tab_id,
            tab_type_name,
            display_name,
            icon,
            pinned: false,
            is_docked: false,
        };
        match side {
            SidebarSide::Left => self.left_sidebar.add_tab(entry),
            SidebarSide::Right => self.right_sidebar.add_tab(entry),
        }

        log::info!("[Sidebar] Moved tab {} to {:?} sidebar", tab_id.0, side);
        true
    }

    /// 사이드바에서 탭을 복원 (사이드바에서 제거 → DockTree에 추가)
    /// Called from sidebar context menu "Restore to dock" or DragResult::RestoreFromSidebar handler
    #[allow(dead_code)]
    pub fn restore_tab_from_sidebar(&mut self, tab_id: TabId) -> bool {
        // 어느 사이드바에 있는지 찾기
        let removed = self.left_sidebar.remove_tab(tab_id)
            .or_else(|| self.right_sidebar.remove_tab(tab_id));

        if removed.is_some() {
            // DockTree에 다시 추가
            self.tree.add_tab(tab_id);
            log::info!("[Sidebar] Restored tab {} to dock tree", tab_id.0);
            true
        } else {
            false
        }
    }

    /// 레이아웃 업데이트
    pub fn update_layout(&mut self, rect: super::NodeRect, tab_style: &super::TabStackStyle) {
        self.tree.compute_layout(rect, tab_style);
    }

    /// 위젯 트리 빌드/재빌드
    ///
    /// DockTree 데이터에서 라이브 위젯 트리(SDockingArea)를 생성.
    /// 기존 위젯 트리가 있으면 DockTab을 TabRegistry로 복원한 뒤 재구축.
    pub fn rebuild_widget_tree(&mut self, tab_style: &super::TabStackStyle) {
        // 기존 위젯 트리에서 DockTab 소유권을 TabRegistry로 복원
        if let Some(ref mut area) = self.dock_area {
            DockTree::collect_tabs_from_widget_tree(area, &mut self.tabs);
        }

        // DockTree 데이터 → 새 위젯 트리 빌드 (TabRegistry에서 DockTab 추출)
        self.dock_area = Some(self.tree.build_widget_tree(&mut self.tabs, tab_style));
    }
}
