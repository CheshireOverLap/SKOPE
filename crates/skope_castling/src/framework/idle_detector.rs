//! UI Idle Detection — UI 유휴 감지 시스템
//!
//! UI에 변경이 없을 때 틱/렌더를 스킵하여 CPU/GPU 사용을 줄입니다.
//! 애니메이션 활성 여부, 입력 이벤트, 무효화 상태를 종합 판단합니다.

/// UI 활동 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiActivityState {
    /// 활발한 업데이트 필요 (애니메이션, 입력 등)
    Active,
    /// 곧 유휴 전환 (마지막 활동 후 대기 중)
    TransitionToIdle,
    /// 유휴 — 변경 없음, 렌더 스킵 가능
    Idle,
}

/// 유휴 감지 설정
#[derive(Debug, Clone)]
pub struct IdleDetectorConfig {
    /// 마지막 활동 후 유휴 전환까지의 시간 (초)
    pub idle_timeout: f64,
    /// 유휴 시 최소 틱 간격 (초) — 완전히 멈추지 않고 저빈도 폴링
    pub idle_tick_interval: f64,
    /// 유휴 시에도 항상 활성 상태를 유지하는 조건 수
    pub always_active_threshold: u32,
}

impl Default for IdleDetectorConfig {
    fn default() -> Self {
        Self {
            idle_timeout: 0.5,
            idle_tick_interval: 0.1,
            always_active_threshold: 0,
        }
    }
}

/// UI 유휴 감지기
pub struct UiIdleDetector {
    config: IdleDetectorConfig,
    state: UiActivityState,
    last_activity_time: f64,
    last_idle_tick_time: f64,
    active_animations: u32,
    pending_invalidations: u32,
    input_events_this_frame: u32,
    frames_since_activity: u32,
    total_idle_frames: u64,
    total_active_frames: u64,
}

impl UiIdleDetector {
    pub fn new() -> Self {
        Self::with_config(IdleDetectorConfig::default())
    }

    pub fn with_config(config: IdleDetectorConfig) -> Self {
        Self {
            config,
            state: UiActivityState::Active,
            last_activity_time: 0.0,
            last_idle_tick_time: 0.0,
            active_animations: 0,
            pending_invalidations: 0,
            input_events_this_frame: 0,
            frames_since_activity: 0,
            total_idle_frames: 0,
            total_active_frames: 0,
        }
    }

    pub fn state(&self) -> UiActivityState { self.state }
    pub fn active_animations(&self) -> u32 { self.active_animations }
    pub fn is_idle(&self) -> bool { self.state == UiActivityState::Idle }
    pub fn is_active(&self) -> bool { self.state == UiActivityState::Active }

    /// 프레임 시작 시 호출
    pub fn begin_frame(&mut self, current_time: f64) {
        // 상태 결정 — 카운터 리셋 전에 체크
        let has_activity = self.active_animations > 0
            || self.input_events_this_frame > 0
            || self.pending_invalidations > 0
            || (self.config.always_active_threshold > 0
                && self.active_animations >= self.config.always_active_threshold);

        // 프레임 로컬 카운터 리셋
        self.input_events_this_frame = 0;
        self.pending_invalidations = 0;

        // 첫 프레임은 항상 활성
        let is_first_frame = self.total_active_frames == 0 && self.total_idle_frames == 0;

        if has_activity || is_first_frame {
            self.state = UiActivityState::Active;
            self.last_activity_time = current_time;
            self.frames_since_activity = 0;
        } else {
            let elapsed = current_time - self.last_activity_time;
            if elapsed < self.config.idle_timeout {
                self.state = UiActivityState::TransitionToIdle;
            } else {
                self.state = UiActivityState::Idle;
            }
            self.frames_since_activity += 1;
        }

        match self.state {
            UiActivityState::Idle => self.total_idle_frames += 1,
            _ => self.total_active_frames += 1,
        }
    }

    /// 유휴 상태에서 이번 틱을 스킵할 수 있는지
    pub fn should_skip_tick(&self, current_time: f64) -> bool {
        if self.state != UiActivityState::Idle { return false; }
        current_time - self.last_idle_tick_time < self.config.idle_tick_interval
    }

    /// 유휴 상태에서 틱 처리 후 호출
    pub fn record_idle_tick(&mut self, current_time: f64) {
        self.last_idle_tick_time = current_time;
    }

    /// 입력 이벤트 발생 시 호출
    pub fn record_input(&mut self) {
        self.input_events_this_frame += 1;
        self.state = UiActivityState::Active;
    }

    /// 위젯 무효화 발생 시 호출
    pub fn record_invalidation(&mut self) {
        self.pending_invalidations += 1;
        self.state = UiActivityState::Active;
    }

    /// 활성 애니메이션 수 설정
    pub fn set_active_animations(&mut self, count: u32) {
        self.active_animations = count;
    }

    /// 강제 활성 전환
    pub fn wake_up(&mut self, current_time: f64) {
        self.state = UiActivityState::Active;
        self.last_activity_time = current_time;
        self.frames_since_activity = 0;
    }

    /// 통계
    pub fn idle_ratio(&self) -> f32 {
        let total = self.total_idle_frames + self.total_active_frames;
        if total == 0 { return 0.0; }
        self.total_idle_frames as f32 / total as f32
    }

    pub fn total_idle_frames(&self) -> u64 { self.total_idle_frames }
    pub fn total_active_frames(&self) -> u64 { self.total_active_frames }
}

/// 히트테스트 캐시 — 최근 히트테스트 결과를 캐싱
#[derive(Debug)]
pub struct HitTestCache {
    entries: Vec<HitTestCacheEntry>,
    max_entries: usize,
    hits: u64,
    misses: u64,
}

#[derive(Debug, Clone)]
struct HitTestCacheEntry {
    /// 스크린 좌표 (양자화됨)
    position_key: (i32, i32),
    /// 히트된 위젯 ID
    widget_id: u64,
    /// 캐시 생성 프레임
    generation: u64,
}

impl HitTestCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::with_capacity(max_entries),
            max_entries,
            hits: 0,
            misses: 0,
        }
    }

    /// 위치를 양자화 (2px 단위)
    fn quantize(x: f32, y: f32) -> (i32, i32) {
        ((x / 2.0) as i32, (y / 2.0) as i32)
    }

    pub fn lookup(&mut self, x: f32, y: f32, generation: u64) -> Option<u64> {
        let key = Self::quantize(x, y);
        for entry in &self.entries {
            if entry.position_key == key && entry.generation == generation {
                self.hits += 1;
                return Some(entry.widget_id);
            }
        }
        self.misses += 1;
        None
    }

    pub fn insert(&mut self, x: f32, y: f32, widget_id: u64, generation: u64) {
        let key = Self::quantize(x, y);
        // LRU 퇴거
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(HitTestCacheEntry {
            position_key: key,
            widget_id,
            generation,
        });
    }

    pub fn invalidate(&mut self) {
        self.entries.clear();
    }

    pub fn hit_ratio(&self) -> f32 {
        let total = self.hits + self.misses;
        if total == 0 { return 0.0; }
        self.hits as f32 / total as f32
    }

    pub fn entry_count(&self) -> usize { self.entries.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_detector_initial() {
        let detector = UiIdleDetector::new();
        assert_eq!(detector.state(), UiActivityState::Active);
        assert!(!detector.is_idle());
    }

    #[test]
    fn test_idle_detector_transition() {
        let mut d = UiIdleDetector::new();
        d.begin_frame(0.0);
        assert_eq!(d.state(), UiActivityState::Active); // 첫 프레임
        // 활동 없이 시간 경과
        d.begin_frame(0.3);
        assert_eq!(d.state(), UiActivityState::TransitionToIdle);
        d.begin_frame(0.6); // > 0.5 timeout
        assert_eq!(d.state(), UiActivityState::Idle);
    }

    #[test]
    fn test_idle_detector_wake_up() {
        let mut d = UiIdleDetector::new();
        d.begin_frame(0.0);
        d.begin_frame(1.0); // idle
        assert!(d.is_idle());
        d.record_input();
        assert!(d.is_active());
    }

    #[test]
    fn test_idle_detector_animation_keeps_active() {
        let mut d = UiIdleDetector::new();
        d.set_active_animations(1);
        d.begin_frame(0.0);
        assert!(d.is_active());
        d.begin_frame(1.0); // 시간 지남
        assert!(d.is_active()); // 애니메이션으로 활성 유지
    }

    #[test]
    fn test_idle_skip_tick() {
        let mut d = UiIdleDetector::new();
        d.begin_frame(0.0);
        d.begin_frame(1.0);
        // idle 상태
        d.record_idle_tick(1.0);
        assert!(d.should_skip_tick(1.05)); // 0.05 < 0.1 interval
        assert!(!d.should_skip_tick(1.15)); // 0.15 > 0.1 interval
    }

    #[test]
    fn test_hit_test_cache() {
        let mut cache = HitTestCache::new(10);
        cache.insert(100.0, 200.0, 42, 1);
        assert_eq!(cache.lookup(100.0, 200.0, 1), Some(42));
        assert_eq!(cache.lookup(100.0, 200.0, 2), None); // generation mismatch
        assert_eq!(cache.lookup(105.0, 200.0, 1), None); // position mismatch
    }

    #[test]
    fn test_hit_test_cache_lru() {
        let mut cache = HitTestCache::new(2);
        cache.insert(0.0, 0.0, 1, 1);
        cache.insert(10.0, 10.0, 2, 1);
        cache.insert(20.0, 20.0, 3, 1); // 0,0 evicted
        assert_eq!(cache.entry_count(), 2);
        assert_eq!(cache.lookup(0.0, 0.0, 1), None); // evicted
        assert_eq!(cache.lookup(10.0, 10.0, 1), Some(2));
    }

    #[test]
    fn test_hit_test_cache_invalidate() {
        let mut cache = HitTestCache::new(10);
        cache.insert(0.0, 0.0, 1, 1);
        cache.invalidate();
        assert_eq!(cache.entry_count(), 0);
    }

    #[test]
    fn test_idle_ratio() {
        let mut d = UiIdleDetector::new();
        d.begin_frame(0.0); // active
        d.begin_frame(1.0); // idle
        d.begin_frame(2.0); // idle
        // 1 active + 2 idle = 66.7% idle
        assert!(d.idle_ratio() > 0.6);
    }
}
