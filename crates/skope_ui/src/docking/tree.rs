//! 도킹 트리 관리
//!
//! 트리 구조 조작 및 레이아웃 계산

use super::{
    NodeId, TabId, SplitDirection, DockPosition, NodeRect,
    DockNode, DockArea, DockSplitter, DockTabStack,
    TabStackStyle, SplitterStyle,
    DockLayout, LayoutNode, TabLayoutInfo, TabState,
};
use glam::Vec2;
use serde::{Serialize, Deserialize};

/// 스플리터 핸들 정보 (렌더링 및 히트테스트용)
#[derive(Debug, Clone)]
pub struct SplitterHandleInfo {
    /// 스플리터 노드 ID
    pub splitter_id: NodeId,
    /// 조절 대상 자식 인덱스
    pub child_index: usize,
    /// 핸들 영역
    pub rect: NodeRect,
    /// 분할 방향
    pub direction: SplitDirection,
}

/// 도킹 트리
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// 배치 레이아웃 모드 (true일 때 recompute_layout 억제)
    #[serde(skip)]
    batch_layout: bool,
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
            batch_layout: false,
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

    /// 루트 영역 rect (area-level 도킹 타겟용)
    pub fn root_rect(&self) -> NodeRect {
        self.root.rect
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

    /// Area-level 루트 도킹 (UE SDockingTarget 외곽 4방향)
    ///
    /// 전체 트리를 감싸는 루트 레벨 분할 생성.
    /// Center → 기존 첫 스택에 병합, 방향 → 기존 루트를 스플리터로 감싸고 새 스택 삽입.
    pub fn dock_tab_at_root(&mut self, tab_id: TabId, position: DockPosition) -> bool {
        match position {
            DockPosition::Center => {
                // Center: 첫 스택에 병합
                if let Some(first_id) = self.first_tab_stack_id() {
                    return self.add_tab_to_stack(first_id, tab_id);
                }
                // 트리가 비어있으면 새 스택 추가
                self.add_tab(tab_id);
                return true;
            }
            _ => {
                let direction = match position.split_direction() {
                    Some(d) => d,
                    None => return false,
                };

                // 새 탭 스택 생성
                let new_stack_id = self.next_node_id();
                let new_stack = DockTabStack::with_tab(new_stack_id, tab_id);
                let new_node = DockNode::TabStack(new_stack);

                if self.root.child.is_none() {
                    // 빈 트리: 그냥 루트에 추가
                    self.root.set_child(new_node);
                    self.recompute_layout();
                    return true;
                }

                // 기존 루트 자식을 꺼내서 스플리터로 감싸기
                let existing = self.root.child.take().unwrap();
                let splitter_id = self.next_node_id();
                let (first, second) = if position.is_first_child() {
                    (new_node, *existing)
                } else {
                    (*existing, new_node)
                };
                let splitter = DockSplitter::with_children(splitter_id, direction, first, second);
                self.root.set_child(DockNode::Splitter(splitter));
                self.recompute_layout();
                return true;
            }
        }
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

        // 탭이 현재 있는 스택에서 먼저 제거
        if let Some(source_stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(source_stack_id) {
                stack.remove_tab(tab_id);
            }
        }

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

    /// 모든 TabStack을 DFS 순서로 수집
    pub fn collect_all_tab_stacks(&self) -> Vec<NodeId> {
        let mut stacks = Vec::new();
        if let Some(child) = &self.root.child {
            Self::collect_tab_stacks_recursive(child, &mut stacks);
        }
        stacks
    }

    fn collect_tab_stacks_recursive(node: &DockNode, stacks: &mut Vec<NodeId>) {
        match node {
            DockNode::TabStack(stack) => {
                if !stack.is_empty() {
                    stacks.push(stack.id);
                }
            }
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    Self::collect_tab_stacks_recursive(child, stacks);
                }
            }
            DockNode::Area(area) => {
                if let Some(child) = &area.child {
                    Self::collect_tab_stacks_recursive(child, stacks);
                }
            }
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

    /// 탭 ID로 활성화 (해당 탭이 속한 스택에서 활성 탭으로 설정)
    pub fn activate_tab(&mut self, tab_id: TabId) -> bool {
        if let Some(stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                return stack.activate_tab_by_id(tab_id);
            }
        }
        false
    }

    /// 탭이 속한 스택의 hide_tab_well 설정 (UE SetTabWellHidden)
    pub fn set_hide_tab_well(&mut self, tab_id: TabId, hide: bool) -> bool {
        if let Some(stack_id) = self.find_tab_stack_containing(tab_id) {
            if let Some(stack) = self.find_tab_stack_mut(stack_id) {
                stack.hide_tab_well = hide;
                self.recompute_layout();
                return true;
            }
        }
        false
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

    /// 첫 번째 탭 스택의 ID 반환 (깊이 우선 탐색)
    pub fn first_tab_stack_id(&self) -> Option<NodeId> {
        Self::first_tab_stack_recursive(self.root.child.as_ref()?)
    }

    fn first_tab_stack_recursive(node: &DockNode) -> Option<NodeId> {
        match node {
            DockNode::TabStack(stack) => Some(stack.id),
            DockNode::Splitter(splitter) => {
                for child in &splitter.children {
                    if let Some(id) = Self::first_tab_stack_recursive(child) {
                        return Some(id);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// 좌표로 스플리터 핸들 찾기 (히트 테스트)
    /// 반환값: (스플리터 ID, 자식 인덱스, 핸들 rect)
    pub fn find_splitter_handle_at(&self, point: Vec2) -> Option<(NodeId, usize, NodeRect)> {
        Self::find_splitter_handle_recursive(
            self.root.child.as_ref()?,
            point,
            self.splitter_style.hit_area,
        )
    }

    fn find_splitter_handle_recursive(
        node: &DockNode,
        point: Vec2,
        hit_area: f32,
    ) -> Option<(NodeId, usize, NodeRect)> {
        if let DockNode::Splitter(splitter) = node {
            // 스플리터 핸들 영역 체크 (각 자식 사이의 간격)
            let children_len = splitter.children.len();
            if children_len > 1 {
                let _offset = 0.0;
                for (i, child) in splitter.children.iter().enumerate() {
                    // 현재 자식의 크기
                    let child_rect = match child {
                        DockNode::TabStack(s) => &s.rect,
                        DockNode::Splitter(s) => &s.rect,
                        DockNode::Area(a) => &a.rect,
                    };

                    // 마지막 자식이 아니면 핸들 영역 체크
                    if i < children_len - 1 {
                        let handle_rect = match splitter.direction {
                            SplitDirection::Horizontal => {
                                // 가로 분할: 자식 오른쪽 끝에 수직 핸들
                                let x = child_rect.position.x + child_rect.size.x;
                                NodeRect::new(
                                    x - hit_area / 2.0,
                                    child_rect.position.y,
                                    hit_area,
                                    child_rect.size.y,
                                )
                            }
                            SplitDirection::Vertical => {
                                // 세로 분할: 자식 아래쪽 끝에 수평 핸들
                                let y = child_rect.position.y + child_rect.size.y;
                                NodeRect::new(
                                    child_rect.position.x,
                                    y - hit_area / 2.0,
                                    child_rect.size.x,
                                    hit_area,
                                )
                            }
                        };

                        if handle_rect.contains(point) {
                            return Some((splitter.id, i, handle_rect));
                        }
                    }
                }
            }

            // 자식 스플리터 재귀 탐색
            for child in &splitter.children {
                if let Some(result) = Self::find_splitter_handle_recursive(child, point, hit_area) {
                    return Some(result);
                }
            }
        }
        None
    }

    /// ID로 스플리터 찾기
    pub fn find_splitter(&self, id: NodeId) -> Option<&DockSplitter> {
        Self::find_splitter_recursive(self.root.child.as_ref()?, id)
    }

    fn find_splitter_recursive(node: &DockNode, id: NodeId) -> Option<&DockSplitter> {
        match node {
            DockNode::Splitter(splitter) => {
                if splitter.id == id {
                    return Some(splitter);
                }
                for child in &splitter.children {
                    if let Some(s) = Self::find_splitter_recursive(child, id) {
                        return Some(s);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// ID로 스플리터 찾기 (mutable)
    pub fn find_splitter_mut(&mut self, id: NodeId) -> Option<&mut DockSplitter> {
        Self::find_splitter_mut_recursive(self.root.child.as_mut()?, id)
    }

    fn find_splitter_mut_recursive(node: &mut DockNode, id: NodeId) -> Option<&mut DockSplitter> {
        // ID 일치 확인 (첫 번째 borrow scope)
        let is_match = matches!(node, DockNode::Splitter(s) if s.id == id);
        if is_match {
            if let DockNode::Splitter(s) = node {
                return Some(s);
            }
        }
        // 자식 재귀 탐색 (별도 borrow scope)
        if let DockNode::Splitter(s) = node {
            for child in &mut s.children {
                if let Some(found) = Self::find_splitter_mut_recursive(child, id) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// 스플리터 비율 조정
    pub fn adjust_splitter(&mut self, splitter_id: NodeId, child_index: usize, delta: f32) {
        if let Some(child) = &mut self.root.child {
            Self::adjust_splitter_recursive(child, splitter_id, child_index, delta);
            // 레이아웃 재계산
            self.recompute_layout();
        }
    }

    fn adjust_splitter_recursive(
        node: &mut DockNode,
        splitter_id: NodeId,
        child_index: usize,
        delta: f32,
    ) -> bool {
        if let DockNode::Splitter(splitter) = node {
            if splitter.id == splitter_id {
                splitter.adjust_split(child_index, delta);
                return true;
            }
            for child in &mut splitter.children {
                if Self::adjust_splitter_recursive(child, splitter_id, child_index, delta) {
                    return true;
                }
            }
        }
        false
    }

    /// 모든 스플리터 핸들 정보 수집 (렌더링용)
    pub fn collect_splitter_handles(&self) -> Vec<SplitterHandleInfo> {
        let mut handles = Vec::new();
        if let Some(child) = &self.root.child {
            Self::collect_handles_recursive(child, &self.splitter_style, &mut handles);
        }
        handles
    }

    fn collect_handles_recursive(
        node: &DockNode,
        style: &SplitterStyle,
        handles: &mut Vec<SplitterHandleInfo>,
    ) {
        if let DockNode::Splitter(splitter) = node {
            let children_len = splitter.children.len();
            if children_len > 1 {
                for (i, child) in splitter.children.iter().enumerate() {
                    if i < children_len - 1 {
                        let child_rect = match child {
                            DockNode::TabStack(s) => &s.rect,
                            DockNode::Splitter(s) => &s.rect,
                            DockNode::Area(a) => &a.rect,
                        };

                        let handle_rect = match splitter.direction {
                            SplitDirection::Horizontal => {
                                let x = child_rect.position.x + child_rect.size.x;
                                NodeRect::new(
                                    x - style.thickness / 2.0,
                                    child_rect.position.y,
                                    style.thickness,
                                    child_rect.size.y,
                                )
                            }
                            SplitDirection::Vertical => {
                                let y = child_rect.position.y + child_rect.size.y;
                                NodeRect::new(
                                    child_rect.position.x,
                                    y - style.thickness / 2.0,
                                    child_rect.size.x,
                                    style.thickness,
                                )
                            }
                        };

                        handles.push(SplitterHandleInfo {
                            splitter_id: splitter.id,
                            child_index: i,
                            rect: handle_rect,
                            direction: splitter.direction,
                        });
                    }
                }
            }

            // 재귀
            for child in &splitter.children {
                Self::collect_handles_recursive(child, style, handles);
            }
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

    /// 배치 레이아웃 모드 시작 (중간 recompute_layout 억제)
    pub fn begin_batch_layout(&mut self) {
        self.batch_layout = true;
    }

    /// 배치 레이아웃 모드 종료 + 한 번 레이아웃 재계산
    pub fn end_batch_layout(&mut self) {
        self.batch_layout = false;
        self.recompute_layout();
    }

    /// 마지막 레이아웃 rect로 레이아웃 재계산
    pub fn recompute_layout(&mut self) {
        if self.batch_layout {
            return;
        }
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
                // 애니메이션된 탭바 높이 (0 ~ tab_bar_height)
                let anim_bar_h = tab_style.tab_bar_height * stack.tab_well_anim_t;
                if anim_bar_h < 0.5 {
                    // 탭 바 숨김: 콘텐츠가 전체 영역 사용
                    stack.tab_bar_rect = NodeRect::new(
                        rect.position.x,
                        rect.position.y,
                        rect.size.x,
                        0.0,
                    );
                    stack.content_rect = rect;
                } else {
                    stack.tab_bar_rect = NodeRect::new(
                        rect.position.x,
                        rect.position.y,
                        rect.size.x,
                        anim_bar_h,
                    );
                    stack.content_rect = NodeRect::new(
                        rect.position.x,
                        rect.position.y + anim_bar_h,
                        rect.size.x,
                        rect.size.y - anim_bar_h,
                    );
                }
                stack.compute_tab_widths(rect.size.x, tab_style);
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
            DockNode::TabStack(stack) => {
                f(stack);
            }
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

    // ========================================================================
    // 레이아웃 저장/복원 (언리얼 FTabManager::FLayout)
    // ========================================================================

    /// 현재 레이아웃 저장 (언리얼 ToJson)
    ///
    /// `tab_name_fn`: 탭 ID를 이름으로 변환하는 함수
    pub fn save_layout<F>(&self, name: impl Into<String>, tab_name_fn: F) -> DockLayout
    where
        F: Fn(TabId) -> Option<String>,
    {
        let mut layout = DockLayout::new(name);

        // 루트 노드 저장
        if let Some(child) = &self.root.child {
            layout.root = Some(Self::save_node_recursive(child, &tab_name_fn));
        }

        // 탭 이름 매핑 저장
        self.for_each_tab_stack(|stack| {
            for &tab_id in &stack.tabs {
                if let Some(name) = tab_name_fn(tab_id) {
                    layout.tab_names.insert(tab_id.0, name);
                }
            }
        });

        layout
    }

    fn save_node_recursive<F>(node: &DockNode, tab_name_fn: &F) -> LayoutNode
    where
        F: Fn(TabId) -> Option<String>,
    {
        match node {
            DockNode::TabStack(stack) => {
                let tabs: Vec<TabLayoutInfo> = stack.tabs
                    .iter()
                    .map(|&tab_id| {
                        let name = tab_name_fn(tab_id).unwrap_or_else(|| format!("Tab_{}", tab_id.0));
                        TabLayoutInfo::new(tab_id, name)
                    })
                    .collect();

                let mut node = LayoutNode::new_stack(
                    stack.id,
                    tabs,
                    stack.active_tab,
                    1.0, // 기본 coefficient (스플리터에서 덮어씀)
                );
                // hide_tab_well 플래그 보존
                if let LayoutNode::Stack { ref mut hide_tab_well, .. } = node {
                    *hide_tab_well = stack.hide_tab_well;
                }
                node
            }
            DockNode::Splitter(splitter) => {
                let nodes: Vec<LayoutNode> = splitter.children
                    .iter()
                    .map(|child| Self::save_node_recursive(child, tab_name_fn))
                    .collect();

                LayoutNode::new_splitter(
                    splitter.id,
                    splitter.direction,
                    nodes,
                    splitter.ratios.clone(),
                )
            }
            DockNode::Area(area) => {
                // Area는 루트에서만 사용되므로 여기서는 빈 스택으로 처리
                LayoutNode::new_stack(area.id, vec![], 0, 1.0)
            }
        }
    }

    /// 레이아웃 복원 (언리얼 NewFromJson)
    ///
    /// `tab_restore_fn`: 탭 이름으로 새 탭 ID를 생성하는 함수
    ///
    /// 반환값: 복원 실패한 탭 이름 목록
    pub fn restore_layout<F>(&mut self, layout: &DockLayout, tab_restore_fn: &mut F) -> Vec<String>
    where
        F: FnMut(&str) -> Option<TabId>,
    {
        let mut failed_tabs = Vec::new();

        // 기존 구조 초기화
        self.root.child = None;

        // 레이아웃 복원
        if let Some(ref layout_node) = layout.root {
            self.root.child = Self::restore_node_recursive(
                layout_node,
                tab_restore_fn,
                &mut self.next_node_id,
                &mut failed_tabs,
            ).map(Box::new);
        }

        // 레이아웃 재계산
        self.recompute_layout();

        failed_tabs
    }

    fn restore_node_recursive<F>(
        layout_node: &LayoutNode,
        tab_restore_fn: &mut F,
        next_id: &mut u64,
        failed_tabs: &mut Vec<String>,
    ) -> Option<DockNode>
    where
        F: FnMut(&str) -> Option<TabId>,
    {
        match layout_node {
            LayoutNode::Stack { tabs, active_tab, hide_tab_well, .. } => {
                let node_id = NodeId::new(*next_id);
                *next_id += 1;

                let mut stack = DockTabStack::new(node_id);
                stack.hide_tab_well = *hide_tab_well;

                for tab_info in tabs {
                    // 닫힌 탭은 복원하지 않음
                    if tab_info.state == TabState::Closed {
                        continue;
                    }

                    if let Some(tab_id) = tab_restore_fn(&tab_info.tab_name) {
                        stack.add_tab(tab_id);
                    } else {
                        failed_tabs.push(tab_info.tab_name.clone());
                    }
                }

                // 빈 스택은 생성하지 않음
                if stack.is_empty() {
                    return None;
                }

                // 활성 탭 복원
                stack.active_tab = (*active_tab).min(stack.tabs.len().saturating_sub(1));

                Some(DockNode::TabStack(stack))
            }
            LayoutNode::Splitter { orientation, nodes, coefficients, .. } => {
                let node_id = NodeId::new(*next_id);
                *next_id += 1;

                let mut splitter = DockSplitter::new(node_id, *orientation);

                // 자식 노드 복원
                for (i, child_layout) in nodes.iter().enumerate() {
                    if let Some(child_node) = Self::restore_node_recursive(
                        child_layout,
                        tab_restore_fn,
                        next_id,
                        failed_tabs,
                    ) {
                        let ratio = coefficients.get(i).copied().unwrap_or(0.5);
                        splitter.children.push(child_node);
                        splitter.ratios.push(ratio);
                    }
                }

                // 자식이 없거나 하나만 있으면 처리
                match splitter.children.len() {
                    0 => None,
                    1 => Some(splitter.children.remove(0)),
                    _ => {
                        splitter.normalize_ratios();
                        Some(DockNode::Splitter(splitter))
                    }
                }
            }
        }
    }

    /// JSON으로 레이아웃 저장
    pub fn save_layout_json<F>(&self, name: impl Into<String>, tab_name_fn: F) -> Result<String, serde_json::Error>
    where
        F: Fn(TabId) -> Option<String>,
    {
        let layout = self.save_layout(name, tab_name_fn);
        layout.to_json()
    }

    /// JSON에서 레이아웃 복원
    pub fn restore_layout_json<F>(&mut self, json: &str, mut tab_restore_fn: F) -> Result<Vec<String>, serde_json::Error>
    where
        F: FnMut(&str) -> Option<TabId>,
    {
        let layout = DockLayout::from_json(json)?;
        Ok(self.restore_layout(&layout, &mut tab_restore_fn))
    }
}
