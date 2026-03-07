#![allow(dead_code)]
//! ProgressNotification — 백그라운드 작업 진행률 표시
//!
//! UE 참조: `SNotificationItem`, `IProgressNotificationHandler`
//!
//! 장시간 실행되는 작업의 진행률을 상태바 또는 토스트에 표시합니다.

use std::sync::atomic::{AtomicU64, Ordering};

/// 고유 진행률 알림 ID 생성
static NEXT_PROGRESS_ID: AtomicU64 = AtomicU64::new(1);

fn next_progress_id() -> u64 {
    NEXT_PROGRESS_ID.fetch_add(1, Ordering::Relaxed)
}

/// 진행률 알림 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    /// 실행 중
    InProgress,
    /// 완료
    Completed,
    /// 취소됨
    Cancelled,
    /// 실패
    Failed,
}

/// 백그라운드 작업 진행률 알림
pub struct ProgressNotification {
    /// 고유 ID
    pub id: u64,
    /// 작업 제목
    pub title: String,
    /// 현재 진행률 (0.0 ~ 1.0)
    pub progress: f32,
    /// 현재 상태 텍스트
    pub status_text: String,
    /// 상태
    pub state: ProgressState,
    /// 취소 가능 여부
    pub is_cancellable: bool,
    /// 취소 콜백
    on_cancel: Option<Box<dyn FnMut() + Send + Sync>>,
    /// 완료 콜백
    on_complete: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl ProgressNotification {
    /// 새 진행률 알림 생성
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            id: next_progress_id(),
            title: title.into(),
            progress: 0.0,
            status_text: String::new(),
            state: ProgressState::InProgress,
            is_cancellable: false,
            on_cancel: None,
            on_complete: None,
        }
    }

    /// 취소 가능하게 설정
    pub fn cancellable(mut self, callback: impl FnMut() + Send + Sync + 'static) -> Self {
        self.is_cancellable = true;
        self.on_cancel = Some(Box::new(callback));
        self
    }

    /// 완료 콜백 설정
    pub fn on_complete(mut self, callback: impl FnOnce() + Send + Sync + 'static) -> Self {
        self.on_complete = Some(Box::new(callback));
        self
    }

    /// 진행률 업데이트
    pub fn update_progress(&mut self, progress: f32, status: impl Into<String>) {
        self.progress = progress.clamp(0.0, 1.0);
        self.status_text = status.into();
    }

    /// 완료 처리
    pub fn complete(&mut self, message: impl Into<String>) {
        self.progress = 1.0;
        self.status_text = message.into();
        self.state = ProgressState::Completed;
        if let Some(cb) = self.on_complete.take() {
            cb();
        }
    }

    /// 취소 처리
    pub fn cancel(&mut self) {
        if self.is_cancellable {
            self.state = ProgressState::Cancelled;
            if let Some(ref mut cb) = self.on_cancel {
                cb();
            }
        }
    }

    /// 실패 처리
    pub fn fail(&mut self, message: impl Into<String>) {
        self.status_text = message.into();
        self.state = ProgressState::Failed;
    }

    /// 진행 중인지
    pub fn is_active(&self) -> bool {
        self.state == ProgressState::InProgress
    }

    /// 퍼센트 문자열 (예: "75%")
    pub fn percent_text(&self) -> String {
        format!("{}%", (self.progress * 100.0) as u32)
    }
}

/// 진행률 알림 핸들러 인터페이스
///
/// 상태바 등 UI 컴포넌트가 구현하여 진행률 알림을 수신합니다.
pub trait IProgressNotificationHandler: Send + Sync {
    /// 진행률이 업데이트됨
    fn on_progress_updated(&self, id: u64, progress: f32, status: &str);
    /// 작업 완료
    fn on_completed(&self, id: u64);
    /// 작업 취소됨
    fn on_cancelled(&self, id: u64);
    /// 작업 실패
    fn on_failed(&self, id: u64, message: &str);
}

/// 진행률 알림 매니저
pub struct ProgressNotificationManager {
    notifications: Vec<ProgressNotification>,
    handlers: Vec<Box<dyn IProgressNotificationHandler>>,
}

impl Default for ProgressNotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressNotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            handlers: Vec::new(),
        }
    }

    /// 핸들러 등록
    pub fn add_handler(&mut self, handler: Box<dyn IProgressNotificationHandler>) {
        self.handlers.push(handler);
    }

    /// 진행률 알림 추가
    pub fn push(&mut self, notification: ProgressNotification) -> u64 {
        let id = notification.id;
        self.notifications.push(notification);
        id
    }

    /// 진행률 업데이트
    pub fn update_progress(&mut self, id: u64, progress: f32, status: impl Into<String>) {
        let status_str = status.into();
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == id) {
            notif.update_progress(progress, &status_str);
        }
        // 핸들러 알림
        for handler in &self.handlers {
            handler.on_progress_updated(id, progress, &status_str);
        }
    }

    /// 완료 처리
    pub fn complete(&mut self, id: u64, message: impl Into<String>) {
        let msg = message.into();
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == id) {
            notif.complete(&msg);
        }
        for handler in &self.handlers {
            handler.on_completed(id);
        }
    }

    /// 취소 처리
    pub fn cancel(&mut self, id: u64) {
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == id) {
            notif.cancel();
        }
        for handler in &self.handlers {
            handler.on_cancelled(id);
        }
    }

    /// 활성 진행률 알림 수
    pub fn active_count(&self) -> usize {
        self.notifications.iter().filter(|n| n.is_active()).count()
    }

    /// 모든 진행률 알림 (읽기)
    pub fn notifications(&self) -> &[ProgressNotification] {
        &self.notifications
    }

    /// 완료/취소/실패된 알림 정리
    pub fn cleanup_finished(&mut self) {
        self.notifications.retain(|n| n.is_active());
    }

    /// ID로 알림 조회
    pub fn get(&self, id: u64) -> Option<&ProgressNotification> {
        self.notifications.iter().find(|n| n.id == id)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_progress_notification_lifecycle() {
        let mut notif = ProgressNotification::new("Building...");
        assert!(notif.is_active());
        assert_eq!(notif.progress, 0.0);

        notif.update_progress(0.5, "Compiling 50%");
        assert_eq!(notif.progress, 0.5);
        assert_eq!(notif.status_text, "Compiling 50%");
        assert_eq!(notif.percent_text(), "50%");

        notif.complete("Done!");
        assert!(!notif.is_active());
        assert_eq!(notif.state, ProgressState::Completed);
        assert_eq!(notif.progress, 1.0);
    }

    #[test]
    fn test_progress_cancel() {
        let cancelled = Arc::new(Mutex::new(false));
        let cancelled_clone = cancelled.clone();

        let mut notif = ProgressNotification::new("Long task")
            .cancellable(move || {
                *cancelled_clone.lock().unwrap() = true;
            });

        assert!(notif.is_cancellable);
        notif.cancel();
        assert_eq!(notif.state, ProgressState::Cancelled);
        assert!(*cancelled.lock().unwrap());
    }

    #[test]
    fn test_progress_manager() {
        let mut mgr = ProgressNotificationManager::new();

        let n1 = ProgressNotification::new("Task 1");
        let n2 = ProgressNotification::new("Task 2");
        let id1 = mgr.push(n1);
        let id2 = mgr.push(n2);

        assert_eq!(mgr.active_count(), 2);

        mgr.update_progress(id1, 0.75, "Almost done");
        assert_eq!(mgr.get(id1).unwrap().progress, 0.75);

        mgr.complete(id2, "Finished");
        assert_eq!(mgr.active_count(), 1);

        mgr.cleanup_finished();
        assert_eq!(mgr.notifications().len(), 1);
    }

    #[test]
    fn test_progress_handler() {
        struct TestHandler {
            updates: Arc<Mutex<Vec<(u64, f32)>>>,
        }
        impl IProgressNotificationHandler for TestHandler {
            fn on_progress_updated(&self, id: u64, progress: f32, _status: &str) {
                self.updates.lock().unwrap().push((id, progress));
            }
            fn on_completed(&self, _id: u64) {}
            fn on_cancelled(&self, _id: u64) {}
            fn on_failed(&self, _id: u64, _message: &str) {}
        }

        let updates = Arc::new(Mutex::new(Vec::new()));
        let handler = TestHandler { updates: updates.clone() };

        let mut mgr = ProgressNotificationManager::new();
        mgr.add_handler(Box::new(handler));

        let notif = ProgressNotification::new("Test");
        let id = mgr.push(notif);

        mgr.update_progress(id, 0.5, "Half");
        mgr.update_progress(id, 1.0, "Done");

        let updates = updates.lock().unwrap();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0], (id, 0.5));
        assert_eq!(updates[1], (id, 1.0));
    }

    #[test]
    fn test_progress_clamp() {
        let mut notif = ProgressNotification::new("Test");
        notif.update_progress(1.5, "Over");
        assert_eq!(notif.progress, 1.0);

        notif.update_progress(-0.5, "Under");
        assert_eq!(notif.progress, 0.0);
    }

    #[test]
    fn test_progress_fail() {
        let mut notif = ProgressNotification::new("Failing task");
        notif.update_progress(0.3, "Working...");
        notif.fail("Network error");
        assert_eq!(notif.state, ProgressState::Failed);
        assert_eq!(notif.status_text, "Network error");
        assert!(!notif.is_active());
    }
}
