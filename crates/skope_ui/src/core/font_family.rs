//! Font family types for multi-font support

/// 폰트 패밀리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontFamily {
    /// UI 폰트 (Noto Sans KR 등)
    #[default]
    UI,
    /// 모노스페이스 폰트 (JetBrains Mono + D2Coding 폴백)
    Monospace,
}
