//! Notification System — 토스트 알림
//!
//! 화면 우하단에 표시되는 자동 소멸 알림 시스템

use glam::Vec2;
use crate::core::{Color, PaintGeometry};
use crate::widget::{DrawElementList, PaintArgs};

/// 알림 레벨
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationLevel {
    /// 레벨별 배경색
    pub fn background_color(&self) -> Color {
        match self {
            Self::Info => Color::rgba(0.141, 0.141, 0.141, 0.98),
            Self::Success => Color::rgba(0.10, 0.25, 0.10, 0.95),
            Self::Warning => Color::rgba(0.30, 0.25, 0.05, 0.95),
            Self::Error => Color::rgba(0.30, 0.08, 0.08, 0.95),
        }
    }

    /// 레벨별 테두리색
    pub fn border_color(&self) -> Color {
        match self {
            Self::Info => Color::rgba(0.298, 0.298, 0.298, 1.0),
            Self::Success => Color::rgba(0.122, 0.894, 0.294, 1.0),
            Self::Warning => Color::rgba(1.0, 0.722, 0.0, 1.0),
            Self::Error => Color::rgba(0.937, 0.208, 0.208, 1.0),
        }
    }
}

/// 알림 내 인터랙티브 액션 (버튼)
///
/// UE의 `SNotificationItem` 버튼에 해당.
pub struct NotificationAction {
    /// 버튼 라벨
    pub label: String,
    /// 클릭 콜백
    pub callback: Box<dyn FnMut() + Send + Sync>,
}

impl NotificationAction {
    pub fn new(label: impl Into<String>, callback: impl FnMut() + Send + Sync + 'static) -> Self {
        Self {
            label: label.into(),
            callback: Box::new(callback),
        }
    }
}

impl std::fmt::Debug for NotificationAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NotificationAction")
            .field("label", &self.label)
            .finish()
    }
}

/// 알림 항목
pub struct Notification {
    pub id: u64,
    pub level: NotificationLevel,
    pub title: String,
    pub message: String,
    /// 표시 지속 시간 (초)
    pub duration: f32,
    /// 경과 시간
    pub elapsed: f32,
    /// 페이드 진행도 (0=보임, 1=사라짐)
    pub fade_progress: f32,
    /// 인터랙티브 액션 버튼들
    pub actions: Vec<NotificationAction>,
    /// 만료 콜백 (알림이 자동 소멸될 때 호출)
    pub on_expired: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl Notification {
    /// 남은 시간 비율 (1.0 → 0.0)
    pub fn remaining_ratio(&self) -> f32 {
        1.0 - (self.elapsed / self.duration).min(1.0)
    }

    /// 페이드 아웃 시작했는지
    pub fn is_fading(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// 완전히 사라졌는지
    pub fn is_expired(&self) -> bool {
        self.fade_progress >= 1.0
    }
}

/// 알림 관리자
pub struct NotificationManager {
    notifications: Vec<Notification>,
    next_id: u64,
    /// 최대 동시 표시 수
    pub max_visible: usize,
    /// 윈도우 크기
    window_size: Vec2,
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            next_id: 1,
            max_visible: 5,
            window_size: Vec2::new(1920.0, 1080.0),
        }
    }

    /// 윈도우 크기 설정
    pub fn set_window_size(&mut self, size: Vec2) {
        self.window_size = size;
    }

    /// 알림 추가 (기본 5초)
    pub fn push(&mut self, level: NotificationLevel, title: impl Into<String>, message: impl Into<String>) -> u64 {
        self.push_with_duration(level, title, message, 5.0)
    }

    /// 알림 추가 (지속 시간 지정)
    pub fn push_with_duration(
        &mut self,
        level: NotificationLevel,
        title: impl Into<String>,
        message: impl Into<String>,
        duration: f32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        self.notifications.push(Notification {
            id,
            level,
            title: title.into(),
            message: message.into(),
            duration,
            elapsed: 0.0,
            fade_progress: 0.0,
            actions: Vec::new(),
            on_expired: None,
        });

        id
    }

    /// 알림 추가 (액션 버튼 포함)
    pub fn push_with_actions(
        &mut self,
        level: NotificationLevel,
        title: impl Into<String>,
        message: impl Into<String>,
        actions: Vec<NotificationAction>,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        self.notifications.push(Notification {
            id,
            level,
            title: title.into(),
            message: message.into(),
            duration: 8.0, // 액션 있으면 좀 더 오래 표시
            elapsed: 0.0,
            fade_progress: 0.0,
            actions,
            on_expired: None,
        });

        id
    }

    /// 만료 콜백 설정
    pub fn set_expiry_callback(
        &mut self,
        id: u64,
        callback: impl FnOnce() + Send + Sync + 'static,
    ) {
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == id) {
            notif.on_expired = Some(Box::new(callback));
        }
    }

    /// 알림 액션 실행
    pub fn execute_action(&mut self, notification_id: u64, action_index: usize) {
        if let Some(notif) = self.notifications.iter_mut().find(|n| n.id == notification_id) {
            if let Some(action) = notif.actions.get_mut(action_index) {
                (action.callback)();
            }
        }
    }

    /// 수동 닫기
    pub fn dismiss(&mut self, id: u64) {
        self.notifications.retain(|n| n.id != id);
    }

    /// 모두 닫기
    pub fn dismiss_all(&mut self) {
        self.notifications.clear();
    }

    /// 프레임 업데이트
    pub fn tick(&mut self, delta_time: f32) {
        for notif in &mut self.notifications {
            notif.elapsed += delta_time;
            if notif.is_fading() {
                // 0.5초 페이드 아웃
                notif.fade_progress = ((notif.elapsed - notif.duration) / 0.5).min(1.0);
            }
        }

        // 만료 콜백 수집 및 실행 (borrow 충돌 방지)
        let mut expired_callbacks: Vec<Box<dyn FnOnce() + Send + Sync>> = Vec::new();
        for notif in &mut self.notifications {
            if notif.is_expired() {
                if let Some(cb) = notif.on_expired.take() {
                    expired_callbacks.push(cb);
                }
            }
        }
        for cb in expired_callbacks {
            cb();
        }

        // 완전히 사라진 알림 제거
        self.notifications.retain(|n| !n.is_expired());
    }

    /// 활성 알림 수
    pub fn count(&self) -> usize {
        self.notifications.len()
    }

    /// 알림이 있는지
    pub fn has_notifications(&self) -> bool {
        !self.notifications.is_empty()
    }

    /// 렌더링 — 우하단 스택
    pub fn paint(
        &self,
        _args: &PaintArgs,
        draw_elements: &mut DrawElementList,
        base_layer: u32,
    ) -> u32 {
        let mut layer = base_layer;

        let notif_width = 320.0_f32;
        let notif_height = 60.0_f32;
        let margin = 10.0_f32;
        let padding = 8.0_f32;

        let visible = self.notifications.iter()
            .rev()
            .take(self.max_visible);

        let mut y_offset = 0.0_f32;

        for notif in visible {
            let alpha = 1.0 - notif.fade_progress;
            if alpha <= 0.0 {
                continue;
            }

            let x = self.window_size.x - notif_width - margin;
            let y = self.window_size.y - notif_height - margin - y_offset;

            // 배경
            let bg = notif.level.background_color();
            let bg_alpha = Color::rgba(bg.r, bg.g, bg.b, bg.a * alpha);
            let border = notif.level.border_color();
            let border_alpha = Color::rgba(border.r, border.g, border.b, border.a * alpha);

            let geo = PaintGeometry::new(Vec2::new(x, y), Vec2::new(notif_width, notif_height), 1.0);
            draw_elements.add_border(layer, geo, bg_alpha, border_alpha, 1.0);
            layer += 1;

            // 타이틀
            let title_geo = PaintGeometry::new(
                Vec2::new(x + padding, y + padding),
                Vec2::new(notif_width - padding * 2.0, 16.0),
                1.0,
            );
            let title_color = Color::rgba(1.0, 1.0, 1.0, alpha);
            draw_elements.add_text(layer, title_geo, notif.title.clone(), title_color, 14.0);
            layer += 1;

            // 메시지
            let msg_geo = PaintGeometry::new(
                Vec2::new(x + padding, y + padding + 20.0),
                Vec2::new(notif_width - padding * 2.0, 14.0),
                1.0,
            );
            let msg_color = Color::rgba(0.8, 0.8, 0.8, alpha);
            draw_elements.add_text(layer, msg_geo, notif.message.clone(), msg_color, 12.0);
            layer += 1;

            y_offset += notif_height + 4.0;
        }

        layer
    }
}
