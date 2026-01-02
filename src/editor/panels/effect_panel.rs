//! Effect Panel
//!
//! 이펙트 생성/제어 UI 패널
//! - 이펙트 선택 및 스폰
//! - 재생 제어 (Speed, Pause/Resume)
//! - 활성 이펙트 목록

use fyrox_core::pool::Handle;
use fyrox_ui::{
    button::{ButtonBuilder, ButtonMessage},
    message::{MessageDirection, UiMessage},
    numeric::{NumericUpDownBuilder, NumericUpDownMessage},
    stack_panel::StackPanelBuilder,
    text::{TextBuilder, TextMessage},
    text_box::TextBoxBuilder,
    widget::WidgetBuilder,
    window::{WindowBuilder, WindowTitle},
    Orientation, Thickness, UiNode, UserInterface,
};

/// Effect Panel 액션
#[derive(Debug, Clone)]
pub enum EffectAction {
    /// 이펙트 스폰
    Spawn {
        name: String,
        position: [f32; 3],
        speed: f32,
        scale: f32,
    },
    /// 이펙트 중지
    Stop(u64),
    /// 모든 이펙트 중지
    StopAll,
    /// 속도 변경
    SetSpeed(u64, f32),
}

/// Effect Panel
pub struct EffectPanel {
    /// 윈도우 핸들
    pub window: Handle<UiNode>,
    /// 이펙트 이름 입력
    effect_name_input: Handle<UiNode>,
    /// Position X 입력
    pos_x_input: Handle<UiNode>,
    /// Position Y 입력
    pos_y_input: Handle<UiNode>,
    /// Position Z 입력
    pos_z_input: Handle<UiNode>,
    /// Speed 슬라이더
    speed_input: Handle<UiNode>,
    /// Scale 슬라이더
    scale_input: Handle<UiNode>,
    /// Spawn 버튼
    spawn_button: Handle<UiNode>,
    /// Stop All 버튼
    stop_all_button: Handle<UiNode>,
    /// 상태 텍스트
    status_text: Handle<UiNode>,
    /// 현재 설정값
    current_name: String,
    current_pos: [f32; 3],
    current_speed: f32,
    current_scale: f32,
    /// 활성 이펙트 수
    active_count: usize,
}

impl EffectPanel {
    /// 새 Effect Panel 생성
    pub fn new(ui: &mut UserInterface) -> Self {
        let ctx = &mut ui.build_ctx();

        // 이펙트 이름 라벨
        let name_label = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(20.0),
        )
        .with_text("Effect Name:")
        .build(ctx);

        // 이펙트 이름 입력
        let effect_name_input = TextBoxBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(26.0),
        )
        .with_text("fireball")
        .build(ctx);

        // Position 라벨
        let pos_label = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(20.0),
        )
        .with_text("Position (X, Y, Z):")
        .build(ctx);

        // Position X 입력
        let pos_x_input = NumericUpDownBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0)
                .with_height(26.0),
        )
        .with_value(0.0f32)
        .with_step(0.1)
        .build(ctx);

        // Position Y 입력
        let pos_y_input = NumericUpDownBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0)
                .with_height(26.0),
        )
        .with_value(1.0f32)
        .with_step(0.1)
        .build(ctx);

        // Position Z 입력
        let pos_z_input = NumericUpDownBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(2.0))
                .with_width(60.0)
                .with_height(26.0),
        )
        .with_value(0.0f32)
        .with_step(0.1)
        .build(ctx);

        // Position 스택
        let pos_stack = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(pos_x_input)
                .with_child(pos_y_input)
                .with_child(pos_z_input),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // Speed 라벨
        let speed_label = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(20.0),
        )
        .with_text("Speed:")
        .build(ctx);

        // Speed 입력
        let speed_input = NumericUpDownBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(26.0),
        )
        .with_value(1.0f32)
        .with_min_value(0.1)
        .with_max_value(10.0)
        .with_step(0.1)
        .build(ctx);

        // Scale 라벨
        let scale_label = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(20.0),
        )
        .with_text("Scale:")
        .build(ctx);

        // Scale 입력
        let scale_input = NumericUpDownBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(26.0),
        )
        .with_value(1.0f32)
        .with_min_value(0.1)
        .with_max_value(10.0)
        .with_step(0.1)
        .build(ctx);

        // Spawn 버튼
        let spawn_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(30.0),
        )
        .with_text("Spawn Effect")
        .build(ctx);

        // Stop All 버튼
        let stop_all_button = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(30.0),
        )
        .with_text("Stop All")
        .build(ctx);

        // 버튼 스택
        let button_stack = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(spawn_button)
                .with_child(stop_all_button),
        )
        .with_orientation(Orientation::Horizontal)
        .build(ctx);

        // 상태 텍스트
        let status_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(20.0),
        )
        .with_text("Active Effects: 0")
        .build(ctx);

        // 메인 컨텐츠
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(name_label)
                .with_child(effect_name_input)
                .with_child(pos_label)
                .with_child(pos_stack)
                .with_child(speed_label)
                .with_child(speed_input)
                .with_child(scale_label)
                .with_child(scale_input)
                .with_child(button_stack)
                .with_child(status_text),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 윈도우
        let window = WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(280.0)
                .with_height(380.0)
                .with_desired_position(fyrox_core::algebra::Vector2::new(900.0, 50.0)),
        )
        .with_title(WindowTitle::text("Effects"))
        .with_content(content)
        .build(ctx);

        Self {
            window,
            effect_name_input,
            pos_x_input,
            pos_y_input,
            pos_z_input,
            speed_input,
            scale_input,
            spawn_button,
            stop_all_button,
            status_text,
            current_name: "fireball".to_string(),
            current_pos: [0.0, 1.0, 0.0],
            current_speed: 1.0,
            current_scale: 1.0,
            active_count: 0,
        }
    }

    /// UI 메시지 처리
    pub fn handle_message(&mut self, message: &UiMessage) -> Option<EffectAction> {
        // 이펙트 이름 입력
        if let Some(TextMessage::Text(text)) = message.data::<TextMessage>() {
            if message.destination() == self.effect_name_input
                && message.direction() == MessageDirection::FromWidget
            {
                self.current_name = text.clone();
            }
        }

        // Numeric 입력
        if let Some(NumericUpDownMessage::Value(value)) = message.data::<NumericUpDownMessage<f32>>() {
            if message.direction() == MessageDirection::FromWidget {
                if message.destination() == self.pos_x_input {
                    self.current_pos[0] = *value;
                } else if message.destination() == self.pos_y_input {
                    self.current_pos[1] = *value;
                } else if message.destination() == self.pos_z_input {
                    self.current_pos[2] = *value;
                } else if message.destination() == self.speed_input {
                    self.current_speed = *value;
                } else if message.destination() == self.scale_input {
                    self.current_scale = *value;
                }
            }
        }

        // 버튼 클릭
        if let Some(ButtonMessage::Click) = message.data::<ButtonMessage>() {
            if message.destination() == self.spawn_button {
                log::info!(
                    "[EffectPanel] Spawn: {} at ({}, {}, {}), speed={}, scale={}",
                    self.current_name,
                    self.current_pos[0],
                    self.current_pos[1],
                    self.current_pos[2],
                    self.current_speed,
                    self.current_scale
                );
                return Some(EffectAction::Spawn {
                    name: self.current_name.clone(),
                    position: self.current_pos,
                    speed: self.current_speed,
                    scale: self.current_scale,
                });
            } else if message.destination() == self.stop_all_button {
                log::info!("[EffectPanel] Stop All clicked");
                return Some(EffectAction::StopAll);
            }
        }

        None
    }

    /// 활성 이펙트 수 업데이트
    pub fn update_active_count(&mut self, count: usize, ui: &UserInterface) {
        if self.active_count != count {
            self.active_count = count;
            ui.send_message(
                UiMessage::for_widget(
                    self.status_text,
                    TextMessage::Text(format!("Active Effects: {}", count)),
                )
                .with_direction(MessageDirection::ToWidget),
            );
        }
    }
}
