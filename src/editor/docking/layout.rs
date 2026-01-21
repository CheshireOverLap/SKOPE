//! Layout Save/Load System
//!
//! RON serialization for docking layouts and preset layouts
//!
//! ## 확장된 기능 (Phase 5)
//! - EditorLayout: 전체 에디터 레이아웃 (메인 윈도우 + 플로팅 윈도우)
//! - WindowState: 윈도우 위치/크기/최대화 상태
//! - FloatingWindowState: 플로팅 윈도우 정보
//! - 멀티모니터 복원 로직

use serde::{Deserialize, Serialize};
use egui_dock::{DockState, NodeIndex};
use super::Tab;
use std::path::Path;

// ============================================================================
// Extended Layout Types (Phase 5)
// ============================================================================

/// 윈도우 상태 (위치, 크기, 최대화)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    /// 윈도우 위치 (None이면 시스템 기본)
    pub position: Option<(i32, i32)>,
    /// 윈도우 크기
    pub size: (u32, u32),
    /// 최대화 상태
    pub maximized: bool,
    /// 모니터 인덱스 (0 = 주 모니터)
    pub monitor_index: usize,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            position: None,
            size: (1920, 1080),
            maximized: false,
            monitor_index: 0,
        }
    }
}

impl WindowState {
    /// 새 WindowState 생성
    pub fn new(position: Option<(i32, i32)>, size: (u32, u32), maximized: bool) -> Self {
        Self {
            position,
            size,
            maximized,
            monitor_index: 0,
        }
    }

    /// 위치가 지정된 모니터 범위 내에 있는지 확인
    pub fn is_visible_on_monitors(&self, monitors: &[MonitorInfo]) -> bool {
        let Some((x, y)) = self.position else {
            return true; // 위치가 없으면 시스템이 처리
        };

        for monitor in monitors {
            if monitor.contains_point(x, y) {
                return true;
            }
        }

        false
    }

    /// 가장 가까운 모니터로 위치 조정
    pub fn clamp_to_monitors(&mut self, monitors: &[MonitorInfo]) {
        if monitors.is_empty() {
            return;
        }

        let Some((x, y)) = self.position else {
            return;
        };

        // 현재 위치가 어느 모니터에도 없으면 주 모니터로 이동
        if !self.is_visible_on_monitors(monitors) {
            let primary = &monitors[0];
            self.position = Some((
                primary.x + 100,
                primary.y + 100,
            ));
            self.monitor_index = 0;
            log::info!("[Layout] Window position clamped to primary monitor");
        }
    }
}

/// 모니터 정보
#[derive(Debug, Clone)]
pub struct MonitorInfo {
    /// 모니터 이름
    pub name: String,
    /// 왼쪽 위 X 좌표
    pub x: i32,
    /// 왼쪽 위 Y 좌표
    pub y: i32,
    /// 너비
    pub width: u32,
    /// 높이
    pub height: u32,
    /// 주 모니터 여부
    pub is_primary: bool,
}

impl MonitorInfo {
    /// 점이 모니터 범위 내에 있는지 확인
    pub fn contains_point(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && x < self.x + self.width as i32
            && y >= self.y
            && y < self.y + self.height as i32
    }

    /// 영역의 중심 좌표
    pub fn center(&self) -> (i32, i32) {
        (
            self.x + self.width as i32 / 2,
            self.y + self.height as i32 / 2,
        )
    }
}

/// 플로팅 윈도우 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatingWindowState {
    /// 탭 종류
    pub tab: String,
    /// 윈도우 상태
    pub window: WindowState,
}

/// 전체 에디터 레이아웃 (Phase 5 확장)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorLayout {
    /// 레이아웃 이름
    pub name: String,
    /// 버전 (마이그레이션용)
    pub version: u32,
    /// 메인 윈도우 상태
    pub main_window: WindowState,
    /// Dock 레이아웃 (탭 구조)
    pub dock_state: LayoutStructure,
    /// 플로팅 윈도우들
    pub floating_windows: Vec<FloatingWindowState>,
}

impl EditorLayout {
    /// 현재 레이아웃 버전
    pub const CURRENT_VERSION: u32 = 2;

    /// 새 레이아웃 생성
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: Self::CURRENT_VERSION,
            main_window: WindowState::default(),
            dock_state: LayoutStructure { regions: vec![] },
            floating_windows: Vec::new(),
        }
    }

    /// 파일에 저장
    pub fn save_to_file(&self, path: &Path) -> Result<(), LayoutError> {
        let ron_str = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| LayoutError::SerializationError(e.to_string()))?;
        std::fs::write(path, ron_str)
            .map_err(|e| LayoutError::IoError(e.to_string()))?;
        log::info!("[EditorLayout] Saved to {:?}", path);
        Ok(())
    }

    /// 파일에서 로드
    pub fn load_from_file(path: &Path) -> Result<Self, LayoutError> {
        let ron_str = std::fs::read_to_string(path)
            .map_err(|e| LayoutError::IoError(e.to_string()))?;
        let layout: EditorLayout = ron::from_str(&ron_str)
            .map_err(|e| LayoutError::DeserializationError(e.to_string()))?;
        log::info!("[EditorLayout] Loaded from {:?}", path);
        Ok(layout)
    }

    /// 모니터 정보를 사용하여 레이아웃 복원
    ///
    /// 화면 밖에 있는 윈도우를 가시 영역으로 이동시킵니다.
    pub fn restore_with_monitors(&mut self, monitors: &[MonitorInfo]) {
        if monitors.is_empty() {
            log::warn!("[EditorLayout] No monitors available for restoration");
            return;
        }

        // 메인 윈도우 위치 조정
        self.main_window.clamp_to_monitors(monitors);

        // 플로팅 윈도우 위치 조정
        for floating in &mut self.floating_windows {
            floating.window.clamp_to_monitors(monitors);
        }

        log::info!(
            "[EditorLayout] Restored layout '{}' for {} monitors",
            self.name,
            monitors.len()
        );
    }

    /// 플로팅 윈도우 추가
    pub fn add_floating_window(&mut self, tab: &str, state: WindowState) {
        self.floating_windows.push(FloatingWindowState {
            tab: tab.to_string(),
            window: state,
        });
    }

    /// 플로팅 윈도우 제거
    pub fn remove_floating_window(&mut self, tab: &str) {
        self.floating_windows.retain(|f| f.tab != tab);
    }
}

/// 레이아웃 프리셋 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutPresetType {
    /// 기본 (Unity 스타일)
    Standard,
    /// 코딩용 (Console/Inspector 확대)
    Coding,
    /// 레벨 디자인용 (Scene 뷰 최대화)
    LevelDesign,
    /// 애니메이션 작업용 (Timeline 확대)
    Animation,
    /// 디버깅용 (Console/Debug 확대)
    Debugging,
}

impl LayoutPresetType {
    /// 프리셋 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Coding => "Coding",
            Self::LevelDesign => "Level Design",
            Self::Animation => "Animation",
            Self::Debugging => "Debugging",
        }
    }

    /// 모든 프리셋 목록
    pub fn all() -> &'static [Self] {
        &[
            Self::Standard,
            Self::Coding,
            Self::LevelDesign,
            Self::Animation,
            Self::Debugging,
        ]
    }
}

// ============================================================================
// Legacy Layout Types (Compatibility)
// ============================================================================

/// Serializable layout representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutData {
    /// Layout name
    pub name: String,
    /// Layout version (for migration)
    pub version: u32,
    /// Simplified layout structure (tab names in order)
    pub structure: LayoutStructure,
}

/// Simplified layout structure for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutStructure {
    /// Active tabs in each region
    pub regions: Vec<LayoutRegion>,
}

/// A region in the layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutRegion {
    /// Position hint (e.g., "left", "center", "right", "bottom")
    pub position: String,
    /// Fraction of parent (0.0-1.0)
    pub fraction: f32,
    /// Tabs in this region
    pub tabs: Vec<String>,
}

impl LayoutData {
    /// Current layout version
    pub const CURRENT_VERSION: u32 = 1;

    /// Create from preset name
    pub fn from_preset(name: &str) -> Self {
        Self {
            name: name.to_string(),
            version: Self::CURRENT_VERSION,
            structure: LayoutStructure { regions: vec![] },
        }
    }

    /// Save to file (RON format)
    pub fn save_to_file(&self, path: &Path) -> Result<(), LayoutError> {
        let ron_str = ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| LayoutError::SerializationError(e.to_string()))?;
        std::fs::write(path, ron_str)
            .map_err(|e| LayoutError::IoError(e.to_string()))?;
        log::info!("[Layout] Saved to {:?}", path);
        Ok(())
    }

    /// Load from file (RON format)
    pub fn load_from_file(path: &Path) -> Result<Self, LayoutError> {
        let ron_str = std::fs::read_to_string(path)
            .map_err(|e| LayoutError::IoError(e.to_string()))?;
        let layout: LayoutData = ron::from_str(&ron_str)
            .map_err(|e| LayoutError::DeserializationError(e.to_string()))?;
        log::info!("[Layout] Loaded from {:?}", path);
        Ok(layout)
    }
}

/// Layout error types
#[derive(Debug)]
pub enum LayoutError {
    IoError(String),
    SerializationError(String),
    DeserializationError(String),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::IoError(e) => write!(f, "IO error: {}", e),
            LayoutError::SerializationError(e) => write!(f, "Serialization error: {}", e),
            LayoutError::DeserializationError(e) => write!(f, "Deserialization error: {}", e),
        }
    }
}

/// Preset layouts
pub struct LayoutPresets;

impl LayoutPresets {
    /// Default Unity-style layout (Game view is PiP in Scene)
    pub fn default_layout() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene]);

        // Inspector on right (full height, 20%)
        let [left_area, _inspector] = dock_state.main_surface_mut()
            .split_right(NodeIndex::root(), 0.80, vec![Tab::Inspector]);

        // Bottom: Assets + Console (28%)
        let [top_area, _bottom] = dock_state.main_surface_mut()
            .split_below(left_area, 0.72, vec![Tab::Assets, Tab::Console]);

        // Left: Hierarchy (22%)
        let [_hierarchy, _viewport] = dock_state.main_surface_mut()
            .split_left(top_area, 0.22, vec![Tab::Hierarchy]);

        dock_state
    }

    /// 2x3 Grid layout (Game view is PiP in Scene)
    pub fn layout_2x3() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene]);

        // Split into 2 rows
        let [top, bottom] = dock_state.main_surface_mut()
            .split_below(NodeIndex::root(), 0.5, vec![Tab::Assets]);

        // Top row: 2 columns (Scene + Inspector)
        let [_top_left, _top_right] = dock_state.main_surface_mut()
            .split_right(top, 0.6, vec![Tab::Inspector]);

        // Bottom row: 3 columns
        let [_bottom_left, bottom_mid] = dock_state.main_surface_mut()
            .split_right(bottom, 0.33, vec![Tab::Hierarchy]);
        let [_bottom_mid, _bottom_right] = dock_state.main_surface_mut()
            .split_right(bottom_mid, 0.5, vec![Tab::Console]);

        dock_state
    }

    /// 4-Split layout (quad view, Game is PiP)
    pub fn layout_4_split() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene]);

        // Split into 2x2
        let [top, bottom] = dock_state.main_surface_mut()
            .split_below(NodeIndex::root(), 0.5, vec![Tab::Assets, Tab::Console]);

        let [_top_left, _top_right] = dock_state.main_surface_mut()
            .split_right(top, 0.5, vec![Tab::Inspector]);

        let [_bottom_left, _bottom_right] = dock_state.main_surface_mut()
            .split_right(bottom, 0.5, vec![Tab::Hierarchy]);

        dock_state
    }

    /// Wide layout (Scene in center, panels on sides)
    pub fn layout_wide() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene]);

        // Right panel (narrow)
        let [center, _right] = dock_state.main_surface_mut()
            .split_right(NodeIndex::root(), 0.85, vec![Tab::Inspector]);

        // Left panel (narrow)
        let [_left, center] = dock_state.main_surface_mut()
            .split_left(center, 0.15, vec![Tab::Hierarchy]);

        // Bottom (very narrow)
        let [_viewport, _bottom] = dock_state.main_surface_mut()
            .split_below(center, 0.88, vec![Tab::Assets, Tab::Console]);

        dock_state
    }

    /// Tall layout (vertical emphasis)
    pub fn layout_tall() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene]);

        // Split into 3 vertical columns
        let [left, _right] = dock_state.main_surface_mut()
            .split_right(NodeIndex::root(), 0.7, vec![Tab::Inspector]);

        let [_hierarchy, _viewport] = dock_state.main_surface_mut()
            .split_left(left, 0.25, vec![Tab::Hierarchy, Tab::Assets, Tab::Console]);

        dock_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_presets() {
        // Just ensure they don't panic
        let _ = LayoutPresets::default_layout();
        let _ = LayoutPresets::layout_2x3();
        let _ = LayoutPresets::layout_4_split();
        let _ = LayoutPresets::layout_wide();
        let _ = LayoutPresets::layout_tall();
    }
}
