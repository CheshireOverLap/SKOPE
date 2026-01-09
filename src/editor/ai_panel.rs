//! AI Panel State and MCP Integration
//!
//! AI 어시스턴트 패널 상태 관리 및 MCP 서버 연결

use egui::{self, Color32, Ui, TextEdit};

/// 채팅 메시지
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: f64,
    pub tool_calls: Vec<ToolCallInfo>,
}

/// 메시지 역할
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// 도구 호출 정보
#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub name: String,
    pub success: bool,
    pub result_preview: Option<String>,
}

/// TODO 항목
#[derive(Debug, Clone)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

/// TODO 상태
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

/// 프로젝트 노트
#[derive(Debug, Clone)]
pub struct Note {
    pub id: String,
    pub title: String,
    pub content: String,
}

/// AI 패널 상태
pub struct AiPanelState {
    // 채팅
    pub messages: Vec<ChatMessage>,
    pub input_text: String,
    pub is_processing: bool,

    // 메모리
    pub notes: Vec<Note>,
    pub new_note_title: String,
    pub new_note_content: String,

    // TODO
    pub todos: Vec<TodoItem>,
    pub new_todo_text: String,

    // 연결 상태
    pub mcp_connected: bool,
    pub connection_error: Option<String>,

    // 아이콘
    pub icon_ai: Option<egui::TextureId>,
}

impl Default for AiPanelState {
    fn default() -> Self {
        Self::new()
    }
}

impl AiPanelState {
    pub fn new() -> Self {
        Self {
            // 채팅
            messages: vec![
                ChatMessage {
                    role: MessageRole::System,
                    content: "SKOPE AI Assistant initialized. Ready to help with your game development!".to_string(),
                    timestamp: 0.0,
                    tool_calls: vec![],
                }
            ],
            input_text: String::new(),
            is_processing: false,

            // 메모리
            notes: vec![],
            new_note_title: String::new(),
            new_note_content: String::new(),

            // TODO
            todos: vec![],
            new_todo_text: String::new(),

            // 연결 상태
            mcp_connected: false,
            connection_error: None,

            // 아이콘
            icon_ai: None,
        }
    }

    /// AI 아이콘 설정
    pub fn set_icon(&mut self, icon_ai: Option<egui::TextureId>) {
        self.icon_ai = icon_ai;
    }

    /// 사용자 메시지 전송
    pub fn send_message(&mut self) {
        if self.input_text.trim().is_empty() || self.is_processing {
            return;
        }

        let content = std::mem::take(&mut self.input_text);

        // 사용자 메시지 추가
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: content.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            tool_calls: vec![],
        });

        // AI 응답 대기 상태
        self.is_processing = true;

        // TODO: 실제 MCP 서버에 요청 전송
        // 지금은 플레이스홀더 응답
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: format!("I received your message: \"{}\"\n\n(MCP server not yet connected)", content),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64(),
            tool_calls: vec![],
        });

        self.is_processing = false;
    }

    /// 노트 추가
    pub fn add_note(&mut self) {
        if self.new_note_title.trim().is_empty() {
            return;
        }

        self.notes.push(Note {
            id: uuid_simple(),
            title: std::mem::take(&mut self.new_note_title),
            content: std::mem::take(&mut self.new_note_content),
        });
    }

    /// 노트 삭제
    pub fn remove_note(&mut self, id: &str) {
        self.notes.retain(|n| n.id != id);
    }

    /// TODO 추가
    pub fn add_todo(&mut self) {
        if self.new_todo_text.trim().is_empty() {
            return;
        }

        self.todos.push(TodoItem {
            id: uuid_simple(),
            content: std::mem::take(&mut self.new_todo_text),
            status: TodoStatus::Pending,
        });
    }

    /// TODO 상태 변경
    pub fn toggle_todo(&mut self, id: &str) {
        if let Some(todo) = self.todos.iter_mut().find(|t| t.id == id) {
            todo.status = match todo.status {
                TodoStatus::Pending => TodoStatus::InProgress,
                TodoStatus::InProgress => TodoStatus::Completed,
                TodoStatus::Completed => TodoStatus::Pending,
            };
        }
    }

    /// TODO 삭제
    pub fn remove_todo(&mut self, id: &str) {
        self.todos.retain(|t| t.id != id);
    }

    /// Chat 탭 UI
    pub fn chat_ui(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            // 연결 상태 표시
            ui.horizontal(|ui| {
                // AI 아이콘
                if let Some(icon) = self.icon_ai {
                    ui.image((icon, egui::vec2(14.0, 14.0)));
                }

                let (color, text) = if self.mcp_connected {
                    (Color32::from_rgb(100, 200, 100), "Connected")
                } else {
                    (Color32::from_rgb(200, 100, 100), "Disconnected")
                };
                ui.label(egui::RichText::new("●").color(color));
                ui.label(egui::RichText::new(text).size(11.0).color(Color32::from_rgb(150, 150, 160)));
            });

            ui.separator();

            // 메시지 영역
            let available_height = ui.available_height() - 60.0; // 입력창 공간 확보

            egui::ScrollArea::vertical()
                .max_height(available_height)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for msg in &self.messages {
                        self.message_ui(ui, msg);
                        ui.add_space(8.0);
                    }

                    if self.is_processing {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("Thinking...").color(Color32::from_rgb(150, 150, 160)));
                        });
                    }
                });

            ui.separator();

            // 입력 영역
            ui.horizontal(|ui| {
                let text_edit = TextEdit::singleline(&mut self.input_text)
                    .hint_text("Ask AI anything...")
                    .desired_width(ui.available_width() - 60.0);

                let response = ui.add(text_edit);

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.send_message();
                }

                if ui.button("Send").clicked() {
                    self.send_message();
                }
            });
        });
    }

    /// 개별 메시지 UI
    fn message_ui(&self, ui: &mut Ui, msg: &ChatMessage) {
        let (bg_color, label_color, label) = match msg.role {
            MessageRole::User => (
                Color32::from_rgb(45, 55, 75),
                Color32::from_rgb(100, 180, 255),
                "You"
            ),
            MessageRole::Assistant => (
                Color32::from_rgb(55, 65, 55),
                Color32::from_rgb(100, 200, 120),
                "AI"
            ),
            MessageRole::System => (
                Color32::from_rgb(55, 55, 65),
                Color32::from_rgb(180, 180, 100),
                "System"
            ),
        };

        egui::Frame::new()
            .fill(bg_color)
            .corner_radius(6.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Assistant 메시지에 AI 아이콘 표시
                    if msg.role == MessageRole::Assistant {
                        if let Some(icon) = self.icon_ai {
                            ui.image((icon, egui::vec2(12.0, 12.0)));
                        }
                    }
                    ui.label(egui::RichText::new(label).color(label_color).strong().size(11.0));
                });
                ui.add_space(4.0);
                ui.label(&msg.content);

                // 도구 호출 표시
                for tool in &msg.tool_calls {
                    ui.horizontal(|ui| {
                        let icon = if tool.success { "✓" } else { "✗" };
                        let color = if tool.success {
                            Color32::from_rgb(100, 200, 100)
                        } else {
                            Color32::from_rgb(200, 100, 100)
                        };
                        ui.label(egui::RichText::new(icon).color(color));
                        ui.label(egui::RichText::new(&tool.name).size(11.0).color(Color32::from_rgb(150, 150, 160)));
                    });
                }
            });
    }

    /// Memory 탭 UI
    pub fn memory_ui(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("Project Notes");
            ui.separator();

            // 기존 노트 표시
            let mut note_to_remove = None;

            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 120.0)
                .show(ui, |ui| {
                    for note in &self.notes {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(45, 45, 55))
                            .corner_radius(4.0)
                            .inner_margin(8.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&note.title).strong());
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.small_button("✕").clicked() {
                                            note_to_remove = Some(note.id.clone());
                                        }
                                    });
                                });
                                if !note.content.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(&note.content);
                                }
                            });
                        ui.add_space(4.0);
                    }

                    if self.notes.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("📝").size(24.0).color(Color32::from_rgb(70, 75, 85)));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("No notes yet").size(12.0).color(Color32::from_rgb(100, 105, 115)));
                            ui.label(egui::RichText::new("Add notes below to remember important things").size(10.0).color(Color32::from_rgb(80, 85, 95)));
                        });
                    }
                });

            if let Some(id) = note_to_remove {
                self.remove_note(&id);
            }

            ui.separator();

            // 새 노트 입력
            ui.label(egui::RichText::new("Add Note:").strong());
            ui.horizontal(|ui| {
                ui.label("Title:");
                ui.text_edit_singleline(&mut self.new_note_title);
            });
            ui.text_edit_multiline(&mut self.new_note_content);
            if ui.button("Add Note").clicked() {
                self.add_note();
            }
        });
    }

    /// TODOs 탭 UI
    pub fn todos_ui(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("TODO List");
            ui.separator();

            // TODO 목록
            let mut todo_to_toggle = None;
            let mut todo_to_remove = None;

            egui::ScrollArea::vertical()
                .max_height(ui.available_height() - 60.0)
                .show(ui, |ui| {
                    // In Progress
                    let in_progress: Vec<_> = self.todos.iter().filter(|t| t.status == TodoStatus::InProgress).collect();
                    if !in_progress.is_empty() {
                        ui.label(egui::RichText::new("In Progress").color(Color32::from_rgb(100, 180, 255)).strong());
                        for todo in in_progress {
                            self.todo_item_ui(ui, todo, &mut todo_to_toggle, &mut todo_to_remove);
                        }
                        ui.add_space(8.0);
                    }

                    // Pending
                    let pending: Vec<_> = self.todos.iter().filter(|t| t.status == TodoStatus::Pending).collect();
                    if !pending.is_empty() {
                        ui.label(egui::RichText::new("Pending").color(Color32::from_rgb(200, 180, 100)).strong());
                        for todo in pending {
                            self.todo_item_ui(ui, todo, &mut todo_to_toggle, &mut todo_to_remove);
                        }
                        ui.add_space(8.0);
                    }

                    // Completed
                    let completed: Vec<_> = self.todos.iter().filter(|t| t.status == TodoStatus::Completed).collect();
                    if !completed.is_empty() {
                        ui.label(egui::RichText::new("Completed").color(Color32::from_rgb(100, 200, 100)).strong());
                        for todo in completed {
                            self.todo_item_ui(ui, todo, &mut todo_to_toggle, &mut todo_to_remove);
                        }
                    }

                    if self.todos.is_empty() {
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("✓").size(24.0).color(Color32::from_rgb(70, 75, 85)));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("No tasks yet").size(12.0).color(Color32::from_rgb(100, 105, 115)));
                            ui.label(egui::RichText::new("Add tasks below to track your progress").size(10.0).color(Color32::from_rgb(80, 85, 95)));
                        });
                    }
                });

            if let Some(id) = todo_to_toggle {
                self.toggle_todo(&id);
            }
            if let Some(id) = todo_to_remove {
                self.remove_todo(&id);
            }

            ui.separator();

            // 새 TODO 입력
            ui.horizontal(|ui| {
                let text_edit = TextEdit::singleline(&mut self.new_todo_text)
                    .hint_text("Add a task...")
                    .desired_width(ui.available_width() - 60.0);
                ui.add(text_edit);
                if ui.button("Add").clicked() {
                    self.add_todo();
                }
            });
        });
    }

    fn todo_item_ui(&self, ui: &mut Ui, todo: &TodoItem, toggle: &mut Option<String>, remove: &mut Option<String>) {
        ui.horizontal(|ui| {
            let checkbox = match todo.status {
                TodoStatus::Pending => "☐",
                TodoStatus::InProgress => "◐",
                TodoStatus::Completed => "☑",
            };

            let text_color = if todo.status == TodoStatus::Completed {
                Color32::from_rgb(100, 100, 110)
            } else {
                Color32::from_rgb(200, 200, 210)
            };

            if ui.button(checkbox).clicked() {
                *toggle = Some(todo.id.clone());
            }
            ui.label(egui::RichText::new(&todo.content).color(text_color));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✕").clicked() {
                    *remove = Some(todo.id.clone());
                }
            });
        });
    }
}

/// 간단한 UUID 생성 (full uuid 크레이트 없이)
fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", time)
}
