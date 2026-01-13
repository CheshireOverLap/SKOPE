//! SKOPE Game UI System
//!
//! RON-based declarative UI with data binding and animations.
//!
//! # Features
//! - `gpu`: Enable wgpu-dependent code (renderer, text_renderer)
//!
//! # Architecture
//! UiSystem은 여러 독립적인 서브시스템으로 구성됨:
//! - InputSystem: 마우스/키보드 입력 처리
//! - LayoutSystem: Flexbox 기반 레이아웃
//! - AnimationSystem: 키프레임 애니메이션
//! - BindingSystem: 데이터 바인딩
//! - StateManager: 위젯 상태 관리

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(clippy::too_many_arguments)]

// Core modules
pub mod types;
pub mod parser;
pub mod layout;
pub mod binding;
pub mod animation;
pub mod style;
pub mod hot_reload;
pub mod ui_asset;

// Subsystems
pub mod systems;

// Main UiSystem
pub mod ui_system;

// GPU-dependent modules
#[cfg(feature = "gpu")]
pub mod renderer;
#[cfg(feature = "gpu")]
pub mod text_renderer;

// ==================== Re-exports ====================

// Types
pub use types::*;

// Parser
pub use parser::*;

// Layout (기존 함수들 호환성 유지)
pub use layout::{calculate_widget_layout, hit_test};

// Binding
pub use binding::{BindingValue, BindingContext};

// Animation
pub use animation::{
    ActiveAnimation, AnimationRepeat, AnimationTrack, AnimationBuilder,
    AnimatedProperty, AnimatedValue, Keyframe, create_state_transition,
    presets as animation_presets,
};

// Style
pub use style::*;

// Hot reload
pub use hot_reload::*;

// UiSystem and related types
pub use ui_system::{UiSystem, UiError, WidgetHandle};
pub use systems::UiEvent;

// UiAsset (for .ui.ron files)
pub use ui_asset::{UiAsset, UiAssetMetadata, AiNote, UiAssetError, CURRENT_SCHEMA};

// Subsystems (for advanced usage)
pub use systems::{
    InputSystem, LayoutSystem, AnimationSystem, BindingSystem, StateManager,
};
pub use systems::input_system::{
    DragState, TooltipInfo, DragRenderInfo, SpecialKey,
};

// GPU modules
#[cfg(feature = "gpu")]
pub use renderer::*;
#[cfg(feature = "gpu")]
pub use text_renderer::TextRenderer;

// ==================== 호환성을 위한 re-export ====================

/// ScrollView 컨텐츠 크기 계산 (호환성)
pub use layout::calculate_widget_layout as calculate_layout;

/// calculate_scroll_content_size 호환성 함수
pub fn calculate_scroll_content_size(widget: &mut Widget) {
    if !matches!(widget.widget_type, WidgetType::ScrollView { .. }) {
        return;
    }

    let mut max_x: f32 = 0.0;
    let mut max_y: f32 = 0.0;

    for child in &widget.children {
        let child_right = child.computed_rect.x + child.computed_rect.width - widget.computed_rect.x;
        let child_bottom = child.computed_rect.y + child.computed_rect.height - widget.computed_rect.y;

        max_x = max_x.max(child_right);
        max_y = max_y.max(child_bottom);
    }

    widget.content_size = (max_x, max_y);
}
