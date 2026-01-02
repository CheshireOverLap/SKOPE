//! Scene Menu 패널
//!
//! 씬 저장/로드 및 새 씬 생성 기능

use fyrox_core::pool::Handle;
use fyrox_ui::{
    button::{ButtonBuilder, ButtonMessage},
    message::{MessageDirection, UiMessage},
    stack_panel::StackPanelBuilder,
    text::{TextBuilder, TextMessage},
    text_box::TextBoxBuilder,
    widget::WidgetBuilder,
    window::{WindowBuilder, WindowTitle},
    Orientation, Thickness, UiNode, UserInterface,
};

/// 씬 액션
#[derive(Debug, Clone, PartialEq)]
pub enum SceneAction {
    /// 새 씬
    New,
    /// 씬 저장
    Save(String),
    /// 씬 로드
    Load(String),
}

/// Scene Menu 패널
pub struct SceneMenuPanel {
    /// 윈도우 핸들
    pub window: Handle<UiNode>,
    /// 새 씬 버튼
    new_button: Handle<UiNode>,
    /// 저장 버튼
    save_button: Handle<UiNode>,
    /// 로드 버튼
    load_button: Handle<UiNode>,
    /// 파일 경로 입력
    path_input: Handle<UiNode>,
    /// 현재 경로
    current_path: String,
    /// 대기 중인 액션
    pending_action: Option<SceneAction>,
}

impl SceneMenuPanel {
    /// 새 Scene Menu 패널 생성
    pub fn new(ui: &mut UserInterface) -> Self {
        let ctx = &mut ui.build_ctx();

        // 타이틀
        let title_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(24.0),
        )
        .with_text("Scene File")
        .build(ctx);

        // 파일 경로 입력
        let path_input = TextBoxBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(26.0),
        )
        .with_text("scenes/my_scene.skope")
        .build(ctx);

        // 버튼들
        let new_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(30.0),
        )
        .with_text("New Scene")
        .build(ctx);

        let save_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(30.0),
        )
        .with_text("Save Scene")
        .build(ctx);

        let load_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(30.0),
        )
        .with_text("Load Scene")
        .build(ctx);

        // 상태 텍스트
        let status_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(20.0),
        )
        .with_text("Ready")
        .build(ctx);

        // 버튼 스택
        let button_stack = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(new_button)
                .with_child(save_button)
                .with_child(load_button),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // 메인 컨텐츠
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(title_text)
                .with_child(path_input)
                .with_child(button_stack)
                .with_child(status_text),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 윈도우
        let window = WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(350.0)
                .with_height(180.0)
                .with_desired_position(fyrox_core::algebra::Vector2::new(50.0, 50.0)),
        )
        .with_title(WindowTitle::text("Scene"))
        .with_content(content)
        .build(ctx);

        Self {
            window,
            new_button,
            save_button,
            load_button,
            path_input,
            current_path: "scenes/my_scene.skope".to_string(),
            pending_action: None,
        }
    }

    /// UI 메시지 처리
    pub fn handle_message(&mut self, message: &UiMessage) -> Option<SceneAction> {
        // Text 메시지 (TextBox에서 전달됨)
        if let Some(TextMessage::Text(text)) = message.data::<TextMessage>() {
            if message.destination() == self.path_input
                && message.direction() == MessageDirection::FromWidget
            {
                self.current_path = text.clone();
            }
        }

        // 버튼 클릭
        if let Some(ButtonMessage::Click) = message.data::<ButtonMessage>() {
            if message.destination() == self.new_button {
                log::info!("[SceneMenu] New Scene clicked");
                return Some(SceneAction::New);
            } else if message.destination() == self.save_button {
                log::info!("[SceneMenu] Save Scene clicked: {}", self.current_path);
                return Some(SceneAction::Save(self.current_path.clone()));
            } else if message.destination() == self.load_button {
                log::info!("[SceneMenu] Load Scene clicked: {}", self.current_path);
                return Some(SceneAction::Load(self.current_path.clone()));
            }
        }

        None
    }

    /// 경로 설정
    #[allow(dead_code)]
    pub fn set_path(&mut self, path: &str, ui: &UserInterface) {
        self.current_path = path.to_string();
        ui.send(self.path_input, TextMessage::Text(path.to_string()));
    }
}
