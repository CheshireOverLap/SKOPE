//! WorkspaceItem — 탭 타입 계층적 분류/브라우징 (UE5 FWorkspaceItem 대응)
//!
//! 탭 스포너를 트리 구조로 분류하여 메뉴/브라우저에서 계층적으로 표시합니다.
//! 예: General > Viewport, General > Hierarchy, Debug > OutputLog

use std::collections::HashMap;

/// 워크스페이스 아이템 — 탭 타입 트리의 노드
///
/// UE5의 `FWorkspaceItem`에 해당합니다.
/// 카테고리(폴더) 또는 탭 스포너(잎) 역할을 합니다.
#[derive(Debug, Clone)]
pub struct WorkspaceItem {
    /// 고유 이름 (경로 세그먼트)
    pub name: String,
    /// 표시 이름
    pub display_name: String,
    /// 아이콘 (옵션)
    pub icon: Option<String>,
    /// 자식 아이템 (카테고리인 경우)
    pub children: Vec<WorkspaceItem>,
    /// 연결된 탭 스포너 타입명 (잎인 경우)
    pub spawner_type: Option<String>,
    /// 정렬 순서 (낮을수록 먼저)
    pub sort_order: i32,
    /// 툴팁
    pub tooltip: Option<String>,
}

impl WorkspaceItem {
    /// 카테고리(폴더) 생성
    pub fn category(name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            icon: None,
            children: Vec::new(),
            spawner_type: None,
            sort_order: 0,
            tooltip: None,
        }
    }

    /// 탭 스포너 잎 노드 생성
    pub fn tab(name: impl Into<String>, display_name: impl Into<String>, spawner_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
            icon: None,
            children: Vec::new(),
            spawner_type: Some(spawner_type.into()),
            sort_order: 0,
            tooltip: None,
        }
    }

    /// 아이콘 설정
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 정렬 순서 설정
    pub fn with_sort_order(mut self, order: i32) -> Self {
        self.sort_order = order;
        self
    }

    /// 툴팁 설정
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// 자식 추가
    pub fn add_child(&mut self, child: WorkspaceItem) {
        self.children.push(child);
    }

    /// 빌더 스타일 자식 추가
    pub fn child(mut self, child: WorkspaceItem) -> Self {
        self.children.push(child);
        self
    }

    /// 카테고리인지 (자식 또는 spawner 없이 카테고리 역할)
    pub fn is_category(&self) -> bool {
        self.spawner_type.is_none()
    }

    /// 잎(탭 스포너)인지
    pub fn is_leaf(&self) -> bool {
        self.spawner_type.is_some()
    }

    /// 경로로 자식 찾기 (예: "Debug/OutputLog")
    pub fn find_by_path(&self, path: &str) -> Option<&WorkspaceItem> {
        let mut segments = path.splitn(2, '/');
        let first = segments.next()?;
        let rest = segments.next();

        let child = self.children.iter().find(|c| c.name == first)?;
        match rest {
            Some(remaining) => child.find_by_path(remaining),
            None => Some(child),
        }
    }

    /// 경로로 자식 찾기 (mutable)
    pub fn find_by_path_mut(&mut self, path: &str) -> Option<&mut WorkspaceItem> {
        let mut segments = path.splitn(2, '/');
        let first = segments.next()?;
        let rest = segments.next();

        let child = self.children.iter_mut().find(|c| c.name == first)?;
        match rest {
            Some(remaining) => child.find_by_path_mut(remaining),
            None => Some(child),
        }
    }

    /// 정렬 순서로 자식 정렬
    pub fn sort_children(&mut self) {
        self.children.sort_by_key(|c| c.sort_order);
        for child in &mut self.children {
            child.sort_children();
        }
    }

    /// 모든 잎(탭 스포너) 수집
    pub fn collect_leaves(&self) -> Vec<&WorkspaceItem> {
        let mut result = Vec::new();
        self.collect_leaves_inner(&mut result);
        result
    }

    fn collect_leaves_inner<'a>(&'a self, out: &mut Vec<&'a WorkspaceItem>) {
        if self.is_leaf() {
            out.push(self);
        }
        for child in &self.children {
            child.collect_leaves_inner(out);
        }
    }

    /// 깊이 (루트에서의 거리)
    pub fn max_depth(&self) -> usize {
        if self.children.is_empty() {
            0
        } else {
            1 + self.children.iter().map(|c| c.max_depth()).max().unwrap_or(0)
        }
    }

    /// 총 아이템 수 (자기 포함)
    pub fn total_count(&self) -> usize {
        1 + self.children.iter().map(|c| c.total_count()).sum::<usize>()
    }
}

/// 워크스페이스 메뉴 빌더
///
/// `TabSpawnerRegistry`에서 자동으로 `WorkspaceItem` 트리를 빌드합니다.
pub struct WorkspaceMenuBuilder;

impl WorkspaceMenuBuilder {
    /// TabSpawnerRegistry에서 WorkspaceItem 루트 트리 빌드
    ///
    /// 각 TabSpawnerEntry의 menu_group을 카테고리로 사용합니다.
    pub fn build_from_registry(registry: &super::TabSpawnerRegistry) -> WorkspaceItem {
        let mut root = WorkspaceItem::category("root", "Window");
        let groups = registry.entries_by_group();

        for (group_name, entries) in groups {
            let mut category = WorkspaceItem::category(group_name.clone(), group_name);

            for entry in entries {
                let mut item = WorkspaceItem::tab(
                    entry.tab_type_name.clone(),
                    entry.display_name.clone(),
                    entry.tab_type_name.clone(),
                );
                if let Some(ref icon) = entry.icon {
                    item = item.with_icon(icon.clone());
                }
                category.add_child(item);
            }

            root.add_child(category);
        }

        root.sort_children();
        root
    }
}

// ============================================================================
// TabInstanceId — 복합 탭 식별자 (TabType + InstanceId)
// ============================================================================

/// 복합 탭 인스턴스 식별자
///
/// UE5에서 탭을 (TabType, InstanceId) 쌍으로 고유 식별하는 패턴.
/// 동일 타입의 여러 탭 인스턴스를 구분합니다.
///
/// 예: ("BlueprintEditor", "/Game/BP_Player")
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TabInstanceId {
    /// 탭 타입 이름 (스포너 타입)
    pub tab_type: String,
    /// 인스턴스 ID (에셋 경로, 고유 키 등)
    pub instance_id: String,
}

impl TabInstanceId {
    pub fn new(tab_type: impl Into<String>, instance_id: impl Into<String>) -> Self {
        Self {
            tab_type: tab_type.into(),
            instance_id: instance_id.into(),
        }
    }

    /// 싱글턴 탭 (인스턴스 ID 없음)
    pub fn singleton(tab_type: impl Into<String>) -> Self {
        Self {
            tab_type: tab_type.into(),
            instance_id: String::new(),
        }
    }

    /// DockTab에서 TabInstanceId 추출
    pub fn from_dock_tab(tab: &super::DockTab) -> Option<Self> {
        let tab_type = tab.tab_type.as_ref()?;
        Some(Self {
            tab_type: tab_type.clone(),
            instance_id: tab.instance_id.clone().unwrap_or_default(),
        })
    }

    /// 같은 타입인지
    pub fn same_type(&self, other: &TabInstanceId) -> bool {
        self.tab_type == other.tab_type
    }
}

impl std::fmt::Display for TabInstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.instance_id.is_empty() {
            write!(f, "{}", self.tab_type)
        } else {
            write!(f, "{}:{}", self.tab_type, self.instance_id)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_item_category() {
        let root = WorkspaceItem::category("root", "Window")
            .child(WorkspaceItem::category("general", "General")
                .child(WorkspaceItem::tab("viewport", "Viewport", "Viewport"))
                .child(WorkspaceItem::tab("hierarchy", "Hierarchy", "Hierarchy"))
            )
            .child(WorkspaceItem::category("debug", "Debug")
                .child(WorkspaceItem::tab("output_log", "Output Log", "OutputLog"))
            );

        assert!(root.is_category());
        assert_eq!(root.children.len(), 2);
        assert_eq!(root.total_count(), 6); // root + 2 categories + 3 tabs
    }

    #[test]
    fn test_workspace_find_by_path() {
        let root = WorkspaceItem::category("root", "Window")
            .child(WorkspaceItem::category("debug", "Debug")
                .child(WorkspaceItem::tab("output_log", "Output Log", "OutputLog"))
            );

        let found = root.find_by_path("debug/output_log");
        assert!(found.is_some());
        assert_eq!(found.unwrap().spawner_type.as_deref(), Some("OutputLog"));

        assert!(root.find_by_path("nonexistent").is_none());
    }

    #[test]
    fn test_workspace_collect_leaves() {
        let root = WorkspaceItem::category("root", "Window")
            .child(WorkspaceItem::category("a", "A")
                .child(WorkspaceItem::tab("t1", "T1", "Type1"))
            )
            .child(WorkspaceItem::tab("t2", "T2", "Type2"));

        let leaves = root.collect_leaves();
        assert_eq!(leaves.len(), 2);
    }

    #[test]
    fn test_workspace_sort() {
        let mut root = WorkspaceItem::category("root", "Root")
            .child(WorkspaceItem::category("b", "B").with_sort_order(2))
            .child(WorkspaceItem::category("a", "A").with_sort_order(1));

        root.sort_children();
        assert_eq!(root.children[0].name, "a");
        assert_eq!(root.children[1].name, "b");
    }

    #[test]
    fn test_tab_instance_id() {
        let id1 = TabInstanceId::new("BlueprintEditor", "/Game/BP_Player");
        let id2 = TabInstanceId::new("BlueprintEditor", "/Game/BP_Enemy");
        let id3 = TabInstanceId::singleton("OutputLog");

        assert!(id1.same_type(&id2));
        assert!(!id1.same_type(&id3));
        assert_eq!(id3.instance_id, "");
        assert_eq!(format!("{}", id1), "BlueprintEditor:/Game/BP_Player");
        assert_eq!(format!("{}", id3), "OutputLog");
    }

    #[test]
    fn test_tab_instance_id_hash_eq() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(TabInstanceId::new("A", "1"));
        set.insert(TabInstanceId::new("A", "2"));
        set.insert(TabInstanceId::new("A", "1")); // duplicate

        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_workspace_max_depth() {
        let root = WorkspaceItem::category("root", "Root")
            .child(WorkspaceItem::category("a", "A")
                .child(WorkspaceItem::category("b", "B")
                    .child(WorkspaceItem::tab("t", "T", "Type"))
                )
            );

        assert_eq!(root.max_depth(), 3);
    }
}
