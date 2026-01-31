//! 비동기 알림 시스템 — 스레드 안전 큐 기반 알림 처리
//!
//! 백그라운드 작업에서 UI 알림을 안전하게 전달합니다.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// 알림 우선순위
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum NotificationPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// 알림 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationState {
    /// 대기 중 — 아직 표시되지 않음
    Pending,
    /// 표시 중
    Active,
    /// 페이드아웃 중
    FadingOut,
    /// 완료
    Completed,
    /// 취소됨
    Cancelled,
}

/// 비동기 알림 항목
#[derive(Debug, Clone)]
pub struct AsyncNotification {
    pub id: u64,
    pub title: String,
    pub message: String,
    pub priority: NotificationPriority,
    pub state: NotificationState,
    pub progress: Option<f32>,
    pub timestamp: f64,
    pub duration: f64,
    pub is_dismissible: bool,
    pub category: String,
}

impl AsyncNotification {
    pub fn new(id: u64, title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            message: message.into(),
            priority: NotificationPriority::Normal,
            state: NotificationState::Pending,
            progress: None,
            timestamp: 0.0,
            duration: 5.0,
            is_dismissible: true,
            category: "default".into(),
        }
    }

    pub fn with_priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_progress(mut self, progress: f32) -> Self {
        self.progress = Some(progress.clamp(0.0, 1.0));
        self
    }

    pub fn with_duration(mut self, duration: f64) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }

    /// 진행률이 완료인지
    pub fn is_progress_complete(&self) -> bool {
        self.progress.map_or(false, |p| p >= 1.0)
    }
}

/// 스레드 안전 알림 큐
pub struct ThreadSafeNotificationQueue {
    inner: Arc<Mutex<NotificationQueueInner>>,
}

struct NotificationQueueInner {
    pending: VecDeque<AsyncNotification>,
    next_id: u64,
}

impl ThreadSafeNotificationQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(NotificationQueueInner {
                pending: VecDeque::new(),
                next_id: 1,
            })),
        }
    }

    /// 스레드에서 호출 가능한 핸들 생성
    pub fn create_handle(&self) -> NotificationQueueHandle {
        NotificationQueueHandle {
            inner: self.inner.clone(),
        }
    }

    /// UI 스레드에서 대기 중인 알림 꺼내기
    pub fn drain_pending(&self) -> Vec<AsyncNotification> {
        let mut guard = self.inner.lock().unwrap();
        guard.pending.drain(..).collect()
    }

    /// 대기 중인 알림 수
    pub fn pending_count(&self) -> usize {
        let guard = self.inner.lock().unwrap();
        guard.pending.len()
    }
}

impl Clone for ThreadSafeNotificationQueue {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// 스레드 안전 큐 핸들 — Send + Sync
#[derive(Clone)]
pub struct NotificationQueueHandle {
    inner: Arc<Mutex<NotificationQueueInner>>,
}

impl NotificationQueueHandle {
    /// 백그라운드 스레드에서 알림 추가
    pub fn push(&self, title: impl Into<String>, message: impl Into<String>) -> u64 {
        let mut guard = self.inner.lock().unwrap();
        let id = guard.next_id;
        guard.next_id += 1;
        guard.pending.push_back(AsyncNotification::new(id, title, message));
        id
    }

    /// 우선순위 지정 알림 추가
    pub fn push_with_priority(
        &self,
        title: impl Into<String>,
        message: impl Into<String>,
        priority: NotificationPriority,
    ) -> u64 {
        let mut guard = self.inner.lock().unwrap();
        let id = guard.next_id;
        guard.next_id += 1;
        let notif = AsyncNotification::new(id, title, message)
            .with_priority(priority);
        guard.pending.push_back(notif);
        id
    }

    /// 진행률 알림 추가
    pub fn push_progress(
        &self,
        title: impl Into<String>,
        message: impl Into<String>,
        progress: f32,
    ) -> u64 {
        let mut guard = self.inner.lock().unwrap();
        let id = guard.next_id;
        guard.next_id += 1;
        let notif = AsyncNotification::new(id, title, message)
            .with_progress(progress);
        guard.pending.push_back(notif);
        id
    }
}

/// 알림 관리자 (UI 스레드)
pub struct AsyncNotificationManager {
    queue: ThreadSafeNotificationQueue,
    active: Vec<AsyncNotification>,
    max_visible: usize,
    fade_duration: f64,
}

impl AsyncNotificationManager {
    pub fn new() -> Self {
        Self {
            queue: ThreadSafeNotificationQueue::new(),
            active: Vec::new(),
            max_visible: 5,
            fade_duration: 0.3,
        }
    }

    pub fn queue(&self) -> &ThreadSafeNotificationQueue {
        &self.queue
    }

    pub fn create_handle(&self) -> NotificationQueueHandle {
        self.queue.create_handle()
    }

    /// 매 프레임 호출 — 큐에서 알림 꺼내고 상태 업데이트
    pub fn update(&mut self, current_time: f64) {
        // 큐에서 새 알림 가져오기
        let new_notifications = self.queue.drain_pending();
        for mut notif in new_notifications {
            notif.state = NotificationState::Active;
            notif.timestamp = current_time;
            self.active.push(notif);
        }

        // 상태 업데이트
        for notif in &mut self.active {
            match notif.state {
                NotificationState::Active => {
                    let elapsed = current_time - notif.timestamp;
                    if elapsed >= notif.duration {
                        notif.state = NotificationState::FadingOut;
                        notif.timestamp = current_time;
                    }
                }
                NotificationState::FadingOut => {
                    let elapsed = current_time - notif.timestamp;
                    if elapsed >= self.fade_duration {
                        notif.state = NotificationState::Completed;
                    }
                }
                _ => {}
            }
        }

        // 완료/취소된 알림 제거
        self.active.retain(|n| {
            n.state != NotificationState::Completed
            && n.state != NotificationState::Cancelled
        });
    }

    /// 알림 해제
    pub fn dismiss(&mut self, id: u64) {
        if let Some(notif) = self.active.iter_mut().find(|n| n.id == id) {
            notif.state = NotificationState::Cancelled;
        }
    }

    /// 특정 카테고리 알림 전체 해제
    pub fn dismiss_category(&mut self, category: &str) {
        for notif in &mut self.active {
            if notif.category == category {
                notif.state = NotificationState::Cancelled;
            }
        }
    }

    pub fn active_notifications(&self) -> &[AsyncNotification] { &self.active }
    pub fn active_count(&self) -> usize { self.active.len() }

    /// 보이는 알림 (max_visible 제한)
    pub fn visible_notifications(&self) -> &[AsyncNotification] {
        let count = self.active.len().min(self.max_visible);
        &self.active[..count]
    }

    pub fn set_max_visible(&mut self, max: usize) {
        self.max_visible = max;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_creation() {
        let notif = AsyncNotification::new(1, "Build", "Build completed")
            .with_priority(NotificationPriority::High)
            .with_duration(3.0);
        assert_eq!(notif.priority, NotificationPriority::High);
        assert_eq!(notif.duration, 3.0);
    }

    #[test]
    fn test_notification_progress() {
        let notif = AsyncNotification::new(1, "Download", "50%")
            .with_progress(0.5);
        assert!(!notif.is_progress_complete());
        let done = AsyncNotification::new(2, "Done", "100%")
            .with_progress(1.0);
        assert!(done.is_progress_complete());
    }

    #[test]
    fn test_thread_safe_queue() {
        let queue = ThreadSafeNotificationQueue::new();
        let handle = queue.create_handle();
        handle.push("Test", "Hello");
        handle.push("Test2", "World");
        assert_eq!(queue.pending_count(), 2);
        let drained = queue.drain_pending();
        assert_eq!(drained.len(), 2);
        assert_eq!(queue.pending_count(), 0);
    }

    #[test]
    fn test_thread_safe_queue_from_thread() {
        let queue = ThreadSafeNotificationQueue::new();
        let handle = queue.create_handle();
        let t = std::thread::spawn(move || {
            handle.push("From Thread", "Message");
        });
        t.join().unwrap();
        assert_eq!(queue.pending_count(), 1);
    }

    #[test]
    fn test_notification_manager_lifecycle() {
        let mut mgr = AsyncNotificationManager::new();
        let handle = mgr.create_handle();
        handle.push("Test", "Message");

        mgr.update(0.0);
        assert_eq!(mgr.active_count(), 1);
        assert_eq!(mgr.active_notifications()[0].state, NotificationState::Active);

        // 시간 경과 → FadingOut
        mgr.update(6.0); // > 5.0 duration
        assert_eq!(mgr.active_notifications()[0].state, NotificationState::FadingOut);

        // 페이드 완료 → 제거
        mgr.update(6.5); // > 0.3 fade
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_notification_dismiss() {
        let mut mgr = AsyncNotificationManager::new();
        let handle = mgr.create_handle();
        let id = handle.push("Dismiss Me", "Now");
        mgr.update(0.0);
        mgr.dismiss(id);
        mgr.update(0.1);
        assert_eq!(mgr.active_count(), 0);
    }

    #[test]
    fn test_notification_priority_ordering() {
        assert!(NotificationPriority::Critical > NotificationPriority::High);
        assert!(NotificationPriority::High > NotificationPriority::Normal);
        assert!(NotificationPriority::Normal > NotificationPriority::Low);
    }

    #[test]
    fn test_max_visible() {
        let mut mgr = AsyncNotificationManager::new();
        mgr.set_max_visible(2);
        let handle = mgr.create_handle();
        for i in 0..5 {
            handle.push(format!("N{}", i), "msg");
        }
        mgr.update(0.0);
        assert_eq!(mgr.active_count(), 5);
        assert_eq!(mgr.visible_notifications().len(), 2);
    }
}
