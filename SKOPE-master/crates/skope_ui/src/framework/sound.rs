//! Sound System - UI 사운드 피드백
//!
//! 버튼 클릭, 호버, 토글 등의 UI 사운드를 관리합니다.

use std::sync::Arc;

// ============================================================================
// UISoundEvent
// ============================================================================

/// UI 사운드 이벤트
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UISoundEvent {
    /// 버튼 클릭
    ButtonClick,
    /// 버튼 호버
    ButtonHover,
    /// 체크박스 체크
    CheckOn,
    /// 체크박스 해제
    CheckOff,
    /// 슬라이더 틱 (값 변경)
    SliderTick,
    /// 슬라이더 드래그 시작
    SliderDragStart,
    /// 슬라이더 드래그 종료
    SliderDragEnd,
    /// 메뉴 열기
    MenuOpen,
    /// 메뉴 닫기
    MenuClose,
    /// 메뉴 아이템 호버
    MenuItemHover,
    /// 탭 전환
    TabSwitch,
    /// 드롭다운 열기
    DropdownOpen,
    /// 드롭다운 닫기
    DropdownClose,
    /// 드롭다운 아이템 선택
    DropdownSelect,
    /// 텍스트 입력
    TextInput,
    /// 에러/실패
    Error,
    /// 성공/완료
    Success,
    /// 경고
    Warning,
    /// 알림
    Notification,
    /// 포커스 획득
    FocusGain,
    /// 포커스 손실
    FocusLost,
    /// 탐색 (Tab/화살표)
    Navigate,
    /// 스크롤
    Scroll,
    /// 드래그 시작
    DragStart,
    /// 드래그 종료
    DragEnd,
    /// 드롭
    Drop,
    /// 확장 (트리 노드 등)
    Expand,
    /// 축소
    Collapse,
}

// ============================================================================
// SoundId
// ============================================================================

/// 사운드 리소스 식별자
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SoundId(pub u32);

impl SoundId {
    /// 유효하지 않은 ID
    pub const INVALID: Self = Self(0);

    /// 유효한지 확인
    pub fn is_valid(&self) -> bool {
        self.0 != 0
    }
}

impl Default for SoundId {
    fn default() -> Self {
        Self::INVALID
    }
}

// ============================================================================
// SoundMapping
// ============================================================================

/// 이벤트별 사운드 매핑
#[derive(Debug, Clone, Default)]
pub struct SoundMapping {
    /// 버튼 클릭 사운드
    pub button_click: SoundId,
    /// 버튼 호버 사운드
    pub button_hover: SoundId,
    /// 체크 온 사운드
    pub check_on: SoundId,
    /// 체크 오프 사운드
    pub check_off: SoundId,
    /// 슬라이더 틱 사운드
    pub slider_tick: SoundId,
    /// 메뉴 열기 사운드
    pub menu_open: SoundId,
    /// 메뉴 닫기 사운드
    pub menu_close: SoundId,
    /// 탭 전환 사운드
    pub tab_switch: SoundId,
    /// 에러 사운드
    pub error: SoundId,
    /// 성공 사운드
    pub success: SoundId,
    /// 경고 사운드
    pub warning: SoundId,
    /// 탐색 사운드
    pub navigate: SoundId,
}

impl SoundMapping {
    /// 새 매핑 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 이벤트에 해당하는 사운드 ID 가져오기
    pub fn get(&self, event: UISoundEvent) -> SoundId {
        match event {
            UISoundEvent::ButtonClick => self.button_click,
            UISoundEvent::ButtonHover => self.button_hover,
            UISoundEvent::CheckOn => self.check_on,
            UISoundEvent::CheckOff => self.check_off,
            UISoundEvent::SliderTick | UISoundEvent::SliderDragStart | UISoundEvent::SliderDragEnd => {
                self.slider_tick
            }
            UISoundEvent::MenuOpen | UISoundEvent::DropdownOpen => self.menu_open,
            UISoundEvent::MenuClose | UISoundEvent::DropdownClose => self.menu_close,
            UISoundEvent::MenuItemHover => self.button_hover,
            UISoundEvent::TabSwitch | UISoundEvent::DropdownSelect => self.tab_switch,
            UISoundEvent::Error => self.error,
            UISoundEvent::Success => self.success,
            UISoundEvent::Warning => self.warning,
            UISoundEvent::Navigate | UISoundEvent::FocusGain | UISoundEvent::FocusLost => {
                self.navigate
            }
            _ => SoundId::INVALID,
        }
    }
}

// ============================================================================
// SoundPlayParams
// ============================================================================

/// 사운드 재생 파라미터
#[derive(Debug, Clone)]
pub struct SoundPlayParams {
    /// 볼륨 (0.0 ~ 1.0)
    pub volume: f32,
    /// 피치 (1.0 = 기본)
    pub pitch: f32,
    /// 피치 변조 (랜덤 범위)
    pub pitch_variation: f32,
    /// 볼륨 변조 (랜덤 범위)
    pub volume_variation: f32,
}

impl Default for SoundPlayParams {
    fn default() -> Self {
        Self {
            volume: 1.0,
            pitch: 1.0,
            pitch_variation: 0.0,
            volume_variation: 0.0,
        }
    }
}

impl SoundPlayParams {
    /// 기본 파라미터
    pub fn new() -> Self {
        Self::default()
    }

    /// 볼륨 설정
    pub fn with_volume(mut self, volume: f32) -> Self {
        self.volume = volume.clamp(0.0, 1.0);
        self
    }

    /// 피치 설정
    pub fn with_pitch(mut self, pitch: f32) -> Self {
        self.pitch = pitch.max(0.1);
        self
    }

    /// 피치 변조 설정
    pub fn with_pitch_variation(mut self, variation: f32) -> Self {
        self.pitch_variation = variation.abs();
        self
    }

    /// UI 클릭용 기본 설정
    pub fn ui_click() -> Self {
        Self {
            volume: 0.5,
            pitch: 1.0,
            pitch_variation: 0.05,
            volume_variation: 0.1,
        }
    }

    /// UI 호버용 기본 설정
    pub fn ui_hover() -> Self {
        Self {
            volume: 0.3,
            pitch: 1.2,
            pitch_variation: 0.02,
            volume_variation: 0.05,
        }
    }
}

// ============================================================================
// SoundHandler
// ============================================================================

/// 사운드 재생 핸들러 타입
pub type SoundPlayHandler = Arc<dyn Fn(UISoundEvent, SoundPlayParams) + Send + Sync>;

// ============================================================================
// UISoundManager
// ============================================================================

/// UI 사운드 관리자
pub struct UISoundManager {
    /// 사운드 재생 핸들러
    play_handler: Option<SoundPlayHandler>,
    /// 사운드 매핑
    mapping: SoundMapping,
    /// 마스터 볼륨
    master_volume: f32,
    /// 활성화 여부
    enabled: bool,
    /// 이벤트별 기본 파라미터
    event_params: std::collections::HashMap<UISoundEvent, SoundPlayParams>,
    /// 최근 재생 시간 (스팸 방지)
    last_play_time: std::collections::HashMap<UISoundEvent, f64>,
    /// 최소 재생 간격 (초)
    min_play_interval: f32,
    /// 현재 시간
    current_time: f64,
}

impl Default for UISoundManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UISoundManager {
    /// 새 사운드 관리자
    pub fn new() -> Self {
        Self {
            play_handler: None,
            mapping: SoundMapping::new(),
            master_volume: 1.0,
            enabled: true,
            event_params: std::collections::HashMap::new(),
            last_play_time: std::collections::HashMap::new(),
            min_play_interval: 0.05, // 50ms
            current_time: 0.0,
        }
    }

    /// 사운드 핸들러 설정
    pub fn set_handler<F>(&mut self, handler: F)
    where
        F: Fn(UISoundEvent, SoundPlayParams) + Send + Sync + 'static,
    {
        self.play_handler = Some(Arc::new(handler));
    }

    /// 사운드 매핑 설정
    pub fn set_mapping(&mut self, mapping: SoundMapping) {
        self.mapping = mapping;
    }

    /// 마스터 볼륨 설정
    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
    }

    /// 활성화/비활성화
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// 활성화 여부
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 이벤트별 기본 파라미터 설정
    pub fn set_event_params(&mut self, event: UISoundEvent, params: SoundPlayParams) {
        self.event_params.insert(event, params);
    }

    /// 최소 재생 간격 설정
    pub fn set_min_play_interval(&mut self, interval: f32) {
        self.min_play_interval = interval.max(0.0);
    }

    /// 시간 업데이트
    pub fn update_time(&mut self, time: f64) {
        self.current_time = time;
    }

    /// 사운드 재생
    pub fn play(&mut self, event: UISoundEvent) {
        self.play_with_params(event, None);
    }

    /// 파라미터와 함께 사운드 재생
    pub fn play_with_params(&mut self, event: UISoundEvent, params: Option<SoundPlayParams>) {
        if !self.enabled {
            return;
        }

        // 스팸 방지 체크
        if let Some(&last_time) = self.last_play_time.get(&event) {
            if self.current_time - last_time < self.min_play_interval as f64 {
                return;
            }
        }

        // 핸들러 확인
        let Some(ref handler) = self.play_handler else {
            return;
        };

        // 사운드 ID 확인
        let sound_id = self.mapping.get(event);
        if !sound_id.is_valid() {
            // 매핑된 사운드가 없어도 이벤트는 전달
        }

        // 파라미터 결정
        let mut final_params = params
            .or_else(|| self.event_params.get(&event).cloned())
            .unwrap_or_else(SoundPlayParams::default);

        // 마스터 볼륨 적용
        final_params.volume *= self.master_volume;

        // 변조 적용
        if final_params.pitch_variation > 0.0 {
            let variation = (rand_simple() * 2.0 - 1.0) * final_params.pitch_variation;
            final_params.pitch += variation;
        }
        if final_params.volume_variation > 0.0 {
            let variation = (rand_simple() * 2.0 - 1.0) * final_params.volume_variation;
            final_params.volume = (final_params.volume + variation).clamp(0.0, 1.0);
        }

        // 재생
        handler(event, final_params);

        // 재생 시간 기록
        self.last_play_time.insert(event, self.current_time);
    }

    /// 버튼 클릭 사운드
    pub fn play_button_click(&mut self) {
        self.play(UISoundEvent::ButtonClick);
    }

    /// 버튼 호버 사운드
    pub fn play_button_hover(&mut self) {
        self.play(UISoundEvent::ButtonHover);
    }

    /// 체크박스 토글 사운드
    pub fn play_checkbox_toggle(&mut self, checked: bool) {
        self.play(if checked {
            UISoundEvent::CheckOn
        } else {
            UISoundEvent::CheckOff
        });
    }

    /// 메뉴 열기 사운드
    pub fn play_menu_open(&mut self) {
        self.play(UISoundEvent::MenuOpen);
    }

    /// 메뉴 닫기 사운드
    pub fn play_menu_close(&mut self) {
        self.play(UISoundEvent::MenuClose);
    }

    /// 에러 사운드
    pub fn play_error(&mut self) {
        self.play(UISoundEvent::Error);
    }

    /// 성공 사운드
    pub fn play_success(&mut self) {
        self.play(UISoundEvent::Success);
    }

    /// 탐색 사운드
    pub fn play_navigate(&mut self) {
        self.play(UISoundEvent::Navigate);
    }
}

// ============================================================================
// Simple random (no external dependency)
// ============================================================================

/// 간단한 랜덤 (0.0 ~ 1.0)
fn rand_simple() -> f32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};

    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    hasher.write_u64(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64);
    (hasher.finish() % 10000) as f32 / 10000.0
}

// ============================================================================
// SlateSound (언리얼 호환)
// ============================================================================

/// 슬레이트 사운드 (언리얼 FSlateSound 호환)
#[derive(Debug, Clone, Default)]
pub struct SlateSound {
    /// 사운드 리소스 경로
    pub resource_path: String,
}

impl SlateSound {
    /// 새 슬레이트 사운드
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            resource_path: path.into(),
        }
    }

    /// 빈 사운드인지
    pub fn is_empty(&self) -> bool {
        self.resource_path.is_empty()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_sound_manager() {
        let play_count = Arc::new(AtomicU32::new(0));
        let play_count_clone = Arc::clone(&play_count);

        let mut manager = UISoundManager::new();
        manager.set_handler(move |_event, _params| {
            play_count_clone.fetch_add(1, Ordering::Relaxed);
        });

        manager.play_button_click();
        assert_eq!(play_count.load(Ordering::Relaxed), 1);

        manager.play_button_hover();
        assert_eq!(play_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_sound_disabled() {
        let play_count = Arc::new(AtomicU32::new(0));
        let play_count_clone = Arc::clone(&play_count);

        let mut manager = UISoundManager::new();
        manager.set_handler(move |_event, _params| {
            play_count_clone.fetch_add(1, Ordering::Relaxed);
        });
        manager.set_enabled(false);

        manager.play_button_click();
        assert_eq!(play_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_sound_params() {
        let params = SoundPlayParams::ui_click()
            .with_volume(0.8)
            .with_pitch(1.1);

        assert_eq!(params.volume, 0.8);
        assert_eq!(params.pitch, 1.1);
    }

    #[test]
    fn test_sound_mapping() {
        let mut mapping = SoundMapping::new();
        mapping.button_click = SoundId(1);
        mapping.error = SoundId(2);

        assert_eq!(mapping.get(UISoundEvent::ButtonClick), SoundId(1));
        assert_eq!(mapping.get(UISoundEvent::Error), SoundId(2));
        assert_eq!(mapping.get(UISoundEvent::Scroll), SoundId::INVALID);
    }
}
