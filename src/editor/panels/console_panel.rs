//! Console Panel
//!
//! 스크립트 에러/경고 표시 및 Lua 코드 실행 UI 패널
//! - 에러/경고/정보 로그 표시
//! - Lua 코드 입력 및 실행
//! - 로그 필터링 (severity 별)

use fyrox_core::pool::Handle;
use fyrox_ui::{
    button::{ButtonBuilder, ButtonMessage},
    check_box::{CheckBoxBuilder, CheckBoxMessage},
    list_view::ListViewBuilder,
    message::{MessageDirection, UiMessage},
    scroll_viewer::ScrollViewerBuilder,
    stack_panel::StackPanelBuilder,
    text::{TextBuilder, TextMessage},
    text_box::TextBoxBuilder,
    widget::WidgetBuilder,
    window::{WindowBuilder, WindowTitle},
    Orientation, Thickness, UiNode, UserInterface,
};
use std::collections::VecDeque;

use crate::scripting::{ErrorSeverity, LuaErrorInfo};

/// 최대 로그 항목 수
const MAX_LOG_ENTRIES: usize = 500;

/// 로그 항목
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub severity: LogSeverity,
    pub message: String,
    pub timestamp: f64,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// 로그 심각도
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSeverity {
    Info,
    Warning,
    Error,
}

impl From<ErrorSeverity> for LogSeverity {
    fn from(severity: ErrorSeverity) -> Self {
        match severity {
            ErrorSeverity::Warning => LogSeverity::Warning,
            ErrorSeverity::Error | ErrorSeverity::Critical => LogSeverity::Error,
        }
    }
}

/// Console Panel 액션
#[derive(Debug, Clone)]
pub enum ConsoleAction {
    /// Lua 코드 실행
    Execute(String),
    /// 로그 클리어
    Clear,
}

/// Console Panel
pub struct ConsolePanel {
    /// 윈도우 핸들
    pub window: Handle<UiNode>,
    /// 코드 입력 필드
    code_input: Handle<UiNode>,
    /// 실행 버튼
    execute_button: Handle<UiNode>,
    /// 클리어 버튼
    clear_button: Handle<UiNode>,
    /// 로그 리스트
    log_list: Handle<UiNode>,
    /// Info 필터 체크박스
    filter_info: Handle<UiNode>,
    /// Warning 필터 체크박스
    filter_warning: Handle<UiNode>,
    /// Error 필터 체크박스
    filter_error: Handle<UiNode>,
    /// 로그 항목들
    log_entries: VecDeque<LogEntry>,
    /// 필터 상태
    show_info: bool,
    show_warning: bool,
    show_error: bool,
    /// 현재 입력 코드
    current_code: String,
}

impl ConsolePanel {
    /// 새 Console Panel 생성
    pub fn new(ui: &mut UserInterface) -> Self {
        let ctx = &mut ui.build_ctx();

        // 필터 체크박스들
        let filter_info = CheckBoxBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0),
        )
        .checked(Some(true))
        .build(ctx);

        let info_label = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_height(20.0),
        )
        .with_text("Info")
        .build(ctx);

        let filter_warning = CheckBoxBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0),
        )
        .checked(Some(true))
        .build(ctx);

        let warning_label = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_height(20.0),
        )
        .with_text("Warn")
        .build(ctx);

        let filter_error = CheckBoxBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0),
        )
        .checked(Some(true))
        .build(ctx);

        let error_label = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_height(20.0),
        )
        .with_text("Error")
        .build(ctx);

        // 필터 바
        let filter_bar = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_height(26.0)
                .with_child(filter_info)
                .with_child(info_label)
                .with_child(filter_warning)
                .with_child(warning_label)
                .with_child(filter_error)
                .with_child(error_label),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // 로그 리스트
        let log_list = ListViewBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0)),
        )
        .build(ctx);

        // 스크롤 뷰어로 감싸기
        let scroll_viewer = ScrollViewerBuilder::new(
            WidgetBuilder::new()
                .with_height(200.0)
                .with_child(log_list),
        )
        .build(ctx);

        // 코드 입력
        let code_input = TextBoxBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_height(26.0),
        )
        .with_text("")
        .build(ctx);

        // 실행 버튼
        let execute_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0)
                .with_height(26.0),
        )
        .with_text("Run")
        .build(ctx);

        // 클리어 버튼
        let clear_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0)
                .with_height(26.0),
        )
        .with_text("Clear")
        .build(ctx);

        // 입력 바
        let input_bar = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_height(30.0)
                .with_child(code_input)
                .with_child(execute_button)
                .with_child(clear_button),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // 메인 컨텐츠
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(filter_bar)
                .with_child(scroll_viewer)
                .with_child(input_bar),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 윈도우
        let window = WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(500.0)
                .with_height(300.0)
                .with_desired_position(fyrox_core::algebra::Vector2::new(50.0, 450.0)),
        )
        .with_title(WindowTitle::text("Console"))
        .with_content(content)
        .build(ctx);

        Self {
            window,
            code_input,
            execute_button,
            clear_button,
            log_list,
            filter_info,
            filter_warning,
            filter_error,
            log_entries: VecDeque::with_capacity(MAX_LOG_ENTRIES),
            show_info: true,
            show_warning: true,
            show_error: true,
            current_code: String::new(),
        }
    }

    /// UI 메시지 처리
    pub fn handle_message(&mut self, message: &UiMessage) -> Option<ConsoleAction> {
        // 텍스트 입력
        if let Some(TextMessage::Text(text)) = message.data::<TextMessage>() {
            if message.destination() == self.code_input
                && message.direction() == MessageDirection::FromWidget
            {
                self.current_code = text.clone();
            }
        }

        // 체크박스 변경
        if let Some(CheckBoxMessage::Check(Some(checked))) = message.data::<CheckBoxMessage>() {
            if message.direction() == MessageDirection::FromWidget {
                if message.destination() == self.filter_info {
                    self.show_info = *checked;
                } else if message.destination() == self.filter_warning {
                    self.show_warning = *checked;
                } else if message.destination() == self.filter_error {
                    self.show_error = *checked;
                }
            }
        }

        // 버튼 클릭
        if let Some(ButtonMessage::Click) = message.data::<ButtonMessage>() {
            if message.destination() == self.execute_button {
                if !self.current_code.is_empty() {
                    let code = self.current_code.clone();
                    log::info!("[Console] Executing: {}", code);
                    return Some(ConsoleAction::Execute(code));
                }
            } else if message.destination() == self.clear_button {
                log::info!("[Console] Clear clicked");
                return Some(ConsoleAction::Clear);
            }
        }

        None
    }

    /// 로그 추가
    pub fn add_log(&mut self, severity: LogSeverity, message: String, timestamp: f64) {
        let entry = LogEntry {
            severity,
            message,
            timestamp,
            file: None,
            line: None,
        };

        self.log_entries.push_back(entry);

        // 최대 개수 유지
        while self.log_entries.len() > MAX_LOG_ENTRIES {
            self.log_entries.pop_front();
        }
    }

    /// 에러 정보 추가
    pub fn add_error(&mut self, error: &LuaErrorInfo) {
        let entry = LogEntry {
            severity: error.severity.into(),
            message: error.message.clone(),
            timestamp: error.timestamp,
            file: Some(error.file.clone()),
            line: error.line,
        };

        self.log_entries.push_back(entry);

        while self.log_entries.len() > MAX_LOG_ENTRIES {
            self.log_entries.pop_front();
        }
    }

    /// 실행 결과 추가
    pub fn add_result(&mut self, result: &str, timestamp: f64) {
        if !result.is_empty() {
            self.add_log(LogSeverity::Info, format!("> {}", result), timestamp);
        }
    }

    /// 로그 클리어
    pub fn clear(&mut self) {
        self.log_entries.clear();
    }

    /// 필터링된 로그 가져오기
    pub fn filtered_entries(&self) -> Vec<&LogEntry> {
        self.log_entries
            .iter()
            .filter(|e| match e.severity {
                LogSeverity::Info => self.show_info,
                LogSeverity::Warning => self.show_warning,
                LogSeverity::Error => self.show_error,
            })
            .collect()
    }

    /// 에러 개수
    pub fn error_count(&self) -> usize {
        self.log_entries
            .iter()
            .filter(|e| e.severity == LogSeverity::Error)
            .count()
    }

    /// 경고 개수
    pub fn warning_count(&self) -> usize {
        self.log_entries
            .iter()
            .filter(|e| e.severity == LogSeverity::Warning)
            .count()
    }

    /// 로그 포맷팅 (리스트뷰용)
    pub fn format_entry(entry: &LogEntry) -> String {
        let prefix = match entry.severity {
            LogSeverity::Info => "[I]",
            LogSeverity::Warning => "[W]",
            LogSeverity::Error => "[E]",
        };

        let location = match (&entry.file, entry.line) {
            (Some(f), Some(l)) => format!(" ({}:{})", f, l),
            (Some(f), None) => format!(" ({})", f),
            _ => String::new(),
        };

        format!("{} {}{}", prefix, entry.message, location)
    }
}
