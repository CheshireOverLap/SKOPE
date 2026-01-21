//! Contextual Suggestion System - AI 제안 토스트
//!
//! 사용자가 요청하지 않아도 AI가 맥락을 파악해 먼저 제안하는 시스템입니다.
//! Scene View 우측 상단에 토스트 형태로 표시됩니다.

use std::time::{Duration, Instant};

use bevy_ecs::entity::Entity;
use egui::{Color32, Pos2, Rect, Ui, Vec2};

use super::command_palette::PaletteAction;

/// 제안 타입
#[derive(Debug, Clone)]
pub enum SuggestionType {
    /// 컴포넌트 추가 제안
    MissingComponent {
        entity: Entity,
        component_name: String,
    },
    /// 코드/스크립트 생성 제안
    CodeGeneration {
        description: String,
    },
    /// 성능 최적화 제안
    Optimization {
        issue: String,
        solution: String,
    },
    /// 일반 팁
    Tip {
        message: String,
    },
    /// 에러/경고 해결 제안
    ErrorFix {
        error_message: String,
        suggestion: String,
    },
}

/// 제안 액션 버튼 스타일
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuggestionButtonStyle {
    #[default]
    Primary,
    Secondary,
    Danger,
}

/// 제안 액션 버튼
#[derive(Debug, Clone)]
pub struct SuggestionAction {
    /// 버튼 라벨
    pub label: String,
    /// 액션 타입
    pub action_type: SuggestionActionType,
    /// 버튼 스타일
    pub style: SuggestionButtonStyle,
}

/// 제안 액션 타입
#[derive(Debug, Clone)]
pub enum SuggestionActionType {
    /// 제안 수락 (자동 실행)
    Accept(PaletteAction),
    /// 미리보기 (Diff View)
    Preview,
    /// 무시 (이번만)
    Dismiss,
    /// 이런 제안 다시 안 보기
    NeverShow,
}

impl SuggestionAction {
    /// Accept 버튼 생성
    pub fn accept(label: impl Into<String>, action: PaletteAction) -> Self {
        Self {
            label: label.into(),
            action_type: SuggestionActionType::Accept(action),
            style: SuggestionButtonStyle::Primary,
        }
    }

    /// Dismiss 버튼 생성
    pub fn dismiss(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action_type: SuggestionActionType::Dismiss,
            style: SuggestionButtonStyle::Secondary,
        }
    }

    /// Never Show 버튼 생성
    pub fn never_show() -> Self {
        Self {
            label: "Don't show again".into(),
            action_type: SuggestionActionType::NeverShow,
            style: SuggestionButtonStyle::Danger,
        }
    }
}

/// AI 제안 토스트
#[derive(Debug, Clone)]
pub struct SuggestionToast {
    /// 고유 ID
    pub id: u64,
    /// 제목
    pub title: String,
    /// 설명
    pub description: String,
    /// 제안 타입
    pub suggestion_type: SuggestionType,
    /// 관련도 점수 (0.0 ~ 1.0)
    pub relevance: f32,
    /// 액션 버튼들
    pub actions: Vec<SuggestionAction>,
    /// 생성 시간
    pub created_at: Instant,
    /// 만료 시간 (None이면 무기한)
    pub expires_at: Option<Instant>,
    /// 무시됨 (닫힘)
    pub dismissed: bool,
    /// 애니메이션 진행도 (0.0 ~ 1.0)
    pub anim_progress: f32,
}

impl SuggestionToast {
    /// 새 토스트 생성
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        suggestion_type: SuggestionType,
    ) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            title: title.into(),
            description: description.into(),
            suggestion_type,
            relevance: 0.5,
            actions: vec![
                SuggestionAction::dismiss("Dismiss"),
            ],
            created_at: Instant::now(),
            expires_at: Some(Instant::now() + Duration::from_secs(10)), // 기본 10초
            dismissed: false,
            anim_progress: 0.0,
        }
    }

    /// 컴포넌트 누락 제안 생성
    pub fn missing_component(
        entity: Entity,
        entity_name: &str,
        component_name: &str,
    ) -> Self {
        let mut toast = Self::new(
            format!("Missing {}", component_name),
            format!(
                "'{}' might need a {} component.\nWould you like to add it?",
                entity_name, component_name
            ),
            SuggestionType::MissingComponent {
                entity,
                component_name: component_name.to_string(),
            },
        );

        toast.actions = vec![
            SuggestionAction::accept(
                format!("Add {}", component_name),
                PaletteAction::Command(format!("component.add_{}", component_name.to_lowercase())),
            ),
            SuggestionAction::dismiss("Not now"),
        ];

        toast.relevance = 0.8;
        toast
    }

    /// 팁 제안 생성
    pub fn tip(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(
            title,
            message,
            SuggestionType::Tip { message: String::new() },
        )
    }

    /// 관련도 설정
    pub fn with_relevance(mut self, relevance: f32) -> Self {
        self.relevance = relevance.clamp(0.0, 1.0);
        self
    }

    /// 만료 시간 설정
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.expires_at = Some(Instant::now() + duration);
        self
    }

    /// 무기한 (자동 닫힘 없음)
    pub fn permanent(mut self) -> Self {
        self.expires_at = None;
        self
    }

    /// 액션 설정
    pub fn with_actions(mut self, actions: Vec<SuggestionAction>) -> Self {
        self.actions = actions;
        self
    }

    /// 만료 여부 확인
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Instant::now() > expires_at
        } else {
            false
        }
    }

    /// 표시해야 하는지 확인
    pub fn should_show(&self) -> bool {
        !self.dismissed && !self.is_expired()
    }
}

/// 제안 매니저
#[derive(Debug, Default)]
pub struct SuggestionManager {
    /// 활성 토스트 목록
    pub toasts: Vec<SuggestionToast>,
    /// 최대 동시 표시 개수
    pub max_visible: usize,
    /// "다시 보지 않기"로 무시된 제안 타입 키
    pub never_show: Vec<String>,
}

impl SuggestionManager {
    /// 새 매니저 생성
    pub fn new() -> Self {
        Self {
            toasts: Vec::new(),
            max_visible: 3,
            never_show: Vec::new(),
        }
    }

    /// 토스트 추가
    pub fn show(&mut self, toast: SuggestionToast) {
        // "다시 보지 않기" 체크
        let type_key = self.get_type_key(&toast.suggestion_type);
        if self.never_show.contains(&type_key) {
            log::debug!("[SuggestionManager] Skipped (never show): {}", toast.title);
            return;
        }

        log::info!("[SuggestionManager] Showing suggestion: {}", toast.title);
        self.toasts.push(toast);
    }

    /// 토스트 무시
    pub fn dismiss(&mut self, id: u64) {
        if let Some(toast) = self.toasts.iter_mut().find(|t| t.id == id) {
            toast.dismissed = true;
            log::debug!("[SuggestionManager] Dismissed: {}", toast.title);
        }
    }

    /// "다시 보지 않기" 처리
    pub fn never_show_again(&mut self, id: u64) {
        if let Some(toast) = self.toasts.iter().find(|t| t.id == id) {
            let type_key = self.get_type_key(&toast.suggestion_type);
            if !self.never_show.contains(&type_key) {
                self.never_show.push(type_key);
                log::info!("[SuggestionManager] Never show again: {}", toast.title);
            }
        }
        self.dismiss(id);
    }

    /// 제안 타입 키 생성
    fn get_type_key(&self, suggestion_type: &SuggestionType) -> String {
        match suggestion_type {
            SuggestionType::MissingComponent { component_name, .. } => {
                format!("missing_component:{}", component_name)
            }
            SuggestionType::CodeGeneration { .. } => "code_generation".to_string(),
            SuggestionType::Optimization { .. } => "optimization".to_string(),
            SuggestionType::Tip { .. } => "tip".to_string(),
            SuggestionType::ErrorFix { .. } => "error_fix".to_string(),
        }
    }

    /// 만료된 토스트 정리
    pub fn cleanup(&mut self) {
        self.toasts.retain(|t| t.should_show());
    }

    /// 애니메이션 업데이트
    pub fn update(&mut self, dt: f32) {
        for toast in &mut self.toasts {
            if toast.should_show() && toast.anim_progress < 1.0 {
                toast.anim_progress = (toast.anim_progress + dt * 4.0).min(1.0);
            }
        }

        self.cleanup();
    }

    /// 표시할 토스트 목록 (최대 max_visible개)
    pub fn visible_toasts(&self) -> impl Iterator<Item = &SuggestionToast> {
        self.toasts
            .iter()
            .filter(|t| t.should_show())
            .take(self.max_visible)
    }

    /// 토스트 개수
    pub fn count(&self) -> usize {
        self.toasts.iter().filter(|t| t.should_show()).count()
    }
}

/// 제안 토스트 UI 렌더링
pub fn render_suggestions(
    ui: &mut Ui,
    manager: &mut SuggestionManager,
    viewport_rect: Rect,
) -> Option<PaletteAction> {
    let mut action_result: Option<PaletteAction> = None;
    let mut to_dismiss: Vec<u64> = Vec::new();
    let mut to_never_show: Vec<u64> = Vec::new();

    // 우측 상단에 스택으로 표시
    let visible: Vec<_> = manager.visible_toasts().collect();

    for (i, toast) in visible.iter().enumerate() {
        let y_offset = i as f32 * 110.0;
        let toast_width = 280.0;
        let toast_x = viewport_rect.max.x - toast_width - 10.0;
        let toast_y = viewport_rect.min.y + 10.0 + y_offset;

        // 슬라이드 인 애니메이션
        let slide_offset = (1.0 - toast.anim_progress) * 50.0;
        let toast_pos = Pos2::new(toast_x + slide_offset, toast_y);

        // 투명도 애니메이션
        let alpha = (toast.anim_progress * 255.0) as u8;

        egui::Area::new(egui::Id::new(format!("suggestion_{}", toast.id)))
            .fixed_pos(toast_pos)
            .order(egui::Order::Foreground)
            .show(ui.ctx(), |ui| {
                egui::Frame::new()
                    .fill(Color32::from_rgba_unmultiplied(30, 30, 35, alpha))
                    .stroke(egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(138, 92, 246, alpha)))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 2],
                        blur: 8,
                        spread: 0,
                        color: Color32::from_rgba_unmultiplied(0, 0, 0, alpha / 2),
                    })
                    .corner_radius(8.0)
                    .show(ui, |ui| {
                        ui.set_width(toast_width - 20.0);

                        // 헤더
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("✨")
                                    .size(14.0)
                                    .color(Color32::from_rgba_unmultiplied(138, 92, 246, alpha))
                            );
                            ui.label(
                                egui::RichText::new(&toast.title)
                                    .strong()
                                    .size(13.0)
                                    .color(Color32::from_rgba_unmultiplied(220, 220, 230, alpha))
                            );

                            // 닫기 버튼
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("✕").clicked() {
                                    to_dismiss.push(toast.id);
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // 설명
                        ui.label(
                            egui::RichText::new(&toast.description)
                                .size(12.0)
                                .color(Color32::from_rgba_unmultiplied(180, 180, 190, alpha))
                        );

                        ui.add_space(8.0);

                        // 액션 버튼
                        ui.horizontal(|ui| {
                            for action in &toast.actions {
                                let btn_color = match action.style {
                                    SuggestionButtonStyle::Primary => Color32::from_rgb(80, 120, 200),
                                    SuggestionButtonStyle::Secondary => Color32::from_rgb(60, 60, 70),
                                    SuggestionButtonStyle::Danger => Color32::from_rgb(180, 60, 60),
                                };

                                let btn = egui::Button::new(
                                    egui::RichText::new(&action.label)
                                        .size(11.0)
                                        .color(Color32::WHITE)
                                )
                                .fill(btn_color)
                                .rounding(4.0);

                                if ui.add(btn).clicked() {
                                    match &action.action_type {
                                        SuggestionActionType::Accept(palette_action) => {
                                            action_result = Some(palette_action.clone());
                                            to_dismiss.push(toast.id);
                                        }
                                        SuggestionActionType::Dismiss => {
                                            to_dismiss.push(toast.id);
                                        }
                                        SuggestionActionType::NeverShow => {
                                            to_never_show.push(toast.id);
                                        }
                                        SuggestionActionType::Preview => {
                                            // TODO: 미리보기 구현
                                        }
                                    }
                                }
                            }
                        });
                    });
            });
    }

    // 액션 처리
    for id in to_dismiss {
        manager.dismiss(id);
    }
    for id in to_never_show {
        manager.never_show_again(id);
    }

    action_result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggestion_lifecycle() {
        let mut manager = SuggestionManager::new();

        let toast = SuggestionToast::tip("Test", "This is a test")
            .with_duration(Duration::from_secs(5));

        manager.show(toast);
        assert_eq!(manager.count(), 1);

        let id = manager.toasts[0].id;
        manager.dismiss(id);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_never_show_again() {
        let mut manager = SuggestionManager::new();

        let toast1 = SuggestionToast::tip("Tip 1", "First tip");
        let toast2 = SuggestionToast::tip("Tip 2", "Second tip");

        manager.show(toast1);
        let id = manager.toasts[0].id;
        manager.never_show_again(id);

        // "다시 보지 않기" 후에는 같은 타입의 제안이 표시되지 않음
        manager.show(toast2);
        assert_eq!(manager.count(), 0); // tip 타입은 이제 무시됨
    }

    #[test]
    fn test_expiration() {
        let toast = SuggestionToast::tip("Test", "Expiring")
            .with_duration(Duration::from_millis(1));

        std::thread::sleep(Duration::from_millis(10));

        assert!(toast.is_expired());
        assert!(!toast.should_show());
    }
}
