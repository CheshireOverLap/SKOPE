//! RT 헬스 모니터링 — 패닉 감지/격리
//!
//! UE5: GIsRenderingThreadHealthy + GRenderingThreadError
//! GT가 읽고 RT가 패닉 시 쓰는 공유 헬스 상태.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// GT가 읽고 RT가 패닉 시 쓰는 공유 헬스 상태
///
/// UE5: `GIsRenderingThreadHealthy` + `GRenderingThreadError`
#[derive(Clone)]
pub struct RenderThreadHealth {
    healthy: Arc<AtomicBool>,
    error: Arc<Mutex<Option<String>>>,
}

impl RenderThreadHealth {
    pub fn new() -> Self {
        Self {
            healthy: Arc::new(AtomicBool::new(true)),
            error: Arc::new(Mutex::new(None)),
        }
    }

    /// RT가 정상 동작 중인지 확인 (GT에서 호출)
    pub fn is_healthy(&self) -> bool {
        self.healthy.load(Ordering::Acquire)
    }

    /// RT 패닉 메시지 조회 (GT에서 호출)
    pub fn get_error(&self) -> Option<String> {
        self.error.lock().unwrap().clone()
    }

    /// RT 패닉 상태 기록 (RT catch_unwind에서 호출)
    pub(crate) fn set_panicked(&self, message: String) {
        *self.error.lock().unwrap() = Some(message);
        self.healthy.store(false, Ordering::Release);
    }
}
