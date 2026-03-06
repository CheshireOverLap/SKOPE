//! 탭 스포너 시스템 (언리얼 FTabSpawnerEntry / FGlobalTabmanager 대응)
//!
//! 탭을 문자열 ID(타입명)로 등록하고, invoke_tab()으로 생성/활성화.
//! Window 메뉴는 등록된 스포너에서 자동 생성.

use std::collections::HashMap;
use super::{TabRole, ReadOnlyBehavior};
use crate::widget::Widget;

/// 메뉴 표시 모드 (UE5 ETabSpawnerMenuType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuType {
    /// 메뉴에 표시 (기본)
    Enabled,
    /// 메뉴에 표시하되 비활성
    Disabled,
    /// 메뉴에 숨김
    Hidden,
}

impl Default for MenuType {
    fn default() -> Self {
        Self::Enabled
    }
}

/// 탭 스포너 항목 (언리얼 FTabSpawnerEntry 대응)
pub struct TabSpawnerEntry {
    /// 고유 식별자 (예: "Viewport", "Hierarchy", "OutputLog")
    pub tab_type_name: String,
    /// 메뉴 표시명
    pub display_name: String,
    /// 아이콘 (옵션)
    pub icon: Option<String>,
    /// 탭 역할
    pub role: TabRole,
    /// 메뉴 카테고리 (예: "General", "Debug")
    pub menu_group: String,
    /// 팩토리 콜백 — 새 탭 콘텐츠 위젯 생성
    pub factory: Box<dyn Fn() -> Box<dyn Widget> + Send + Sync>,
    /// 싱글턴 여부 (true면 동일 타입 탭이 하나만 존재)
    pub singleton: bool,
    /// 생성 조건 가드 (UE5 CanSpawnTab)
    pub can_spawn: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    /// 재사용 탭 검색 (UE5 OnFindTabToReuse)
    pub on_find_tab_to_reuse: Option<Box<dyn Fn() -> Option<super::TabId> + Send + Sync>>,
    /// 싱글톤 추적 - 생성된 탭 ID (약참조 대용)
    pub spawned_tab_id: Option<super::TabId>,
    /// 툴팁 텍스트 (UE5 SetTooltipText)
    pub tooltip: Option<String>,
    /// 메뉴 표시 모드 (UE5 SetMenuType)
    pub menu_type: MenuType,
    /// 메뉴 자동 생성 여부 (UE5 SetAutoGenerateMenuEntry)
    pub auto_generate_menu: bool,
    /// 사이드바 허용 여부 (UE5 SetCanSidebarTab)
    pub can_sidebar_tab: bool,
    /// 탭 잠금 상태 (UE5 IsTabLocked — 잠금 시 닫기/이동 제한)
    pub is_locked: bool,
    /// 탭 이름 숨김 여부 (UE5 IsTabNameHidden — true이면 아이콘만 표시)
    pub is_tab_name_hidden: bool,
    /// 읽기전용 모드 동작 (UE5 SetReadOnlyBehavior)
    pub read_only_behavior: ReadOnlyBehavior,
}

impl TabSpawnerEntry {
    /// 빌더 패턴 시작
    pub fn new(
        tab_type_name: impl Into<String>,
        factory: impl Fn() -> Box<dyn Widget> + Send + Sync + 'static,
    ) -> Self {
        let name = tab_type_name.into();
        Self {
            display_name: name.clone(),
            tab_type_name: name,
            icon: None,
            role: TabRole::Panel,
            menu_group: "General".to_string(),
            factory: Box::new(factory),
            singleton: true,
            can_spawn: None,
            on_find_tab_to_reuse: None,
            spawned_tab_id: None,
            tooltip: None,
            menu_type: MenuType::Enabled,
            auto_generate_menu: true,
            can_sidebar_tab: true,
            is_locked: false,
            is_tab_name_hidden: false,
            read_only_behavior: ReadOnlyBehavior::default(),
        }
    }

    pub fn display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = name.into();
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn role(mut self, role: TabRole) -> Self {
        self.role = role;
        // Document 탭은 기본적으로 싱글턴 아님
        if role == TabRole::Document {
            self.singleton = false;
        }
        self
    }

    pub fn menu_group(mut self, group: impl Into<String>) -> Self {
        self.menu_group = group.into();
        self
    }

    pub fn singleton(mut self, singleton: bool) -> Self {
        self.singleton = singleton;
        self
    }

    /// 툴팁 텍스트 설정 (UE5 SetTooltipText)
    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// 메뉴 표시 모드 설정 (UE5 SetMenuType)
    pub fn menu_type(mut self, menu_type: MenuType) -> Self {
        self.menu_type = menu_type;
        self
    }

    /// 메뉴 자동 생성 설정 (UE5 SetAutoGenerateMenuEntry)
    pub fn auto_generate_menu(mut self, auto: bool) -> Self {
        self.auto_generate_menu = auto;
        self
    }

    /// 사이드바 허용 설정 (UE5 SetCanSidebarTab)
    pub fn can_sidebar_tab(mut self, allowed: bool) -> Self {
        self.can_sidebar_tab = allowed;
        self
    }

    /// 읽기전용 동작 설정 (UE5 SetReadOnlyBehavior)
    pub fn read_only_behavior(mut self, behavior: ReadOnlyBehavior) -> Self {
        self.read_only_behavior = behavior;
        self
    }

    /// 메뉴에서 숨겨야 하는지 (UE5 IsHidden — menu_type 기반)
    pub fn is_hidden(&self) -> bool {
        self.menu_type == MenuType::Hidden
    }
}

/// 탭 스포너 레지스트리 (언리얼 FGlobalTabmanager의 스포너 관리 부분)
pub struct TabSpawnerRegistry {
    entries: HashMap<String, TabSpawnerEntry>,
    /// 등록 순서 보존 (메뉴 표시용)
    order: Vec<String>,
}

impl Default for TabSpawnerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TabSpawnerRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// 스포너 등록
    pub fn register(&mut self, entry: TabSpawnerEntry) {
        let name = entry.tab_type_name.clone();
        if !self.entries.contains_key(&name) {
            self.order.push(name.clone());
        }
        self.entries.insert(name, entry);
    }

    /// 스포너 조회
    pub fn get(&self, tab_type_name: &str) -> Option<&TabSpawnerEntry> {
        self.entries.get(tab_type_name)
    }

    /// 팩토리 호출하여 새 탭 콘텐츠 생성
    pub fn create_content(&self, tab_type_name: &str) -> Option<Box<dyn Widget>> {
        self.entries.get(tab_type_name).map(|e| (e.factory)())
    }

    /// 등록 순서대로 모든 항목 반환
    pub fn entries_ordered(&self) -> Vec<&TabSpawnerEntry> {
        self.order.iter()
            .filter_map(|name| self.entries.get(name))
            .collect()
    }

    /// 메뉴 그룹별로 묶어서 반환 (Window 메뉴 빌드용)
    pub fn entries_by_group(&self) -> Vec<(String, Vec<&TabSpawnerEntry>)> {
        let mut groups: Vec<(String, Vec<&TabSpawnerEntry>)> = Vec::new();
        let mut group_map: HashMap<String, usize> = HashMap::new();

        for name in &self.order {
            if let Some(entry) = self.entries.get(name) {
                if let Some(&idx) = group_map.get(&entry.menu_group) {
                    groups[idx].1.push(entry);
                } else {
                    let idx = groups.len();
                    group_map.insert(entry.menu_group.clone(), idx);
                    groups.push((entry.menu_group.clone(), vec![entry]));
                }
            }
        }

        groups
    }

    /// 등록된 스포너 수
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 비었는지
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 스포너 존재 여부
    pub fn contains(&self, tab_type_name: &str) -> bool {
        self.entries.contains_key(tab_type_name)
    }

    /// 스포너 제거 (UE5 UnregisterTabSpawner)
    pub fn unregister(&mut self, tab_type_name: &str) -> bool {
        if self.entries.remove(tab_type_name).is_some() {
            self.order.retain(|n| n != tab_type_name);
            true
        } else {
            false
        }
    }

    /// 전체 스포너 제거 (UE5 UnregisterAllTabSpawners)
    pub fn unregister_all(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    /// 탭 생성 시도 (UE의 TryInvokeTab 패턴)
    ///
    /// 스포너가 등록되어 있으면 콘텐츠를 생성하고 메타데이터를 반환합니다.
    /// 호출자가 DockTree에 탭을 추가하는 책임을 집니다.
    pub fn try_invoke_tab(&mut self, tab_type_name: &str) -> Option<TryInvokeResult> {
        let entry = self.entries.get(tab_type_name)?;
        // can_spawn 가드 체크
        if let Some(ref can_spawn) = entry.can_spawn {
            if !can_spawn() {
                return None;
            }
        }
        // 싱글턴 + 이미 생성된 탭이 있으면 재사용
        if entry.singleton {
            if let Some(existing_id) = entry.spawned_tab_id {
                return Some(TryInvokeResult::Reuse(existing_id));
            }
        }
        // on_find_tab_to_reuse 체크
        if let Some(ref find_reuse) = entry.on_find_tab_to_reuse {
            if let Some(existing_id) = find_reuse() {
                // 재사용 가능한 탭 ID를 호출자에게 전달
                return Some(TryInvokeResult::Reuse(existing_id));
            }
        }
        Some(TryInvokeResult::Spawned(TabSpawnResult {
            tab_type_name: entry.tab_type_name.clone(),
            display_name: entry.display_name.clone(),
            icon: entry.icon.clone(),
            role: entry.role,
            content: (entry.factory)(),
            singleton: entry.singleton,
        }))
    }

    /// 스포너에 생성된 탭 ID를 기록 (싱글턴 추적용)
    ///
    /// 호출자가 탭을 실제로 DockTree에 추가한 뒤 호출해야 합니다.
    pub fn set_spawned_tab_id(&mut self, tab_type_name: &str, tab_id: super::TabId) {
        if let Some(entry) = self.entries.get_mut(tab_type_name) {
            entry.spawned_tab_id = Some(tab_id);
        }
    }

    /// 스포너의 생성된 탭 ID를 지움 (탭이 닫힐 때 호출)
    pub fn clear_spawned_tab_id(&mut self, tab_type_name: &str) {
        if let Some(entry) = self.entries.get_mut(tab_type_name) {
            entry.spawned_tab_id = None;
        }
    }
}

/// TryInvokeTab 결과 (생성 vs 재사용)
pub enum TryInvokeResult {
    /// 새 탭 콘텐츠가 생성됨
    Spawned(TabSpawnResult),
    /// 기존 탭을 재사용 (호출자가 해당 탭을 활성화해야 함)
    Reuse(super::TabId),
}

/// 새로 생성된 탭 메타데이터 + 콘텐츠
pub struct TabSpawnResult {
    pub tab_type_name: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub role: TabRole,
    pub content: Box<dyn Widget>,
    pub singleton: bool,
}
