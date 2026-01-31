//! UI 사운드 이벤트 시스템 — 스타일셋 내 SlateSound 통합
//!
//! 위젯 상호작용에 따른 사운드 재생을 관리합니다.

use std::collections::HashMap;

/// UI 사운드 이벤트 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiSoundEvent {
    /// 버튼 호버
    ButtonHover,
    /// 버튼 클릭
    ButtonPress,
    /// 체크박스 토글
    CheckboxToggle,
    /// 슬라이더 드래그
    SliderDrag,
    /// 탭 전환
    TabSwitch,
    /// 메뉴 열기
    MenuOpen,
    /// 메뉴 닫기
    MenuClose,
    /// 콤보박스 열기
    ComboBoxOpen,
    /// 텍스트 입력
    TextInput,
    /// 알림 표시
    NotificationShow,
    /// 에러 발생
    Error,
    /// 포커스 변경
    FocusChange,
    /// 윈도우 열기
    WindowOpen,
    /// 윈도우 닫기
    WindowClose,
    /// 드래그 시작
    DragStart,
    /// 드래그 드롭
    DragDrop,
}

/// 사운드 리소스 참조
#[derive(Debug, Clone)]
pub struct SlateSound {
    /// 사운드 리소스 경로
    pub resource_path: String,
    /// 볼륨 (0..1)
    pub volume: f32,
    /// 피치 (0.5..2.0)
    pub pitch: f32,
    /// 피치 변동 범위 (랜덤)
    pub pitch_variation: f32,
}

impl SlateSound {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            resource_path: path.into(),
            volume: 1.0,
            pitch: 1.0,
            pitch_variation: 0.0,
        }
    }

    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch.clamp(0.5, 2.0);
        self
    }

    pub fn with_pitch_variation(mut self, variation: f32) -> Self {
        self.pitch_variation = variation.clamp(0.0, 0.5);
        self
    }

    /// 실제 피치 (변동 적용, 랜덤 시드 기반)
    pub fn effective_pitch(&self, random_seed: f32) -> f32 {
        let variation = (random_seed * 2.0 - 1.0) * self.pitch_variation;
        (self.pitch + variation).clamp(0.5, 2.0)
    }
}

impl Default for SlateSound {
    fn default() -> Self {
        Self::new("")
    }
}

/// UI 사운드 매핑 — 이벤트별 사운드 설정
pub struct UiSoundMap {
    sounds: HashMap<UiSoundEvent, SlateSound>,
    global_volume: f32,
    is_muted: bool,
}

impl UiSoundMap {
    pub fn new() -> Self {
        Self {
            sounds: HashMap::new(),
            global_volume: 1.0,
            is_muted: false,
        }
    }

    /// 기본 사운드 매핑 생성
    pub fn with_defaults() -> Self {
        let mut map = Self::new();
        map.set(UiSoundEvent::ButtonHover, SlateSound::new("ui/hover").with_volume(0.3));
        map.set(UiSoundEvent::ButtonPress, SlateSound::new("ui/click").with_volume(0.5));
        map.set(UiSoundEvent::CheckboxToggle, SlateSound::new("ui/toggle").with_volume(0.4));
        map.set(UiSoundEvent::MenuOpen, SlateSound::new("ui/menu_open").with_volume(0.4));
        map.set(UiSoundEvent::MenuClose, SlateSound::new("ui/menu_close").with_volume(0.3));
        map.set(UiSoundEvent::Error, SlateSound::new("ui/error").with_volume(0.6));
        map.set(UiSoundEvent::NotificationShow, SlateSound::new("ui/notification").with_volume(0.5));
        map
    }

    pub fn set(&mut self, event: UiSoundEvent, sound: SlateSound) {
        self.sounds.insert(event, sound);
    }

    pub fn get(&self, event: UiSoundEvent) -> Option<&SlateSound> {
        self.sounds.get(&event)
    }

    pub fn remove(&mut self, event: UiSoundEvent) {
        self.sounds.remove(&event);
    }

    pub fn set_global_volume(&mut self, volume: f32) {
        self.global_volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_muted(&mut self, muted: bool) {
        self.is_muted = muted;
    }

    pub fn global_volume(&self) -> f32 { self.global_volume }
    pub fn is_muted(&self) -> bool { self.is_muted }
    pub fn sound_count(&self) -> usize { self.sounds.len() }
}

/// UI 사운드 이벤트 큐 — 한 프레임 내 사운드 이벤트 수집
pub struct UiSoundEventQueue {
    events: Vec<PendingSoundEvent>,
    sound_map: UiSoundMap,
    max_concurrent: usize,
    cooldown_ms: f64,
    last_play_times: HashMap<UiSoundEvent, f64>,
}

/// 대기 중인 사운드 이벤트
#[derive(Debug, Clone)]
struct PendingSoundEvent {
    event: UiSoundEvent,
    volume_override: Option<f32>,
    timestamp: f64,
}

impl UiSoundEventQueue {
    pub fn new(sound_map: UiSoundMap) -> Self {
        Self {
            events: Vec::new(),
            sound_map,
            max_concurrent: 8,
            cooldown_ms: 50.0,
            last_play_times: HashMap::new(),
        }
    }

    /// 사운드 이벤트 발행
    pub fn play(&mut self, event: UiSoundEvent, current_time: f64) {
        if self.sound_map.is_muted() { return; }
        if self.events.len() >= self.max_concurrent { return; }

        // 쿨다운 체크
        if let Some(&last_time) = self.last_play_times.get(&event) {
            if (current_time - last_time) * 1000.0 < self.cooldown_ms {
                return;
            }
        }

        if self.sound_map.get(event).is_some() {
            self.events.push(PendingSoundEvent {
                event,
                volume_override: None,
                timestamp: current_time,
            });
            self.last_play_times.insert(event, current_time);
        }
    }

    /// 볼륨 오버라이드로 사운드 발행
    pub fn play_with_volume(&mut self, event: UiSoundEvent, volume: f32, current_time: f64) {
        if self.sound_map.is_muted() { return; }
        if self.events.len() >= self.max_concurrent { return; }

        if self.sound_map.get(event).is_some() {
            self.events.push(PendingSoundEvent {
                event,
                volume_override: Some(volume),
                timestamp: current_time,
            });
            self.last_play_times.insert(event, current_time);
        }
    }

    /// 큐에서 이벤트 추출 (렌더/오디오 스레드용)
    pub fn drain(&mut self) -> Vec<(UiSoundEvent, SlateSound)> {
        let result: Vec<_> = self.events.drain(..)
            .filter_map(|pe| {
                self.sound_map.get(pe.event).map(|s| {
                    let mut sound = s.clone();
                    if let Some(vol) = pe.volume_override {
                        sound.volume = vol;
                    }
                    sound.volume *= self.sound_map.global_volume();
                    (pe.event, sound)
                })
            })
            .collect();
        result
    }

    pub fn pending_count(&self) -> usize { self.events.len() }
    pub fn sound_map(&self) -> &UiSoundMap { &self.sound_map }
    pub fn sound_map_mut(&mut self) -> &mut UiSoundMap { &mut self.sound_map }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slate_sound() {
        let sound = SlateSound::new("ui/click")
            .with_volume(0.8)
            .with_pitch(1.2);
        assert_eq!(sound.volume, 0.8);
        assert_eq!(sound.pitch, 1.2);
    }

    #[test]
    fn test_slate_sound_pitch_variation() {
        let sound = SlateSound::new("ui/click")
            .with_pitch(1.0)
            .with_pitch_variation(0.1);
        let p1 = sound.effective_pitch(0.0); // -0.1
        let p2 = sound.effective_pitch(1.0); // +0.1
        assert!(p1 < p2);
    }

    #[test]
    fn test_sound_map() {
        let mut map = UiSoundMap::new();
        map.set(UiSoundEvent::ButtonPress, SlateSound::new("click"));
        assert!(map.get(UiSoundEvent::ButtonPress).is_some());
        assert!(map.get(UiSoundEvent::MenuOpen).is_none());
        assert_eq!(map.sound_count(), 1);
    }

    #[test]
    fn test_sound_map_defaults() {
        let map = UiSoundMap::with_defaults();
        assert!(map.get(UiSoundEvent::ButtonPress).is_some());
        assert!(map.get(UiSoundEvent::Error).is_some());
        assert!(map.sound_count() >= 5);
    }

    #[test]
    fn test_sound_event_queue() {
        let map = UiSoundMap::with_defaults();
        let mut queue = UiSoundEventQueue::new(map);
        queue.play(UiSoundEvent::ButtonPress, 0.0);
        assert_eq!(queue.pending_count(), 1);
        let drained = queue.drain();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].0, UiSoundEvent::ButtonPress);
    }

    #[test]
    fn test_sound_queue_muted() {
        let mut map = UiSoundMap::with_defaults();
        map.set_muted(true);
        let mut queue = UiSoundEventQueue::new(map);
        queue.play(UiSoundEvent::ButtonPress, 0.0);
        assert_eq!(queue.pending_count(), 0); // muted
    }

    #[test]
    fn test_sound_queue_cooldown() {
        let map = UiSoundMap::with_defaults();
        let mut queue = UiSoundEventQueue::new(map);
        queue.play(UiSoundEvent::ButtonPress, 0.0);
        queue.play(UiSoundEvent::ButtonPress, 0.01); // < 50ms cooldown
        assert_eq!(queue.pending_count(), 1); // second one skipped
    }

    #[test]
    fn test_sound_queue_global_volume() {
        let mut map = UiSoundMap::with_defaults();
        map.set_global_volume(0.5);
        let mut queue = UiSoundEventQueue::new(map);
        queue.play(UiSoundEvent::ButtonPress, 0.0);
        let drained = queue.drain();
        // 원래 0.5 * global 0.5 = 0.25
        assert!(drained[0].1.volume < 0.5);
    }
}
