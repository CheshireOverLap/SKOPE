//! ElementBatcher — 드로우 엘리먼트 배칭 시스템
//!
//! 동일한 셰이더/텍스처/레이어를 사용하는 드로우 엘리먼트를 병합하여
//! GPU 드로우 콜 수를 최소화합니다.

use std::collections::HashMap;

/// 배치 키 — 동일한 키를 가진 엘리먼트끼리 합침
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BatchKey {
    pub layer: u32,
    pub shader_type: ShaderType,
    pub texture_id: u32,
    pub draw_effects: DrawEffects,
}

/// 셰이더 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderType {
    Default,
    Font,
    RoundedBox,
    Line,
    Spline,
    PostProcess,
    Custom(u32),
}

/// 드로우 이펙트 비트마스크
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DrawEffects(u32);

impl DrawEffects {
    pub const NONE: Self = Self(0);
    pub const GAMMA_CORRECT: Self = Self(1 << 0);
    pub const DISABLE_EFFECT: Self = Self(1 << 1);
    pub const IGNORE_TEXTURE_ALPHA: Self = Self(1 << 2);
    pub const PREMULTIPLIED_ALPHA: Self = Self(1 << 3);
    pub const NO_BLENDING: Self = Self(1 << 4);
    pub const WIREFRAME: Self = Self(1 << 5);
    pub const PIXEL_SNAPPING: Self = Self(1 << 6);

    pub fn contains(&self, other: Self) -> bool { self.0 & other.0 == other.0 }
    pub fn with(self, other: Self) -> Self { Self(self.0 | other.0) }
}

impl std::ops::BitOr for DrawEffects {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}

/// 배치된 엘리먼트 그룹
#[derive(Debug)]
pub struct ElementBatch {
    pub key: BatchKey,
    pub element_indices: Vec<usize>,
    pub vertex_count: usize,
    pub index_count: usize,
}

impl ElementBatch {
    pub fn new(key: BatchKey) -> Self {
        Self {
            key,
            element_indices: Vec::new(),
            vertex_count: 0,
            index_count: 0,
        }
    }

    pub fn add_element(&mut self, element_index: usize, vertices: usize, indices: usize) {
        self.element_indices.push(element_index);
        self.vertex_count += vertices;
        self.index_count += indices;
    }

    pub fn is_empty(&self) -> bool { self.element_indices.is_empty() }
}

/// 엘리먼트 배처
pub struct ElementBatcher {
    batches: Vec<ElementBatch>,
    batch_map: HashMap<BatchKey, usize>,
    total_vertices: usize,
    total_indices: usize,
    total_draw_calls: usize,
    max_batch_size: usize,
}

impl ElementBatcher {
    pub fn new() -> Self {
        Self {
            batches: Vec::new(),
            batch_map: HashMap::new(),
            total_vertices: 0,
            total_indices: 0,
            total_draw_calls: 0,
            max_batch_size: 65536, // 64K vertices per batch
        }
    }

    pub fn with_max_batch_size(mut self, size: usize) -> Self {
        self.max_batch_size = size;
        self
    }

    pub fn begin_frame(&mut self) {
        self.batches.clear();
        self.batch_map.clear();
        self.total_vertices = 0;
        self.total_indices = 0;
        self.total_draw_calls = 0;
    }

    pub fn add_element(&mut self, key: BatchKey, element_index: usize,
                       vertex_count: usize, index_count: usize) {
        let batch_idx = if let Some(&idx) = self.batch_map.get(&key) {
            // 기존 배치에 추가 가능한지 체크
            if self.batches[idx].vertex_count + vertex_count <= self.max_batch_size {
                idx
            } else {
                // 새 배치 생성 (크기 초과)
                let new_idx = self.batches.len();
                self.batches.push(ElementBatch::new(key));
                self.batch_map.insert(key, new_idx);
                new_idx
            }
        } else {
            let new_idx = self.batches.len();
            self.batches.push(ElementBatch::new(key));
            self.batch_map.insert(key, new_idx);
            new_idx
        };

        self.batches[batch_idx].add_element(element_index, vertex_count, index_count);
        self.total_vertices += vertex_count;
        self.total_indices += index_count;
    }

    pub fn finish(&mut self) {
        // 레이어 순서로 정렬
        self.batches.sort_by_key(|b| b.key.layer);
        self.total_draw_calls = self.batches.len();
    }

    pub fn batches(&self) -> &[ElementBatch] { &self.batches }
    pub fn batch_count(&self) -> usize { self.batches.len() }
    pub fn total_vertices(&self) -> usize { self.total_vertices }
    pub fn total_indices(&self) -> usize { self.total_indices }
    pub fn total_draw_calls(&self) -> usize { self.total_draw_calls }

    /// 배칭 효율 (원래 엘리먼트 수 대비 배치 수)
    pub fn efficiency(&self) -> f32 {
        let total_elements: usize = self.batches.iter()
            .map(|b| b.element_indices.len())
            .sum();
        if total_elements == 0 { return 1.0; }
        1.0 - (self.batches.len() as f32 / total_elements as f32)
    }
}

// ============================================================================
// BatchKey 유틸리티
// ============================================================================

impl BatchKey {
    /// DrawElement에서 BatchKey 생성 (통계 비교용)
    #[cfg(feature = "slate_debugging")]
    pub fn from_draw_element(elem: &crate::widget::DrawElement, layer: u32) -> Self {
        use crate::widget::DrawElement;
        let (shader_type, texture_id) = match elem {
            DrawElement::Text { .. } | DrawElement::StyledText { .. } => (ShaderType::Font, 0),
            DrawElement::RoundedBox { .. } => (ShaderType::RoundedBox, 0),
            DrawElement::Image { path, .. } => (ShaderType::Default, hash_str(path)),
            DrawElement::Spline { .. } => (ShaderType::Spline, 0),
            _ => (ShaderType::Default, 0),
        };
        BatchKey { layer, shader_type, texture_id, draw_effects: DrawEffects::NONE }
    }
}

#[cfg(feature = "slate_debugging")]
fn hash_str(s: &str) -> u32 {
    let mut h: u32 = 0;
    for b in s.bytes() { h = h.wrapping_mul(31).wrapping_add(b as u32); }
    h
}

/// 배치 통계
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    pub frame_count: u64,
    pub avg_batch_count: f32,
    pub avg_draw_calls: f32,
    pub avg_vertices: f32,
    pub peak_batches: usize,
    pub peak_draw_calls: usize,
}

impl BatchStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_frame(&mut self, batcher: &ElementBatcher) {
        self.frame_count += 1;
        let n = self.frame_count as f32;
        self.avg_batch_count += (batcher.batch_count() as f32 - self.avg_batch_count) / n;
        self.avg_draw_calls += (batcher.total_draw_calls() as f32 - self.avg_draw_calls) / n;
        self.avg_vertices += (batcher.total_vertices() as f32 - self.avg_vertices) / n;
        self.peak_batches = self.peak_batches.max(batcher.batch_count());
        self.peak_draw_calls = self.peak_draw_calls.max(batcher.total_draw_calls());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_key(layer: u32) -> BatchKey {
        BatchKey {
            layer,
            shader_type: ShaderType::Default,
            texture_id: 0,
            draw_effects: DrawEffects::NONE,
        }
    }

    #[test]
    fn test_batcher_creation() {
        let batcher = ElementBatcher::new();
        assert_eq!(batcher.batch_count(), 0);
        assert_eq!(batcher.total_vertices(), 0);
    }

    #[test]
    fn test_batcher_single_batch() {
        let mut batcher = ElementBatcher::new();
        batcher.begin_frame();
        let key = default_key(0);
        batcher.add_element(key, 0, 4, 6);
        batcher.add_element(key, 1, 4, 6);
        batcher.add_element(key, 2, 4, 6);
        batcher.finish();
        assert_eq!(batcher.batch_count(), 1); // 동일 키 → 1 배치
        assert_eq!(batcher.total_vertices(), 12);
        assert_eq!(batcher.total_indices(), 18);
    }

    #[test]
    fn test_batcher_multi_batch() {
        let mut batcher = ElementBatcher::new();
        batcher.begin_frame();
        batcher.add_element(default_key(0), 0, 4, 6);
        batcher.add_element(BatchKey {
            layer: 0, shader_type: ShaderType::Font,
            texture_id: 1, draw_effects: DrawEffects::NONE,
        }, 1, 4, 6);
        batcher.finish();
        assert_eq!(batcher.batch_count(), 2); // 다른 키 → 2 배치
    }

    #[test]
    fn test_batcher_layer_sorting() {
        let mut batcher = ElementBatcher::new();
        batcher.begin_frame();
        batcher.add_element(default_key(2), 0, 4, 6);
        batcher.add_element(default_key(0), 1, 4, 6);
        batcher.add_element(default_key(1), 2, 4, 6);
        batcher.finish();
        assert_eq!(batcher.batches()[0].key.layer, 0);
        assert_eq!(batcher.batches()[1].key.layer, 1);
        assert_eq!(batcher.batches()[2].key.layer, 2);
    }

    #[test]
    fn test_batcher_max_size() {
        let mut batcher = ElementBatcher::new().with_max_batch_size(6);
        batcher.begin_frame();
        let key = default_key(0);
        batcher.add_element(key, 0, 4, 6);
        batcher.add_element(key, 1, 4, 6); // 4+4=8 > 6 → new batch
        batcher.finish();
        assert_eq!(batcher.batch_count(), 2);
    }

    #[test]
    fn test_draw_effects() {
        let e = DrawEffects::GAMMA_CORRECT | DrawEffects::PIXEL_SNAPPING;
        assert!(e.contains(DrawEffects::GAMMA_CORRECT));
        assert!(e.contains(DrawEffects::PIXEL_SNAPPING));
        assert!(!e.contains(DrawEffects::WIREFRAME));
    }

    #[test]
    fn test_batch_stats() {
        let mut stats = BatchStats::new();
        let mut batcher = ElementBatcher::new();
        batcher.begin_frame();
        batcher.add_element(default_key(0), 0, 4, 6);
        batcher.finish();
        stats.record_frame(&batcher);
        assert_eq!(stats.frame_count, 1);
        assert_eq!(stats.peak_batches, 1);
    }
}
