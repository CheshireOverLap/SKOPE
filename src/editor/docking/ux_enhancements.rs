//! Docking UX Enhancements
//!
//! Visual Studio / Unity style docking UX improvements:
//! - Docking Compass (diamond drop guide)
//! - Ghost Preview (semi-transparent position preview)
//! - Snap Animation (smooth transition)
//! - ESC to cancel drag
//! - Tab overflow menu
//! - Layout undo system

use egui::{self, Color32, Context, Id, Pos2, Rect, Stroke, Vec2, Ui};
use std::collections::VecDeque;
use egui_dock::DockState;
use super::Tab;

/// Maximum number of layout states to keep for undo
const MAX_UNDO_HISTORY: usize = 20;

/// Animation duration in seconds
const SNAP_ANIMATION_DURATION: f32 = 0.15;

/// Drop zone detection threshold
const DROP_ZONE_THRESHOLD: f32 = 0.25;

// ============================================================================
// Docking Compass (Diamond Drop Guide)
// ============================================================================

/// Drop direction for docking compass
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropDirection {
    Left,
    Right,
    Top,
    Bottom,
    Center,  // Tab group
}

impl DropDirection {
    /// Get all directions
    pub fn all() -> &'static [DropDirection] {
        &[
            DropDirection::Left,
            DropDirection::Right,
            DropDirection::Top,
            DropDirection::Bottom,
            DropDirection::Center,
        ]
    }

    /// Get icon for direction
    pub fn icon(&self) -> &'static str {
        match self {
            DropDirection::Left => "◀",
            DropDirection::Right => "▶",
            DropDirection::Top => "▲",
            DropDirection::Bottom => "▼",
            DropDirection::Center => "◆",
        }
    }
}

/// Docking compass state
#[derive(Debug, Default)]
pub struct DockingCompass {
    /// Whether compass is visible
    pub visible: bool,
    /// Center position
    pub center: Pos2,
    /// Hovered direction
    pub hovered: Option<DropDirection>,
    /// Target rect (leaf being hovered)
    pub target_rect: Option<Rect>,
    /// Animation progress (0.0 - 1.0)
    pub anim_progress: f32,
}

impl DockingCompass {
    /// Create new compass
    pub fn new() -> Self {
        Self::default()
    }

    /// Show compass at position
    pub fn show(&mut self, center: Pos2, target_rect: Rect) {
        self.visible = true;
        self.center = center;
        self.target_rect = Some(target_rect);
    }

    /// Hide compass
    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered = None;
        self.target_rect = None;
        self.anim_progress = 0.0;
    }

    /// Update animation
    pub fn update(&mut self, dt: f32) {
        if self.visible {
            self.anim_progress = (self.anim_progress + dt / SNAP_ANIMATION_DURATION).min(1.0);
        } else {
            self.anim_progress = (self.anim_progress - dt / SNAP_ANIMATION_DURATION).max(0.0);
        }
    }

    /// Check if position is over a drop zone and return direction
    pub fn hit_test(&self, pos: Pos2) -> Option<DropDirection> {
        if !self.visible || self.anim_progress < 0.5 {
            return None;
        }

        let button_size = 32.0;
        let spacing = 40.0;

        for dir in DropDirection::all() {
            let btn_center = self.button_position(*dir, spacing);
            let btn_rect = Rect::from_center_size(btn_center, Vec2::splat(button_size));
            if btn_rect.contains(pos) {
                return Some(*dir);
            }
        }
        None
    }

    /// Get button position for direction
    fn button_position(&self, dir: DropDirection, spacing: f32) -> Pos2 {
        match dir {
            DropDirection::Left => Pos2::new(self.center.x - spacing, self.center.y),
            DropDirection::Right => Pos2::new(self.center.x + spacing, self.center.y),
            DropDirection::Top => Pos2::new(self.center.x, self.center.y - spacing),
            DropDirection::Bottom => Pos2::new(self.center.x, self.center.y + spacing),
            DropDirection::Center => self.center,
        }
    }

    /// Draw the compass
    pub fn draw(&self, ui: &mut Ui, pointer_pos: Option<Pos2>) {
        if self.anim_progress <= 0.0 {
            return;
        }

        let painter = ui.painter();
        let alpha = (self.anim_progress * 255.0) as u8;

        let button_size = 32.0;
        let spacing = 40.0;

        // Draw connecting lines (optional, subtle)
        let line_color = Color32::from_rgba_unmultiplied(80, 120, 180, alpha / 3);
        for dir in [DropDirection::Left, DropDirection::Right, DropDirection::Top, DropDirection::Bottom] {
            let btn_pos = self.button_position(dir, spacing);
            painter.line_segment(
                [self.center, btn_pos],
                Stroke::new(1.0, line_color),
            );
        }

        // Draw diamond background
        let diamond_bg = Color32::from_rgba_unmultiplied(30, 35, 45, alpha * 9 / 10);
        let diamond_stroke = Color32::from_rgba_unmultiplied(60, 100, 160, alpha);

        // Diamond shape points
        let diamond_size = spacing + button_size / 2.0 + 8.0;
        let points = [
            Pos2::new(self.center.x, self.center.y - diamond_size),  // top
            Pos2::new(self.center.x + diamond_size, self.center.y),  // right
            Pos2::new(self.center.x, self.center.y + diamond_size),  // bottom
            Pos2::new(self.center.x - diamond_size, self.center.y),  // left
        ];
        painter.add(egui::Shape::convex_polygon(
            points.to_vec(),
            diamond_bg,
            Stroke::new(1.5, diamond_stroke),
        ));

        // Get hovered direction
        let hovered_dir = pointer_pos.and_then(|p| self.hit_test(p));

        // Draw direction buttons
        for dir in DropDirection::all() {
            let btn_center = self.button_position(*dir, spacing);
            let is_hovered = hovered_dir == Some(*dir);

            let (bg_color, border_color, icon_color) = if is_hovered {
                (
                    Color32::from_rgba_unmultiplied(60, 130, 220, alpha),
                    Color32::from_rgba_unmultiplied(100, 180, 255, alpha),
                    Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                )
            } else {
                (
                    Color32::from_rgba_unmultiplied(45, 50, 60, alpha),
                    Color32::from_rgba_unmultiplied(70, 80, 100, alpha),
                    Color32::from_rgba_unmultiplied(180, 190, 200, alpha),
                )
            };

            // Button background
            painter.rect_filled(
                Rect::from_center_size(btn_center, Vec2::splat(button_size)),
                6.0,
                bg_color,
            );
            painter.rect_stroke(
                Rect::from_center_size(btn_center, Vec2::splat(button_size)),
                6.0,
                Stroke::new(1.5, border_color),
                egui::StrokeKind::Outside,
            );

            // Icon
            painter.text(
                btn_center,
                egui::Align2::CENTER_CENTER,
                dir.icon(),
                egui::FontId::proportional(if *dir == DropDirection::Center { 18.0 } else { 14.0 }),
                icon_color,
            );
        }
    }

    /// Get preview rect for a direction
    pub fn get_preview_rect(&self, dir: DropDirection) -> Option<Rect> {
        self.target_rect.map(|rect| {
            match dir {
                DropDirection::Left => Rect::from_min_max(
                    rect.min,
                    Pos2::new(rect.min.x + rect.width() * DROP_ZONE_THRESHOLD, rect.max.y),
                ),
                DropDirection::Right => Rect::from_min_max(
                    Pos2::new(rect.max.x - rect.width() * DROP_ZONE_THRESHOLD, rect.min.y),
                    rect.max,
                ),
                DropDirection::Top => Rect::from_min_max(
                    rect.min,
                    Pos2::new(rect.max.x, rect.min.y + rect.height() * DROP_ZONE_THRESHOLD),
                ),
                DropDirection::Bottom => Rect::from_min_max(
                    Pos2::new(rect.min.x, rect.max.y - rect.height() * DROP_ZONE_THRESHOLD),
                    rect.max,
                ),
                DropDirection::Center => rect,
            }
        })
    }
}

// ============================================================================
// Ghost Preview
// ============================================================================

/// Ghost preview for drag operations
#[derive(Debug)]
pub struct GhostPreview {
    /// Whether preview is visible
    pub visible: bool,
    /// Preview rect
    pub rect: Rect,
    /// Animation progress
    pub anim_progress: f32,
    /// Tab title being dragged
    pub tab_title: String,
}

impl Default for GhostPreview {
    fn default() -> Self {
        Self {
            visible: false,
            rect: Rect::NOTHING,
            anim_progress: 0.0,
            tab_title: String::new(),
        }
    }
}

impl GhostPreview {
    /// Create new ghost preview
    pub fn new() -> Self {
        Self::default()
    }

    /// Show ghost at rect
    pub fn show(&mut self, rect: Rect, tab_title: &str) {
        self.visible = true;
        self.rect = rect;
        self.tab_title = tab_title.to_string();
    }

    /// Hide ghost
    pub fn hide(&mut self) {
        self.visible = false;
        self.anim_progress = 0.0;
    }

    /// Update animation
    pub fn update(&mut self, dt: f32) {
        if self.visible {
            self.anim_progress = (self.anim_progress + dt / (SNAP_ANIMATION_DURATION * 0.5)).min(1.0);
        } else {
            self.anim_progress = 0.0;
        }
    }

    /// Draw the ghost preview
    pub fn draw(&self, ui: &mut Ui) {
        if !self.visible || self.anim_progress <= 0.0 {
            return;
        }

        let painter = ui.painter();
        let alpha = (self.anim_progress * 180.0) as u8;

        // Semi-transparent fill
        let fill_color = Color32::from_rgba_unmultiplied(60, 130, 200, alpha / 2);
        let border_color = Color32::from_rgba_unmultiplied(100, 180, 255, alpha);

        // Draw preview rect
        painter.rect_filled(self.rect, 4.0, fill_color);
        painter.rect_stroke(
            self.rect,
            4.0,
            Stroke::new(2.0, border_color),
            egui::StrokeKind::Inside,
        );

        // Draw tab title hint at top
        if !self.tab_title.is_empty() {
            let text_pos = Pos2::new(self.rect.min.x + 8.0, self.rect.min.y + 8.0);
            painter.text(
                text_pos,
                egui::Align2::LEFT_TOP,
                &self.tab_title,
                egui::FontId::proportional(12.0),
                Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
            );
        }

        // Diagonal stripe pattern (optional - indicates drop zone)
        let stripe_color = Color32::from_rgba_unmultiplied(100, 180, 255, alpha / 4);
        let stripe_spacing = 20.0;
        let rect = self.rect;

        // Draw diagonal stripes
        let mut x = rect.min.x - rect.height();
        while x < rect.max.x {
            let start = Pos2::new(
                x.max(rect.min.x),
                if x < rect.min.x { rect.min.y + (rect.min.x - x) } else { rect.min.y },
            );
            let end_x = x + rect.height();
            let end = Pos2::new(
                end_x.min(rect.max.x),
                if end_x > rect.max.x { rect.max.y - (end_x - rect.max.x) } else { rect.max.y },
            );

            if rect.contains(start) || rect.contains(end) {
                painter.line_segment([start, end], Stroke::new(1.0, stripe_color));
            }
            x += stripe_spacing;
        }
    }
}

// ============================================================================
// Snap Animation
// ============================================================================

/// Snap animation state
#[derive(Debug, Clone)]
pub struct SnapAnimation {
    /// Start rect
    pub from: Rect,
    /// End rect
    pub to: Rect,
    /// Progress (0.0 - 1.0)
    pub progress: f32,
    /// Whether animation is active
    pub active: bool,
}

impl Default for SnapAnimation {
    fn default() -> Self {
        Self {
            from: Rect::NOTHING,
            to: Rect::NOTHING,
            progress: 1.0,
            active: false,
        }
    }
}

impl SnapAnimation {
    /// Start animation
    pub fn start(&mut self, from: Rect, to: Rect) {
        self.from = from;
        self.to = to;
        self.progress = 0.0;
        self.active = true;
    }

    /// Update animation
    pub fn update(&mut self, dt: f32) {
        if self.active {
            self.progress += dt / SNAP_ANIMATION_DURATION;
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.active = false;
            }
        }
    }

    /// Get current interpolated rect
    pub fn current_rect(&self) -> Rect {
        if !self.active {
            return self.to;
        }

        // Ease-out cubic
        let t = 1.0 - (1.0 - self.progress).powi(3);

        Rect::from_min_max(
            Pos2::new(
                lerp(self.from.min.x, self.to.min.x, t),
                lerp(self.from.min.y, self.to.min.y, t),
            ),
            Pos2::new(
                lerp(self.from.max.x, self.to.max.x, t),
                lerp(self.from.max.y, self.to.max.y, t),
            ),
        )
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// ============================================================================
// Drag Cancel (ESC)
// ============================================================================

/// Drag state for cancel support
#[derive(Debug, Default)]
pub struct DragState {
    /// Whether a drag is in progress
    pub dragging: bool,
    /// Original position before drag
    pub original_pos: Option<Pos2>,
    /// Tab being dragged
    pub dragged_tab: Option<Tab>,
    /// Whether drag was cancelled
    pub cancelled: bool,
}

impl DragState {
    /// Start drag
    pub fn start(&mut self, tab: Tab, pos: Pos2) {
        self.dragging = true;
        self.original_pos = Some(pos);
        self.dragged_tab = Some(tab);
        self.cancelled = false;
    }

    /// Cancel drag (ESC pressed)
    pub fn cancel(&mut self) {
        if self.dragging {
            self.cancelled = true;
            self.dragging = false;
        }
    }

    /// End drag normally
    pub fn end(&mut self) {
        self.dragging = false;
        self.original_pos = None;
        self.dragged_tab = None;
    }

    /// Check for ESC key and cancel if pressed
    pub fn check_cancel(&mut self, ctx: &Context) -> bool {
        if self.dragging && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel();
            return true;
        }
        false
    }
}

// ============================================================================
// Tab Overflow Menu
// ============================================================================

/// Tab overflow state for handling many tabs
#[derive(Debug, Default)]
pub struct TabOverflow {
    /// Whether overflow menu is open
    pub menu_open: bool,
    /// Overflow button rect (for menu positioning)
    pub button_rect: Option<Rect>,
    /// Visible tab count
    pub visible_count: usize,
    /// Total tab count
    pub total_count: usize,
}

impl TabOverflow {
    /// Calculate overflow based on available width
    pub fn calculate(&mut self, tabs: &[Tab], available_width: f32, tab_width: f32) {
        let max_tabs = (available_width / tab_width).floor() as usize;
        self.total_count = tabs.len();
        self.visible_count = max_tabs.min(tabs.len());
    }

    /// Check if overflow exists
    pub fn has_overflow(&self) -> bool {
        self.visible_count < self.total_count
    }

    /// Get overflow count
    pub fn overflow_count(&self) -> usize {
        self.total_count.saturating_sub(self.visible_count)
    }

    /// Draw overflow button
    pub fn draw_button(&mut self, ui: &mut Ui) -> bool {
        if !self.has_overflow() {
            return false;
        }

        let overflow_text = format!("+{}", self.overflow_count());
        let response = ui.button(egui::RichText::new(&overflow_text).size(10.0));
        self.button_rect = Some(response.rect);

        if response.clicked() {
            self.menu_open = !self.menu_open;
        }

        self.menu_open
    }
}

// ============================================================================
// Layout Undo System
// ============================================================================

/// Serializable layout snapshot
#[derive(Debug, Clone)]
pub struct LayoutSnapshot {
    /// Snapshot name/description
    pub description: String,
    /// Timestamp
    pub timestamp: std::time::Instant,
    /// Serialized dock state (simplified - just tab positions)
    pub tab_positions: Vec<(Tab, String)>,  // (Tab, position_hint)
}

/// Layout undo/redo history
#[derive(Debug, Default)]
pub struct LayoutHistory {
    /// Undo stack
    undo_stack: VecDeque<LayoutSnapshot>,
    /// Redo stack
    redo_stack: Vec<LayoutSnapshot>,
    /// Whether history is enabled
    pub enabled: bool,
}

impl LayoutHistory {
    /// Create new history
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            enabled: true,
        }
    }

    /// Push current state to history
    pub fn push(&mut self, description: &str, dock_state: &DockState<Tab>) {
        if !self.enabled {
            return;
        }

        // Clear redo stack on new action
        self.redo_stack.clear();

        // Create snapshot
        let snapshot = LayoutSnapshot {
            description: description.to_string(),
            timestamp: std::time::Instant::now(),
            tab_positions: Self::extract_tab_positions(dock_state),
        };

        self.undo_stack.push_back(snapshot);

        // Trim to max size
        while self.undo_stack.len() > MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
    }

    /// Extract tab positions from dock state
    fn extract_tab_positions(dock_state: &DockState<Tab>) -> Vec<(Tab, String)> {
        let mut positions = Vec::new();
        for ((_surface, _node), tab) in dock_state.iter_all_tabs() {
            positions.push((*tab, "unknown".to_string()));
        }
        positions
    }

    /// Can undo?
    pub fn can_undo(&self) -> bool {
        self.undo_stack.len() > 1  // Keep at least one state
    }

    /// Can redo?
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Undo last change
    pub fn undo(&mut self) -> Option<LayoutSnapshot> {
        if !self.can_undo() {
            return None;
        }

        // Pop current state and push to redo
        if let Some(current) = self.undo_stack.pop_back() {
            self.redo_stack.push(current);
        }

        // Return previous state
        self.undo_stack.back().cloned()
    }

    /// Redo last undone change
    pub fn redo(&mut self) -> Option<LayoutSnapshot> {
        if let Some(state) = self.redo_stack.pop() {
            self.undo_stack.push_back(state.clone());
            Some(state)
        } else {
            None
        }
    }

    /// Get undo stack size
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len().saturating_sub(1)
    }

    /// Get redo stack size
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

// ============================================================================
// Combined UX Manager
// ============================================================================

/// Combined UX enhancements manager
#[derive(Debug, Default)]
pub struct DockingUxManager {
    /// Docking compass
    pub compass: DockingCompass,
    /// Ghost preview
    pub ghost: GhostPreview,
    /// Snap animation
    pub snap_anim: SnapAnimation,
    /// Drag state
    pub drag: DragState,
    /// Tab overflow
    pub overflow: TabOverflow,
    /// Layout history
    pub history: LayoutHistory,
    /// Last frame time for animations
    last_frame_time: Option<std::time::Instant>,
}

impl DockingUxManager {
    /// Create new UX manager
    pub fn new() -> Self {
        Self {
            history: LayoutHistory::new(),
            ..Default::default()
        }
    }

    /// Update all animations
    pub fn update(&mut self, ctx: &Context) {
        let now = std::time::Instant::now();
        let dt = self.last_frame_time
            .map(|t| now.duration_since(t).as_secs_f32())
            .unwrap_or(0.016);
        self.last_frame_time = Some(now);

        // Update animations
        self.compass.update(dt);
        self.ghost.update(dt);
        self.snap_anim.update(dt);

        // Check for ESC cancel
        self.drag.check_cancel(ctx);

        // Request repaint if any animation is active
        if self.compass.anim_progress > 0.0 && self.compass.anim_progress < 1.0
            || self.ghost.anim_progress > 0.0 && self.ghost.anim_progress < 1.0
            || self.snap_anim.active
        {
            ctx.request_repaint();
        }
    }

    /// Draw all overlays
    pub fn draw_overlays(&self, ui: &mut Ui) {
        let pointer_pos = ui.ctx().input(|i| i.pointer.hover_pos());

        // Draw in order: ghost (bottom) -> compass (top)
        self.ghost.draw(ui);

        // Only draw compass if visible
        if self.compass.visible {
            // Create a mutable copy for drawing (workaround for borrow)
            let compass = &self.compass;
            let painter = ui.painter();
            let alpha = (compass.anim_progress * 255.0) as u8;

            if alpha > 0 {
                let button_size = 32.0;
                let spacing = 40.0;

                // Draw connecting lines
                let line_color = Color32::from_rgba_unmultiplied(80, 120, 180, alpha / 3);
                for dir in [DropDirection::Left, DropDirection::Right, DropDirection::Top, DropDirection::Bottom] {
                    let btn_pos = compass.button_position(dir, spacing);
                    painter.line_segment(
                        [compass.center, btn_pos],
                        Stroke::new(1.0, line_color),
                    );
                }

                // Diamond background
                let diamond_bg = Color32::from_rgba_unmultiplied(30, 35, 45, alpha * 9 / 10);
                let diamond_stroke = Color32::from_rgba_unmultiplied(60, 100, 160, alpha);
                let diamond_size = spacing + button_size / 2.0 + 8.0;
                let points = [
                    Pos2::new(compass.center.x, compass.center.y - diamond_size),
                    Pos2::new(compass.center.x + diamond_size, compass.center.y),
                    Pos2::new(compass.center.x, compass.center.y + diamond_size),
                    Pos2::new(compass.center.x - diamond_size, compass.center.y),
                ];
                painter.add(egui::Shape::convex_polygon(
                    points.to_vec(),
                    diamond_bg,
                    Stroke::new(1.5, diamond_stroke),
                ));

                // Get hovered direction
                let hovered_dir = pointer_pos.and_then(|p| compass.hit_test(p));

                // Draw direction buttons
                for dir in DropDirection::all() {
                    let btn_center = compass.button_position(*dir, spacing);
                    let is_hovered = hovered_dir == Some(*dir);

                    let (bg_color, border_color, icon_color) = if is_hovered {
                        (
                            Color32::from_rgba_unmultiplied(60, 130, 220, alpha),
                            Color32::from_rgba_unmultiplied(100, 180, 255, alpha),
                            Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                        )
                    } else {
                        (
                            Color32::from_rgba_unmultiplied(45, 50, 60, alpha),
                            Color32::from_rgba_unmultiplied(70, 80, 100, alpha),
                            Color32::from_rgba_unmultiplied(180, 190, 200, alpha),
                        )
                    };

                    painter.rect_filled(
                        Rect::from_center_size(btn_center, Vec2::splat(button_size)),
                        6.0,
                        bg_color,
                    );
                    painter.rect_stroke(
                        Rect::from_center_size(btn_center, Vec2::splat(button_size)),
                        6.0,
                        Stroke::new(1.5, border_color),
                        egui::StrokeKind::Outside,
                    );

                    painter.text(
                        btn_center,
                        egui::Align2::CENTER_CENTER,
                        dir.icon(),
                        egui::FontId::proportional(if *dir == DropDirection::Center { 18.0 } else { 14.0 }),
                        icon_color,
                    );
                }
            }
        }
    }

    /// Handle tab drag start
    pub fn on_drag_start(&mut self, tab: Tab, pos: Pos2, dock_state: &DockState<Tab>) {
        self.drag.start(tab, pos);
        self.history.push("Before drag", dock_state);
    }

    /// Handle tab drag over leaf
    pub fn on_drag_over(&mut self, leaf_rect: Rect, tab_title: &str) {
        let center = leaf_rect.center();
        self.compass.show(center, leaf_rect);

        // Show ghost preview based on hovered direction
        // (actual direction detection happens in draw)
    }

    /// Handle tab drag end
    pub fn on_drag_end(&mut self, dropped: bool, dock_state: &DockState<Tab>) {
        self.compass.hide();
        self.ghost.hide();

        if dropped && !self.drag.cancelled {
            self.history.push("After drop", dock_state);
        }

        self.drag.end();
    }

    /// Undo layout change
    pub fn undo(&mut self) -> Option<LayoutSnapshot> {
        self.history.undo()
    }

    /// Redo layout change
    pub fn redo(&mut self) -> Option<LayoutSnapshot> {
        self.history.redo()
    }

    /// Get current hovered drop direction
    pub fn get_hovered_direction(&self, pointer_pos: Option<Pos2>) -> Option<DropDirection> {
        pointer_pos.and_then(|p| self.compass.hit_test(p))
    }

    /// Update ghost preview for direction
    pub fn update_ghost_for_direction(&mut self, dir: Option<DropDirection>, tab_title: &str) {
        if let Some(d) = dir {
            if let Some(rect) = self.compass.get_preview_rect(d) {
                self.ghost.show(rect, tab_title);
            }
        } else {
            self.ghost.hide();
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_direction_hit_test() {
        let mut compass = DockingCompass::new();
        compass.show(Pos2::new(100.0, 100.0), Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 200.0)));
        compass.anim_progress = 1.0;

        // Center should hit center
        assert_eq!(compass.hit_test(Pos2::new(100.0, 100.0)), Some(DropDirection::Center));

        // Left button
        assert_eq!(compass.hit_test(Pos2::new(60.0, 100.0)), Some(DropDirection::Left));

        // Outside should be None
        assert_eq!(compass.hit_test(Pos2::new(0.0, 0.0)), None);
    }

    #[test]
    fn test_layout_history() {
        let mut history = LayoutHistory::new();

        // Initially empty
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_snap_animation() {
        let mut anim = SnapAnimation::default();
        let from = Rect::from_min_size(Pos2::ZERO, Vec2::new(100.0, 100.0));
        let to = Rect::from_min_size(Pos2::new(50.0, 50.0), Vec2::new(100.0, 100.0));

        anim.start(from, to);
        assert!(anim.active);
        assert_eq!(anim.progress, 0.0);

        // After full duration
        anim.update(SNAP_ANIMATION_DURATION);
        assert!(!anim.active);
        assert_eq!(anim.current_rect(), to);
    }
}
