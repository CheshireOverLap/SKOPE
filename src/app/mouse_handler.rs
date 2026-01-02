//! Mouse Event Handler
//!
//! 마우스 이벤트 처리 로직 분리
//! 향후 main.rs에서 이 메서드들로 점진적 이동 예정

#![allow(dead_code)]

use crate::App;

impl App {
    /// 마우스 휠: 카메라 줌
    pub fn handle_camera_zoom(&mut self, delta: f32) {
        if !self.editor_mode.is_edit() {
            return;
        }

        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.on_scroll(delta);
        }
    }

    /// 마우스 위치 업데이트
    pub fn update_mouse_position(&mut self, x: f32, y: f32) {
        self.last_mouse_pos = (x, y);

        // Game UI 업데이트
        self.game_ui.on_mouse_move(x, y);
    }

    /// 마우스 이동 처리 (Scene Viewer에 위임)
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) {
        if !self.editor_mode.is_edit() {
            return;
        }

        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.on_mouse_move(
                glam::Vec2::new(x, y),
                &mut self.world,
            );
        }
    }
}
