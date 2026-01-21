//! Command Palette - 통합 명령 팔레트 UI
//!
//! VS Code 스타일의 Command Palette입니다.
//! Ctrl+Shift+P로 열고, 명령 검색 및 실행을 지원합니다.
//! "@ai" 또는 ">" 입력 시 AI 모드로 전환됩니다.

use std::collections::VecDeque;

use egui::{Color32, Key, Pos2, Ui, Vec2};

use super::command_registry::{CommandRegistry, RegisteredCommand};

/// Command Palette 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaletteMode {
    /// 일반 명령 검색 (Ctrl+Shift+P)
    #[default]
    Command,
    /// 파일 빠른 열기 (Ctrl+P)
    QuickOpen,
    /// AI 질문 모드 ("@ai" 또는 ">" 입력)
    AI,
    /// 전체 검색 (Ctrl+Shift+F)
    Search,
}

impl PaletteMode {
    /// 모드별 힌트 텍스트
    pub fn hint_text(&self) -> &'static str {
        match self {
            PaletteMode::Command => "Type a command...",
            PaletteMode::QuickOpen => "Type a file name...",
            PaletteMode::AI => "Ask AI anything...",
            PaletteMode::Search => "Search in project...",
        }
    }

    /// 모드별 아이콘
    pub fn icon(&self) -> &'static str {
        match self {
            PaletteMode::Command => ">",
            PaletteMode::QuickOpen => "📄",
            PaletteMode::AI => "🤖",
            PaletteMode::Search => "🔍",
        }
    }

    /// 모드별 테두리 색상
    pub fn border_color(&self) -> Color32 {
        match self {
            PaletteMode::AI => Color32::from_rgb(138, 92, 246), // 보라색
            _ => Color32::from_rgb(60, 60, 70),
        }
    }

    /// 모드별 배경 색상
    pub fn background_color(&self) -> Color32 {
        match self {
            PaletteMode::AI => Color32::from_rgb(30, 27, 46), // 어두운 보라
            _ => Color32::from_rgb(30, 30, 35),
        }
    }
}

/// Palette 아이템 (검색 결과)
#[derive(Debug, Clone)]
pub struct PaletteItem {
    /// 아이템 ID
    pub id: String,
    /// 표시 라벨
    pub label: String,
    /// 카테고리/경로
    pub category: String,
    /// 단축키 문자열
    pub shortcut: Option<String>,
    /// 아이콘
    pub icon: Option<String>,
    /// 액션 타입
    pub action: PaletteAction,
    /// 검색 점수 (정렬용)
    pub score: f32,
}

impl PaletteItem {
    /// 명령에서 PaletteItem 생성
    pub fn from_command(cmd: &RegisteredCommand, score: f32) -> Self {
        Self {
            id: cmd.id.clone(),
            label: cmd.label.clone(),
            category: cmd.category.display_name().to_string(),
            shortcut: cmd.shortcut.as_ref().map(|s| s.to_display_string()),
            icon: cmd.icon.clone(),
            action: PaletteAction::Command(cmd.id.clone()),
            score,
        }
    }
}

/// Palette 액션
#[derive(Debug, Clone)]
pub enum PaletteAction {
    /// 에디터 명령 실행
    Command(String),
    /// 파일 열기
    OpenFile(String),
    /// 파일 위치로 이동
    NavigateTo { file: String, line: usize },
    /// AI 질의
    AiQuery(String),
    /// 아무것도 안함
    None,
}

/// Command Palette 상태
#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    /// 표시 여부
    pub visible: bool,
    /// 현재 모드
    pub mode: PaletteMode,
    /// 입력 텍스트
    pub input: String,
    /// 필터링된 아이템 목록
    pub filtered_items: Vec<PaletteItem>,
    /// 선택된 인덱스
    pub selected_index: usize,
    /// AI 응답 대기 중
    pub ai_processing: bool,
    /// 입력 히스토리
    pub history: VecDeque<String>,
    /// 히스토리 최대 크기
    pub max_history: usize,
    /// 입력 필드에 포커스 필요
    pub needs_focus: bool,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            visible: false,
            mode: PaletteMode::Command,
            input: String::new(),
            filtered_items: Vec::new(),
            selected_index: 0,
            ai_processing: false,
            history: VecDeque::with_capacity(50),
            max_history: 50,
            needs_focus: false,
        }
    }
}

impl CommandPaletteState {
    /// 새 상태 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 팔레트 열기
    pub fn open(&mut self, mode: PaletteMode) {
        self.visible = true;
        self.mode = mode;
        self.input.clear();
        self.filtered_items.clear();
        self.selected_index = 0;
        self.needs_focus = true;

        log::debug!("[CommandPalette] Opened in {:?} mode", mode);
    }

    /// 팔레트 닫기
    pub fn close(&mut self) {
        self.visible = false;
        self.input.clear();
        self.filtered_items.clear();
        self.selected_index = 0;
        self.ai_processing = false;
        self.needs_focus = false;

        log::debug!("[CommandPalette] Closed");
    }

    /// 토글
    pub fn toggle(&mut self, mode: PaletteMode) {
        if self.visible && self.mode == mode {
            self.close();
        } else {
            self.open(mode);
        }
    }

    /// 입력 업데이트 및 모드 자동 감지
    pub fn update_input(&mut self, new_input: String, registry: &CommandRegistry) {
        self.input = new_input;
        self.detect_mode();
        self.update_filter(registry);
    }

    /// 모드 자동 감지
    fn detect_mode(&mut self) {
        let input = self.input.trim();

        if input.starts_with("@ai ") || input.starts_with(">") {
            if self.mode != PaletteMode::AI {
                self.mode = PaletteMode::AI;
                log::debug!("[CommandPalette] Mode switched to AI");
            }
        } else if self.mode == PaletteMode::AI && !input.starts_with("@ai") && !input.starts_with(">") {
            // AI 프리픽스가 없으면 Command 모드로 복귀
            self.mode = PaletteMode::Command;
            log::debug!("[CommandPalette] Mode switched back to Command");
        }
    }

    /// 필터 업데이트
    pub fn update_filter(&mut self, registry: &CommandRegistry) {
        match self.mode {
            PaletteMode::Command => {
                // 명령 검색
                let results = registry.search(&self.input);
                self.filtered_items = results
                    .into_iter()
                    .map(|cmd| PaletteItem::from_command(cmd, cmd.matches(&self.input)))
                    .collect();
            }
            PaletteMode::AI => {
                // AI 모드에서는 필터링 없음
                self.filtered_items.clear();
            }
            PaletteMode::QuickOpen => {
                // TODO: 파일 검색 구현
                self.filtered_items.clear();
            }
            PaletteMode::Search => {
                // TODO: 전체 검색 구현
                self.filtered_items.clear();
            }
        }

        // 선택 인덱스 범위 제한
        if !self.filtered_items.is_empty() {
            self.selected_index = self.selected_index.min(self.filtered_items.len() - 1);
        } else {
            self.selected_index = 0;
        }
    }

    /// 선택 위로 이동
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    /// 선택 아래로 이동
    pub fn select_next(&mut self) {
        if !self.filtered_items.is_empty() && self.selected_index < self.filtered_items.len() - 1 {
            self.selected_index += 1;
        }
    }

    /// 현재 선택된 아이템 가져오기
    pub fn selected_item(&self) -> Option<&PaletteItem> {
        self.filtered_items.get(self.selected_index)
    }

    /// 현재 선택 실행
    pub fn execute_selected(&mut self) -> Option<PaletteAction> {
        if self.mode == PaletteMode::AI {
            // AI 모드에서는 입력 텍스트를 AI 쿼리로 반환
            let input_clone = self.input.clone();
            let query = input_clone.trim();
            let query = query.strip_prefix("@ai ").or_else(|| query.strip_prefix(">")).unwrap_or(query);

            if !query.is_empty() {
                let query_string = query.to_string();
                self.add_to_history(&input_clone);
                return Some(PaletteAction::AiQuery(query_string));
            }
        } else if let Some(item) = self.selected_item() {
            let action = item.action.clone();
            let input_clone = self.input.clone();
            self.add_to_history(&input_clone);
            return Some(action);
        }

        None
    }

    /// 히스토리에 추가
    fn add_to_history(&mut self, input: &str) {
        if input.is_empty() {
            return;
        }

        // 중복 제거
        self.history.retain(|h| h != input);

        // 앞에 추가
        self.history.push_front(input.to_string());

        // 최대 크기 제한
        while self.history.len() > self.max_history {
            self.history.pop_back();
        }
    }
}

/// Command Palette UI 렌더링
pub fn render_command_palette(
    ctx: &egui::Context,
    state: &mut CommandPaletteState,
    registry: &CommandRegistry,
) -> Option<PaletteAction> {
    if !state.visible {
        return None;
    }

    let mut action_result: Option<PaletteAction> = None;
    let mut should_close = false;

    // 팔레트 크기 및 위치 계산
    let screen_rect = ctx.screen_rect();
    let palette_width = 600.0_f32.min(screen_rect.width() * 0.8);
    let palette_x = (screen_rect.width() - palette_width) / 2.0;
    let palette_y = screen_rect.height() * 0.15; // 상단 15% 위치

    // 스타일
    let border_color = state.mode.border_color();
    let bg_color = state.mode.background_color();
    let border_width = if state.mode == PaletteMode::AI { 2.0 } else { 1.0 };

    // 배경 딤 레이어 (클릭 시 닫기)
    egui::Area::new(egui::Id::new("command_palette_dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .show(ctx, |ui| {
            let response = ui.allocate_response(screen_rect.size(), egui::Sense::click());
            ui.painter().rect_filled(
                screen_rect,
                0.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 100),
            );

            if response.clicked() {
                should_close = true;
            }
        });

    // 팔레트 윈도우
    egui::Area::new(egui::Id::new("command_palette"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(palette_x, palette_y))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(bg_color)
                .stroke(egui::Stroke::new(border_width, border_color))
                .corner_radius(8.0)
                .shadow(egui::epaint::Shadow {
                    offset: [0, 4],
                    blur: 16,
                    spread: 0,
                    color: Color32::from_black_alpha(100),
                })
                .show(ui, |ui| {
                    ui.set_width(palette_width);

                    // 입력 영역
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(egui::RichText::new(state.mode.icon()).size(16.0));
                        ui.add_space(4.0);

                        let input_response = ui.add(
                            egui::TextEdit::singleline(&mut state.input)
                                .hint_text(state.mode.hint_text())
                                .desired_width(palette_width - 60.0)
                                .frame(false)
                                .font(egui::TextStyle::Body)
                        );

                        // 포커스 요청
                        if state.needs_focus {
                            input_response.request_focus();
                            state.needs_focus = false;
                        }

                        // 입력 변경 시 필터 업데이트
                        if input_response.changed() {
                            state.detect_mode();
                            state.update_filter(registry);
                        }

                        // 키보드 이벤트 처리
                        if input_response.has_focus() {
                            let events = ui.input(|i| i.events.clone());
                            for event in events {
                                match event {
                                    egui::Event::Key { key: Key::Escape, pressed: true, .. } => {
                                        should_close = true;
                                    }
                                    egui::Event::Key { key: Key::ArrowUp, pressed: true, .. } => {
                                        state.select_previous();
                                    }
                                    egui::Event::Key { key: Key::ArrowDown, pressed: true, .. } => {
                                        state.select_next();
                                    }
                                    egui::Event::Key { key: Key::Enter, pressed: true, .. } => {
                                        action_result = state.execute_selected();
                                        should_close = true;
                                    }
                                    _ => {}
                                }
                            }
                        }

                        ui.add_space(8.0);
                    });

                    ui.add_space(4.0);

                    // AI 모드 컨텍스트 표시
                    if state.mode == PaletteMode::AI {
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            ui.label(
                                egui::RichText::new("📎 AI Context: Current Selection")
                                    .size(11.0)
                                    .color(Color32::from_rgb(150, 150, 160))
                            );
                        });
                        ui.add_space(4.0);
                    }

                    ui.separator();

                    // 결과 목록
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            if state.mode == PaletteMode::AI {
                                // AI 모드: 안내 메시지
                                ui.vertical_centered(|ui| {
                                    ui.add_space(20.0);
                                    if state.ai_processing {
                                        ui.label(
                                            egui::RichText::new("💭 AI is thinking...")
                                                .color(Color32::from_rgb(138, 92, 246))
                                        );
                                        ui.spinner();
                                    } else {
                                        ui.label(
                                            egui::RichText::new("Type your question and press Enter")
                                                .color(Color32::from_rgb(120, 120, 130))
                                        );
                                    }
                                    ui.add_space(20.0);
                                });
                            } else if state.filtered_items.is_empty() {
                                // 결과 없음
                                ui.vertical_centered(|ui| {
                                    ui.add_space(20.0);
                                    ui.label(
                                        egui::RichText::new("No results found")
                                            .color(Color32::from_rgb(120, 120, 130))
                                    );
                                    ui.add_space(20.0);
                                });
                            } else {
                                // 결과 목록 렌더링
                                // Clone filtered items to avoid borrow issues
                                let items: Vec<_> = state.filtered_items.clone();
                                let mut clicked_index: Option<usize> = None;
                                let mut hovered_index: Option<usize> = None;

                                for (index, item) in items.iter().enumerate() {
                                    let is_selected = index == state.selected_index;

                                    let response = render_palette_item(ui, item, is_selected, palette_width);

                                    if response.clicked() {
                                        clicked_index = Some(index);
                                    }

                                    if response.hovered() {
                                        hovered_index = Some(index);
                                    }
                                }

                                // Handle hover (outside iterator)
                                if let Some(index) = hovered_index {
                                    state.selected_index = index;
                                }

                                // Handle click (outside iterator)
                                if let Some(index) = clicked_index {
                                    state.selected_index = index;
                                    action_result = state.execute_selected();
                                    should_close = true;
                                }
                            }
                        });

                    // 하단 도움말
                    ui.add_space(4.0);
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new("↑↓ Navigate  ⏎ Select  Esc Close")
                                .size(10.0)
                                .color(Color32::from_rgb(100, 100, 110))
                        );
                    });
                    ui.add_space(6.0);
                });
        });

    if should_close {
        state.close();
    }

    action_result
}

/// 팔레트 아이템 렌더링
fn render_palette_item(
    ui: &mut Ui,
    item: &PaletteItem,
    is_selected: bool,
    width: f32,
) -> egui::Response {
    let bg_color = if is_selected {
        Color32::from_rgb(50, 50, 60)
    } else {
        Color32::TRANSPARENT
    };

    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(width, 32.0),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        // 배경
        ui.painter().rect_filled(rect, 0.0, bg_color);

        // 아이콘
        if let Some(ref icon) = item.icon {
            ui.painter().text(
                Pos2::new(rect.left() + 16.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(14.0),
                Color32::from_rgb(180, 180, 190),
            );
        }

        // 라벨
        let label_x = if item.icon.is_some() { 40.0 } else { 16.0 };
        ui.painter().text(
            Pos2::new(rect.left() + label_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            &item.label,
            egui::FontId::proportional(13.0),
            Color32::from_rgb(220, 220, 230),
        );

        // 카테고리
        ui.painter().text(
            Pos2::new(rect.right() - 100.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            &item.category,
            egui::FontId::proportional(11.0),
            Color32::from_rgb(100, 100, 110),
        );

        // 단축키
        if let Some(ref shortcut) = item.shortcut {
            ui.painter().text(
                Pos2::new(rect.right() - 12.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                shortcut,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(80, 80, 90),
            );
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_detection() {
        let mut state = CommandPaletteState::new();
        let registry = CommandRegistry::with_defaults();

        // 일반 입력
        state.update_input("save".to_string(), &registry);
        assert_eq!(state.mode, PaletteMode::Command);

        // AI 모드 전환
        state.update_input("@ai how to".to_string(), &registry);
        assert_eq!(state.mode, PaletteMode::AI);

        // > 프리픽스
        state.update_input("> what is".to_string(), &registry);
        assert_eq!(state.mode, PaletteMode::AI);
    }

    #[test]
    fn test_navigation() {
        let mut state = CommandPaletteState::new();
        let registry = CommandRegistry::with_defaults();

        state.open(PaletteMode::Command);
        state.update_input("".to_string(), &registry);

        assert!(state.filtered_items.len() > 1);
        assert_eq!(state.selected_index, 0);

        state.select_next();
        assert_eq!(state.selected_index, 1);

        state.select_previous();
        assert_eq!(state.selected_index, 0);

        // 경계 테스트
        state.select_previous();
        assert_eq!(state.selected_index, 0); // 0 이하로 가지 않음
    }
}
