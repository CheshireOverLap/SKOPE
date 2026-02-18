//! WidgetPath — 루트에서 위젯까지의 전체 경로
//!
//! UE 참조: `FWidgetPath`, `SWidget::ParentWidgetPtr`
//!
//! 위젯 트리에서 특정 위젯까지의 경로를 표현합니다.
//! 포커스/입력 라우팅, 디버깅, 위젯 검색에 활용됩니다.

use crate::widget::Widget;
use std::fmt;

/// 위젯 경로의 각 항목
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetPathEntry {
    /// 위젯 고유 ID
    pub widget_id: u64,
    /// 위젯 타입명 (디버깅용)
    pub type_name: &'static str,
}

impl WidgetPathEntry {
    pub fn new(widget_id: u64, type_name: &'static str) -> Self {
        Self { widget_id, type_name }
    }

    /// 위젯에서 직접 생성
    pub fn from_widget(widget: &dyn Widget) -> Self {
        Self {
            widget_id: widget.widget_id(),
            type_name: widget.type_name(),
        }
    }
}

/// 루트에서 대상 위젯까지의 전체 경로
///
/// UE의 `FWidgetPath`에 해당.
/// `entries[0]`이 루트, `entries[last]`가 대상 위젯입니다.
#[derive(Debug, Clone, Default)]
pub struct WidgetPath {
    /// 루트부터 대상까지의 위젯 엔트리 목록
    pub entries: Vec<WidgetPathEntry>,
}

impl WidgetPath {
    /// 빈 경로 생성
    pub fn empty() -> Self {
        Self { entries: Vec::new() }
    }

    /// 단일 위젯 경로 생성
    pub fn single(widget: &dyn Widget) -> Self {
        Self {
            entries: vec![WidgetPathEntry::from_widget(widget)],
        }
    }

    /// 엔트리로부터 생성
    pub fn from_entries(entries: Vec<WidgetPathEntry>) -> Self {
        Self { entries }
    }

    /// 경로가 비어있는지
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 경로 길이 (깊이)
    pub fn depth(&self) -> usize {
        self.entries.len()
    }

    /// 대상 위젯 (경로의 마지막)
    pub fn target(&self) -> Option<&WidgetPathEntry> {
        self.entries.last()
    }

    /// 루트 위젯 (경로의 첫번째)
    pub fn root(&self) -> Option<&WidgetPathEntry> {
        self.entries.first()
    }

    /// 특정 위젯 ID가 경로에 포함되어 있는지
    pub fn contains_id(&self, widget_id: u64) -> bool {
        self.entries.iter().any(|e| e.widget_id == widget_id)
    }

    /// 특정 위젯 ID가 다른 ID의 조상인지 (경로 내에서)
    ///
    /// `ancestor_id`가 `descendant_id`보다 경로 앞쪽에 있으면 true.
    pub fn is_ancestor_of(&self, ancestor_id: u64, descendant_id: u64) -> bool {
        let ancestor_idx = self.entries.iter().position(|e| e.widget_id == ancestor_id);
        let descendant_idx = self.entries.iter().position(|e| e.widget_id == descendant_id);
        match (ancestor_idx, descendant_idx) {
            (Some(a), Some(d)) => a < d,
            _ => false,
        }
    }

    /// 두 경로의 공통 조상 깊이 (공통 prefix 길이)
    pub fn common_ancestor_depth(&self, other: &WidgetPath) -> usize {
        self.entries.iter()
            .zip(other.entries.iter())
            .take_while(|(a, b)| a.widget_id == b.widget_id)
            .count()
    }

    /// 경로에 엔트리 추가 (맨 뒤)
    pub fn push(&mut self, entry: WidgetPathEntry) {
        self.entries.push(entry);
    }

    /// 경로에서 엔트리 제거 (맨 뒤)
    pub fn pop(&mut self) -> Option<WidgetPathEntry> {
        self.entries.pop()
    }

    /// 부분 경로 (처음부터 depth까지)
    pub fn truncated(&self, depth: usize) -> WidgetPath {
        WidgetPath {
            entries: self.entries[..depth.min(self.entries.len())].to_vec(),
        }
    }

    /// 슬래시 구분 표시 문자열 생성 (예: "SWindow/SBox/SButton")
    pub fn display_string(&self) -> String {
        self.entries
            .iter()
            .map(|e| e.type_name)
            .collect::<Vec<_>>()
            .join("/")
    }
}

impl fmt::Display for WidgetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_string())
    }
}

/// 위젯 트리에서 대상 위젯까지의 경로를 DFS로 검색
///
/// UE의 `FindPathToWidget()`에 해당.
/// `root`부터 시작하여 `target_id`를 가진 위젯까지의 경로를 반환합니다.
pub fn find_path_to_widget(root: &dyn Widget, target_id: u64) -> Option<WidgetPath> {
    let mut path = Vec::new();
    if find_path_recursive(root, target_id, &mut path) {
        Some(WidgetPath::from_entries(path))
    } else {
        None
    }
}

/// DFS 재귀 탐색 (내부)
fn find_path_recursive(
    widget: &dyn Widget,
    target_id: u64,
    path: &mut Vec<WidgetPathEntry>,
) -> bool {
    let entry = WidgetPathEntry::from_widget(widget);
    path.push(entry);

    // 현재 위젯이 대상인 경우
    if widget.widget_id() == target_id {
        return true;
    }

    // 자식 탐색
    for i in 0..widget.num_children() {
        if let Some(child) = widget.get_child(i) {
            if find_path_recursive(child, target_id, path) {
                return true;
            }
        }
    }

    // 이 경로에 대상 없음 — 백트랙
    path.pop();
    false
}

/// 기존 경로가 여전히 유효한지 검증
///
/// UE의 `ValidatePathToChild()`에 해당.
/// 경로의 각 엔트리가 부모-자식 관계를 유지하는지 확인합니다.
pub fn validate_path(root: &dyn Widget, path: &WidgetPath) -> bool {
    if path.is_empty() {
        return true;
    }

    // 루트 일치 확인
    if path.entries[0].widget_id != root.widget_id() {
        return false;
    }

    // 각 단계의 부모-자식 관계 확인
    let mut current: &dyn Widget = root;
    for entry in path.entries.iter().skip(1) {
        let mut found = false;
        for i in 0..current.num_children() {
            if let Some(child) = current.get_child(i) {
                if child.widget_id() == entry.widget_id {
                    current = child;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return false;
        }
    }

    true
}

/// 특정 위젯이 다른 위젯의 자손인지 확인
///
/// UE의 `IsDescendantOf()`에 해당.
/// 위젯 트리를 탐색하여 ancestor 아래에 descendant가 있는지 확인합니다.
pub fn is_descendant_of(ancestor: &dyn Widget, descendant_id: u64) -> bool {
    if ancestor.widget_id() == descendant_id {
        return true;
    }

    for i in 0..ancestor.num_children() {
        if let Some(child) = ancestor.get_child(i) {
            if is_descendant_of(child, descendant_id) {
                return true;
            }
        }
    }

    false
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_path_empty() {
        let path = WidgetPath::empty();
        assert!(path.is_empty());
        assert_eq!(path.depth(), 0);
        assert!(path.target().is_none());
        assert!(path.root().is_none());
    }

    #[test]
    fn test_widget_path_push_pop() {
        let mut path = WidgetPath::empty();
        path.push(WidgetPathEntry::new(1, "SWindow"));
        path.push(WidgetPathEntry::new(2, "SBox"));
        path.push(WidgetPathEntry::new(3, "SButton"));

        assert_eq!(path.depth(), 3);
        assert_eq!(path.root().unwrap().type_name, "SWindow");
        assert_eq!(path.target().unwrap().type_name, "SButton");
        assert_eq!(path.display_string(), "SWindow/SBox/SButton");

        let popped = path.pop().unwrap();
        assert_eq!(popped.widget_id, 3);
        assert_eq!(path.depth(), 2);
    }

    #[test]
    fn test_widget_path_contains_and_ancestor() {
        let path = WidgetPath::from_entries(vec![
            WidgetPathEntry::new(10, "SWindow"),
            WidgetPathEntry::new(20, "SBox"),
            WidgetPathEntry::new(30, "SButton"),
        ]);

        assert!(path.contains_id(20));
        assert!(!path.contains_id(99));

        assert!(path.is_ancestor_of(10, 30));
        assert!(path.is_ancestor_of(10, 20));
        assert!(!path.is_ancestor_of(30, 10));
        assert!(!path.is_ancestor_of(99, 10));
    }

    #[test]
    fn test_widget_path_common_ancestor() {
        let path_a = WidgetPath::from_entries(vec![
            WidgetPathEntry::new(1, "Root"),
            WidgetPathEntry::new(2, "Panel"),
            WidgetPathEntry::new(3, "BtnA"),
        ]);
        let path_b = WidgetPath::from_entries(vec![
            WidgetPathEntry::new(1, "Root"),
            WidgetPathEntry::new(2, "Panel"),
            WidgetPathEntry::new(4, "BtnB"),
        ]);

        assert_eq!(path_a.common_ancestor_depth(&path_b), 2);
    }

    #[test]
    fn test_widget_path_truncated() {
        let path = WidgetPath::from_entries(vec![
            WidgetPathEntry::new(1, "A"),
            WidgetPathEntry::new(2, "B"),
            WidgetPathEntry::new(3, "C"),
        ]);

        let truncated = path.truncated(2);
        assert_eq!(truncated.depth(), 2);
        assert_eq!(truncated.display_string(), "A/B");

        // Over-truncate should clamp
        let over = path.truncated(100);
        assert_eq!(over.depth(), 3);
    }

    #[test]
    fn test_widget_path_display() {
        let path = WidgetPath::from_entries(vec![
            WidgetPathEntry::new(1, "SWindow"),
            WidgetPathEntry::new(2, "SOverlay"),
            WidgetPathEntry::new(3, "STextBlock"),
        ]);
        assert_eq!(format!("{}", path), "SWindow/SOverlay/STextBlock");
    }

    #[test]
    fn test_validate_empty_path() {
        // Empty path is always valid
        use crate::widget::SNullWidget;
        let root = SNullWidget::new();
        let path = WidgetPath::empty();
        assert!(validate_path(&root, &path));
    }

    #[test]
    fn test_find_path_single_widget() {
        use crate::widget::SNullWidget;
        let root = SNullWidget::new();
        let root_id = root.widget_id();

        // widget_id가 유효하면 찾을 수 있어야 함
        let result = find_path_to_widget(&root, root_id);
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path.depth(), 1);
        assert_eq!(path.target().unwrap().type_name, "SNullWidget");
    }

    #[test]
    fn test_is_descendant_self() {
        use crate::widget::SNullWidget;
        let root = SNullWidget::new();
        let root_id = root.widget_id();
        // 자기 자신은 자신의 자손 (자기 포함 정의)
        assert!(is_descendant_of(&root, root_id));
    }
}
