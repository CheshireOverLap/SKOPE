//! 렌더 커맨드 펜스 -- Condvar 기반 GT/RT 동기화
//!
//! UE5 FRenderCommandFence 패턴: GT에서 펜스 생성 -> RT로 전송 -> RT 시그널 -> GT 대기

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

/// GT/RT 동기화 펜스
///
/// GT에서 생성하여 `SignalFence` 커맨드로 RT에 전송.
/// RT가 해당 커맨드를 처리하면 Condvar를 시그널하여 GT를 깨움.
pub struct RenderCommandFence {
    fence_id: u64,
    signal: Arc<(Mutex<bool>, Condvar)>,
}

impl RenderCommandFence {
    /// 새 펜스 생성 (고유 ID 자동 할당)
    pub fn new(counter: &AtomicU64) -> Self {
        let fence_id = counter.fetch_add(1, Ordering::Relaxed);
        Self {
            fence_id,
            signal: Arc::new((Mutex::new(false), Condvar::new())),
        }
    }

    /// 펜스 ID
    pub fn id(&self) -> u64 {
        self.fence_id
    }

    /// RT에 전달할 시그널 쌍 (Arc clone)
    pub fn signal_pair(&self) -> Arc<(Mutex<bool>, Condvar)> {
        Arc::clone(&self.signal)
    }

    /// RT 시그널 대기 (블로킹)
    ///
    /// 모달 리사이즈, 종료 전 동기화 등에 사용.
    /// 타임아웃 5초 -- 교착 방지 (RT가 이미 종료된 경우).
    pub fn wait(&self) {
        let (lock, cvar) = &*self.signal;
        let mut signaled = lock.lock().unwrap();
        let timeout = Duration::from_secs(5);
        while !*signaled {
            let result = cvar.wait_timeout(signaled, timeout).unwrap();
            signaled = result.0;
            if result.1.timed_out() {
                log::warn!("[RenderCommandFence] Fence {} timed out (5s)", self.fence_id);
                break;
            }
        }
    }

    /// 논블로킹 완료 체크
    pub fn is_complete(&self) -> bool {
        let (lock, _) = &*self.signal;
        *lock.lock().unwrap()
    }
}
