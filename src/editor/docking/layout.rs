//! Layout Save/Load System
//!
//! RON serialization for docking layouts and preset layouts

use serde::{Deserialize, Serialize};
use egui_dock::{DockState, NodeIndex};
use super::Tab;
use std::path::Path;

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
    /// Default Unity-style layout
    pub fn default_layout() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene, Tab::Game]);

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

    /// 2x3 Grid layout
    pub fn layout_2x3() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene]);

        // Split into 2 rows
        let [top, bottom] = dock_state.main_surface_mut()
            .split_below(NodeIndex::root(), 0.5, vec![Tab::Assets]);

        // Top row: 3 columns
        let [_top_left, top_mid] = dock_state.main_surface_mut()
            .split_right(top, 0.33, vec![Tab::Game]);
        let [_top_mid, _top_right] = dock_state.main_surface_mut()
            .split_right(top_mid, 0.5, vec![Tab::Inspector]);

        // Bottom row: 3 columns
        let [_bottom_left, bottom_mid] = dock_state.main_surface_mut()
            .split_right(bottom, 0.33, vec![Tab::Hierarchy]);
        let [_bottom_mid, _bottom_right] = dock_state.main_surface_mut()
            .split_right(bottom_mid, 0.5, vec![Tab::Console]);

        dock_state
    }

    /// 4-Split layout (quad view)
    pub fn layout_4_split() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene]);

        // Split into 2x2
        let [top, bottom] = dock_state.main_surface_mut()
            .split_below(NodeIndex::root(), 0.5, vec![Tab::Assets, Tab::Console]);

        let [_top_left, _top_right] = dock_state.main_surface_mut()
            .split_right(top, 0.5, vec![Tab::Game]);

        let [_bottom_left, _bottom_right] = dock_state.main_surface_mut()
            .split_right(bottom, 0.5, vec![Tab::Hierarchy, Tab::Inspector]);

        dock_state
    }

    /// Wide layout (Scene in center, panels on sides)
    pub fn layout_wide() -> DockState<Tab> {
        let mut dock_state = DockState::new(vec![Tab::Scene, Tab::Game]);

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
        let mut dock_state = DockState::new(vec![Tab::Scene, Tab::Game]);

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
