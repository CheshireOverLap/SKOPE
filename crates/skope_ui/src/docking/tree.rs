//! 도킹 트리 관리
//!
//! 트리 구조 조작 및 레이아웃 계산

use super::{
    NodeId, TabId, SplitDirection, DockPosition, NodeRect,
    DockNode, DockArea, DockSplitter, DockTabStack,
    TabStackStyle, SplitterStyle,
};
use glam::Vec2;
use serde::{Serialize, Deserialize};

/// 도킹 트리
#[derive(Serialize, Deserialize)]
pub struct DockTree {
    /// 루트 노드
    root: DockArea,
    /// 다음 노드 ID
    next_node_id: u64,
    /// 탭 스택 스타일
    #[serde(skip)]
    pub tab_style: TabStackStyle,
    /// 스플리터 스타일
    #[serde(skip)]
    pub splitter_style: SplitterStyle,
    /// 마지막 레이아웃 rect (구조 변경 시 자동 재계산용)
    #[serde(skip)]
    last_layout_rect: Option<NodeRect>,
}

impl DockTree {
    /// 새 도킹 트리 생성
    pub fn new(title: impl Into<String>) -> Self {
        let root = DockArea::new(NodeId::new(0), title);
        Self {
            root,
            next_node_id: 1,
            tab_style: TabStackStyle::default(),
            splitter_style: SplitterStyle::default(),
            last_layout_rect: None,
        }
    }

    /// 새 노드 ID 생성
    pub fn next_node_id(&mut self) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        id
    }

    /// 루트 노드 참조
    pub fn root(&self) -> &DockArea {
        &self.root
    }

    /// 루트 노드 참조 (mutable)
    pub fn root_mut(&mut self) -> &mut DockArea {
        &mut self.root
    }

    /// 루트가 비었는지
    pub fn is_empty(&self) -> bool {
        self.root.child.is_none()
    }

    /// 빈 스택 정리 (드래그 완료 후 호출)
    pub fn cleanup_empty_stacks(&mut self) {
        // 빈 탭 스택 ID 수집
        let empty_stack_ids: Vec<NodeId> = self.collect_empty_stack_ids();

        if empty_stack_ids.is_empty() {
            return;
        }

        // 각 빈 스택 제거
        for stack_id in empty_stack_ids {
            self.remove_empty_stack(stack_id);
        }

        // 구조 변경됨 - 레이아웃 재계산
        self.recompute_layout();
    }

    /// 빈 탭 스택 ID 수집
    fn collect_empty_stack_ids(&self) -> Vec<NodeId> {
        let mut empty_ids = Vec::new();
        if let Some(child) = &self.root.child {
            Self::collect_empty_stacks_recursive(child, &mut empty_ids);
        }
        empty_ids
    }

    fn collect_empty_stacks_recursive(node: &DockNode, empty_ids: &mut Vec<NodeId>) {
        match node {
            DockNode::TabStack(stack) => {
                if stack.is_empty() {
                    empty_ids.push(stack.id);
                }
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::collect_empty_stacks_recursive(child, empty_ids);
                }
            }
            DockNode::Area(_) => {
                // Area는 루트에서만 사용되므로 무시
            }
        }
    }

    /// 탭 추가 (루트에 단일 탭 스택으로)
    pub fn add_tab(&mut self, tab_id: TabId) {
        if self.root.child.is_none() {
            // 첫 탭 - 새 탭 스택 생성
            let stack_id = self.next_node_id();
            let stack = DockTabStack::with_tab(stack_id, tab_id);
            self.root.set_child(DockNode::TabStack(stack));
            // 구조 변경됨 - 레이아웃 재계산
            self.recompute_layout();
        } else {
            // 기존 구조에 탭 추가 - 첫 번째 탭 스택 찾아서 추가
            if let Some(stack) = self.find_first_tab_stack_mut() {
                stack.add_tab(tab_id);
            }
        }
    }

    /// 특정 탭 스택에 탭 추가
    pub fn add_tab_to_stack(&mut self, stack_id: NodeId, tab_id: TabId) -> bool {
        if let Some(stack) = self.find_tab_stack_mut(stack_id) {
            stack.add_tab(tab_id);
            true
        } else {
            false
        }
    }

    /// 탭 제거
    pub fn remove_tab(&mut self, tab_id: TabId) -> bool {
        // 탭이 속한 스택 찾기
        if let Some(stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                stack.remove_tab(tab_id);

                // 스택이 비었으면 스택 제거
                if stack.is_empty() {
                    self.remove_empty_stack(stack_id);
                }
                return true;
            }
        }
        false
    }

    /// 탭 도킹
    ///
    /// `tab_id`를 `target_stack_id`의 `position` 위치에 도킹
    pub fn dock_tab(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
    ) -> bool {
        let success = match position {
            DockPosition::Center => {
                // 병합 - 타겟 스택에 탭 추가
                self.add_tab_to_stack(target_stack_id, tab_id)
            }
            _ => {
                // 분할 도킹
                self.split_dock_tab(tab_id, target_stack_id, position)
            }
        };

        // 구조가 변경되었으면 레이아웃 재계산
        if success {
            self.recompute_layout();
        }

        success
    }

    /// 분할 도킹
    fn split_dock_tab(
        &mut self,
        tab_id: TabId,
        target_stack_id: NodeId,
        position: DockPosition,
    ) -> bool {
        let direction = match position.split_direction() {
            Some(d) => d,
            None => return false,
        };

        // 새 탭 스택 생성
        let new_stack_id = self.next_node_id();
        let new_stack = DockTabStack::with_tab(new_stack_id, tab_id);
        let new_node = DockNode::TabStack(new_stack);

        // 타겟 노드를 분리하고 스플리터로 교체
        self.insert_split(target_stack_id, new_node, direction, position.is_first_child())
    }

    /// 노드를 분리하고 스플리터 삽입
    fn insert_split(
        &mut self,
        target_id: NodeId,
        new_node: DockNode,
        direction: SplitDirection,
        new_is_first: bool,
    ) -> bool {
        // 루트의 자식인 경우
        if let Some(child) = &self.root.child {
            if child.id() == target_id {
                let target_node = self.root.child.take().unwrap();
                let splitter_id = self.next_node_id();

                let (first, second) = if new_is_first {
                    (new_node, *target_node)
                } else {
                    (*target_node, new_node)
                };

                let splitter = DockSplitter::with_children(splitter_id, direction, first, second);
                self.root.set_child(DockNode::Splitter(splitter));
                return true;
            }
        }

        // 트리 내부 노드인 경우 - 부모 찾아서 처리
        Self::insert_split_recursive_static(
            &mut self.root.child,
            target_id,
            new_node,
            direction,
            new_is_first,
            &mut self.next_node_id,
        )
    }

    fn insert_split_recursive_static(
        node_opt: &mut Option<Box<DockNode>>,
        target_id: NodeId,
        new_node: DockNode,
        direction: SplitDirection,
        new_is_first: bool,
        next_id: &mut u64,
    ) -> bool {
        let node = match node_opt {
            Some(n) => n,
            None => return false,
        };

        if let DockNode::Splitter(splitter) = node.as_mut() {
            // 자식 중에 타겟이 있는지 확인
            for i in 0..splitter.children.len() {
                if splitter.children[i].id() == target_id {
                    let target_node = splitter.children.remove(i);
                    let splitter_id = NodeId::new(*next_id);
                    *next_id += 1;

                    let (first, second) = if new_is_first {
                        (new_node, target_node)
                    } else {
                        (target_node, new_node)
                    };

                    let new_splitter = DockSplitter::with_children(splitter_id, direction, first, second);
                    splitter.children.insert(i, DockNode::Splitter(new_splitter));

                    // 비율 유지
                    if splitter.ratios.len() < splitter.children.len() {
                        splitter.ratios.push(0.5);
                        splitter.normalize_ratios();
                    }

                    return true;
                }
            }

            // 자식 노드들 재귀 탐색
            for child in &mut splitter.children {
                if let DockNode::Splitter(_) = child {
                    let mut child_opt = Some(Box::new(std::mem::take(child)));
                    if Self::insert_split_recursive_static(
                        &mut child_opt,
                        target_id,
                        new_node.clone(),
                        direction,
                        new_is_first,
                        next_id,
                    ) {
                        if let Some(c) = child_opt {
                            *child = *c;
                        }
                        return true;
                    }
                    if let Some(c) = child_opt {
                        *child = *c;
                    }
                }
            }
        }
        false
    }

    /// 빈 탭 스택 제거
    fn remove_empty_stack(&mut self, stack_id: NodeId) {
        // 루트 자식이 해당 스택인 경우
        if let Some(child) = &self.root.child {
            if child.id() == stack_id {
                self.root.child = None;
                return;
            }
        }

        // 트리 내부에서 제거
        if let Some(child) = &mut self.root.child {
            Self::remove_node_recursive(child, stack_id);
        }

        // 트리 정리 (자식이 하나뿐인 스플리터 축소)
        self.collapse_single_child_splitters();
    }

    fn remove_node_recursive(node: &mut DockNode, target_id: NodeId) -> bool {
        if let DockNode::Splitter(splitter) = node {
            // 자식 중에 타겟이 있는지 확인
            if let Some(index) = splitter.children.iter().position(|c| c.id() == target_id) {
                splitter.children.remove(index);
                if index < splitter.ratios.len() {
                    splitter.ratios.remove(index);
                }
                splitter.normalize_ratios();
                return true;
            }

            // 재귀 탐색
            for child in &mut splitter.children {
                if Self::remove_node_recursive(child, target_id) {
                    return true;
                }
            }
        }
        false
    }

    /// 자식이 하나뿐인 스플리터 축소
    fn collapse_single_child_splitters(&mut self) {
        // 루트 자식 확인
        if let Some(child) = &mut self.root.child {
            if let DockNode::Splitter(splitter) = child.as_mut() {
                if splitter.children.len() == 1 {
                    let only_child = splitter.children.remove(0);
                    self.root.child = Some(Box::new(only_child));
                }
            }
        }

        // 재귀적으로 내부 스플리터도 정리
        if let Some(child) = &mut self.root.child {
            Self::collapse_recursive(child);
        }
    }

    fn collapse_recursive(node: &mut DockNode) {
        if let DockNode::Splitter(splitter) = node {
            // 먼저 자식들 정리
            for child in &mut splitter.children {
                Self::collapse_recursive(child);
            }

            // 자식 중에 자식이 하나뿐인 스플리터가 있으면 축소
            let mut i = 0;
            while i < splitter.children.len() {
                if let DockNode::Splitter(child_splitter) = &splitter.children[i] {
                    if child_splitter.children.len() == 1 {
                        let only_grandchild = if let DockNode::Splitter(s) = &mut splitter.children[i] {
                            s.children.remove(0)
                        } else {
                            i += 1;
                            continue;
                        };
                        splitter.children[i] = only_grandchild;
                        continue; // 다시 확인
                    }
                }
                i += 1;
            }
        }
    }

    /// 첫 번째 탭 스택 찾기
    fn find_first_tab_stack_mut(&mut self) -> Option<&mut DockTabStack> {
        Self::find_first_tab_stack_recursive(self.root.child.as_mut()?)
    }

    fn find_first_tab_stack_recursive(node: &mut DockNode) -> Option<&mut DockTabStack> {
        match node {
            DockNode::TabStack(stack) => Some(stack),
            DockNode::Splitter(splitter) => {
                for child in &mut splitter.children {
                    if let Some(stack) = Self::find_first_tab_stack_recursive(child) {
                        return Some(stack);
                    }
                }
                None
            }
            DockNode::Area(_) => None,
        }
    }

    /// ID로 탭 스택 찾기
    pub fn find_tab_stack(&self, id: NodeId) -> Option<&DockTabStack> {
        Self::find_tab_stack_recursive(self.root.child.as_ref()?, id)
    }

    fn find_tab_stack_recursive(node: &DockNode, id: NodeId) -> Option<&DockTabStack> {
        match node {
            DockNode::TabStack(stack) if stack.id == id => Some(stack),
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    if let Some(stack) = Self::find_tab_stack_recursive(child, id) {
                        return Some(stack);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// ID로 탭 스택 찾기 (mutable)
    pub fn find_tab_stack_mut(&mut self, id: NodeId) -> Option<&mut DockTabStack> {
        Self::find_tab_stack_mut_recursive(self.root.child.as_mut()?, id)
    }

    fn find_tab_stack_mut_recursive(node: &mut DockNode, id: NodeId) -> Option<&mut DockTabStack> {
        match node {
            DockNode::TabStack(stack) if stack.id == id => Some(stack),
            DockNode::Splitter(splitter) => {
                for child in &mut splitter.children {
                    if let Some(stack) = Self::find_tab_stack_mut_recursive(child, id) {
                        return Some(stack);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 탭이 속한 스택 ID 찾기
    pub fn find_tab_stack_containing(&self, tab_id: TabId) -> Option<NodeId> {
        Self::find_tab_stack_containing_recursive(self.root.child.as_ref()?, tab_id)
    }

    fn find_tab_stack_containing_recursive(node: &DockNode, tab_id: TabId) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => {
                if stack.tabs.contains(&tab_id) {
                    Some(stack.id)
                } else {
                    None
                }
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    if let Some(id) = Self::find_tab_stack_containing_recursive(child, tab_id) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 좌표로 탭 스택 찾기 (히트 테스트)
    pub fn find_tab_stack_at(&self, point: Vec2) -> Option<NodeId> {
        Self::find_tab_stack_at_recursive(self.root.child.as_ref()?, point)
    }

    fn find_tab_stack_at_recursive(node: &DockNode, point: Vec2) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => {
                if stack.rect.contains(point) {
                    Some(stack.id)
                } else {
                    None
                }
            }
            DockNode::Splitter(splitter) => {
                // 역순으로 검색 (나중에 그려진 것이 위에 있으므로)
                for child in splitter.children.iter().rev() {
                    if let Some(id) = Self::find_tab_stack_at_recursive(child, point) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 레이아웃 계산
    pub fn compute_layout(&mut self, available_rect: NodeRect) {
        self.last_layout_rect = Some(available_rect);
        self.root.rect = available_rect;
        if let Some(child) = &mut self.root.child {
            Self::compute_node_layout_static(
                child,
                available_rect,
                &self.tab_style,
                &self.splitter_style,
            );
        }
    }

    /// 마지막 레이아웃 rect로 레이아웃 재계산
    pub fn recompute_layout(&mut self) {
        if let Some(rect) = self.last_layout_rect {
            self.compute_layout(rect);
        }
    }

    fn compute_node_layout_static(
        node: &mut DockNode,
        rect: NodeRect,
        tab_style: &TabStackStyle,
        splitter_style: &SplitterStyle,
    ) {
        match node {
            DockNode::TabStack(stack) => {
                stack.rect = rect;
                stack.tab_bar_rect = NodeRect::new(
                    rect.position.x,
                    rect.position.y,
                    rect.size.x,
                    tab_style.tab_bar_height,
                );
                stack.content_rect = NodeRect::new(
                    rect.position.x,
                    rect.position.y + tab_style.tab_bar_height,
                    rect.size.x,
                    rect.size.y - tab_style.tab_bar_height,
                );
            }
            DockNode::Splitter(splitter) => {
                splitter.rect = rect;
                let mut offset = 0.0;
                let children_len = splitter.children.len();
                let ratios: Vec<f32> = splitter.ratios.clone();

                for (i, child) in splitter.children.iter_mut().enumerate() {
                    let ratio = ratios.get(i).copied().unwrap_or(0.5);
                    let is_last = i == children_len - 1;
                    let splitter_gap = if is_last { 0.0 } else { splitter_style.thickness };

                    let child_rect = match splitter.direction {
                        SplitDirection::Horizontal => {
                            let width = rect.size.x * ratio - splitter_gap;
                            let r = NodeRect::new(
                                rect.position.x + offset,
                                rect.position.y,
                                width,
                                rect.size.y,
                            );
                            offset += width + splitter_gap;
                            r
                        }
                        SplitDirection::Vertical => {
                            let height = rect.size.y * ratio - splitter_gap;
                            let r = NodeRect::new(
                                rect.position.x,
                                rect.position.y + offset,
                                rect.size.x,
                                height,
                            );
                            offset += height + splitter_gap;
                            r
                        }
                    };

                    Self::compute_node_layout_static(child, child_rect, tab_style, splitter_style);
                }
            }
            DockNode::Area(area) => {
                area.rect = rect;
            }
        }
    }

    /// 모든 탭 스택 순회
    pub fn for_each_tab_stack<F>(&self, mut f: F)
    where
        F: FnMut(&DockTabStack),
    {
        if let Some(child) = &self.root.child {
            Self::for_each_tab_stack_recursive(child, &mut f);
        }
    }

    fn for_each_tab_stack_recursive<F>(node: &DockNode, f: &mut F)
    where
        F: FnMut(&DockTabStack),
    {
        match node {
            DockNode::TabStack(stack) => f(stack),
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::for_each_tab_stack_recursive(child, f);
                }
            }
            _ => {}
        }
    }

    /// 모든 탭 스택 순회 (mutable)
    pub fn for_each_tab_stack_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut DockTabStack),
    {
        if let Some(child) = &mut self.root.child {
            Self::for_each_tab_stack_mut_recursive(child, &mut f);
        }
    }

    fn for_each_tab_stack_mut_recursive<F>(node: &mut DockNode, f: &mut F)
    where
        F: FnMut(&mut DockTabStack),
    {
        match node {
            DockNode::TabStack(stack) => f(stack),
            DockNode::Splitter(splitter) => {
                for child in &mut splitter.children {
                    Self::for_each_tab_stack_mut_recursive(child, f);
                }
            }
            _ => {}
        }
    }
}
