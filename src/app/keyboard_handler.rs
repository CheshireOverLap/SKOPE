//! Keyboard Event Handler
//!
//! 키보드 이벤트 처리 로직 분리
//! 향후 main.rs에서 이 메서드들로 점진적 이동 예정

#![allow(dead_code)]

use winit::keyboard::KeyCode;
use crate::App;
use crate::editor;
use crate::editor::gizmo::GizmoMode;

impl App {
    /// F5 키: 에디터 모드 토글 (Edit ↔ Play)
    pub fn handle_mode_toggle(&mut self) {
        self.editor_mode.toggle();
        match self.editor_mode {
            editor::EditorMode::Edit => {
                log::info!("[Editor] Switched to EDIT mode");
            }
            editor::EditorMode::Play => {
                log::info!("[Editor] Switched to PLAY mode");
            }
        }
    }

    /// F4 키: 디버그 시각화 토글
    pub fn handle_debug_viz_toggle(&mut self) {
        if self.editor_mode.is_edit() {
            self.editor_debug_viz.toggle_all();
            log::info!(
                "[Editor] Debug viz toggled: lights={}, colliders={}, cameras={}",
                self.editor_debug_viz.show_lights,
                self.editor_debug_viz.show_colliders,
                self.editor_debug_viz.show_cameras
            );
        }
    }

    /// W/E/R/Q 키: Gizmo 모드 변경
    pub fn handle_gizmo_mode_key(&mut self, key_code: KeyCode) {
        if !self.editor_mode.is_edit() {
            return;
        }

        if let Some(ref mut scene_viewer) = self.scene_viewer {
            match key_code {
                KeyCode::KeyW => scene_viewer.gizmo_mode = GizmoMode::Move,
                KeyCode::KeyE => scene_viewer.gizmo_mode = GizmoMode::Rotate,
                KeyCode::KeyR => scene_viewer.gizmo_mode = GizmoMode::Scale,
                KeyCode::KeyQ => scene_viewer.gizmo_mode = GizmoMode::Select,
                _ => {}
            }
        }
    }

    /// L 키: Local/World 공간 토글
    pub fn handle_space_toggle(&mut self) {
        if !self.editor_mode.is_edit() {
            return;
        }

        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.toggle_space();
            log::info!("[Editor] Transform space toggled");
        }
    }

    /// G 키: 스냅 토글
    pub fn handle_snap_toggle(&mut self) {
        if !self.editor_mode.is_edit() {
            return;
        }

        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.toggle_snap();
            log::info!("[Editor] Snap enabled: {}", scene_viewer.snap_enabled);
        }
    }

    /// F 키: 선택된 엔티티에 포커스
    pub fn handle_focus_selection(&mut self) {
        if !self.editor_mode.is_edit() {
            return;
        }

        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.focus_on_selection(&self.world);
        }
    }

    /// Numpad 카메라 프리셋
    pub fn handle_camera_preset(&mut self, key_code: KeyCode) {
        if !self.editor_mode.is_edit() {
            return;
        }

        if let Some(ref mut scene_viewer) = self.scene_viewer {
            match key_code {
                KeyCode::Numpad7 => scene_viewer.camera.set_top_view(),
                KeyCode::Numpad1 => scene_viewer.camera.set_front_view(),
                KeyCode::Numpad3 => scene_viewer.camera.set_right_view(),
                KeyCode::Numpad0 => scene_viewer.camera.set_perspective_view(),
                _ => {}
            }
        }
    }

    /// Shift+A: 스폰 메뉴 열기 (TODO: egui 기반 spawn menu 구현 필요)
    pub fn handle_spawn_menu(&mut self) {
        if !self.editor_mode.is_edit() {
            return;
        }
        log::debug!("[Editor] Spawn menu requested (not yet implemented)");
    }
}
