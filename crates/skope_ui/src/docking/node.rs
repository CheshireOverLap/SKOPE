//! 도킹 노드 타입
//!
//! 트리를 구성하는 3가지 노드:
//! - DockArea: 루트 노드 (OS 윈도우 하나)
//! - DockSplitter: 가지 노드 (화면 분할)
//! - DockTabStack: 잎 노드 (탭 그룹)

use super::{NodeId, TabId, SplitDirection, NodeRect, TabStackStyle};
use serde::{Serialize, Deserialize};

/// 도킹 노드 (재귀적 트리 구조)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DockNode {
    /// 루트 영역 (OS 윈도우)
    Area(DockArea),
    /// 분할자 (가지 노드)
    Splitter(DockSplitter),
    /// 탭 스택 (잎 노드)
    TabStack(DockTabStack),
}

impl Default for DockNode {
    fn default() -> Self {
        Self::TabStack(DockTabStack::new(NodeId::new(0)))
    }
}

impl DockNode {
    /// 노드 ID
    pub fn id(&self) -> NodeId {
        match self {
            Self::Area(a) => a.id,
            Self::Splitter(s) => s.id,
            Self::TabStack(t) => t.id,
        }
    }

    /// 잎 노드인지 (TabStack)
    pub fn is_leaf(&self) -> bool {
        matches!(self, Self::TabStack(_))
    }

    /// Area인지
    pub fn is_area(&self) -> bool {
        matches!(self, Self::Area(_))
    }

    /// Splitter인지
    pub fn is_splitter(&self) -> bool {
        matches!(self, Self::Splitter(_))
    }

    /// 노드 타입 이름
    pub fn node_type_name(&self) -> &'static str {
        match self {
            Self::Area(_) => "Area",
            Self::Splitter(_) => "Splitter",
            Self::TabStack(_) => "TabStack",
        }
    }

    /// TabStack으로 변환
    pub fn as_tab_stack(&self) -> Option<&DockTabStack> {
        match self {
            Self::TabStack(t) => Some(t),
            _ => None,
        }
    }

    /// TabStack으로 변환 (mutable)
    pub fn as_tab_stack_mut(&mut self) -> Option<&mut DockTabStack> {
        match self {
            Self::TabStack(t) => Some(t),
            _ => None,
        }
    }

    /// Splitter로 변환
    pub fn as_splitter(&self) -> Option<&DockSplitter> {
        match self {
            Self::Splitter(s) => Some(s),
            _ => None,
        }
    }

    /// Splitter로 변환 (mutable)
    pub fn as_splitter_mut(&mut self) -> Option<&mut DockSplitter> {
        match self {
            Self::Splitter(s) => Some(s),
            _ => None,
        }
    }

    /// Area로 변환
    pub fn as_area(&self) -> Option<&DockArea> {
        match self {
            Self::Area(a) => Some(a),
            _ => None,
        }
    }

    /// 자식 노드들 반환
    pub fn children(&self) -> &[DockNode] {
        match self {
            Self::Area(a) => a.child.as_ref().map(|c| std::slice::from_ref(c.as_ref())).unwrap_or(&[]),
            Self::Splitter(s) => &s.children,
            Self::TabStack(_) => &[],
        }
    }

    /// 자식 노드들 반환 (mutable) - 주의: Area는 직접 수정 불가
    pub fn children_mut(&mut self) -> Option<&mut Vec<DockNode>> {
        match self {
            Self::Splitter(s) => Some(&mut s.children),
            _ => None,
        }
    }
}

/// 도킹 영역 (루트 노드)
///
/// OS 윈도우 하나를 담당하는 최상위 컨테이너
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockArea {
    /// 고유 ID
    pub id: NodeId,
    /// 윈도우 제목
    pub title: String,
    /// 자식 노드 (Splitter 또는 TabStack)
    pub child: Option<Box<DockNode>>,
    /// 레이아웃 정보 (런타임)
    #[serde(skip)]
    pub rect: NodeRect,
}

impl DockArea {
    pub fn new(id: NodeId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            child: None,
            rect: NodeRect::default(),
        }
    }

    /// 자식 설정
    pub fn set_child(&mut self, child: DockNode) {
        self.child = Some(Box::new(child));
    }

    /// 자식 가져오기
    pub fn child(&self) -> Option<&DockNode> {
        self.child.as_ref().map(|c| c.as_ref())
    }

    /// 자식 가져오기 (mutable)
    pub fn child_mut(&mut self) -> Option<&mut DockNode> {
        self.child.as_mut().map(|c| c.as_mut())
    }
}

/// 도킹 분할자 (가지 노드)
///
/// 화면을 가로(Horizontal) 또는 세로(Vertical)로 나눔
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockSplitter {
    /// 고유 ID
    pub id: NodeId,
    /// 분할 방향
    pub direction: SplitDirection,
    /// 자식 노드들
    pub children: Vec<DockNode>,
    /// 자식별 크기 비율 (0.0 ~ 1.0, 합이 1.0)
    pub ratios: Vec<f32>,
    /// 레이아웃 정보 (런타임)
    #[serde(skip)]
    pub rect: NodeRect,
}

impl DockSplitter {
    pub fn new(id: NodeId, direction: SplitDirection) -> Self {
        Self {
            id,
            direction,
            children: Vec::new(),
            ratios: Vec::new(),
            rect: NodeRect::default(),
        }
    }

    /// 두 자식으로 생성 (50:50 비율)
    pub fn with_children(id: NodeId, direction: SplitDirection, first: DockNode, second: DockNode) -> Self {
        Self {
            id,
            direction,
            children: vec![first, second],
            ratios: vec![0.5, 0.5],
            rect: NodeRect::default(),
        }
    }

    /// 자식 추가
    pub fn add_child(&mut self, child: DockNode, ratio: f32) {
        self.children.push(child);
        self.ratios.push(ratio);
        self.normalize_ratios();
    }

    /// 자식 제거
    pub fn remove_child(&mut self, index: usize) -> Option<DockNode> {
        if index < self.children.len() {
            self.ratios.remove(index);
            let child = self.children.remove(index);
            self.normalize_ratios();
            Some(child)
        } else {
            None
        }
    }

    /// 비율 정규화 (합이 1.0이 되도록)
    pub fn normalize_ratios(&mut self) {
        let sum: f32 = self.ratios.iter().sum();
        if sum > 0.0 {
            for ratio in &mut self.ratios {
                *ratio /= sum;
            }
        } else if !self.ratios.is_empty() {
            let equal = 1.0 / self.ratios.len() as f32;
            for ratio in &mut self.ratios {
                *ratio = equal;
            }
        }
    }

    /// 특정 인덱스의 분할선 위치 (0.0 ~ 1.0)
    pub fn split_position(&self, index: usize) -> f32 {
        self.ratios.iter().take(index + 1).sum()
    }

    /// 분할선 위치 조정
    pub fn adjust_split(&mut self, index: usize, delta: f32) {
        if index >= self.ratios.len() - 1 {
            return;
        }

        let min_ratio = 0.1; // 최소 10%
        let new_left = (self.ratios[index] + delta).max(min_ratio);
        let new_right = (self.ratios[index + 1] - delta).max(min_ratio);

        // 둘 다 최소 비율을 만족하는 경우만 적용
        if new_left >= min_ratio && new_right >= min_ratio {
            let total = self.ratios[index] + self.ratios[index + 1];
            self.ratios[index] = new_left;
            self.ratios[index + 1] = total - new_left;
        }
    }
}

/// 도킹 탭 스택 (잎 노드)
///
/// 실제 눈에 보이는 "탭들이 모여 있는 그룹"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockTabStack {
    /// 고유 ID
    pub id: NodeId,
    /// 포함된 탭 ID들
    pub tabs: Vec<TabId>,
    /// 현재 활성 탭 인덱스
    pub active_tab: usize,
    /// 레이아웃 정보 (런타임)
    #[serde(skip)]
    pub rect: NodeRect,
    /// 탭 바 영역 (런타임)
    #[serde(skip)]
    pub tab_bar_rect: NodeRect,
    /// 콘텐츠 영역 (런타임)
    #[serde(skip)]
    pub content_rect: NodeRect,
    /// 계산된 탭 너비 (런타임)
    #[serde(skip)]
    pub computed_tab_widths: Vec<f32>,
}

impl DockTabStack {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            tabs: Vec::new(),
            active_tab: 0,
            rect: NodeRect::default(),
            tab_bar_rect: NodeRect::default(),
            content_rect: NodeRect::default(),
            computed_tab_widths: Vec::new(),
        }
    }

    /// 단일 탭으로 생성
    pub fn with_tab(id: NodeId, tab_id: TabId) -> Self {
        Self {
            id,
            tabs: vec![tab_id],
            active_tab: 0,
            rect: NodeRect::default(),
            tab_bar_rect: NodeRect::default(),
            content_rect: NodeRect::default(),
            computed_tab_widths: Vec::new(),
        }
    }

    /// 탭 추가
    pub fn add_tab(&mut self, tab_id: TabId) {
        self.tabs.push(tab_id);
        self.active_tab = self.tabs.len() - 1;
    }

    /// 탭 삽입
    pub fn insert_tab(&mut self, index: usize, tab_id: TabId) {
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, tab_id);
        self.active_tab = index;
    }

    /// 탭 제거
    pub fn remove_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|&id| id == tab_id) {
            self.tabs.remove(index);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// 탭 인덱스로 제거
    pub fn remove_tab_at(&mut self, index: usize) -> Option<TabId> {
        if index < self.tabs.len() {
            let tab_id = self.tabs.remove(index);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                self.active_tab = self.tabs.len() - 1;
            }
            Some(tab_id)
        } else {
            None
        }
    }

    /// 활성 탭 ID
    pub fn active_tab_id(&self) -> Option<TabId> {
        self.tabs.get(self.active_tab).copied()
    }

    /// 탭이 비었는지
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    /// 탭 개수
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// 탭 너비 계산 (가용 폭 기반)
    pub fn compute_tab_widths(&mut self, available_width: f32, style: &TabStackStyle) {
        let n = self.tabs.len();
        if n == 0 {
            self.computed_tab_widths.clear();
            return;
        }
        // 오버랩 고려: N개 탭의 총 폭 = N*w - (N-1)*overlap
        // → w = (usable + (N-1)*overlap) / N
        let total_overlap = style.tab_overlap * (n as f32 - 1.0);
        let usable = available_width - style.tab_padding * 2.0 + total_overlap;
        let per_tab = (usable / n as f32).clamp(style.tab_min_width, style.tab_max_width);
        self.computed_tab_widths = vec![per_tab; n];
    }

    /// 인덱스별 탭 너비 (계산되지 않았으면 fallback)
    pub fn tab_width(&self, index: usize) -> f32 {
        self.computed_tab_widths.get(index).copied().unwrap_or(120.0)
    }

    /// 균일 탭 너비 (첫 번째 값)
    pub fn uniform_tab_width(&self) -> f32 {
        self.computed_tab_widths.first().copied().unwrap_or(120.0)
    }

    /// 탭 활성화
    pub fn activate_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.active_tab = index;
        }
    }

    /// 탭 ID로 활성화
    pub fn activate_tab_by_id(&mut self, tab_id: TabId) -> bool {
        if let Some(index) = self.tabs.iter().position(|&id| id == tab_id) {
            self.active_tab = index;
            true
        } else {
            false
        }
    }

    /// 탭 순서 변경 (언리얼 SDockingTabWell 스타일)
    /// 탭을 현재 위치에서 제거하고 새 위치에 삽입
    pub fn reorder_tab(&mut self, tab_id: TabId, new_index: usize) -> bool {
        // 현재 위치 찾기
        let current_index = match self.tabs.iter().position(|&id| id == tab_id) {
            Some(idx) => idx,
            None => return false,
        };

        // 같은 위치면 아무것도 안 함
        if current_index == new_index {
            return true;
        }

        // 탭 제거 후 새 위치에 삽입
        self.tabs.remove(current_index);
        let insert_index = new_index.min(self.tabs.len());
        self.tabs.insert(insert_index, tab_id);

        // 활성 탭 인덱스 조정
        self.active_tab = insert_index;

        true
    }
}
