//! Active Timer 시스템 — UE5.7 SWidget::RegisterActiveTimer
//!
//! 위젯별 주기적 콜백을 등록하여 애니메이션, 깜빡임 등 시간 기반 업데이트를 수행합니다.
//! 활성 타이머가 없으면 UI는 idle 상태로 CPU를 절약할 수 있습니다.

use std::sync::atomic::{AtomicU64, Ordering};

/// Active Timer ID 타입 — UE5.7 FActiveTimerHandle의 ID
///
/// 타입 안전성을 위한 신규 타입. `register()` 반환값에 사용.
pub type ActiveTimerId = u64;

// ============================================================================
// ActiveTimerReturnType
// ============================================================================

/// Active Timer 콜백 반환 타입 (UE의 EActiveTimerReturnType)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTimerReturnType {
    /// 타이머 중지 및 자동 해제
    Stop,
    /// 타이머 계속 실행
    Continue,
}

// ============================================================================
// ActiveTimerHandle
// ============================================================================

/// 개별 타이머 핸들
///
/// 위젯이 `ActiveTimers::register()`로 생성하며, 고유 ID + 주기 + 다음 실행 시각을 저장합니다.
pub struct ActiveTimerHandle {
    /// 고유 ID
    id: u64,
    /// 실행 주기 (초). 0.0 = 매 프레임
    period: f32,
    /// 다음 실행 시각 (app_start_time 기준 elapsed seconds)
    next_execution_time: f64,
}

/// 타이머 ID 생성기
fn next_timer_id() -> u64 {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

impl ActiveTimerHandle {
    /// 새 타이머 핸들 생성
    ///
    /// `period`: 실행 주기 (초). 0.0이면 매 프레임 실행.
    /// `next_execution_time`은 0.0으로 설정되어 다음 prepass에서 즉시 실행됩니다.
    pub fn new(period: f32) -> Self {
        Self {
            id: next_timer_id(),
            period,
            next_execution_time: 0.0, // 즉시 실행
        }
    }

    /// 타이머 ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 실행 주기
    pub fn period(&self) -> f32 {
        self.period
    }

    /// 현재 시각에서 실행 대기 중인지 확인
    pub fn is_pending(&self, current_time: f64) -> bool {
        current_time >= self.next_execution_time
    }

    /// 다음 실행 시각을 전진
    ///
    /// UE 패턴: 느린 프레임으로 여러 주기가 지나도 콜백은 1회만 실행하고,
    /// `next_execution_time`만 현재 시각 이후로 전진시킵니다.
    pub fn advance(&mut self, current_time: f64) {
        if self.period > 0.0 {
            // 놓친 주기 스킵 — next_execution_time을 current_time 이후로
            while current_time >= self.next_execution_time {
                self.next_execution_time += self.period as f64;
            }
        } else {
            // period == 0: 매 프레임 실행. 아주 작은 값으로 전진.
            self.next_execution_time = current_time + f64::EPSILON;
        }
    }
}

// ============================================================================
// ActiveTimers
// ============================================================================

/// 위젯이 소유하는 타이머 컬렉션
///
/// 각 위젯은 필요 시 `ActiveTimers` 필드를 갖고,
/// `register()` / `unregister()`로 타이머를 관리합니다.
///
/// ```ignore
/// // 위젯 내부:
/// self.timer_id = Some(self.active_timers.register(0.0)); // 매 프레임
/// self.timer_id = Some(self.active_timers.register(0.5)); // 0.5초마다
/// self.active_timers.unregister(id); // 수동 해제
/// ```
pub struct ActiveTimers {
    timers: Vec<ActiveTimerHandle>,
}

impl ActiveTimers {
    /// 빈 타이머 컬렉션 생성
    pub fn new() -> Self {
        Self { timers: Vec::new() }
    }

    /// 타이머 등록 — UE5.7 RegisterActiveTimer
    ///
    /// `period`: 실행 주기 (초). 0.0 = 매 프레임.
    /// 반환: 타이머 ID (해제 시 사용)
    pub fn register(&mut self, period: f32) -> ActiveTimerId {
        let handle = ActiveTimerHandle::new(period);
        let id = handle.id;
        self.timers.push(handle);
        id
    }

    /// 타이머 수동 해제
    pub fn unregister(&mut self, id: u64) {
        self.timers.retain(|h| h.id != id);
    }

    /// 등록된 타이머가 있는지
    pub fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// 등록된 타이머 수
    pub fn len(&self) -> usize {
        self.timers.len()
    }

    /// 모든 타이머 해제 — UE5.7 UnregisterAllActiveTimers
    pub fn unregister_all(&mut self) {
        self.timers.clear();
    }

    /// 타이머 주기 조회 — UE5.7 타이머 인트로스펙션
    pub fn get_timer_period(&self, id: u64) -> Option<f32> {
        self.timers.iter().find(|h| h.id == id).map(|h| h.period)
    }

    /// 등록된 모든 타이머 ID 목록
    pub fn get_timer_ids(&self) -> Vec<u64> {
        self.timers.iter().map(|h| h.id).collect()
    }

    /// 대기 중인 타이머 실행
    ///
    /// 각 대기 중인 타이머에 대해 `callback(timer_id)`를 호출합니다.
    /// 콜백이 `ActiveTimerReturnType::Stop`을 반환하면 해당 타이머는 자동 제거됩니다.
    /// Zero-allocation: `retain_mut`으로 단일 패스 처리.
    pub fn execute_pending<F>(&mut self, current_time: f64, mut callback: F)
    where
        F: FnMut(u64) -> ActiveTimerReturnType,
    {
        self.timers.retain_mut(|handle| {
            if handle.is_pending(current_time) {
                handle.advance(current_time);
                matches!(callback(handle.id), ActiveTimerReturnType::Continue)
            } else {
                true // 아직 대기 시간 미도달, 유지
            }
        });
    }
}

impl Default for ActiveTimers {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_registration() {
        let mut timers = ActiveTimers::new();
        assert!(timers.is_empty());

        let id = timers.register(0.5);
        assert!(!timers.is_empty());
        assert_eq!(timers.len(), 1);

        timers.unregister(id);
        assert!(timers.is_empty());
    }

    #[test]
    fn test_timer_pending_immediately() {
        let mut timers = ActiveTimers::new();
        let _id = timers.register(0.0);

        // next_execution_time = 0.0, current_time = 0.1 → pending
        let mut fired = false;
        timers.execute_pending(0.1, |_| {
            fired = true;
            ActiveTimerReturnType::Continue
        });
        assert!(fired);
    }

    #[test]
    fn test_timer_period() {
        let mut timers = ActiveTimers::new();
        let _id = timers.register(1.0); // 1초마다

        // t=0.5: 아직 아님 (next=0.0이므로 실제론 즉시 실행됨 → next=1.0)
        let mut count = 0;
        timers.execute_pending(0.5, |_| {
            count += 1;
            ActiveTimerReturnType::Continue
        });
        assert_eq!(count, 1); // 첫 실행 (next_execution_time=0.0 < 0.5)

        // t=0.8: next=1.0 이후에야 실행
        count = 0;
        timers.execute_pending(0.8, |_| {
            count += 1;
            ActiveTimerReturnType::Continue
        });
        assert_eq!(count, 0); // 아직

        // t=1.5: next=1.0 < 1.5 → 실행
        count = 0;
        timers.execute_pending(1.5, |_| {
            count += 1;
            ActiveTimerReturnType::Continue
        });
        assert_eq!(count, 1);
    }

    #[test]
    fn test_timer_auto_stop() {
        let mut timers = ActiveTimers::new();
        let _id = timers.register(0.0);

        timers.execute_pending(1.0, |_| ActiveTimerReturnType::Stop);
        assert!(timers.is_empty()); // Stop → 자동 제거
    }

    #[test]
    fn test_multiple_timers() {
        let mut timers = ActiveTimers::new();
        let id1 = timers.register(0.0);
        let _id2 = timers.register(0.0);
        assert_eq!(timers.len(), 2);

        // id1만 Stop
        timers.execute_pending(1.0, |id| {
            if id == id1 {
                ActiveTimerReturnType::Stop
            } else {
                ActiveTimerReturnType::Continue
            }
        });
        assert_eq!(timers.len(), 1);
    }

    #[test]
    fn test_advance_skips_missed() {
        let mut handle = ActiveTimerHandle::new(0.5); // 0.5초 주기
        // next = 0.0, advance at t=2.5
        handle.advance(2.5);
        // next should be > 2.5 (skipped 0.0, 0.5, 1.0, 1.5, 2.0, 2.5 → next = 3.0)
        assert!(handle.next_execution_time > 2.5);
        assert!(!handle.is_pending(2.5));
    }
}
