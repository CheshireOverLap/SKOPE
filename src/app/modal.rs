//! Global Modal System - 전역 모달 다이얼로그 시스템
//!
//! 파일 다이얼로그, 확인 창 등 전역 블로킹 UI를 관리합니다.
//! 모달이 열려있는 동안 다른 UI 인터랙션을 차단합니다.

use egui::{Color32, Id, ViewportId};

use super::commands::EditorCommand;

/// 모달 범위 (어디서 블로킹할지)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalScope {
    /// 전체 앱 블로킹 (모든 윈도우)
    Application,
    /// 특정 윈도우만 블로킹
    Window(ViewportId),
}

impl Default for ModalScope {
    fn default() -> Self {
        ModalScope::Application
    }
}

/// 모달 우선순위
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ModalPriority {
    /// 일반 모달 (파일 다이얼로그 등)
    #[default]
    Normal = 0,
    /// 경고 모달
    Warning = 10,
    /// 에러 모달
    Error = 20,
    /// 치명적 에러 (앱 종료 필요)
    Critical = 30,
}

/// 모달 버튼 스타일
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModalButtonStyle {
    #[default]
    Normal,
    Primary,
    Danger,
}

/// 모달 버튼
#[derive(Debug, Clone)]
pub struct ModalButton {
    /// 버튼 라벨
    pub label: String,
    /// 버튼 결과
    pub result: ModalResult,
    /// 버튼 스타일
    pub style: ModalButtonStyle,
    /// 단축키 (Enter, Escape 등)
    pub shortcut: Option<egui::Key>,
}

impl ModalButton {
    /// 새 버튼 생성
    pub fn new(label: impl Into<String>, result: ModalResult) -> Self {
        Self {
            label: label.into(),
            result,
            style: ModalButtonStyle::Normal,
            shortcut: None,
        }
    }

    /// Primary 스타일 버튼 (파란색, 기본 선택)
    pub fn primary(label: impl Into<String>, result: ModalResult) -> Self {
        Self {
            label: label.into(),
            result,
            style: ModalButtonStyle::Primary,
            shortcut: Some(egui::Key::Enter),
        }
    }

    /// Danger 스타일 버튼 (빨간색)
    pub fn danger(label: impl Into<String>, result: ModalResult) -> Self {
        Self {
            label: label.into(),
            result,
            style: ModalButtonStyle::Danger,
            shortcut: None,
        }
    }

    /// Cancel 버튼 (ESC 단축키)
    pub fn cancel() -> Self {
        Self {
            label: "Cancel".into(),
            result: ModalResult::Cancelled,
            style: ModalButtonStyle::Normal,
            shortcut: Some(egui::Key::Escape),
        }
    }

    /// OK 버튼 (Enter 단축키)
    pub fn ok() -> Self {
        Self::primary("OK", ModalResult::Confirmed)
    }
}

/// 모달 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModalResult {
    /// 아직 결과 없음 (모달 열려있음)
    None,
    /// 확인됨
    Confirmed,
    /// 취소됨
    Cancelled,
    /// 커스텀 결과 (버튼 ID)
    Custom(String),
}

impl ModalResult {
    /// 모달이 닫혀야 하는지 확인
    pub fn should_close(&self) -> bool {
        !matches!(self, ModalResult::None)
    }
}

/// 파일 다이얼로그 상태
#[derive(Debug, Clone, Default)]
pub struct FileDialogState {
    /// 현재 경로
    pub current_path: String,
    /// 선택된 파일
    pub selected_file: Option<String>,
    /// 파일 필터 (확장자)
    pub filter: Vec<String>,
    /// 다이얼로그 타입
    pub dialog_type: FileDialogType,
}

/// 파일 다이얼로그 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileDialogType {
    #[default]
    Open,
    Save,
    SelectFolder,
}

/// 모달 콘텐츠 타입
#[derive(Debug, Clone)]
pub enum ModalContent {
    /// 단순 텍스트 메시지
    Text(String),

    /// 확인 다이얼로그
    Confirm {
        message: String,
        on_confirm: Option<EditorCommand>,
    },

    /// 파일 다이얼로그
    FileDialog(FileDialogState),

    /// 입력 다이얼로그
    Input {
        label: String,
        value: String,
        placeholder: String,
    },

    /// 진행률 다이얼로그
    Progress {
        message: String,
        progress: f32, // 0.0 ~ 1.0
        cancellable: bool,
    },
}

/// 모달 상태
#[derive(Debug, Clone)]
pub struct ModalState {
    /// 모달 ID (고유 식별자)
    pub id: Id,
    /// 모달 범위
    pub scope: ModalScope,
    /// 우선순위
    pub priority: ModalPriority,
    /// 제목
    pub title: String,
    /// 콘텐츠
    pub content: ModalContent,
    /// 버튼들
    pub buttons: Vec<ModalButton>,
    /// 딤 배경 색상
    pub dim_color: Color32,
    /// 외부 클릭으로 닫기 허용
    pub close_on_click_outside: bool,
    /// 현재 결과
    pub result: ModalResult,
}

impl ModalState {
    /// 새 모달 생성
    pub fn new(title: impl Into<String>, content: ModalContent) -> Self {
        Self {
            id: Id::new(format!("modal_{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos())),
            scope: ModalScope::Application,
            priority: ModalPriority::Normal,
            title: title.into(),
            content,
            buttons: vec![ModalButton::ok()],
            dim_color: Color32::from_rgba_unmultiplied(0, 0, 0, 153), // 60% 불투명
            close_on_click_outside: false,
            result: ModalResult::None,
        }
    }

    /// 확인 다이얼로그 생성
    pub fn confirm(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: Id::new("confirm_dialog"),
            scope: ModalScope::Application,
            priority: ModalPriority::Normal,
            title: title.into(),
            content: ModalContent::Confirm {
                message: message.into(),
                on_confirm: None,
            },
            buttons: vec![
                ModalButton::primary("Yes", ModalResult::Confirmed),
                ModalButton::cancel(),
            ],
            dim_color: Color32::from_rgba_unmultiplied(0, 0, 0, 153),
            close_on_click_outside: true,
            result: ModalResult::None,
        }
    }

    /// 경고 다이얼로그 생성
    pub fn warning(title: impl Into<String>, message: impl Into<String>) -> Self {
        let mut modal = Self::new(title, ModalContent::Text(message.into()));
        modal.priority = ModalPriority::Warning;
        modal.buttons = vec![ModalButton::ok()];
        modal
    }

    /// 에러 다이얼로그 생성
    pub fn error(title: impl Into<String>, message: impl Into<String>) -> Self {
        let mut modal = Self::new(title, ModalContent::Text(message.into()));
        modal.priority = ModalPriority::Error;
        modal.buttons = vec![ModalButton::ok()];
        modal
    }

    /// 파일 열기 다이얼로그 생성
    pub fn open_file(title: impl Into<String>, initial_path: impl Into<String>, filter: Vec<String>) -> Self {
        Self {
            id: Id::new("open_file_dialog"),
            scope: ModalScope::Application,
            priority: ModalPriority::Normal,
            title: title.into(),
            content: ModalContent::FileDialog(FileDialogState {
                current_path: initial_path.into(),
                selected_file: None,
                filter,
                dialog_type: FileDialogType::Open,
            }),
            buttons: vec![
                ModalButton::primary("Open", ModalResult::Confirmed),
                ModalButton::cancel(),
            ],
            dim_color: Color32::from_rgba_unmultiplied(0, 0, 0, 153),
            close_on_click_outside: true,
            result: ModalResult::None,
        }
    }

    /// 파일 저장 다이얼로그 생성
    pub fn save_file(title: impl Into<String>, initial_path: impl Into<String>, filter: Vec<String>) -> Self {
        let mut modal = Self::open_file(title, initial_path, filter);
        modal.id = Id::new("save_file_dialog");
        if let ModalContent::FileDialog(ref mut state) = modal.content {
            state.dialog_type = FileDialogType::Save;
        }
        modal.buttons = vec![
            ModalButton::primary("Save", ModalResult::Confirmed),
            ModalButton::cancel(),
        ];
        modal
    }

    /// 입력 다이얼로그 생성
    pub fn input(
        title: impl Into<String>,
        label: impl Into<String>,
        placeholder: impl Into<String>,
    ) -> Self {
        Self {
            id: Id::new("input_dialog"),
            scope: ModalScope::Application,
            priority: ModalPriority::Normal,
            title: title.into(),
            content: ModalContent::Input {
                label: label.into(),
                value: String::new(),
                placeholder: placeholder.into(),
            },
            buttons: vec![
                ModalButton::primary("OK", ModalResult::Confirmed),
                ModalButton::cancel(),
            ],
            dim_color: Color32::from_rgba_unmultiplied(0, 0, 0, 153),
            close_on_click_outside: true,
            result: ModalResult::None,
        }
    }

    // === Builder 패턴 ===

    /// 범위 설정
    pub fn with_scope(mut self, scope: ModalScope) -> Self {
        self.scope = scope;
        self
    }

    /// 우선순위 설정
    pub fn with_priority(mut self, priority: ModalPriority) -> Self {
        self.priority = priority;
        self
    }

    /// 버튼 설정
    pub fn with_buttons(mut self, buttons: Vec<ModalButton>) -> Self {
        self.buttons = buttons;
        self
    }

    /// 딤 색상 설정
    pub fn with_dim_color(mut self, color: Color32) -> Self {
        self.dim_color = color;
        self
    }

    /// 외부 클릭 닫기 설정
    pub fn with_close_on_click_outside(mut self, close: bool) -> Self {
        self.close_on_click_outside = close;
        self
    }

    /// 확인 시 실행할 명령 설정
    pub fn with_on_confirm(mut self, command: EditorCommand) -> Self {
        if let ModalContent::Confirm { ref mut on_confirm, .. } = self.content {
            *on_confirm = Some(command);
        }
        self
    }

    // === 상태 접근 ===

    /// 결과 설정
    pub fn set_result(&mut self, result: ModalResult) {
        self.result = result;
    }

    /// 모달이 닫혀야 하는지 확인
    pub fn should_close(&self) -> bool {
        self.result.should_close()
    }

    /// 입력 값 가져오기 (Input 모달용)
    pub fn get_input_value(&self) -> Option<&str> {
        if let ModalContent::Input { ref value, .. } = self.content {
            Some(value)
        } else {
            None
        }
    }

    /// 입력 값 설정 (Input 모달용)
    pub fn set_input_value(&mut self, new_value: String) {
        if let ModalContent::Input { ref mut value, .. } = self.content {
            *value = new_value;
        }
    }

    /// 선택된 파일 가져오기 (FileDialog 모달용)
    pub fn get_selected_file(&self) -> Option<&str> {
        if let ModalContent::FileDialog(ref state) = self.content {
            state.selected_file.as_deref()
        } else {
            None
        }
    }

    /// 진행률 설정 (Progress 모달용)
    pub fn set_progress(&mut self, progress: f32) {
        if let ModalContent::Progress { progress: ref mut p, .. } = self.content {
            *p = progress.clamp(0.0, 1.0);
        }
    }
}

/// 모달 매니저 (여러 모달 관리)
#[derive(Debug, Default)]
pub struct ModalManager {
    /// 활성 모달 스택 (우선순위 순)
    modals: Vec<ModalState>,
}

impl ModalManager {
    /// 새 ModalManager 생성
    pub fn new() -> Self {
        Self { modals: Vec::new() }
    }

    /// 모달 표시
    pub fn show(&mut self, modal: ModalState) {
        // 우선순위에 따라 삽입
        let insert_pos = self.modals
            .iter()
            .position(|m| m.priority < modal.priority)
            .unwrap_or(self.modals.len());

        log::info!("[ModalManager] Showing modal: {} (priority: {:?})", modal.title, modal.priority);
        self.modals.insert(insert_pos, modal);
    }

    /// 최상위 모달 가져오기
    pub fn top(&self) -> Option<&ModalState> {
        self.modals.first()
    }

    /// 최상위 모달 가져오기 (가변)
    pub fn top_mut(&mut self) -> Option<&mut ModalState> {
        self.modals.first_mut()
    }

    /// 모달이 열려있는지 확인
    pub fn has_modal(&self) -> bool {
        !self.modals.is_empty()
    }

    /// Application 범위 모달이 있는지 확인
    pub fn has_blocking_modal(&self) -> bool {
        self.modals.iter().any(|m| m.scope == ModalScope::Application)
    }

    /// 닫힌 모달 정리 및 결과 반환
    pub fn process_closed(&mut self) -> Vec<(ModalState, ModalResult)> {
        let mut closed = Vec::new();

        self.modals.retain(|modal| {
            if modal.should_close() {
                closed.push((modal.clone(), modal.result.clone()));
                false
            } else {
                true
            }
        });

        for (modal, result) in &closed {
            log::info!("[ModalManager] Modal closed: {} (result: {:?})", modal.title, result);
        }

        closed
    }

    /// 모든 모달 닫기
    pub fn close_all(&mut self) {
        for modal in &mut self.modals {
            modal.set_result(ModalResult::Cancelled);
        }
    }

    /// 모달 개수
    pub fn count(&self) -> usize {
        self.modals.len()
    }
}

// ============================================================================
// Modal Rendering
// ============================================================================

/// 모달 렌더링
///
/// egui Context를 사용하여 모달 딤 레이어와 윈도우를 렌더링합니다.
/// dock_layout.show() 이후에 호출되어야 합니다.
pub fn render_modal(
    ctx: &egui::Context,
    manager: &mut ModalManager,
) -> Option<(ModalResult, Option<EditorCommand>)> {
    // 최상위 모달 가져오기
    let modal = match manager.top_mut() {
        Some(m) => m,
        None => return None,
    };

    let mut result: Option<(ModalResult, Option<EditorCommand>)> = None;

    // 딤 레이어 (Order::Foreground보다 먼저)
    egui::Area::new(egui::Id::new("modal_dim_layer"))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::LEFT_TOP, [0.0, 0.0])
        .show(ctx, |ui| {
            let screen_rect = ctx.screen_rect();
            ui.painter().rect_filled(screen_rect, 0.0, modal.dim_color);

            // 외부 클릭 감지 (모달 창 밖 클릭)
            if modal.close_on_click_outside {
                let response = ui.allocate_rect(screen_rect, egui::Sense::click());
                if response.clicked() {
                    // 클릭 위치가 모달 창 밖인지 확인
                    // (실제 검사는 모달 창 렌더링 후 수행)
                }
            }
        });

    // 모달 윈도우
    let title = modal.title.clone();
    let buttons = modal.buttons.clone();
    let modal_id = modal.id;

    egui::Window::new(&title)
        .id(modal_id)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .collapsible(false)
        .resizable(false)
        .title_bar(true)
        .frame(egui::Frame::window(&ctx.style()).fill(Color32::from_rgb(35, 35, 40)))
        .show(ctx, |ui| {
            ui.set_min_width(300.0);

            // 콘텐츠 렌더링
            match &mut manager.top_mut().unwrap().content {
                ModalContent::Text(text) => {
                    ui.add_space(8.0);
                    ui.label(text.as_str());
                    ui.add_space(8.0);
                }
                ModalContent::Confirm { message, on_confirm } => {
                    ui.add_space(8.0);
                    ui.label(message.as_str());
                    ui.add_space(8.0);

                    // on_confirm 명령 클론 (버튼 클릭 시 사용)
                    if let Some(cmd) = on_confirm.as_ref() {
                        // 결과에 포함시킬 수 있도록 저장
                    }
                }
                ModalContent::FileDialog(state) => {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Path:");
                        ui.text_edit_singleline(&mut state.current_path);
                    });
                    // TODO: 파일 목록 표시
                    ui.add_space(8.0);
                }
                ModalContent::Input { label, value, placeholder } => {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(label.as_str());
                        let response = ui.add(
                            egui::TextEdit::singleline(value)
                                .hint_text(placeholder.as_str())
                                .desired_width(200.0)
                        );
                        // 처음 표시 시 포커스
                        if response.gained_focus() || value.is_empty() {
                            response.request_focus();
                        }
                    });
                    ui.add_space(8.0);
                }
                ModalContent::Progress { message, progress, cancellable } => {
                    ui.add_space(8.0);
                    ui.label(message.as_str());
                    ui.add(egui::ProgressBar::new(*progress).show_percentage());
                    ui.add_space(8.0);
                }
            }

            ui.add_space(4.0);
            ui.separator();
            ui.add_space(4.0);

            // 버튼 렌더링
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    for btn in buttons.iter().rev() {
                        let btn_color = match btn.style {
                            ModalButtonStyle::Primary => Color32::from_rgb(70, 130, 200),
                            ModalButtonStyle::Normal => Color32::from_rgb(55, 55, 65),
                            ModalButtonStyle::Danger => Color32::from_rgb(180, 60, 60),
                        };

                        let button = egui::Button::new(
                            egui::RichText::new(&btn.label)
                                .color(Color32::WHITE)
                        )
                        .fill(btn_color)
                        .rounding(4.0);

                        if ui.add(button).clicked() {
                            result = Some((btn.result.clone(), None));

                            // Confirm 액션에서 on_confirm 명령 추출
                            if let ModalContent::Confirm { on_confirm, .. } = &manager.top().unwrap().content {
                                if btn.result == ModalResult::Confirmed {
                                    if let Some(cmd) = on_confirm {
                                        result = Some((btn.result.clone(), Some(cmd.clone())));
                                    }
                                }
                            }
                        }

                        // 단축키 처리
                        if let Some(key) = btn.shortcut {
                            if ctx.input(|i| i.key_pressed(key)) {
                                result = Some((btn.result.clone(), None));
                            }
                        }
                    }
                });
            });
        });

    // 결과가 있으면 모달에 설정
    if let Some((ref res, _)) = result {
        if let Some(modal) = manager.top_mut() {
            modal.set_result(res.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_dialog() {
        let modal = ModalState::confirm("Delete?", "Are you sure?");
        assert_eq!(modal.buttons.len(), 2);
        assert!(matches!(modal.content, ModalContent::Confirm { .. }));
    }

    #[test]
    fn test_modal_manager_priority() {
        let mut manager = ModalManager::new();

        manager.show(ModalState::new("Normal", ModalContent::Text("test".into())));
        manager.show(ModalState::error("Error", "error message"));

        // Error가 더 높은 우선순위이므로 top
        assert_eq!(manager.top().unwrap().priority, ModalPriority::Error);
    }

    #[test]
    fn test_modal_close() {
        let mut manager = ModalManager::new();
        manager.show(ModalState::confirm("Test", "test"));

        manager.top_mut().unwrap().set_result(ModalResult::Confirmed);

        let closed = manager.process_closed();
        assert_eq!(closed.len(), 1);
        assert!(!manager.has_modal());
    }
}
