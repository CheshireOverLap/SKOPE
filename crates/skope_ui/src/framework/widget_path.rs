//! WidgetPath — 위젯 경로 추적 시스템
//!
//! 위젯 트리에서의 경로를 추적하여 포커스/입력 라우팅에 활용.
//! UE Slate의 FWidgetPath에 해당.

/// 위젯 경로 엔트리
#[derive(Debug, Clone)]
pub struct WidgetPathEntry {
    pub widget_id: u64,
    pub widget_type: String,
    pub child_index: usize,
}

impl WidgetPathEntry {
    pub fn new(widget_id: u64, widget_type: impl Into<String>, child_index: usize) -> Self {
        Self {
            widget_id,
            widget_type: widget_type.into(),
            child_index,
        }
    }
}

/// 위젯 경로 — 루트에서 특정 위젯까지의 경로
#[derive(Debug, Clone)]
pub struct FWidgetPath {
    entries: Vec<WidgetPathEntry>,
}

impl FWidgetPath {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    pub fn from_entries(entries: Vec<WidgetPathEntry>) -> Self {
        Self { entries }
    }

    /// 경로에 엔트리 추가
    pub fn push(&mut self, entry: WidgetPathEntry) {
        self.entries.push(entry);
    }

    /// 마지막 엔트리 제거
    pub fn pop(&mut self) -> Option<WidgetPathEntry> {
        self.entries.pop()
    }

    /// 경로의 마지막 위젯 (가장 깊은)
    pub fn leaf(&self) -> Option<&WidgetPathEntry> {
        self.entries.last()
    }

    /// 경로의 루트 위젯
    pub fn root(&self) -> Option<&WidgetPathEntry> {
        self.entries.first()
    }

    /// 특정 위젯이 경로에 포함되어 있는지
    pub fn contains_widget(&self, widget_id: u64) -> bool {
        self.entries.iter().any(|e| e.widget_id == widget_id)
    }

    /// 두 경로의 공통 조상 찾기
    pub fn common_ancestor(&self, other: &FWidgetPath) -> Option<u64> {
        let mut common = None;
        for (a, b) in self.entries.iter().zip(other.entries.iter()) {
            if a.widget_id == b.widget_id {
                common = Some(a.widget_id);
            } else {
                break;
            }
        }
        common
    }

    /// 경로를 위에서 아래로 순회
    pub fn top_down(&self) -> impl Iterator<Item = &WidgetPathEntry> {
        self.entries.iter()
    }

    /// 경로를 아래에서 위로 순회 (이벤트 버블링)
    pub fn bottom_up(&self) -> impl Iterator<Item = &WidgetPathEntry> {
        self.entries.iter().rev()
    }

    /// 특정 위젯 이후의 서브 경로
    pub fn sub_path_from(&self, widget_id: u64) -> Option<FWidgetPath> {
        if let Some(pos) = self.entries.iter().position(|e| e.widget_id == widget_id) {
            Some(FWidgetPath::from_entries(self.entries[pos..].to_vec()))
        } else {
            None
        }
    }

    pub fn depth(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn entries(&self) -> &[WidgetPathEntry] { &self.entries }

    /// 경로를 문자열로 표현 (디버그용)
    pub fn to_debug_string(&self) -> String {
        self.entries.iter()
            .map(|e| format!("{}({})", e.widget_type, e.widget_id))
            .collect::<Vec<_>>()
            .join(" > ")
    }
}

impl Default for FWidgetPath {
    fn default() -> Self { Self::new() }
}

/// 위젯 경로 기반 포커스/입력 라우팅
pub struct WidgetPathRouter {
    /// 현재 포커스 경로
    focus_path: Option<FWidgetPath>,
    /// 마우스 캡처 경로
    capture_path: Option<FWidgetPath>,
    /// 호버 경로
    hover_path: Option<FWidgetPath>,
    /// 경로 변경 기록
    path_history: Vec<PathChangeRecord>,
    max_history: usize,
}

/// 경로 변경 기록
#[derive(Debug, Clone)]
pub struct PathChangeRecord {
    pub change_type: PathChangeType,
    pub old_leaf: Option<u64>,
    pub new_leaf: Option<u64>,
    pub timestamp: f64,
}

/// 경로 변경 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathChangeType {
    FocusChanged,
    CaptureAcquired,
    CaptureReleased,
    HoverChanged,
}

impl WidgetPathRouter {
    pub fn new() -> Self {
        Self {
            focus_path: None,
            capture_path: None,
            hover_path: None,
            path_history: Vec::new(),
            max_history: 64,
        }
    }

    /// 포커스 경로 설정
    pub fn set_focus_path(&mut self, path: FWidgetPath, timestamp: f64) {
        let old_leaf = self.focus_path.as_ref().and_then(|p| p.leaf().map(|e| e.widget_id));
        let new_leaf = path.leaf().map(|e| e.widget_id);
        self.record_change(PathChangeType::FocusChanged, old_leaf, new_leaf, timestamp);
        self.focus_path = Some(path);
    }

    /// 포커스 경로 클리어
    pub fn clear_focus(&mut self, timestamp: f64) {
        let old_leaf = self.focus_path.as_ref().and_then(|p| p.leaf().map(|e| e.widget_id));
        self.record_change(PathChangeType::FocusChanged, old_leaf, None, timestamp);
        self.focus_path = None;
    }

    /// 마우스 캡처 경로 설정
    pub fn set_capture_path(&mut self, path: FWidgetPath, timestamp: f64) {
        let new_leaf = path.leaf().map(|e| e.widget_id);
        self.record_change(PathChangeType::CaptureAcquired, None, new_leaf, timestamp);
        self.capture_path = Some(path);
    }

    /// 마우스 캡처 해제
    pub fn release_capture(&mut self, timestamp: f64) {
        let old_leaf = self.capture_path.as_ref().and_then(|p| p.leaf().map(|e| e.widget_id));
        self.record_change(PathChangeType::CaptureReleased, old_leaf, None, timestamp);
        self.capture_path = None;
    }

    /// 호버 경로 설정
    pub fn set_hover_path(&mut self, path: FWidgetPath, timestamp: f64) {
        let old_leaf = self.hover_path.as_ref().and_then(|p| p.leaf().map(|e| e.widget_id));
        let new_leaf = path.leaf().map(|e| e.widget_id);
        if old_leaf != new_leaf {
            self.record_change(PathChangeType::HoverChanged, old_leaf, new_leaf, timestamp);
        }
        self.hover_path = Some(path);
    }

    /// 특정 위젯이 포커스 경로에 있는지
    pub fn is_in_focus_path(&self, widget_id: u64) -> bool {
        self.focus_path.as_ref().map_or(false, |p| p.contains_widget(widget_id))
    }

    /// 특정 위젯이 캡처 대상인지
    pub fn has_capture(&self, widget_id: u64) -> bool {
        self.capture_path.as_ref()
            .and_then(|p| p.leaf())
            .map_or(false, |e| e.widget_id == widget_id)
    }

    /// 이벤트를 라우팅할 경로 반환 (캡처 > 포커스 > 호버 우선)
    pub fn event_route(&self) -> Option<&FWidgetPath> {
        self.capture_path.as_ref()
            .or(self.focus_path.as_ref())
            .or(self.hover_path.as_ref())
    }

    pub fn focus_path(&self) -> Option<&FWidgetPath> { self.focus_path.as_ref() }
    pub fn capture_path(&self) -> Option<&FWidgetPath> { self.capture_path.as_ref() }
    pub fn hover_path(&self) -> Option<&FWidgetPath> { self.hover_path.as_ref() }
    pub fn history(&self) -> &[PathChangeRecord] { &self.path_history }

    fn record_change(&mut self, change_type: PathChangeType, old_leaf: Option<u64>, new_leaf: Option<u64>, timestamp: f64) {
        self.path_history.push(PathChangeRecord {
            change_type,
            old_leaf,
            new_leaf,
            timestamp,
        });
        if self.path_history.len() > self.max_history {
            self.path_history.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_path(ids: &[(u64, &str)]) -> FWidgetPath {
        let entries = ids.iter().enumerate()
            .map(|(i, (id, typ))| WidgetPathEntry::new(*id, *typ, i))
            .collect();
        FWidgetPath::from_entries(entries)
    }

    #[test]
    fn test_widget_path_basic() {
        let path = make_path(&[(1, "Window"), (2, "Panel"), (3, "Button")]);
        assert_eq!(path.depth(), 3);
        assert_eq!(path.leaf().unwrap().widget_id, 3);
        assert_eq!(path.root().unwrap().widget_id, 1);
    }

    #[test]
    fn test_widget_path_contains() {
        let path = make_path(&[(1, "A"), (2, "B"), (3, "C")]);
        assert!(path.contains_widget(2));
        assert!(!path.contains_widget(99));
    }

    #[test]
    fn test_widget_path_common_ancestor() {
        let path_a = make_path(&[(1, "R"), (2, "A"), (3, "X")]);
        let path_b = make_path(&[(1, "R"), (2, "A"), (4, "Y")]);
        assert_eq!(path_a.common_ancestor(&path_b), Some(2));
    }

    #[test]
    fn test_widget_path_sub_path() {
        let path = make_path(&[(1, "A"), (2, "B"), (3, "C")]);
        let sub = path.sub_path_from(2).unwrap();
        assert_eq!(sub.depth(), 2);
        assert_eq!(sub.root().unwrap().widget_id, 2);
    }

    #[test]
    fn test_widget_path_debug_string() {
        let path = make_path(&[(1, "Window"), (2, "Button")]);
        assert_eq!(path.to_debug_string(), "Window(1) > Button(2)");
    }

    #[test]
    fn test_router_focus() {
        let mut router = WidgetPathRouter::new();
        let path = make_path(&[(1, "Root"), (2, "Child")]);
        router.set_focus_path(path, 0.0);
        assert!(router.is_in_focus_path(1));
        assert!(router.is_in_focus_path(2));
        assert!(!router.is_in_focus_path(3));
    }

    #[test]
    fn test_router_capture() {
        let mut router = WidgetPathRouter::new();
        let path = make_path(&[(1, "Root"), (2, "Slider")]);
        router.set_capture_path(path, 0.0);
        assert!(router.has_capture(2));
        assert!(!router.has_capture(1)); // only leaf
        router.release_capture(1.0);
        assert!(!router.has_capture(2));
    }

    #[test]
    fn test_router_event_route_priority() {
        let mut router = WidgetPathRouter::new();
        let focus = make_path(&[(1, "Focus")]);
        let capture = make_path(&[(2, "Capture")]);
        router.set_focus_path(focus, 0.0);
        // 캡처 없으면 포커스 경로
        assert_eq!(router.event_route().unwrap().leaf().unwrap().widget_id, 1);
        router.set_capture_path(capture, 0.0);
        // 캡처 있으면 캡처 경로 우선
        assert_eq!(router.event_route().unwrap().leaf().unwrap().widget_id, 2);
    }

    #[test]
    fn test_router_history() {
        let mut router = WidgetPathRouter::new();
        let path = make_path(&[(1, "A")]);
        router.set_focus_path(path, 0.0);
        router.clear_focus(1.0);
        assert_eq!(router.history().len(), 2);
        assert_eq!(router.history()[0].change_type, PathChangeType::FocusChanged);
    }
}
