//! TAttribute Pattern - 런타임 데이터 바인딩 (언리얼 Slate의 TAttribute)
//!
//! 위젯 프로퍼티가 정적 값 또는 동적 바인딩을 가질 수 있게 합니다.

use std::sync::Arc;

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
// OptionalAttribute<T>
// ============================================================================

/// 선택적 속성 (설정되지 않을 수 있음)
pub enum OptionalAttribute<T: Clone + Send + Sync + 'static> {
    /// 설정되지 않음
    Unset,
    /// 정적 값
    Static(T),
    /// 동적 바인딩
    Bound(Arc<dyn Fn() -> T + Send + Sync>),
}

impl<T: Clone + Send + Sync + 'static> OptionalAttribute<T> {
    /// 설정되지 않음
    pub fn unset() -> Self {
        Self::Unset
    }

    /// 정적 값으로 설정
    pub fn from_value(value: T) -> Self {
        Self::Static(value)
    }

    /// 바인딩으로 설정
    pub fn bind<F>(getter: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self::Bound(Arc::new(getter))
    }

    /// 설정되어 있는지
    pub fn is_set(&self) -> bool {
        !matches!(self, Self::Unset)
    }

    /// 값 가져오기 (설정되어 있으면)
    pub fn get(&self) -> Option<T> {
        match self {
            Self::Unset => None,
            Self::Static(v) => Some(v.clone()),
            Self::Bound(f) => Some(f()),
        }
    }

    /// 기본값과 함께 가져오기
    pub fn get_or(&self, default: T) -> T {
        self.get().unwrap_or(default)
    }

    /// 기본값 클로저와 함께 가져오기
    pub fn get_or_else<F: FnOnce() -> T>(&self, default: F) -> T {
        self.get().unwrap_or_else(default)
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for OptionalAttribute<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Unset => Self::Unset,
            Self::Static(v) => Self::Static(v.clone()),
            Self::Bound(f) => Self::Bound(Arc::clone(f)),
        }
    }
}

impl<T: Clone + Send + Sync + 'static> Default for OptionalAttribute<T> {
    fn default() -> Self {
        Self::Unset
    }
}

impl<T: Clone + Send + Sync + 'static> From<T> for OptionalAttribute<T> {
    fn from(value: T) -> Self {
        Self::Static(value)
    }
}

impl<T: Clone + Send + Sync + 'static> From<Option<T>> for OptionalAttribute<T> {
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => Self::Static(v),
            None => Self::Unset,
        }
    }
}

// ============================================================================
// AttributeRef<T> - 참조 기반 바인딩
// ============================================================================

/// 참조 기반 속성 (Arc로 공유된 데이터 참조)
///
/// 데이터 소스가 Arc로 래핑된 경우 편리하게 사용 가능
pub struct AttributeRef<T: Clone + Send + Sync + 'static> {
    source: Arc<dyn Fn() -> T + Send + Sync>,
}

impl<T: Clone + Send + Sync + 'static> AttributeRef<T> {
    /// 새 참조 속성 생성
    pub fn new<F>(getter: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            source: Arc::new(getter),
        }
    }

    /// Arc로 공유된 데이터에서 생성
    pub fn from_arc<S, F>(data: Arc<S>, accessor: F) -> Self
    where
        S: Send + Sync + 'static,
        F: Fn(&S) -> T + Send + Sync + 'static,
    {
        Self {
            source: Arc::new(move || accessor(&data)),
        }
    }

    /// 값 가져오기
    pub fn get(&self) -> T {
        (self.source)()
    }

    /// Attribute로 변환
    pub fn into_attribute(self) -> Attribute<T> {
        Attribute::Bound(self.source)
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for AttributeRef<T> {
    fn clone(&self) -> Self {
        Self {
            source: Arc::clone(&self.source),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

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

    #[test]
    fn test_optional_attribute() {
        let attr: OptionalAttribute<i32> = OptionalAttribute::unset();
        assert!(!attr.is_set());
        assert_eq!(attr.get(), None);
        assert_eq!(attr.get_or(100), 100);

        let attr: OptionalAttribute<i32> = OptionalAttribute::from_value(42);
        assert!(attr.is_set());
        assert_eq!(attr.get(), Some(42));
    }

    #[test]
    fn test_attribute_ref() {
        let flag = Arc::new(AtomicBool::new(false));
        let flag_clone = Arc::clone(&flag);

        let attr_ref = AttributeRef::new(move || flag_clone.load(Ordering::Relaxed));
        assert!(!attr_ref.get());

        flag.store(true, Ordering::Relaxed);
        assert!(attr_ref.get());
    }
}
