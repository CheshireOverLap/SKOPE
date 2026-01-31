//! Texture Atlas — UI 이미지를 단일 텍스처에 패킹 (Shelf-packing)
//!
//! 여러 UI 이미지를 하나의 GPU 텍스처에 합쳐서
//! 드로우콜 배칭 효율을 높입니다.

use std::collections::HashMap;

/// 아틀라스 슬롯 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasSlotId(pub u32);

/// 아틀라스 내 할당된 영역
#[derive(Debug, Clone)]
pub struct AtlasSlot {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// UV 좌표 (u_min, v_min, u_max, v_max) — 0.0~1.0
    pub uv: [f32; 4],
}

/// Shelf-packing 기반 텍스처 아틀라스
pub struct SlateTextureAtlas {
    /// GPU 텍스처
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    /// 아틀라스 크기 (정사각형)
    size: u32,
    /// 할당된 슬롯들
    slots: HashMap<AtlasSlotId, AtlasSlot>,
    /// 이름 → 슬롯 ID 매핑
    name_to_id: HashMap<String, AtlasSlotId>,
    /// 다음 슬롯 ID
    next_id: u32,
    /// Shelf 상태: (현재 x, 현재 y, 현재 선반 높이)
    shelf_cursor: (u32, u32, u32),
    /// dirty 플래그 (CPU → GPU 업로드 필요)
    dirty: bool,
}

impl SlateTextureAtlas {
    pub const DEFAULT_SIZE: u32 = 2048;

    /// 새 아틀라스 생성
    pub fn new(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        size: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Slate UI Atlas"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Slate UI Atlas Bind Group"),
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture,
            texture_view,
            bind_group,
            size,
            slots: HashMap::new(),
            name_to_id: HashMap::new(),
            next_id: 0,
            shelf_cursor: (0, 0, 0),
            dirty: false,
        }
    }

    /// 텍스처 추가 (RGBA 데이터, 즉시 GPU 업로드)
    ///
    /// Shelf-packing: 왼쪽→오른쪽으로 채우고, 행이 차면 다음 행으로.
    pub fn add_texture(
        &mut self,
        queue: &wgpu::Queue,
        name: impl Into<String>,
        width: u32,
        height: u32,
        rgba_data: &[u8],
    ) -> Option<AtlasSlotId> {
        let name = name.into();

        // 이미 등록된 경우
        if let Some(&id) = self.name_to_id.get(&name) {
            return Some(id);
        }

        let padding = 1u32;
        let padded_w = width + padding * 2;
        let padded_h = height + padding * 2;

        // 아틀라스에 맞는지 확인
        if padded_w > self.size || padded_h > self.size {
            return None;
        }

        // 현재 행에 맞는지
        if self.shelf_cursor.0 + padded_w > self.size {
            // 다음 행으로
            self.shelf_cursor.0 = 0;
            self.shelf_cursor.1 += self.shelf_cursor.2;
            self.shelf_cursor.2 = 0;
        }

        // 높이 초과 확인
        if self.shelf_cursor.1 + padded_h > self.size {
            return None; // 아틀라스 꽉 참
        }

        let x = self.shelf_cursor.0 + padding;
        let y = self.shelf_cursor.1 + padding;

        // 커서 전진
        self.shelf_cursor.0 += padded_w;
        self.shelf_cursor.2 = self.shelf_cursor.2.max(padded_h);

        // GPU 업로드
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            rgba_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_f = self.size as f32;
        let slot = AtlasSlot {
            x,
            y,
            width,
            height,
            uv: [
                x as f32 / atlas_f,
                y as f32 / atlas_f,
                (x + width) as f32 / atlas_f,
                (y + height) as f32 / atlas_f,
            ],
        };

        let id = AtlasSlotId(self.next_id);
        self.next_id += 1;
        self.slots.insert(id, slot);
        self.name_to_id.insert(name, id);

        Some(id)
    }

    /// 이름으로 슬롯 조회
    pub fn get_slot_by_name(&self, name: &str) -> Option<&AtlasSlot> {
        self.name_to_id.get(name).and_then(|id| self.slots.get(id))
    }

    /// ID로 슬롯 조회
    pub fn get_slot(&self, id: AtlasSlotId) -> Option<&AtlasSlot> {
        self.slots.get(&id)
    }

    /// 바인드 그룹 참조
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// 텍스처 뷰 참조
    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }

    /// 등록된 슬롯 수
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// 이름이 등록되어 있는지
    pub fn contains(&self, name: &str) -> bool {
        self.name_to_id.contains_key(name)
    }

    /// 아틀라스 크기
    pub fn size(&self) -> u32 {
        self.size
    }
}

// ============================================================================
// Tree-based Rectangle Packing (CPU-only, testable without GPU)
// ============================================================================

/// 트리 기반 직사각형 패킹 (Guillotine / Binary Tree 알고리즘)
///
/// Shelf-packing보다 공간 효율이 높습니다.
/// CPU 전용으로, GPU 텍스처 아틀라스에 할당 위치를 결정합니다.
pub struct TreePacker {
    width: u32,
    height: u32,
    nodes: Vec<PackerNode>,
}

/// 패커 노드 (바이너리 트리)
struct PackerNode {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    /// 사용 중 여부
    used: bool,
    /// 오른쪽 자식 인덱스
    right: Option<usize>,
    /// 아래쪽 자식 인덱스
    down: Option<usize>,
}

/// 패킹 결과
#[derive(Debug, Clone, Copy)]
pub struct PackedRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl TreePacker {
    /// 새 패커 생성
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            nodes: vec![PackerNode {
                x: 0,
                y: 0,
                width,
                height,
                used: false,
                right: None,
                down: None,
            }],
        }
    }

    /// 직사각형 삽입 (패딩 포함)
    ///
    /// 성공 시 할당 위치 반환, 실패 시 None.
    pub fn insert(&mut self, width: u32, height: u32) -> Option<PackedRect> {
        self.insert_at(0, width, height)
    }

    fn insert_at(&mut self, node_idx: usize, w: u32, h: u32) -> Option<PackedRect> {
        // 노드 필드를 로컬로 복사하여 차용 문제 해결
        let (used, right, down, nw, nh, nx, ny) = {
            let node = &self.nodes[node_idx];
            (node.used, node.right, node.down, node.width, node.height, node.x, node.y)
        };

        if used {
            // 이미 사용된 노드 → 자식 탐색
            if let Some(right_idx) = right {
                if let Some(result) = self.insert_at(right_idx, w, h) {
                    return Some(result);
                }
            }
            if let Some(down_idx) = down {
                return self.insert_at(down_idx, w, h);
            }
            return None;
        }

        // 크기 확인
        if w > nw || h > nh {
            return None;
        }

        // 정확히 맞으면 사용
        if w == nw && h == nh {
            self.nodes[node_idx].used = true;
            return Some(PackedRect {
                x: nx,
                y: ny,
                width: w,
                height: h,
            });
        }

        // 분할: 더 긴 축을 기준으로
        let dw = nw - w;
        let dh = nh - h;
        let (x, y) = (nx, ny);

        let (right_node, down_node) = if dw > dh {
            // 수평 분할 우선
            (
                PackerNode { x: x + w, y, width: nw - w, height: h, used: false, right: None, down: None },
                PackerNode { x, y: y + h, width: nw, height: nh - h, used: false, right: None, down: None },
            )
        } else {
            // 수직 분할 우선
            (
                PackerNode { x: x + w, y, width: nw - w, height: nh, used: false, right: None, down: None },
                PackerNode { x, y: y + h, width: w, height: nh - h, used: false, right: None, down: None },
            )
        };

        let right_idx = self.nodes.len();
        self.nodes.push(right_node);
        let down_idx = self.nodes.len();
        self.nodes.push(down_node);

        self.nodes[node_idx].used = true;
        self.nodes[node_idx].right = Some(right_idx);
        self.nodes[node_idx].down = Some(down_idx);

        Some(PackedRect {
            x: nx,
            y: ny,
            width: w,
            height: h,
        })
    }

    /// 사용률 (0.0 ~ 1.0)
    pub fn utilization(&self) -> f32 {
        let total = self.width as f64 * self.height as f64;
        let used: f64 = self.nodes.iter()
            .filter(|n| n.used && n.right.is_some()) // 분할된 노드는 사용 중
            .count() as f64; // 근사치
        // 정확한 계산: 잎 노드 중 used인 것의 면적 합
        let used_area: f64 = self.nodes.iter()
            .filter(|n| n.used && n.right.is_none() && n.down.is_none())
            .map(|n| n.width as f64 * n.height as f64)
            .sum();
        // 실제로 잎+used 면적 합산이 더 정확
        let leaf_used: f64 = self.nodes.iter()
            .filter(|n| n.used)
            .filter(|n| {
                // 자식 없는 used 노드 = 최종 할당
                n.right.is_none() && n.down.is_none()
            })
            .map(|n| n.width as f64 * n.height as f64)
            .sum();

        // 분할된 used 노드는 삽입 시 원래 크기가 할당 크기
        // 정확한 사용률: 할당된 직사각형 면적 / 전체
        // 간단히: 모든 insert 호출의 w*h 누적이 필요하지만, 여기서는 근사치
        (used_area.max(leaf_used) / total) as f32
    }

    /// 아틀라스 크기
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// 노드 수 (디버그용)
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 리셋 (모든 할당 해제)
    pub fn reset(&mut self) {
        self.nodes.clear();
        self.nodes.push(PackerNode {
            x: 0,
            y: 0,
            width: self.width,
            height: self.height,
            used: false,
            right: None,
            down: None,
        });
    }
}

// ============================================================================
// Thread-safe Atlas Wrapper
// ============================================================================

use std::sync::{Arc, RwLock};

/// 스레드 안전 아틀라스 소유권 래퍼
///
/// Game 스레드와 Render 스레드 간 안전한 공유를 위한 래퍼.
/// `Arc<RwLock<T>>` 패턴으로 다중 읽기 / 단일 쓰기를 보장합니다.
pub struct SharedAtlasHandle<T> {
    inner: Arc<RwLock<T>>,
}

impl<T> SharedAtlasHandle<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// 읽기 잠금
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
        self.inner.read().unwrap()
    }

    /// 쓰기 잠금
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        self.inner.write().unwrap()
    }

    /// Arc 클론 (다른 스레드에 전달)
    pub fn share(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Clone for SharedAtlasHandle<T> {
    fn clone(&self) -> Self {
        self.share()
    }
}

// ============================================================================
// Deferred Upload Queue
// ============================================================================

/// 지연 GPU 업로드 항목
pub struct DeferredUpload {
    /// 슬롯 이름
    pub name: String,
    /// 이미지 크기
    pub width: u32,
    pub height: u32,
    /// RGBA 픽셀 데이터
    pub rgba_data: Vec<u8>,
    /// 업로드 우선순위 (높을수록 먼저)
    pub priority: i32,
}

/// 지연 업로드 큐
///
/// CPU 측에서 텍스처 데이터를 큐잉하고, 프레임 시작 시 일괄 GPU 업로드합니다.
/// dirty 플래그 기반으로 불필요한 업로드를 방지합니다.
pub struct DeferredUploadQueue {
    /// 대기 중인 업로드
    pending: Vec<DeferredUpload>,
    /// 프레임당 최대 업로드 수 (성능 제한)
    max_uploads_per_frame: usize,
}

impl Default for DeferredUploadQueue {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            max_uploads_per_frame: 8,
        }
    }
}

impl DeferredUploadQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// 업로드 프레임 제한 설정
    pub fn with_max_per_frame(mut self, max: usize) -> Self {
        self.max_uploads_per_frame = max;
        self
    }

    /// 업로드 예약
    pub fn enqueue(&mut self, upload: DeferredUpload) {
        self.pending.push(upload);
    }

    /// 간편 업로드 예약
    pub fn enqueue_texture(&mut self, name: impl Into<String>, width: u32, height: u32, data: Vec<u8>) {
        self.enqueue(DeferredUpload {
            name: name.into(),
            width,
            height,
            rgba_data: data,
            priority: 0,
        });
    }

    /// 프레임 시작 시 호출 — 우선순위 순으로 최대 N개 업로드 항목 반환
    pub fn drain_batch(&mut self) -> Vec<DeferredUpload> {
        // 우선순위 정렬 (높은 것 먼저)
        self.pending.sort_by(|a, b| b.priority.cmp(&a.priority));
        let count = self.pending.len().min(self.max_uploads_per_frame);
        self.pending.drain(..count).collect()
    }

    /// 대기 중인 업로드 수
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// 비었는지
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// 모든 대기 업로드 취소
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

// ============================================================================
// LRU Eviction
// ============================================================================

/// LRU 퇴거 관리자
///
/// 아틀라스 공간이 부족할 때 최근 사용되지 않은 슬롯을 퇴거합니다.
/// 프레임 카운터 기반 LRU 추적.
pub struct AtlasEvictionManager {
    /// 슬롯별 마지막 사용 프레임
    last_used: HashMap<AtlasSlotId, u64>,
    /// 현재 프레임 번호
    current_frame: u64,
    /// 퇴거 임계값 (이 프레임 수 이상 미사용 시 퇴거 대상)
    eviction_threshold: u64,
}

impl AtlasEvictionManager {
    pub fn new(eviction_threshold: u64) -> Self {
        Self {
            last_used: HashMap::new(),
            current_frame: 0,
            eviction_threshold,
        }
    }

    /// 슬롯 사용 기록 (매 프레임 렌더 시 호출)
    pub fn touch(&mut self, slot_id: AtlasSlotId) {
        self.last_used.insert(slot_id, self.current_frame);
    }

    /// 프레임 전진
    pub fn advance_frame(&mut self) {
        self.current_frame += 1;
    }

    /// 현재 프레임
    pub fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// 퇴거 대상 슬롯 ID 목록
    pub fn eviction_candidates(&self) -> Vec<AtlasSlotId> {
        self.last_used.iter()
            .filter(|(_, &last)| self.current_frame.saturating_sub(last) >= self.eviction_threshold)
            .map(|(&id, _)| id)
            .collect()
    }

    /// 슬롯 추적 제거 (퇴거 후 호출)
    pub fn remove(&mut self, slot_id: AtlasSlotId) {
        self.last_used.remove(&slot_id);
    }

    /// 추적 중인 슬롯 수
    pub fn tracked_count(&self) -> usize {
        self.last_used.len()
    }

    /// 리셋
    pub fn clear(&mut self) {
        self.last_used.clear();
        self.current_frame = 0;
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- TreePacker tests ---

    #[test]
    fn test_tree_packer_basic_insert() {
        let mut packer = TreePacker::new(256, 256);

        let r1 = packer.insert(64, 64);
        assert!(r1.is_some());
        let r1 = r1.unwrap();
        assert_eq!(r1.x, 0);
        assert_eq!(r1.y, 0);
        assert_eq!(r1.width, 64);
        assert_eq!(r1.height, 64);

        let r2 = packer.insert(64, 64);
        assert!(r2.is_some());
        let r2 = r2.unwrap();
        // 두 번째는 첫 번째와 겹치지 않아야 함
        assert!(r2.x >= 64 || r2.y >= 64);
    }

    #[test]
    fn test_tree_packer_overflow() {
        let mut packer = TreePacker::new(64, 64);

        let r1 = packer.insert(64, 64);
        assert!(r1.is_some());

        // 아틀라스가 꼽 찼으므로 실패
        let r2 = packer.insert(1, 1);
        assert!(r2.is_none());
    }

    #[test]
    fn test_tree_packer_too_large() {
        let mut packer = TreePacker::new(128, 128);
        let r = packer.insert(256, 256);
        assert!(r.is_none());
    }

    #[test]
    fn test_tree_packer_many_small() {
        let mut packer = TreePacker::new(128, 128);
        let mut count = 0;
        for _ in 0..256 {
            if packer.insert(16, 16).is_some() {
                count += 1;
            }
        }
        // 128x128에 16x16 = 최대 64개
        assert!(count >= 32 && count <= 64, "count = {}", count);
    }

    #[test]
    fn test_tree_packer_reset() {
        let mut packer = TreePacker::new(128, 128);
        packer.insert(128, 128);
        assert!(packer.insert(1, 1).is_none());

        packer.reset();
        assert!(packer.insert(64, 64).is_some());
    }

    #[test]
    fn test_tree_packer_no_overlap() {
        let mut packer = TreePacker::new(256, 256);
        let mut rects = Vec::new();

        for _ in 0..20 {
            if let Some(r) = packer.insert(32, 48) {
                rects.push(r);
            }
        }

        // 겹침 검사
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let a = &rects[i];
                let b = &rects[j];
                let no_overlap = a.x + a.width <= b.x
                    || b.x + b.width <= a.x
                    || a.y + a.height <= b.y
                    || b.y + b.height <= a.y;
                assert!(no_overlap, "overlap: {:?} vs {:?}", a, b);
            }
        }
    }

    // --- SharedAtlasHandle tests ---

    #[test]
    fn test_shared_atlas_handle() {
        let handle = SharedAtlasHandle::new(42u32);
        let handle2 = handle.share();

        assert_eq!(*handle.read(), 42);
        *handle2.write() = 100;
        assert_eq!(*handle.read(), 100);
    }

    // --- DeferredUploadQueue tests ---

    #[test]
    fn test_deferred_queue_basic() {
        let mut queue = DeferredUploadQueue::new().with_max_per_frame(2);
        queue.enqueue_texture("a", 16, 16, vec![0; 16 * 16 * 4]);
        queue.enqueue_texture("b", 16, 16, vec![0; 16 * 16 * 4]);
        queue.enqueue_texture("c", 16, 16, vec![0; 16 * 16 * 4]);

        assert_eq!(queue.pending_count(), 3);

        let batch = queue.drain_batch();
        assert_eq!(batch.len(), 2); // max 2 per frame
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_deferred_queue_priority() {
        let mut queue = DeferredUploadQueue::new().with_max_per_frame(1);
        queue.enqueue(DeferredUpload {
            name: "low".to_string(),
            width: 8, height: 8,
            rgba_data: vec![],
            priority: 0,
        });
        queue.enqueue(DeferredUpload {
            name: "high".to_string(),
            width: 8, height: 8,
            rgba_data: vec![],
            priority: 10,
        });

        let batch = queue.drain_batch();
        assert_eq!(batch[0].name, "high");
    }

    // --- AtlasEvictionManager tests ---

    #[test]
    fn test_eviction_manager_basic() {
        let mut mgr = AtlasEvictionManager::new(5);

        mgr.touch(AtlasSlotId(1));
        mgr.touch(AtlasSlotId(2));
        mgr.touch(AtlasSlotId(3));

        // 5 프레임 전진
        for _ in 0..5 {
            mgr.advance_frame();
        }

        // slot 1, 2, 3 모두 퇴거 대상
        let candidates = mgr.eviction_candidates();
        assert_eq!(candidates.len(), 3);

        // slot 2를 다시 touch
        mgr.touch(AtlasSlotId(2));
        let candidates = mgr.eviction_candidates();
        assert_eq!(candidates.len(), 2);
        assert!(!candidates.contains(&AtlasSlotId(2)));
    }

    #[test]
    fn test_eviction_manager_remove() {
        let mut mgr = AtlasEvictionManager::new(1);
        mgr.touch(AtlasSlotId(1));
        assert_eq!(mgr.tracked_count(), 1);

        mgr.remove(AtlasSlotId(1));
        assert_eq!(mgr.tracked_count(), 0);
    }

    #[test]
    fn test_eviction_manager_clear() {
        let mut mgr = AtlasEvictionManager::new(1);
        mgr.touch(AtlasSlotId(1));
        mgr.advance_frame();
        mgr.clear();
        assert_eq!(mgr.tracked_count(), 0);
        assert_eq!(mgr.current_frame(), 0);
    }
}
