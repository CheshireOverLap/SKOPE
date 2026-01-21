//! Audio Listener System
//!
//! Scene/Game 뷰 전환 시 오디오 리스너 위치를 자동으로 전환합니다.
//!
//! - Scene 뷰: 에디터 카메라 위치에서 오디오 청취
//! - Game 뷰: 게임 카메라 (또는 플레이어) 위치에서 오디오 청취

use bevy_ecs::entity::Entity;
use glam::{Vec3, Quat};

/// 오디오 리스너 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioListenerMode {
    /// 에디터 카메라 (Scene 뷰)
    #[default]
    EditorCamera,
    /// 게임 카메라 (Game 뷰)
    GameCamera,
    /// 특정 엔티티 (AudioListener 컴포넌트)
    Entity(Entity),
}

impl AudioListenerMode {
    /// 모드 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::EditorCamera => "Editor Camera",
            Self::GameCamera => "Game Camera",
            Self::Entity(_) => "Custom Entity",
        }
    }
}

/// 오디오 리스너 상태
#[derive(Debug, Clone, Default)]
pub struct AudioListenerState {
    /// 현재 모드
    pub mode: AudioListenerMode,
    /// 리스너 위치
    pub position: Vec3,
    /// 리스너 방향 (전방)
    pub forward: Vec3,
    /// 리스너 업 벡터
    pub up: Vec3,
    /// 볼륨 (0.0 ~ 1.0)
    pub volume: f32,
    /// 음소거 여부
    pub muted: bool,
}

impl AudioListenerState {
    /// 새 AudioListenerState 생성
    pub fn new() -> Self {
        Self {
            mode: AudioListenerMode::EditorCamera,
            position: Vec3::ZERO,
            forward: -Vec3::Z,
            up: Vec3::Y,
            volume: 1.0,
            muted: false,
        }
    }

    /// 에디터 카메라에서 리스너 업데이트
    pub fn update_from_editor_camera(&mut self, position: Vec3, rotation: Quat) {
        self.mode = AudioListenerMode::EditorCamera;
        self.position = position;
        self.forward = rotation * -Vec3::Z;
        self.up = rotation * Vec3::Y;
    }

    /// 게임 카메라에서 리스너 업데이트
    pub fn update_from_game_camera(&mut self, position: Vec3, rotation: Quat) {
        self.mode = AudioListenerMode::GameCamera;
        self.position = position;
        self.forward = rotation * -Vec3::Z;
        self.up = rotation * Vec3::Y;
    }

    /// 커스텀 엔티티에서 리스너 업데이트
    pub fn update_from_entity(&mut self, entity: Entity, position: Vec3, rotation: Quat) {
        self.mode = AudioListenerMode::Entity(entity);
        self.position = position;
        self.forward = rotation * -Vec3::Z;
        self.up = rotation * Vec3::Y;
    }

    /// 볼륨 설정
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// 음소거 토글
    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    /// 실제 볼륨 (음소거 고려)
    pub fn effective_volume(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }
}

/// 뷰포트 전환 시 오디오 리스너 전환
pub struct AudioListenerSwitcher {
    /// 현재 리스너 상태
    pub state: AudioListenerState,
    /// 이전 모드 (전환 감지용)
    previous_mode: AudioListenerMode,
    /// 전환 중 페이드 진행도 (0.0 ~ 1.0)
    fade_progress: f32,
    /// 페이드 지속 시간 (초)
    fade_duration: f32,
}

impl Default for AudioListenerSwitcher {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioListenerSwitcher {
    /// 새 AudioListenerSwitcher 생성
    pub fn new() -> Self {
        Self {
            state: AudioListenerState::new(),
            previous_mode: AudioListenerMode::EditorCamera,
            fade_progress: 1.0,
            fade_duration: 0.3,
        }
    }

    /// Scene 뷰로 전환
    pub fn switch_to_scene_view(&mut self) {
        if self.state.mode != AudioListenerMode::EditorCamera {
            self.previous_mode = self.state.mode;
            self.state.mode = AudioListenerMode::EditorCamera;
            self.fade_progress = 0.0;
            log::info!("[AudioListener] Switched to Editor Camera");
        }
    }

    /// Game 뷰로 전환
    pub fn switch_to_game_view(&mut self) {
        if self.state.mode != AudioListenerMode::GameCamera {
            self.previous_mode = self.state.mode;
            self.state.mode = AudioListenerMode::GameCamera;
            self.fade_progress = 0.0;
            log::info!("[AudioListener] Switched to Game Camera");
        }
    }

    /// 업데이트 (매 프레임)
    pub fn update(&mut self, delta_time: f32) {
        // 페이드 진행
        if self.fade_progress < 1.0 {
            self.fade_progress += delta_time / self.fade_duration;
            self.fade_progress = self.fade_progress.min(1.0);
        }
    }

    /// 현재 볼륨 배율 (페이드 적용)
    pub fn volume_multiplier(&self) -> f32 {
        // 부드러운 이징
        let t = self.fade_progress;
        let eased = t * t * (3.0 - 2.0 * t);
        eased * self.state.effective_volume()
    }

    /// 전환 중인지 확인
    pub fn is_transitioning(&self) -> bool {
        self.fade_progress < 1.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_listener_state() {
        let mut state = AudioListenerState::new();
        assert_eq!(state.mode, AudioListenerMode::EditorCamera);

        state.update_from_game_camera(Vec3::new(1.0, 2.0, 3.0), Quat::IDENTITY);
        assert_eq!(state.mode, AudioListenerMode::GameCamera);
        assert_eq!(state.position, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_audio_listener_mute() {
        let mut state = AudioListenerState::new();
        state.set_volume(0.8);

        assert!((state.effective_volume() - 0.8).abs() < 0.001);

        state.toggle_mute();
        assert_eq!(state.effective_volume(), 0.0);

        state.toggle_mute();
        assert!((state.effective_volume() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_audio_listener_switcher() {
        let mut switcher = AudioListenerSwitcher::new();

        switcher.switch_to_game_view();
        assert_eq!(switcher.state.mode, AudioListenerMode::GameCamera);
        assert!(switcher.is_transitioning());

        // 전환 완료
        switcher.update(1.0);
        assert!(!switcher.is_transitioning());
    }
}
