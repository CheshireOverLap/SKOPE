//! StencilClipping — wgpu 스텐실 기반 클리핑 시스템
//!
//! 축 비정렬(non-axis-aligned) 클리핑을 위한 스텐실 버퍼 관리.
//! 렌더 트랜스폼이 적용된 위젯의 정확한 클리핑을 지원합니다.

/// 스텐실 클리핑 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StencilClipMode {
    /// 스텐실 사용 안 함 (축 정렬 scissor로 충분)
    Disabled,
    /// 스텐실 write — 클리핑 영역 마스크 기록
    Write,
    /// 스텐실 test — 마스크 내부만 렌더링
    Test,
    /// 스텐실 write + test (중첩 클리핑)
    WriteAndTest,
}

/// 스텐실 참조값 관리
#[derive(Debug, Clone)]
pub struct StencilRefStack {
    refs: Vec<u8>,
    current_ref: u8,
    max_depth: u8,
}

impl StencilRefStack {
    pub fn new() -> Self {
        Self {
            refs: Vec::new(),
            current_ref: 0,
            max_depth: 255, // 8-bit stencil
        }
    }

    pub fn with_max_depth(max_depth: u8) -> Self {
        Self {
            refs: Vec::new(),
            current_ref: 0,
            max_depth,
        }
    }

    /// 클리핑 레벨 push — 새 스텐실 참조값 할당
    pub fn push(&mut self) -> Option<u8> {
        if self.current_ref >= self.max_depth {
            return None; // 스텐실 깊이 초과
        }
        self.current_ref += 1;
        self.refs.push(self.current_ref);
        Some(self.current_ref)
    }

    /// 클리핑 레벨 pop
    pub fn pop(&mut self) -> Option<u8> {
        self.refs.pop()?;
        self.current_ref = self.refs.last().copied().unwrap_or(0);
        Some(self.current_ref)
    }

    /// 현재 스텐실 참조값
    pub fn current(&self) -> u8 {
        self.current_ref
    }

    /// 현재 클리핑 깊이
    pub fn depth(&self) -> usize {
        self.refs.len()
    }

    /// 전체 리셋
    pub fn clear(&mut self) {
        self.refs.clear();
        self.current_ref = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

/// 스텐실 클리핑 존
#[derive(Debug, Clone)]
pub struct StencilClipZone {
    /// 존 ID
    pub zone_id: u32,
    /// 스텐실 참조값
    pub stencil_ref: u8,
    /// 클리핑 영역 정점 (변환 적용된 사각형)
    pub vertices: [glam::Vec2; 4],
    /// 부모 존 ID (중첩 시)
    pub parent_zone: Option<u32>,
}

impl StencilClipZone {
    pub fn new(zone_id: u32, stencil_ref: u8, vertices: [glam::Vec2; 4]) -> Self {
        Self {
            zone_id,
            stencil_ref,
            vertices,
            parent_zone: None,
        }
    }

    pub fn with_parent(mut self, parent: u32) -> Self {
        self.parent_zone = Some(parent);
        self
    }

    /// AABB 바운딩 박스 계산 (빠른 거부 판정용)
    pub fn bounding_box(&self) -> (glam::Vec2, glam::Vec2) {
        let mut min = self.vertices[0];
        let mut max = self.vertices[0];
        for v in &self.vertices[1..] {
            min = min.min(*v);
            max = max.max(*v);
        }
        (min, max)
    }

    /// 점이 클리핑 존 내부에 있는지 (교차곱 기반)
    pub fn contains_point(&self, point: glam::Vec2) -> bool {
        let v = &self.vertices;
        // 볼록 사각형 내부 판정: 모든 엣지에 대해 같은 방향
        let cross = |a: glam::Vec2, b: glam::Vec2, p: glam::Vec2| -> f32 {
            (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
        };

        let c0 = cross(v[0], v[1], point);
        let c1 = cross(v[1], v[2], point);
        let c2 = cross(v[2], v[3], point);
        let c3 = cross(v[3], v[0], point);

        (c0 >= 0.0 && c1 >= 0.0 && c2 >= 0.0 && c3 >= 0.0)
            || (c0 <= 0.0 && c1 <= 0.0 && c2 <= 0.0 && c3 <= 0.0)
    }
}

/// 스텐실 클리핑 관리자
pub struct StencilClipManager {
    zones: Vec<StencilClipZone>,
    ref_stack: StencilRefStack,
    next_zone_id: u32,
    active_zone: Option<u32>,
    /// 히트테스트에서 스텐실 존 활용
    hit_test_zones: Vec<u32>,
}

impl StencilClipManager {
    pub fn new() -> Self {
        Self {
            zones: Vec::new(),
            ref_stack: StencilRefStack::new(),
            next_zone_id: 0,
            active_zone: None,
            hit_test_zones: Vec::new(),
        }
    }

    /// 프레임 시작 시 리셋
    pub fn begin_frame(&mut self) {
        self.zones.clear();
        self.ref_stack.clear();
        self.next_zone_id = 0;
        self.active_zone = None;
        self.hit_test_zones.clear();
    }

    /// 새 클리핑 존 push
    pub fn push_clip(&mut self, vertices: [glam::Vec2; 4]) -> Option<u32> {
        let stencil_ref = self.ref_stack.push()?;
        let zone_id = self.next_zone_id;
        self.next_zone_id += 1;

        let mut zone = StencilClipZone::new(zone_id, stencil_ref, vertices);
        if let Some(parent) = self.active_zone {
            zone = zone.with_parent(parent);
        }

        self.active_zone = Some(zone_id);
        self.hit_test_zones.push(zone_id);
        self.zones.push(zone);
        Some(zone_id)
    }

    /// 클리핑 존 pop
    pub fn pop_clip(&mut self) {
        self.ref_stack.pop();
        self.hit_test_zones.pop();
        self.active_zone = self.hit_test_zones.last().copied();
    }

    /// 히트테스트 — 포인트가 현재 활성 클리핑 체인 내부인지
    pub fn hit_test(&self, point: glam::Vec2) -> bool {
        if self.zones.is_empty() {
            return true; // 클리핑 없음 → 항상 히트
        }
        // 현재 활성 존 체인을 역추적
        for &zone_id in self.hit_test_zones.iter().rev() {
            if let Some(zone) = self.zones.iter().find(|z| z.zone_id == zone_id) {
                if !zone.contains_point(point) {
                    return false;
                }
            }
        }
        true
    }

    /// 현재 활성 존의 스텐실 참조값
    pub fn current_stencil_ref(&self) -> u8 {
        self.ref_stack.current()
    }

    /// 클리핑 깊이
    pub fn clip_depth(&self) -> usize {
        self.ref_stack.depth()
    }

    pub fn zone_count(&self) -> usize { self.zones.len() }
    pub fn zones(&self) -> &[StencilClipZone] { &self.zones }
    pub fn active_zone_id(&self) -> Option<u32> { self.active_zone }
}

/// 스텐실 파이프라인 설정
#[derive(Debug, Clone)]
pub struct StencilPipelineConfig {
    /// 스텐실 write 시 compare 함수
    pub write_compare: CompareFunction,
    /// 스텐실 test 시 compare 함수
    pub test_compare: CompareFunction,
    /// 스텐실 pass 시 작업
    pub pass_op: StencilOperation,
    /// 스텐실 fail 시 작업
    pub fail_op: StencilOperation,
    /// depth fail 시 작업
    pub depth_fail_op: StencilOperation,
    /// 읽기 마스크
    pub read_mask: u32,
    /// 쓰기 마스크
    pub write_mask: u32,
}

/// 비교 함수 (wgpu::CompareFunction 미러)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareFunction {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// 스텐실 작업 (wgpu::StencilOperation 미러)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StencilOperation {
    Keep,
    Zero,
    Replace,
    IncrementClamp,
    DecrementClamp,
    Invert,
    IncrementWrap,
    DecrementWrap,
}

impl Default for StencilPipelineConfig {
    fn default() -> Self {
        Self {
            write_compare: CompareFunction::Always,
            test_compare: CompareFunction::Equal,
            pass_op: StencilOperation::Replace,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            read_mask: 0xFF,
            write_mask: 0xFF,
        }
    }
}

impl StencilPipelineConfig {
    /// 스텐실 write용 설정
    pub fn for_write() -> Self {
        Self {
            write_compare: CompareFunction::Always,
            test_compare: CompareFunction::Always,
            pass_op: StencilOperation::Replace,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            read_mask: 0xFF,
            write_mask: 0xFF,
        }
    }

    /// 스텐실 test용 설정
    pub fn for_test() -> Self {
        Self {
            write_compare: CompareFunction::Equal,
            test_compare: CompareFunction::Equal,
            pass_op: StencilOperation::Keep,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            read_mask: 0xFF,
            write_mask: 0x00,
        }
    }

    /// 중첩 클리핑용 — increment stencil
    pub fn for_nested_write() -> Self {
        Self {
            write_compare: CompareFunction::Always,
            test_compare: CompareFunction::Always,
            pass_op: StencilOperation::IncrementClamp,
            fail_op: StencilOperation::Keep,
            depth_fail_op: StencilOperation::Keep,
            read_mask: 0xFF,
            write_mask: 0xFF,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    #[test]
    fn test_stencil_ref_stack() {
        let mut stack = StencilRefStack::new();
        assert_eq!(stack.current(), 0);
        assert_eq!(stack.push(), Some(1));
        assert_eq!(stack.push(), Some(2));
        assert_eq!(stack.current(), 2);
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.pop(), Some(1));
        assert_eq!(stack.current(), 1);
        assert_eq!(stack.pop(), Some(0));
        assert!(stack.is_empty());
    }

    #[test]
    fn test_stencil_ref_max_depth() {
        let mut stack = StencilRefStack::with_max_depth(2);
        assert_eq!(stack.push(), Some(1));
        assert_eq!(stack.push(), Some(2));
        assert_eq!(stack.push(), None); // 깊이 초과
    }

    #[test]
    fn test_clip_zone_contains() {
        let zone = StencilClipZone::new(0, 1, [
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 100.0),
        ]);
        assert!(zone.contains_point(Vec2::new(50.0, 50.0)));
        assert!(!zone.contains_point(Vec2::new(150.0, 50.0)));
    }

    #[test]
    fn test_clip_zone_rotated() {
        // 45도 회전된 사각형
        let _s = 50.0_f32;
        let zone = StencilClipZone::new(0, 1, [
            Vec2::new(50.0, 0.0),   // top
            Vec2::new(100.0, 50.0), // right
            Vec2::new(50.0, 100.0), // bottom
            Vec2::new(0.0, 50.0),   // left
        ]);
        assert!(zone.contains_point(Vec2::new(50.0, 50.0))); // center
        assert!(!zone.contains_point(Vec2::new(5.0, 5.0)));   // corner outside
    }

    #[test]
    fn test_clip_zone_bounding_box() {
        let zone = StencilClipZone::new(0, 1, [
            Vec2::new(10.0, 20.0),
            Vec2::new(100.0, 20.0),
            Vec2::new(100.0, 80.0),
            Vec2::new(10.0, 80.0),
        ]);
        let (min, max) = zone.bounding_box();
        assert_eq!(min, Vec2::new(10.0, 20.0));
        assert_eq!(max, Vec2::new(100.0, 80.0));
    }

    #[test]
    fn test_stencil_clip_manager() {
        let mut mgr = StencilClipManager::new();
        mgr.begin_frame();

        let verts = [
            Vec2::new(0.0, 0.0),
            Vec2::new(200.0, 0.0),
            Vec2::new(200.0, 200.0),
            Vec2::new(0.0, 200.0),
        ];
        let id = mgr.push_clip(verts).unwrap();
        assert_eq!(id, 0);
        assert_eq!(mgr.current_stencil_ref(), 1);
        assert_eq!(mgr.clip_depth(), 1);

        // 히트테스트
        assert!(mgr.hit_test(Vec2::new(100.0, 100.0)));
        assert!(!mgr.hit_test(Vec2::new(300.0, 300.0)));

        mgr.pop_clip();
        assert_eq!(mgr.clip_depth(), 0);
    }

    #[test]
    fn test_nested_clipping() {
        let mut mgr = StencilClipManager::new();
        mgr.begin_frame();

        // 외부 클립 0..200
        mgr.push_clip([
            Vec2::new(0.0, 0.0),
            Vec2::new(200.0, 0.0),
            Vec2::new(200.0, 200.0),
            Vec2::new(0.0, 200.0),
        ]);
        // 내부 클립 50..150
        mgr.push_clip([
            Vec2::new(50.0, 50.0),
            Vec2::new(150.0, 50.0),
            Vec2::new(150.0, 150.0),
            Vec2::new(50.0, 150.0),
        ]);

        assert_eq!(mgr.clip_depth(), 2);
        assert_eq!(mgr.current_stencil_ref(), 2);

        // 내부 영역만 히트
        assert!(mgr.hit_test(Vec2::new(100.0, 100.0)));
        assert!(!mgr.hit_test(Vec2::new(25.0, 25.0))); // 외부에는 있지만 내부에 없음

        mgr.pop_clip();
        assert_eq!(mgr.clip_depth(), 1);
    }

    #[test]
    fn test_stencil_pipeline_configs() {
        let write = StencilPipelineConfig::for_write();
        assert_eq!(write.pass_op, StencilOperation::Replace);

        let test = StencilPipelineConfig::for_test();
        assert_eq!(test.write_mask, 0x00); // test only, no write

        let nested = StencilPipelineConfig::for_nested_write();
        assert_eq!(nested.pass_op, StencilOperation::IncrementClamp);
    }
}
