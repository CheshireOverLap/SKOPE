//! 탭 스포너 시스템 (언리얼 FTabSpawnerEntry / FGlobalTabmanager 대응)
//!
//! 탭을 문자열 ID(타입명)로 등록하고, invoke_tab()으로 생성/활성화.
//! Window 메뉴는 등록된 스포너에서 자동 생성.

use std::collections::HashMap;
use super::TabRole;
use crate::widget::Widget;

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

    /// 탭 생성 시도 (UE의 TryInvokeTab 패턴)
    ///
    /// 스포너가 등록되어 있으면 콘텐츠를 생성하고 메타데이터를 반환합니다.
    /// 호출자가 DockTree에 탭을 추가하는 책임을 집니다.
    pub fn try_invoke_tab(&self, tab_type_name: &str) -> Option<TabSpawnResult> {
        let entry = self.entries.get(tab_type_name)?;
        Some(TabSpawnResult {
            tab_type_name: entry.tab_type_name.clone(),
            display_name: entry.display_name.clone(),
            icon: entry.icon.clone(),
            role: entry.role,
            content: (entry.factory)(),
            singleton: entry.singleton,
        })
    }
}

/// TryInvokeTab 결과
pub struct TabSpawnResult {
    pub tab_type_name: String,
    pub display_name: String,
    pub icon: Option<String>,
    pub role: TabRole,
    pub content: Box<dyn Widget>,
    pub singleton: bool,
}
