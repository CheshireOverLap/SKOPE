//! ManagedAttribute<T> — 위젯 속성 자동 관리 시스템
//!
//! 위젯 속성의 변경 감지 및 자동 무효화를 제공합니다.
//! UE Slate의 TAttribute<T>에 해당합니다.

use std::sync::Arc;

/// 속성 바인딩 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeBindingMode {
    /// 직접 값 설정
    Direct,
    /// 바인딩 함수를 통한 동적 값
    Bound,
    /// 캐시된 바인딩 (마지막 값 저장)
    CachedBound,
}

/// 속성 변경 콜백
pub type AttributeChangedCallback<T> = Arc<dyn Fn(&T, &T) + Send + Sync>;

/// 관리형 위젯 속성
///
/// 값의 변경을 추적하고, 바인딩 함수를 통한 동적 값 계산을 지원합니다.
pub struct ManagedAttribute<T: Clone + PartialEq + Send + Sync + 'static> {
    value: T,
    binding: Option<Arc<dyn Fn() -> T + Send + Sync>>,
    mode: AttributeBindingMode,
    generation: u64,
    is_dirty: bool,
    on_changed: Option<AttributeChangedCallback<T>>,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> ManagedAttribute<T> {
    /// 직접 값으로 생성
    pub fn new(value: T) -> Self {
        Self {
            value,
            binding: None,
            mode: AttributeBindingMode::Direct,
            generation: 0,
            is_dirty: true,
            on_changed: None,
        }
    }

    /// 바인딩 함수로 생성
    pub fn bound(binding: impl Fn() -> T + Send + Sync + 'static) -> Self {
        let value = binding();
        Self {
            value,
            binding: Some(Arc::new(binding)),
            mode: AttributeBindingMode::Bound,
            generation: 0,
            is_dirty: true,
            on_changed: None,
        }
    }

    /// 캐시된 바인딩으로 생성
    pub fn cached_bound(binding: impl Fn() -> T + Send + Sync + 'static) -> Self {
        let value = binding();
        Self {
            value,
            binding: Some(Arc::new(binding)),
            mode: AttributeBindingMode::CachedBound,
            generation: 0,
            is_dirty: true,
            on_changed: None,
        }
    }

    /// 현재 값 참조
    pub fn get(&self) -> &T {
        &self.value
    }

    /// 값 직접 설정
    pub fn set(&mut self, new_value: T) {
        if self.value != new_value {
            let old = self.value.clone();
            self.value = new_value;
            self.generation += 1;
            self.is_dirty = true;
            self.binding = None;
            self.mode = AttributeBindingMode::Direct;
            if let Some(ref cb) = self.on_changed {
                cb(&old, &self.value);
            }
        }
    }

    /// 바인딩 설정
    pub fn bind(&mut self, binding: impl Fn() -> T + Send + Sync + 'static) {
        self.binding = Some(Arc::new(binding));
        self.mode = AttributeBindingMode::Bound;
        self.is_dirty = true;
    }

    /// 바인딩 해제 (현재 값 유지)
    pub fn unbind(&mut self) {
        self.binding = None;
        self.mode = AttributeBindingMode::Direct;
    }

    /// 바인딩 업데이트 (매 프레임 호출)
    /// 값이 변경되었으면 true 반환
    pub fn update(&mut self) -> bool {
        if let Some(ref binding) = self.binding {
            let new_value = binding();
            if self.value != new_value {
                let old = self.value.clone();
                self.value = new_value;
                self.generation += 1;
                self.is_dirty = true;
                if let Some(ref cb) = self.on_changed {
                    cb(&old, &self.value);
                }
                return true;
            }
        }
        false
    }

    /// 변경 콜백 설정
    pub fn on_changed(&mut self, callback: impl Fn(&T, &T) + Send + Sync + 'static) {
        self.on_changed = Some(Arc::new(callback));
    }

    /// dirty 플래그 확인 및 클리어
    pub fn take_dirty(&mut self) -> bool {
        let was_dirty = self.is_dirty;
        self.is_dirty = false;
        was_dirty
    }

    pub fn is_dirty(&self) -> bool { self.is_dirty }
    pub fn mode(&self) -> AttributeBindingMode { self.mode }
    pub fn generation(&self) -> u64 { self.generation }
    pub fn is_bound(&self) -> bool { self.binding.is_some() }
}

impl<T: Clone + PartialEq + Send + Sync + Default + 'static> Default for ManagedAttribute<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// 속성 업데이트 순서 관리
///
/// 위젯 속성들의 업데이트 순서를 보장합니다.
/// 의존성 기반으로 순서를 결정하여 한 프레임 내에서
/// 모든 바인딩이 올바른 순서로 평가됩니다.
pub struct AttributeUpdateOrder {
    /// (속성ID, 의존하는 속성ID들)
    dependencies: Vec<(u64, Vec<u64>)>,
    /// 정렬된 업데이트 순서
    sorted_order: Vec<u64>,
    is_sorted: bool,
}

impl AttributeUpdateOrder {
    pub fn new() -> Self {
        Self {
            dependencies: Vec::new(),
            sorted_order: Vec::new(),
            is_sorted: false,
        }
    }

    /// 속성 의존성 등록
    pub fn register(&mut self, attr_id: u64, depends_on: Vec<u64>) {
        self.dependencies.push((attr_id, depends_on));
        self.is_sorted = false;
    }

    /// 속성 제거
    pub fn unregister(&mut self, attr_id: u64) {
        self.dependencies.retain(|(id, _)| *id != attr_id);
        self.is_sorted = false;
    }

    /// 위상 정렬로 업데이트 순서 계산
    pub fn compute_order(&mut self) {
        if self.is_sorted { return; }

        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut visiting = std::collections::HashSet::new();

        // 의존성 맵 구축
        let dep_map: std::collections::HashMap<u64, &Vec<u64>> = self.dependencies.iter()
            .map(|(id, deps)| (*id, deps))
            .collect();

        for (id, _) in &self.dependencies {
            self.topo_visit(*id, &dep_map, &mut visited, &mut visiting, &mut order);
        }

        self.sorted_order = order;
        self.is_sorted = true;
    }

    fn topo_visit(
        &self,
        node: u64,
        deps: &std::collections::HashMap<u64, &Vec<u64>>,
        visited: &mut std::collections::HashSet<u64>,
        visiting: &mut std::collections::HashSet<u64>,
        order: &mut Vec<u64>,
    ) {
        if visited.contains(&node) { return; }
        if visiting.contains(&node) { return; } // 순환 의존성 방지

        visiting.insert(node);
        if let Some(node_deps) = deps.get(&node) {
            for &dep in *node_deps {
                self.topo_visit(dep, deps, visited, visiting, order);
            }
        }
        visiting.remove(&node);
        visited.insert(node);
        order.push(node);
    }

    /// 정렬된 업데이트 순서 반환
    pub fn order(&self) -> &[u64] {
        &self.sorted_order
    }

    pub fn is_sorted(&self) -> bool { self.is_sorted }
    pub fn count(&self) -> usize { self.dependencies.len() }
}

/// 속성 그룹 — 여러 속성을 묶어 일괄 업데이트
pub struct AttributeGroup {
    name: String,
    attribute_ids: Vec<u64>,
    is_enabled: bool,
}

impl AttributeGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attribute_ids: Vec::new(),
            is_enabled: true,
        }
    }

    pub fn add(&mut self, attr_id: u64) {
        if !self.attribute_ids.contains(&attr_id) {
            self.attribute_ids.push(attr_id);
        }
    }

    pub fn remove(&mut self, attr_id: u64) {
        self.attribute_ids.retain(|&id| id != attr_id);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
    }

    pub fn name(&self) -> &str { &self.name }
    pub fn attribute_ids(&self) -> &[u64] { &self.attribute_ids }
    pub fn is_enabled(&self) -> bool { self.is_enabled }
    pub fn count(&self) -> usize { self.attribute_ids.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_managed_attribute_direct() {
        let mut attr = ManagedAttribute::new(42);
        assert_eq!(*attr.get(), 42);
        assert_eq!(attr.mode(), AttributeBindingMode::Direct);
        attr.set(100);
        assert_eq!(*attr.get(), 100);
        assert_eq!(attr.generation(), 1);
    }

    #[test]
    fn test_managed_attribute_no_change() {
        let mut attr = ManagedAttribute::new(42);
        attr.set(42); // 동일 값
        assert_eq!(attr.generation(), 0); // 변경 없음
    }

    #[test]
    fn test_managed_attribute_bound() {
        let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let c = counter.clone();
        let mut attr = ManagedAttribute::bound(move || {
            c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            c.load(std::sync::atomic::Ordering::SeqCst)
        });
        assert_eq!(attr.mode(), AttributeBindingMode::Bound);
        // 초기값은 binding 호출 결과
        assert_eq!(*attr.get(), 1);
        // update 호출 시 바인딩 재평가
        assert!(attr.update());
        assert_eq!(*attr.get(), 2);
    }

    #[test]
    fn test_managed_attribute_dirty() {
        let mut attr = ManagedAttribute::new(0);
        assert!(attr.take_dirty()); // 초기 dirty
        assert!(!attr.take_dirty()); // 이미 클리어
        attr.set(1);
        assert!(attr.is_dirty());
        assert!(attr.take_dirty());
    }

    #[test]
    fn test_managed_attribute_unbind() {
        let mut attr = ManagedAttribute::bound(|| 42);
        assert!(attr.is_bound());
        attr.unbind();
        assert!(!attr.is_bound());
        assert_eq!(attr.mode(), AttributeBindingMode::Direct);
    }

    #[test]
    fn test_attribute_update_order() {
        let mut order = AttributeUpdateOrder::new();
        // B depends on A, C depends on B
        order.register(1, vec![]); // A
        order.register(2, vec![1]); // B -> A
        order.register(3, vec![2]); // C -> B
        order.compute_order();
        let sorted = order.order();
        assert_eq!(sorted.len(), 3);
        // A before B, B before C
        let pos_a = sorted.iter().position(|&x| x == 1).unwrap();
        let pos_b = sorted.iter().position(|&x| x == 2).unwrap();
        let pos_c = sorted.iter().position(|&x| x == 3).unwrap();
        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_attribute_update_order_diamond() {
        let mut order = AttributeUpdateOrder::new();
        // D depends on B,C; B,C depend on A
        order.register(1, vec![]); // A
        order.register(2, vec![1]); // B -> A
        order.register(3, vec![1]); // C -> A
        order.register(4, vec![2, 3]); // D -> B, C
        order.compute_order();
        let sorted = order.order();
        assert_eq!(sorted.len(), 4);
        let pos_a = sorted.iter().position(|&x| x == 1).unwrap();
        let pos_d = sorted.iter().position(|&x| x == 4).unwrap();
        assert!(pos_a < pos_d);
    }

    #[test]
    fn test_attribute_group() {
        let mut group = AttributeGroup::new("transform");
        group.add(1);
        group.add(2);
        group.add(3);
        assert_eq!(group.count(), 3);
        group.add(2); // 중복 무시
        assert_eq!(group.count(), 3);
        group.remove(2);
        assert_eq!(group.count(), 2);
        assert!(group.is_enabled());
    }

    #[test]
    fn test_managed_attribute_default() {
        let attr: ManagedAttribute<i32> = ManagedAttribute::default();
        assert_eq!(*attr.get(), 0);
    }
}
