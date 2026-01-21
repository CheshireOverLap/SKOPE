//! Inspector Local AI Chat
//!
//! Inspector 하단에 표시되는 미니 AI 채팅 패널입니다.
//! 현재 선택된 엔티티에 대한 컨텍스트를 가지고 AI와 대화합니다.

use bevy_ecs::entity::Entity;
use egui::{Color32, Ui};

/// Inspector Local Chat 상태
#[derive(Debug, Clone, Default)]
pub struct InspectorLocalChat {
    /// 입력 텍스트
    pub input: String,
    /// 컨텍스트 엔티티 (현재 선택된 엔티티)
    pub context_entity: Option<Entity>,
    /// 채팅 히스토리 (간략화)
    pub history: Vec<ChatMessage>,
    /// 최대 히스토리 길이
    pub max_history: usize,
    /// AI 처리 중 여부
    pub is_processing: bool,
    /// 패널 확장 여부
    pub is_expanded: bool,
}

/// 채팅 메시지
#[derive(Debug, Clone)]
pub struct ChatMessage {
    /// 메시지 타입 (사용자 또는 AI)
    pub role: ChatRole,
    /// 메시지 내용
    pub content: String,
}

/// 메시지 역할
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}

impl InspectorLocalChat {
    /// 새 InspectorLocalChat 생성
    pub fn new() -> Self {
        Self {
            input: String::new(),
            context_entity: None,
            history: Vec::new(),
            max_history: 10,
            is_processing: false,
            is_expanded: false,
        }
    }

    /// 컨텍스트 엔티티 설정
    pub fn set_context_entity(&mut self, entity: Option<Entity>) {
        // 엔티티가 변경되면 히스토리 클리어
        if self.context_entity != entity {
            self.history.clear();
        }
        self.context_entity = entity;
    }

    /// 메시지 추가
    pub fn add_message(&mut self, role: ChatRole, content: String) {
        self.history.push(ChatMessage { role, content });

        // 최대 길이 초과 시 오래된 메시지 제거
        while self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// UI 렌더링
    pub fn ui(&mut self, ui: &mut Ui) -> Option<LocalChatAction> {
        let mut action = None;

        ui.add_space(4.0);

        // 헤더 (접기/펼치기)
        let header_response = ui.horizontal(|ui| {
            let icon = if self.is_expanded { "▼" } else { "▶" };
            ui.label(
                egui::RichText::new(format!("{} AI Chat", icon))
                    .size(11.0)
                    .color(Color32::from_rgb(138, 92, 246))
            );

            // 프로세싱 인디케이터
            if self.is_processing {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("...")
                            .size(10.0)
                            .color(Color32::from_rgb(100, 100, 110))
                    );
                });
            }
        });

        if header_response.response.interact(egui::Sense::click()).clicked() {
            self.is_expanded = !self.is_expanded;
        }

        if !self.is_expanded {
            return action;
        }

        ui.add_space(4.0);

        // 채팅 히스토리 (컴팩트)
        if !self.history.is_empty() {
            egui::Frame::new()
                .fill(Color32::from_rgb(25, 25, 30))
                .corner_radius(4.0)
                .inner_margin(4.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    egui::ScrollArea::vertical()
                        .max_height(100.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for msg in &self.history {
                                let (color, prefix) = match msg.role {
                                    ChatRole::User => (Color32::from_rgb(180, 180, 200), "You: "),
                                    ChatRole::Assistant => (Color32::from_rgb(138, 92, 246), "AI: "),
                                };

                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new(prefix)
                                            .size(10.0)
                                            .color(color)
                                            .strong()
                                    );
                                    ui.label(
                                        egui::RichText::new(&msg.content)
                                            .size(10.0)
                                            .color(Color32::from_rgb(160, 160, 170))
                                    );
                                });
                            }
                        });
                });

            ui.add_space(4.0);
        }

        // 입력 영역
        ui.horizontal(|ui| {
            let input_response = ui.add(
                egui::TextEdit::singleline(&mut self.input)
                    .hint_text("Ask about this entity...")
                    .desired_width(ui.available_width() - 50.0)
                    .font(egui::TextStyle::Small)
            );

            // Enter 키로 전송
            if input_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                if !self.input.trim().is_empty() {
                    let query = self.input.trim().to_string();
                    self.add_message(ChatRole::User, query.clone());
                    self.input.clear();
                    action = Some(LocalChatAction::SendQuery(query));
                }
            }

            // 전송 버튼
            let send_btn = egui::Button::new(
                egui::RichText::new("→")
                    .size(12.0)
                    .color(Color32::WHITE)
            )
            .fill(Color32::from_rgb(80, 60, 140))
            .corner_radius(4.0);

            if ui.add(send_btn).clicked() && !self.input.trim().is_empty() {
                let query = self.input.trim().to_string();
                self.add_message(ChatRole::User, query.clone());
                self.input.clear();
                action = Some(LocalChatAction::SendQuery(query));
            }
        });

        // 퀵 액션 버튼
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            let quick_actions = [
                ("Explain", "Explain this entity"),
                ("Optimize", "How can I optimize this?"),
                ("Add Component", "What components should I add?"),
            ];

            for (label, query) in quick_actions {
                let btn = egui::Button::new(
                    egui::RichText::new(label)
                        .size(9.0)
                        .color(Color32::from_rgb(140, 140, 150))
                )
                .fill(Color32::from_rgb(35, 35, 40))
                .corner_radius(3.0);

                if ui.add(btn).clicked() {
                    self.add_message(ChatRole::User, query.to_string());
                    action = Some(LocalChatAction::SendQuery(query.to_string()));
                }
            }
        });

        action
    }

    /// AI 응답 받기
    pub fn receive_response(&mut self, response: String) {
        self.is_processing = false;
        self.add_message(ChatRole::Assistant, response);
    }

    /// 처리 시작
    pub fn start_processing(&mut self) {
        self.is_processing = true;
    }
}

/// Local Chat 액션
#[derive(Debug, Clone)]
pub enum LocalChatAction {
    /// AI 쿼리 전송 (엔티티 컨텍스트 포함)
    SendQuery(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_history() {
        let mut chat = InspectorLocalChat::new();

        chat.add_message(ChatRole::User, "Hello".to_string());
        chat.add_message(ChatRole::Assistant, "Hi there!".to_string());

        assert_eq!(chat.history.len(), 2);
        assert_eq!(chat.history[0].role, ChatRole::User);
        assert_eq!(chat.history[1].role, ChatRole::Assistant);
    }

    #[test]
    fn test_context_entity_change() {
        let mut chat = InspectorLocalChat::new();
        let entity = Entity::from_raw(1);

        chat.add_message(ChatRole::User, "Test".to_string());
        assert_eq!(chat.history.len(), 1);

        // 엔티티 변경 시 히스토리 클리어
        chat.set_context_entity(Some(entity));
        assert_eq!(chat.history.len(), 0);
    }
}
