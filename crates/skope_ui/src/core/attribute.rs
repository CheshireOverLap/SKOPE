//! TAttribute Pattern - 런타임 데이터 바인딩 (언리얼 Slate의 TAttribute)
//!
//! 위젯 프로퍼티가 정적 값 또는 동적 바인딩을 가질 수 있게 합니다.
//!
//! ## Invalidation 시스템
//!
//! 언리얼 Slate의 `EInvalidateWidgetReason` + `TSlateAttribute` 패턴 구현:
//! - `InvalidateWidgetReason`: 위젯 무효화 이유 비트플래그
//! - `SlateAttribute<T>`: 캐시된 값 + 변경 감지 + 자동 무효화

use std::sync::Arc;

// ============================================================================
// InvalidateWidgetReason (언리얼 EInvalidateWidgetReason)
// ============================================================================

/// 위젯 무효화 이유 비트플래그
///
/// 언리얼 Slate의 `EInvalidateWidgetReason`에 해당합니다.
/// 각 비트는 독립적인 무효화 이유를 나타내며 조합 가능합니다.
///
/// # 사용 예시
/// ```rust
/// let reason = InvalidateWidgetReason::PAINT | InvalidateWidgetReason::LAYOUT;
/// assert!(reason.contains(InvalidateWidgetReason::PAINT));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct InvalidateWidgetReason(u8);

impl InvalidateWidgetReason {
    /// 무효화 없음
    pub const NONE: Self = Self(0);
    /// 레이아웃 재계산 필요 (크기 변경)
    pub const LAYOUT: Self = Self(1 << 0);
    /// 다시 그려야 함 (시각적 변경)
    pub const PAINT: Self = Self(1 << 1);
    /// Volatility 변경 (업데이트 빈도 변경) — UE5.7 EInvalidateWidgetReason::Volatility
    #[allow(dead_code)]
    pub const VOLATILITY: Self = Self(1 << 2);
    /// 자식 구조 변경 (추가/삭제/순서 변경) — UE5.7 EInvalidateWidgetReason::ChildOrder = 1 << 3
    #[allow(dead_code)]
    pub const CHILD_ORDER: Self = Self(1 << 3);
    /// 렌더 트랜스폼 변경 — UE5.7 EInvalidateWidgetReason::RenderTransform = 1 << 4
    pub const RENDER_TRANSFORM: Self = Self(1 << 4);
    /// 가시성 변경 — UE5.7 EInvalidateWidgetReason::Visibility = 1 << 5
    pub const VISIBILITY: Self = Self(1 << 5);
    /// Prepass 시 desired size 재캐시 필요 — UE5.7 EInvalidateWidgetReason::Prepass = 1 << 7
    #[allow(dead_code)]
    pub const PREPASS: Self = Self(1 << 7);

    /// 편의 조합: Paint + Volatility — UE5.7 completeness, not yet consumed
    #[allow(dead_code)]
    pub const PAINT_AND_VOLATILITY: Self = Self(Self::PAINT.0 | Self::VOLATILITY.0);
    /// 편의 조합: Layout + Volatility — UE5.7 completeness, not yet consumed
    #[allow(dead_code)]
    pub const LAYOUT_AND_VOLATILITY: Self = Self(Self::LAYOUT.0 | Self::VOLATILITY.0);

    /// 비어있는지 (무효화 없음)
    #[inline]
    pub const fn is_empty(self) -> bool { self.0 == 0 }

    /// 특정 플래그를 포함하는지
    #[inline]
    pub const fn contains(self, other: Self) -> bool { (self.0 & other.0) == other.0 }

    /// 모든 플래그를 포함하는지
    #[inline]
    pub const fn all() -> Self { Self(0b1011_1111) }

    /// 내부 비트 값
    #[inline]
    pub const fn bits(self) -> u8 { self.0 }
}

impl std::ops::BitOr for InvalidateWidgetReason {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self { Self(self.0 | rhs.0) }
}

impl std::ops::BitOrAssign for InvalidateWidgetReason {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}

impl std::ops::BitAnd for InvalidateWidgetReason {
    type Output = Self;
    #[inline]
    fn bitand(self, rhs: Self) -> Self { Self(self.0 & rhs.0) }
}

impl std::ops::Not for InvalidateWidgetReason {
    type Output = Self;
    #[inline]
    fn not(self) -> Self { Self(!self.0 & Self::all().0) }
}

// ============================================================================
// SlateAttribute<T> (언리얼 TSlateAttribute)
// ============================================================================

/// 위젯 소유 속성 - 캐시된 값 + 변경 감지 + 자동 무효화
///
/// 언리얼 Slate의 `TSlateAttribute<ObjectType, EInvalidateWidgetReason>`에 해당합니다.
/// Prepass 단계에서 `update()`를 호출하면 바인딩을 재평가하고,
/// 값이 변경된 경우에만 무효화 이유를 반환합니다.
///
/// # 사용 예시
/// ```rust
/// // 정적 값
/// let mut attr = SlateAttribute::new(
///     Attribute::from(Color::WHITE),
///     InvalidateWidgetReason::PAINT,
/// );
///
/// // 동적 바인딩
/// let mut attr = SlateAttribute::new(
///     Attribute::bind(|| get_current_color()),
///     InvalidateWidgetReason::PAINT,
/// );
///
/// // Prepass에서 업데이트
/// if let Some(reason) = attr.update() {
///     dirty_flags |= reason;
/// }
/// ```
pub struct SlateAttribute<T: Clone + PartialEq + Send + Sync + 'static> {
    attr: Attribute<T>,
    cached: T,
    reason: InvalidateWidgetReason,
    /// `set()` 호출로 값이 변경되었는지 추적
    dirty_from_set: bool,
}

impl<T: Clone + PartialEq + Send + Sync + 'static> SlateAttribute<T> {
    /// 새 SlateAttribute 생성
    pub fn new(attr: Attribute<T>, reason: InvalidateWidgetReason) -> Self {
        let cached = attr.get();
        Self { attr, cached, reason, dirty_from_set: false }
    }

    /// 정적 값으로 생성
    pub fn from_value(value: T, reason: InvalidateWidgetReason) -> Self {
        Self {
            cached: value.clone(),
            attr: Attribute::Static(value),
            reason,
            dirty_from_set: false,
        }
    }

    /// Prepass 업데이트: 값 변경 감지 → reason 반환
    ///
    /// 1. `set()` 호출로 값이 변경된 경우 → `Some(reason)` 반환
    /// 2. 바인딩이면 클로저를 호출하고, 캐시와 비교하여 변경 시 `Some(reason)` 반환
    /// 3. 정적 값이고 `set()` 호출 없으면 → `None` (변경 없음)
    pub fn update(&mut self) -> Option<InvalidateWidgetReason> {
        // set() 호출로 인한 변경 감지
        if self.dirty_from_set {
            self.dirty_from_set = false;
            return Some(self.reason);
        }
        // 바인딩 재평가
        if self.attr.is_static() {
            return None;
        }
        let new_val = self.attr.get();
        if new_val != self.cached {
            self.cached = new_val;
            Some(self.reason)
        } else {
            None
        }
    }

    /// 캐시된 값 참조 (Prepass 이후 유효)
    #[inline]
    pub fn get(&self) -> &T { &self.cached }

    /// 캐시된 값 복제
    #[inline]
    pub fn get_cloned(&self) -> T { self.cached.clone() }

    /// 정적 값 직접 설정 (바인딩 해제)
    ///
    /// 값이 변경된 경우 다음 `update()` 호출 시 무효화 이유를 반환합니다.
    pub fn set(&mut self, value: T) {
        if self.cached != value {
            self.cached = value.clone();
            self.attr = Attribute::Static(value);
            self.dirty_from_set = true;
        }
    }

    /// 바인딩 설정
    pub fn bind<F>(&mut self, getter: F)
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        self.attr = Attribute::bind(getter);
    }

    /// Attribute 교체 (Construct 시)
    ///
    /// 새 Attribute의 현재 값이 기존 캐시와 다르면 dirty 처리합니다.
    pub fn assign(&mut self, attr: Attribute<T>) {
        let new_cached = attr.get();
        if new_cached != self.cached {
            self.dirty_from_set = true;
        }
        self.cached = new_cached;
        self.attr = attr;
    }

    /// 바인딩되어 있는지
    pub fn is_bound(&self) -> bool { self.attr.is_bound() }

    /// 무효화 이유
    pub fn invalidation_reason(&self) -> InvalidateWidgetReason { self.reason }
}

impl<T: Clone + PartialEq + Send + Sync + std::fmt::Debug + 'static> std::fmt::Debug
    for SlateAttribute<T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlateAttribute")
            .field("cached", &self.cached)
            .field("bound", &self.attr.is_bound())
            .field("reason", &self.reason)
            .finish()
    }
}

impl<T: Clone + PartialEq + Send + Sync + 'static> Clone for SlateAttribute<T> {
    fn clone(&self) -> Self {
        Self {
            attr: self.attr.clone(),
            cached: self.cached.clone(),
            reason: self.reason,
            dirty_from_set: false,
        }
    }
}

// ============================================================================
// Attribute<T>
// ============================================================================

/// 속성 값 (정적 또는 동적 바인딩)
///
/// 언리얼 Slate의 `TAttribute<T>`에 해당합니다.
///
/// # 사용 예시
/// ```rust
/// // 정적 값
/// let attr = Attribute::from(true);
///
/// // 동적 바인딩
/// let attr = Attribute::bind(|| some_data.is_visible());
///
/// // 값 얻기
/// let value = attr.get();
/// ```
pub enum Attribute<T: Clone + Send + Sync + 'static> {
    /// 정적 값
    Static(T),
    /// 동적 바인딩 (getter 클로저)
    Bound(Arc<dyn Fn() -> T + Send + Sync>),
}

impl<T: Clone + Send + Sync + 'static> Attribute<T> {
    /// 정적 값으로 생성
    pub fn from_value(value: T) -> Self {
        Self::Static(value)
    }

    /// 바인딩으로 생성
    pub fn bind<F>(getter: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self::Bound(Arc::new(getter))
    }

    /// 현재 값 가져오기
    pub fn get(&self) -> T {
        match self {
            Self::Static(v) => v.clone(),
            Self::Bound(f) => f(),
        }
    }

    /// 값이 바인딩되어 있는지
    pub fn is_bound(&self) -> bool {
        matches!(self, Self::Bound(_))
    }

    /// 정적 값인지
    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static(_))
    }

    /// 정적 값이면 참조 반환, 바인딩이면 None
    pub fn get_static(&self) -> Option<&T> {
        match self {
            Self::Static(v) => Some(v),
            Self::Bound(_) => None,
        }
    }

    /// 정적 값 설정 (바인딩 해제)
    pub fn set(&mut self, value: T) {
        *self = Self::Static(value);
    }
}

// Clone 구현
impl<T: Clone + Send + Sync + 'static> Clone for Attribute<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Static(v) => Self::Static(v.clone()),
            Self::Bound(f) => Self::Bound(Arc::clone(f)),
        }
    }
}

// Debug 구현
impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> std::fmt::Debug for Attribute<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Static(v) => f.debug_tuple("Static").field(v).finish(),
            Self::Bound(_) => f.debug_tuple("Bound").field(&"<closure>").finish(),
        }
    }
}

// From 구현 - 값에서 직접 변환
impl<T: Clone + Send + Sync + 'static> From<T> for Attribute<T> {
    fn from(value: T) -> Self {
        Self::Static(value)
    }
}

// Default 구현
impl<T: Clone + Send + Sync + Default + 'static> Default for Attribute<T> {
    fn default() -> Self {
        Self::Static(T::default())
    }
}

// ============================================================================
// update_attributes! 매크로
// ============================================================================

/// SlateAttribute 필드들을 일괄 업데이트하고 무효화 이유를 수집하는 매크로.
///
/// Widget::update_attributes() 구현 시 사용합니다.
///
/// # 사용 예시
/// ```rust
/// fn update_attributes(&mut self) -> InvalidateWidgetReason {
///     update_attributes!(self, text, color, font_size)
/// }
/// ```
#[macro_export]
macro_rules! update_attributes {
    ($self:expr, $($field:ident),+ $(,)?) => {{
        let mut reason = $crate::core::InvalidateWidgetReason::NONE;
        $(
            if let Some(r) = $self.$field.update() {
                reason = reason | r;
            }
        )+
        reason
    }};
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn test_static_attribute() {
        let attr: Attribute<i32> = Attribute::from(42);
        assert!(attr.is_static());
        assert!(!attr.is_bound());
        assert_eq!(attr.get(), 42);
    }

    #[test]
    fn test_bound_attribute() {
        let counter = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let counter_clone = Arc::clone(&counter);

        let attr = Attribute::bind(move || counter_clone.load(Ordering::Relaxed));

        assert!(!attr.is_static());
        assert!(attr.is_bound());
        assert_eq!(attr.get(), 0);

        counter.store(10, Ordering::Relaxed);
        assert_eq!(attr.get(), 10);
    }

    #[test]
    fn test_from_conversion() {
        let attr: Attribute<bool> = true.into();
        assert!(attr.is_static());
        assert!(attr.get());
    }

}
