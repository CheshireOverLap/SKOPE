//! FCombinedChildren — 다중 자식 컬렉션 합성
//!
//! 여러 Vec<Box<dyn Widget>> 컬렉션을 하나의 연속적인 자식 뷰로 합성합니다.
//! 인덱싱과 반복을 투명하게 지원합니다.

use super::Widget;

// ============================================================================
// CombinedChildren
// ============================================================================

/// 다중 자식 컬렉션을 하나로 합성하는 뷰
///
/// 여러 `Vec<Box<dyn Widget>>` 참조를 받아 하나의 연속적인 인덱스 공간으로 제공합니다.
/// 위젯 슬롯이 여러 종류일 때 (예: 헤더 + 콘텐츠 + 푸터) 통합 순회에 사용합니다.
pub struct CombinedChildren<'a> {
    collections: Vec<&'a [Box<dyn Widget>]>,
    total_count: usize,
}

impl<'a> CombinedChildren<'a> {
    /// 빈 CombinedChildren 생성
    pub fn new() -> Self {
        Self {
            collections: Vec::new(),
            total_count: 0,
        }
    }

    /// 자식 컬렉션 추가
    pub fn add(&mut self, children: &'a [Box<dyn Widget>]) {
        self.total_count += children.len();
        self.collections.push(children);
    }

    /// 전체 자식 수
    pub fn len(&self) -> usize {
        self.total_count
    }

    /// 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.total_count == 0
    }

    /// 인덱스로 자식 접근
    pub fn get(&self, index: usize) -> Option<&dyn Widget> {
        let mut remaining = index;
        for collection in &self.collections {
            if remaining < collection.len() {
                return Some(collection[remaining].as_ref());
            }
            remaining -= collection.len();
        }
        None
    }

    /// 모든 자식을 순회하는 반복자
    pub fn iter(&self) -> CombinedChildrenIter<'_> {
        CombinedChildrenIter {
            combined: self,
            current_collection: 0,
            current_index: 0,
        }
    }
}

impl<'a> Default for CombinedChildren<'a> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CombinedChildrenIter
// ============================================================================

/// CombinedChildren 반복자
pub struct CombinedChildrenIter<'a> {
    combined: &'a CombinedChildren<'a>,
    current_collection: usize,
    current_index: usize,
}

impl<'a> Iterator for CombinedChildrenIter<'a> {
    type Item = &'a dyn Widget;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_collection < self.combined.collections.len() {
            let collection = self.combined.collections[self.current_collection];
            if self.current_index < collection.len() {
                let item = collection[self.current_index].as_ref();
                self.current_index += 1;
                return Some(item);
            }
            self.current_collection += 1;
            self.current_index = 0;
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.combined.total_count
            - self.combined.collections[..self.current_collection]
                .iter()
                .map(|c| c.len())
                .sum::<usize>()
            - self.current_index;
        (remaining, Some(remaining))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widget::SSpacer;

    fn make_children(count: usize) -> Vec<Box<dyn Widget>> {
        (0..count)
            .map(|_| Box::new(SSpacer::new().size(10.0, 10.0).build()) as Box<dyn Widget>)
            .collect()
    }

    #[test]
    fn test_combined_children_empty() {
        let combined = CombinedChildren::new();
        assert_eq!(combined.len(), 0);
        assert!(combined.is_empty());
        assert!(combined.get(0).is_none());
    }

    #[test]
    fn test_combined_children_single() {
        let children = make_children(3);
        let mut combined = CombinedChildren::new();
        combined.add(&children);
        assert_eq!(combined.len(), 3);
        assert!(combined.get(0).is_some());
        assert!(combined.get(2).is_some());
        assert!(combined.get(3).is_none());
    }

    #[test]
    fn test_combined_children_multiple() {
        let a = make_children(2);
        let b = make_children(3);
        let mut combined = CombinedChildren::new();
        combined.add(&a);
        combined.add(&b);
        assert_eq!(combined.len(), 5);
        assert!(combined.get(4).is_some());
        assert!(combined.get(5).is_none());
    }

    #[test]
    fn test_combined_children_iter() {
        let a = make_children(2);
        let b = make_children(3);
        let mut combined = CombinedChildren::new();
        combined.add(&a);
        combined.add(&b);
        let count = combined.iter().count();
        assert_eq!(count, 5);
    }
}
