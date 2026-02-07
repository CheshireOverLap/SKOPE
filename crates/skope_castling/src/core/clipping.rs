//! 계층적 클리핑 시스템 (UE5 Slate Clipping.h 대응)
//!
//! UE5의 FSlateClippingManager / FSlateClippingZone / FSlateClippingState 패턴.
//! 축 정렬 클리핑(Scissor)과 비축 정렬 클리핑(Stencil)을 통합 관리.
//!
//! ## 클리핑 합성 규칙
//! ```text
//! intersect=true  + prev=Scissor + cur=AxisAligned → Scissor(교차)
//! intersect=true  + prev=Scissor + cur=Rotated    → Stencil(prev + cur)
//! intersect=true  + prev=Stencil + cur=Any        → Stencil(prev quads + cur)
//! intersect=false → AlwaysClip 조상까지 스캔, 없으면 새로 시작
//! ```

use glam::Vec2;

// ============================================================================
// EWidgetClipping
// ============================================================================

/// 위젯 클리핑 모드 (UE5 EWidgetClipping)
///
/// 대부분의 위젯은 `Inherit` (기본) — 부모의 클리핑 영역을 상속.
/// 스크롤 박스, 캔버스 등 내용을 잘라야 하는 위젯만 `ClipToBounds` 사용.
///
/// **배칭 주의**: 클리핑 영역이 바뀌면 GPU 배칭이 분리됩니다.
/// 불필요한 `ClipToBounds` 사용은 드로우 콜 증가로 이어집니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum EWidgetClipping {
    /// 부모 클리핑 상속 (기본값 — 대부분의 위젯)
    #[default]
    Inherit,
    /// 바운드로 클리핑 (부모 클립과 교차)
    ClipToBounds,
    /// 바운드로 클리핑 (부모 클립 무시 — 독립적 클립)
    ///
    /// `ClipToBoundsAlways`는 무시 불가.
    /// 팝업, 드롭다운 등 기존 클리핑 밖으로 확장해야 할 때 사용.
    ClipToBoundsWithoutIntersecting,
    /// 항상 바운드로 클리핑 (하위 위젯이 무시 불가)
    ///
    /// 하드 배리어로 사용 — 애니메이션/효과가 이 영역을 벗어나지 않음.
    ClipToBoundsAlways,
    /// 필요시 클리핑 (DesiredSize > AllocatedSize일 때만)
    ///
    /// 텍스트 위젯 등에 유용 — 항상 클리핑하면 배칭 손실이 크므로,
    /// 실제로 넘칠 때만 활성화.
    OnDemand,
}

// ============================================================================
// EClippingMethod
// ============================================================================

/// GPU 클리핑 메서드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EClippingMethod {
    /// 축 정렬 — wgpu `set_scissor_rect()` (저비용)
    Scissor,
    /// 비축 정렬 — stencil buffer (고비용)
    Stencil,
}

// ============================================================================
// SlateClippingZone
// ============================================================================

/// 클리핑 존 (UE5 FSlateClippingZone)
///
/// 4코너 좌표로 정의. 축 정렬이면 Scissor, 아니면 Stencil.
/// `from_rect`로 축 정렬 존을, `from_geometry`로 트랜스폼 반영 존을 생성.
#[derive(Debug, Clone, Copy)]
pub struct SlateClippingZone {
    /// 좌상단 (스크린 좌표)
    pub top_left: Vec2,
    /// 우상단
    pub top_right: Vec2,
    /// 좌하단
    pub bottom_left: Vec2,
    /// 우하단
    pub bottom_right: Vec2,
    /// 축 정렬 여부 (true면 Scissor 사용 가능)
    pub is_axis_aligned: bool,
    /// 부모 클립과 교차할지 (false면 독립적 클립)
    pub should_intersect: bool,
    /// 항상 클리핑 (하위에서 무시 불가)
    pub always_clip: bool,
}

impl SlateClippingZone {
    /// 축 정렬 rect [x, y, w, h]로 존 생성
    pub fn from_rect(rect: [f32; 4]) -> Self {
        let [x, y, w, h] = rect;
        Self {
            top_left: Vec2::new(x, y),
            top_right: Vec2::new(x + w, y),
            bottom_left: Vec2::new(x, y + h),
            bottom_right: Vec2::new(x + w, y + h),
            is_axis_aligned: true,
            should_intersect: true,
            always_clip: false,
        }
    }

    /// Geometry에서 존 생성 (렌더 트랜스폼 반영)
    ///
    /// 위젯 클리핑 모드에 따라 `should_intersect`, `always_clip` 설정.
    pub fn from_geometry(geometry: &super::Geometry, clipping: EWidgetClipping) -> Self {
        let should_intersect = clipping != EWidgetClipping::ClipToBoundsWithoutIntersecting;
        let always_clip = clipping == EWidgetClipping::ClipToBoundsAlways;

        if geometry.has_render_transform() {
            // 렌더 트랜스폼 적용 — 4코너를 변환
            let rt = geometry.accumulated_render_transform();
            let local_size = geometry.local_size;

            let tl = rt.transform_point2(Vec2::ZERO);
            let tr = rt.transform_point2(Vec2::new(local_size.x, 0.0));
            let bl = rt.transform_point2(Vec2::new(0.0, local_size.y));
            let br = rt.transform_point2(local_size);

            // 축 정렬 판정 (UE5: 좌변 수직 + 하변 수평이면 축 정렬)
            let tolerance = 0.1;
            let is_aa = Self::check_axis_aligned(tl, tr, bl, br, tolerance);

            let mut zone = if is_aa {
                // 축 정렬 — 정규화된 좌표로 재설정
                let left = tl.x.min(tr.x).min(bl.x).min(br.x);
                let top = tl.y.min(tr.y).min(bl.y).min(br.y);
                let right = tl.x.max(tr.x).max(bl.x).max(br.x);
                let bottom = tl.y.max(tr.y).max(bl.y).max(br.y);
                Self {
                    top_left: Vec2::new(left, top),
                    top_right: Vec2::new(right, top),
                    bottom_left: Vec2::new(left, bottom),
                    bottom_right: Vec2::new(right, bottom),
                    is_axis_aligned: true,
                    should_intersect,
                    always_clip,
                }
            } else {
                Self {
                    top_left: tl,
                    top_right: tr,
                    bottom_left: bl,
                    bottom_right: br,
                    is_axis_aligned: false,
                    should_intersect,
                    always_clip,
                }
            };
            zone.should_intersect = should_intersect;
            zone.always_clip = always_clip;
            zone
        } else {
            // 렌더 트랜스폼 없음 — 축 정렬 AABB
            let abs_pos = geometry.absolute_position;
            let scaled_size = geometry.local_size * geometry.scale;
            Self {
                top_left: abs_pos,
                top_right: Vec2::new(abs_pos.x + scaled_size.x, abs_pos.y),
                bottom_left: Vec2::new(abs_pos.x, abs_pos.y + scaled_size.y),
                bottom_right: abs_pos + scaled_size,
                is_axis_aligned: true,
                should_intersect,
                always_clip,
            }
        }
    }

    /// 축 정렬 판정 (UE5 InitializeFromArbitraryPoints 참고)
    fn check_axis_aligned(tl: Vec2, _tr: Vec2, bl: Vec2, br: Vec2, tolerance: f32) -> bool {
        // TL.x ≈ BL.x (좌변 수직) && BL.y ≈ BR.y (하변 수평)
        if (tl.x - bl.x).abs() < tolerance && (bl.y - br.y).abs() < tolerance {
            return true;
        }
        // 또는 TL.y ≈ BL.y (좌변 수평) && BL.x ≈ BR.x (하변 수직)
        if (tl.y - bl.y).abs() < tolerance && (bl.x - br.x).abs() < tolerance {
            return true;
        }
        false
    }

    /// 축 정렬 시 scissor rect [x, y, w, h] 반환
    pub fn to_scissor_rect(&self) -> [f32; 4] {
        debug_assert!(self.is_axis_aligned, "to_scissor_rect called on non-axis-aligned zone");
        let left = self.top_left.x.min(self.bottom_right.x);
        let top = self.top_left.y.min(self.bottom_right.y);
        let right = self.top_left.x.max(self.bottom_right.x);
        let bottom = self.top_left.y.max(self.bottom_right.y);
        [left, top, right - left, bottom - top]
    }

    /// 보수적 AABB (stencil fallback용)
    pub fn to_aabb(&self) -> [f32; 4] {
        let min_x = self.top_left.x.min(self.top_right.x).min(self.bottom_left.x).min(self.bottom_right.x);
        let min_y = self.top_left.y.min(self.top_right.y).min(self.bottom_left.y).min(self.bottom_right.y);
        let max_x = self.top_left.x.max(self.top_right.x).max(self.bottom_left.x).max(self.bottom_right.x);
        let max_y = self.top_left.y.max(self.top_right.y).max(self.bottom_left.y).max(self.bottom_right.y);
        [min_x, min_y, max_x - min_x, max_y - min_y]
    }

    /// 두 축 정렬 존의 교차 (UE5 FSlateClippingZone::Intersect)
    pub fn intersect_axis_aligned(&self, other: &SlateClippingZone) -> SlateClippingZone {
        debug_assert!(self.is_axis_aligned && other.is_axis_aligned);

        let left = self.top_left.x.max(other.top_left.x);
        let top = self.top_left.y.max(other.top_left.y);
        let right = self.bottom_right.x.min(other.bottom_right.x);
        let bottom = self.bottom_right.y.min(other.bottom_right.y);

        // 빈 교차 방지
        let right = right.max(left);
        let bottom = bottom.max(top);

        SlateClippingZone {
            top_left: Vec2::new(left, top),
            top_right: Vec2::new(right, top),
            bottom_left: Vec2::new(left, bottom),
            bottom_right: Vec2::new(right, bottom),
            is_axis_aligned: true,
            should_intersect: self.should_intersect,
            always_clip: self.always_clip || other.always_clip,
        }
    }

    /// 점이 존 내부에 있는지 (히트테스트용)
    pub fn contains_point(&self, point: Vec2) -> bool {
        if self.is_axis_aligned {
            point.x >= self.top_left.x && point.x <= self.top_right.x
                && point.y >= self.top_left.y && point.y <= self.bottom_left.y
        } else {
            // 2개 삼각형으로 분할하여 판정
            Self::point_in_triangle(point, self.top_left, self.top_right, self.bottom_left)
                || Self::point_in_triangle(point, self.bottom_left, self.top_right, self.bottom_right)
        }
    }

    /// 삼각형 내부 판정 (UE5 IsPointInTriangle)
    fn point_in_triangle(p: Vec2, a: Vec2, b: Vec2, c: Vec2) -> bool {
        let ba = Self::vector_sign(b, a, p);
        let cb = Self::vector_sign(c, b, p);
        let ac = Self::vector_sign(a, c, p);
        ba == cb && cb == ac
    }

    fn vector_sign(v: Vec2, a: Vec2, b: Vec2) -> i32 {
        let cross = (a.x - v.x) * (b.y - v.y) - (a.y - v.y) * (b.x - v.x);
        if cross > 0.0 { 1 } else if cross < 0.0 { -1 } else { 0 }
    }
}

impl PartialEq for SlateClippingZone {
    fn eq(&self, other: &Self) -> bool {
        self.is_axis_aligned == other.is_axis_aligned
            && self.should_intersect == other.should_intersect
            && self.always_clip == other.always_clip
            && (self.top_left - other.top_left).length_squared() < 0.01
            && (self.top_right - other.top_right).length_squared() < 0.01
            && (self.bottom_left - other.bottom_left).length_squared() < 0.01
            && (self.bottom_right - other.bottom_right).length_squared() < 0.01
    }
}

// ============================================================================
// SlateClippingState
// ============================================================================

/// 클리핑 상태 — 하나의 드로우 배치에 적용되는 클리핑 설정
///
/// Scissor (축 정렬) 또는 Stencil (비축 정렬 quads 누적).
/// UE5 FSlateClippingState에 해당.
#[derive(Debug, Clone)]
pub struct SlateClippingState {
    /// 축 정렬이면 scissor rect [x, y, w, h]
    pub scissor_rect: Option<[f32; 4]>,
    /// 비축 정렬이면 stencil quads (복수 가능)
    pub stencil_quads: Vec<SlateClippingZone>,
    /// 항상 클리핑 플래그 (하위에서 무시 불가)
    pub always_clip: bool,
}

impl SlateClippingState {
    /// 클리핑 메서드 결정
    pub fn clipping_method(&self) -> EClippingMethod {
        if self.scissor_rect.is_some() {
            EClippingMethod::Scissor
        } else {
            EClippingMethod::Stencil
        }
    }

    /// 빈 영역인지 (scissor 전용)
    pub fn has_zero_area(&self) -> bool {
        if let Some([_, _, w, h]) = self.scissor_rect {
            w.abs() < 0.001 || h.abs() < 0.001
        } else {
            false
        }
    }

    /// Scissor rect 또는 stencil AABB 반환 (렌더러 fallback용)
    pub fn to_scissor_or_aabb(&self) -> [f32; 4] {
        if let Some(rect) = self.scissor_rect {
            rect
        } else if !self.stencil_quads.is_empty() {
            // 모든 stencil quads의 보수적 AABB
            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;
            for quad in &self.stencil_quads {
                let [qx, qy, qw, qh] = quad.to_aabb();
                min_x = min_x.min(qx);
                min_y = min_y.min(qy);
                max_x = max_x.max(qx + qw);
                max_y = max_y.max(qy + qh);
            }
            [min_x, min_y, max_x - min_x, max_y - min_y]
        } else {
            [0.0, 0.0, 0.0, 0.0]
        }
    }

    /// 점이 클리핑 영역 내부에 있는지
    pub fn contains_point(&self, point: Vec2) -> bool {
        if let Some([x, y, w, h]) = self.scissor_rect {
            point.x >= x && point.x <= x + w && point.y >= y && point.y <= y + h
        } else {
            // 모든 stencil quad 내부여야 함
            self.stencil_quads.iter().all(|q| q.contains_point(point))
        }
    }
}

impl PartialEq for SlateClippingState {
    fn eq(&self, other: &Self) -> bool {
        self.always_clip == other.always_clip
            && self.scissor_rect == other.scissor_rect
            && self.stencil_quads == other.stencil_quads
    }
}

// ============================================================================
// SlateClippingManager
// ============================================================================

/// 클리핑 매니저 (UE5 FSlateClippingManager)
///
/// 위젯 트리 순회 중 push/pop으로 클리핑 존을 관리.
/// 각 push 시 이전 상태와 합성하여 새 `SlateClippingState` 생성.
/// DrawElementList에서 사용.
pub struct SlateClippingManager {
    /// 클리핑 상태 풀 (인덱스로 참조, 중복 제거)
    states: Vec<SlateClippingState>,
    /// 클리핑 스택 (states 인덱스)
    stack: Vec<usize>,
}

impl Default for SlateClippingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SlateClippingManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlateClippingManager")
            .field("states_count", &self.states.len())
            .field("stack_depth", &self.stack.len())
            .finish()
    }
}

impl SlateClippingManager {
    /// 새 매니저 생성
    pub fn new() -> Self {
        Self {
            states: Vec::new(),
            stack: Vec::new(),
        }
    }

    /// 프레임 시작 시 초기화
    pub fn reset(&mut self) {
        self.states.clear();
        self.stack.clear();
    }

    /// 클리핑 존 push (UE5 FSlateClippingManager::PushClip)
    ///
    /// 이전 상태와 합성하여 새 클리핑 상태 생성.
    /// 반환: 새 상태의 인덱스.
    pub fn push_clip(&mut self, zone: SlateClippingZone) -> usize {
        let new_state = self.create_clipping_state(&zone);
        let index = self.add_unique_state(new_state);
        self.stack.push(index);
        index
    }

    /// 클리핑 pop (UE5 FSlateClippingManager::PopClip)
    pub fn pop_clip(&mut self) {
        if self.stack.is_empty() {
            log::error!("[SlateClippingManager] pop_clip below 0");
        } else {
            self.stack.pop();
        }
    }

    /// 현재 클립 상태 인덱스 (None = 클리핑 없음)
    pub fn current_clip_index(&self) -> Option<usize> {
        self.stack.last().copied()
    }

    /// 인덱스로 상태 참조
    pub fn get_state(&self, index: usize) -> &SlateClippingState {
        &self.states[index]
    }

    /// 스택 깊이
    pub fn stack_depth(&self) -> usize {
        self.stack.len()
    }

    /// 모든 상태 참조 (렌더러에서 사용)
    pub fn states(&self) -> &[SlateClippingState] {
        &self.states
    }

    // ========== 내부 합성 로직 ==========

    /// 이전 클리핑 상태 조회 (UE5 GetPreviousClippingState)
    fn get_previous_state(&self, will_intersect: bool) -> Option<&SlateClippingState> {
        if !will_intersect {
            // intersect=false → AlwaysClip인 조상까지 역스캔
            for &index in self.stack.iter().rev() {
                let state = &self.states[index];
                if state.always_clip {
                    return Some(state);
                }
            }
            None
        } else if let Some(&top) = self.stack.last() {
            Some(&self.states[top])
        } else {
            None
        }
    }

    /// 클리핑 상태 합성 (UE5 CreateClippingState)
    fn create_clipping_state(&self, zone: &SlateClippingZone) -> SlateClippingState {
        let previous = self.get_previous_state(zone.should_intersect);
        let always_clip = zone.always_clip;

        match previous {
            None => {
                // 이전 상태 없음 — 새로 시작
                if zone.is_axis_aligned {
                    SlateClippingState {
                        scissor_rect: Some(zone.to_scissor_rect()),
                        stencil_quads: Vec::new(),
                        always_clip,
                    }
                } else {
                    SlateClippingState {
                        scissor_rect: None,
                        stencil_quads: vec![*zone],
                        always_clip,
                    }
                }
            }
            Some(prev) => {
                match prev.clipping_method() {
                    EClippingMethod::Scissor => {
                        if zone.is_axis_aligned {
                            // Scissor + AxisAligned → Scissor 교차
                            let prev_zone = SlateClippingZone::from_rect(prev.scissor_rect.unwrap());
                            let intersected = prev_zone.intersect_axis_aligned(zone);
                            SlateClippingState {
                                scissor_rect: Some(intersected.to_scissor_rect()),
                                stencil_quads: Vec::new(),
                                always_clip: always_clip || prev.always_clip,
                            }
                        } else {
                            // Scissor + Rotated → Stencil (이전 scissor quad + 현재 quad)
                            let prev_zone = SlateClippingZone::from_rect(prev.scissor_rect.unwrap());
                            SlateClippingState {
                                scissor_rect: None,
                                stencil_quads: vec![prev_zone, *zone],
                                always_clip: always_clip || prev.always_clip,
                            }
                        }
                    }
                    EClippingMethod::Stencil => {
                        // Stencil + Any → Stencil (기존 quads + 현재 quad)
                        let mut quads = prev.stencil_quads.clone();
                        quads.push(*zone);
                        SlateClippingState {
                            scissor_rect: None,
                            stencil_quads: quads,
                            always_clip: always_clip || prev.always_clip,
                        }
                    }
                }
            }
        }
    }

    /// 상태 풀에 추가 (중복이면 기존 인덱스 반환)
    fn add_unique_state(&mut self, state: SlateClippingState) -> usize {
        // 간단한 선형 탐색 (상태 수가 적으므로 충분)
        for (i, existing) in self.states.iter().enumerate() {
            if existing == &state {
                return i;
            }
        }
        let index = self.states.len();
        self.states.push(state);
        index
    }
}
