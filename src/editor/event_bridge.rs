//! winit → fyrox-ui 이벤트 브릿지
//!
//! winit의 WindowEvent를 fyrox-ui의 OsEvent로 변환

use fyrox_ui::message::{
    ButtonState, KeyCode, KeyboardModifiers, MouseButton, OsEvent, TouchPhase,
};
use fyrox_ui::UserInterface;
use fyrox_core::algebra::Vector2;
use winit::event::{ElementState, MouseButton as WinitMouseButton, WindowEvent};
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};

/// winit 이벤트를 fyrox-ui로 전달하는 브릿지
pub struct WinitEventBridge {
    /// 현재 키보드 모디파이어 상태
    modifiers: KeyboardModifiers,
    /// 현재 커서 위치
    cursor_position: Vector2<f32>,
}

impl WinitEventBridge {
    pub fn new() -> Self {
        Self {
            modifiers: KeyboardModifiers::default(),
            cursor_position: Vector2::new(0.0, 0.0),
        }
    }

    /// winit WindowEvent를 fyrox-ui OsEvent로 변환하여 UI에 전달
    /// 반환값: 이벤트가 UI에 의해 처리되었는지 여부
    pub fn translate_and_send(
        &mut self,
        ui: &mut UserInterface,
        event: &WindowEvent,
    ) -> bool {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = Vector2::new(position.x as f32, position.y as f32);
                ui.process_os_event(&OsEvent::CursorMoved {
                    position: self.cursor_position,
                })
            }

            WindowEvent::MouseInput { state, button, .. } => {
                let fyrox_button = translate_mouse_button(*button);
                let fyrox_state = translate_element_state(*state);
                ui.process_os_event(&OsEvent::MouseInput {
                    button: fyrox_button,
                    state: fyrox_state,
                })
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        (pos.x as f32 / 120.0, pos.y as f32 / 120.0)
                    }
                };
                ui.process_os_event(&OsEvent::MouseWheel(x, y))
            }

            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key_code) = event.physical_key {
                    let fyrox_key = translate_key_code(key_code);
                    let fyrox_state = translate_element_state(event.state);
                    let text = event.text.as_ref()
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    ui.process_os_event(&OsEvent::KeyboardInput {
                        button: fyrox_key,
                        state: fyrox_state,
                        text,
                    })
                } else {
                    false
                }
            }

            WindowEvent::ModifiersChanged(modifiers) => {
                let state = modifiers.state();
                self.modifiers = KeyboardModifiers {
                    alt: state.alt_key(),
                    shift: state.shift_key(),
                    control: state.control_key(),
                    system: state.super_key(),
                };
                ui.process_os_event(&OsEvent::KeyboardModifiers(self.modifiers))
            }

            WindowEvent::Touch(touch) => {
                let phase = match touch.phase {
                    winit::event::TouchPhase::Started => TouchPhase::Started,
                    winit::event::TouchPhase::Moved => TouchPhase::Moved,
                    winit::event::TouchPhase::Ended => TouchPhase::Ended,
                    winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
                };
                ui.process_os_event(&OsEvent::Touch {
                    phase,
                    location: Vector2::new(touch.location.x as f32, touch.location.y as f32),
                    force: None, // fyrox Force와 winit Force가 다름
                    id: touch.id,
                })
            }

            _ => false,
        }
    }

    /// 현재 커서 위치 반환
    #[allow(dead_code)]
    pub fn cursor_position(&self) -> Vector2<f32> {
        self.cursor_position
    }

    /// 현재 모디파이어 상태 반환
    #[allow(dead_code)]
    pub fn modifiers(&self) -> KeyboardModifiers {
        self.modifiers
    }
}

impl Default for WinitEventBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// winit MouseButton → fyrox MouseButton 변환
fn translate_mouse_button(button: WinitMouseButton) -> MouseButton {
    match button {
        WinitMouseButton::Left => MouseButton::Left,
        WinitMouseButton::Right => MouseButton::Right,
        WinitMouseButton::Middle => MouseButton::Middle,
        WinitMouseButton::Back => MouseButton::Back,
        WinitMouseButton::Forward => MouseButton::Forward,
        WinitMouseButton::Other(id) => MouseButton::Other(id),
    }
}

/// winit ElementState → fyrox ButtonState 변환
fn translate_element_state(state: ElementState) -> ButtonState {
    match state {
        ElementState::Pressed => ButtonState::Pressed,
        ElementState::Released => ButtonState::Released,
    }
}

/// winit KeyCode → fyrox KeyCode 변환
fn translate_key_code(key: WinitKeyCode) -> KeyCode {
    match key {
        WinitKeyCode::Backquote => KeyCode::Backquote,
        WinitKeyCode::Backslash => KeyCode::Backslash,
        WinitKeyCode::BracketLeft => KeyCode::BracketLeft,
        WinitKeyCode::BracketRight => KeyCode::BracketRight,
        WinitKeyCode::Comma => KeyCode::Comma,
        WinitKeyCode::Digit0 => KeyCode::Digit0,
        WinitKeyCode::Digit1 => KeyCode::Digit1,
        WinitKeyCode::Digit2 => KeyCode::Digit2,
        WinitKeyCode::Digit3 => KeyCode::Digit3,
        WinitKeyCode::Digit4 => KeyCode::Digit4,
        WinitKeyCode::Digit5 => KeyCode::Digit5,
        WinitKeyCode::Digit6 => KeyCode::Digit6,
        WinitKeyCode::Digit7 => KeyCode::Digit7,
        WinitKeyCode::Digit8 => KeyCode::Digit8,
        WinitKeyCode::Digit9 => KeyCode::Digit9,
        WinitKeyCode::Equal => KeyCode::Equal,
        WinitKeyCode::IntlBackslash => KeyCode::IntlBackslash,
        WinitKeyCode::IntlRo => KeyCode::IntlRo,
        WinitKeyCode::IntlYen => KeyCode::IntlYen,
        WinitKeyCode::KeyA => KeyCode::KeyA,
        WinitKeyCode::KeyB => KeyCode::KeyB,
        WinitKeyCode::KeyC => KeyCode::KeyC,
        WinitKeyCode::KeyD => KeyCode::KeyD,
        WinitKeyCode::KeyE => KeyCode::KeyE,
        WinitKeyCode::KeyF => KeyCode::KeyF,
        WinitKeyCode::KeyG => KeyCode::KeyG,
        WinitKeyCode::KeyH => KeyCode::KeyH,
        WinitKeyCode::KeyI => KeyCode::KeyI,
        WinitKeyCode::KeyJ => KeyCode::KeyJ,
        WinitKeyCode::KeyK => KeyCode::KeyK,
        WinitKeyCode::KeyL => KeyCode::KeyL,
        WinitKeyCode::KeyM => KeyCode::KeyM,
        WinitKeyCode::KeyN => KeyCode::KeyN,
        WinitKeyCode::KeyO => KeyCode::KeyO,
        WinitKeyCode::KeyP => KeyCode::KeyP,
        WinitKeyCode::KeyQ => KeyCode::KeyQ,
        WinitKeyCode::KeyR => KeyCode::KeyR,
        WinitKeyCode::KeyS => KeyCode::KeyS,
        WinitKeyCode::KeyT => KeyCode::KeyT,
        WinitKeyCode::KeyU => KeyCode::KeyU,
        WinitKeyCode::KeyV => KeyCode::KeyV,
        WinitKeyCode::KeyW => KeyCode::KeyW,
        WinitKeyCode::KeyX => KeyCode::KeyX,
        WinitKeyCode::KeyY => KeyCode::KeyY,
        WinitKeyCode::KeyZ => KeyCode::KeyZ,
        WinitKeyCode::Minus => KeyCode::Minus,
        WinitKeyCode::Period => KeyCode::Period,
        WinitKeyCode::Quote => KeyCode::Quote,
        WinitKeyCode::Semicolon => KeyCode::Semicolon,
        WinitKeyCode::Slash => KeyCode::Slash,
        WinitKeyCode::AltLeft => KeyCode::AltLeft,
        WinitKeyCode::AltRight => KeyCode::AltRight,
        WinitKeyCode::Backspace => KeyCode::Backspace,
        WinitKeyCode::CapsLock => KeyCode::CapsLock,
        WinitKeyCode::ContextMenu => KeyCode::ContextMenu,
        WinitKeyCode::ControlLeft => KeyCode::ControlLeft,
        WinitKeyCode::ControlRight => KeyCode::ControlRight,
        WinitKeyCode::Enter => KeyCode::Enter,
        WinitKeyCode::SuperLeft => KeyCode::SuperLeft,
        WinitKeyCode::SuperRight => KeyCode::SuperRight,
        WinitKeyCode::ShiftLeft => KeyCode::ShiftLeft,
        WinitKeyCode::ShiftRight => KeyCode::ShiftRight,
        WinitKeyCode::Space => KeyCode::Space,
        WinitKeyCode::Tab => KeyCode::Tab,
        WinitKeyCode::Convert => KeyCode::Convert,
        WinitKeyCode::KanaMode => KeyCode::KanaMode,
        WinitKeyCode::Lang1 => KeyCode::Lang1,
        WinitKeyCode::Lang2 => KeyCode::Lang2,
        WinitKeyCode::Lang3 => KeyCode::Lang3,
        WinitKeyCode::Lang4 => KeyCode::Lang4,
        WinitKeyCode::Lang5 => KeyCode::Lang5,
        WinitKeyCode::NonConvert => KeyCode::NonConvert,
        WinitKeyCode::Delete => KeyCode::Delete,
        WinitKeyCode::End => KeyCode::End,
        WinitKeyCode::Help => KeyCode::Help,
        WinitKeyCode::Home => KeyCode::Home,
        WinitKeyCode::Insert => KeyCode::Insert,
        WinitKeyCode::PageDown => KeyCode::PageDown,
        WinitKeyCode::PageUp => KeyCode::PageUp,
        WinitKeyCode::ArrowDown => KeyCode::ArrowDown,
        WinitKeyCode::ArrowLeft => KeyCode::ArrowLeft,
        WinitKeyCode::ArrowRight => KeyCode::ArrowRight,
        WinitKeyCode::ArrowUp => KeyCode::ArrowUp,
        WinitKeyCode::NumLock => KeyCode::NumLock,
        WinitKeyCode::Numpad0 => KeyCode::Numpad0,
        WinitKeyCode::Numpad1 => KeyCode::Numpad1,
        WinitKeyCode::Numpad2 => KeyCode::Numpad2,
        WinitKeyCode::Numpad3 => KeyCode::Numpad3,
        WinitKeyCode::Numpad4 => KeyCode::Numpad4,
        WinitKeyCode::Numpad5 => KeyCode::Numpad5,
        WinitKeyCode::Numpad6 => KeyCode::Numpad6,
        WinitKeyCode::Numpad7 => KeyCode::Numpad7,
        WinitKeyCode::Numpad8 => KeyCode::Numpad8,
        WinitKeyCode::Numpad9 => KeyCode::Numpad9,
        WinitKeyCode::NumpadAdd => KeyCode::NumpadAdd,
        WinitKeyCode::NumpadBackspace => KeyCode::NumpadBackspace,
        WinitKeyCode::NumpadClear => KeyCode::NumpadClear,
        WinitKeyCode::NumpadClearEntry => KeyCode::NumpadClearEntry,
        WinitKeyCode::NumpadComma => KeyCode::NumpadComma,
        WinitKeyCode::NumpadDecimal => KeyCode::NumpadDecimal,
        WinitKeyCode::NumpadDivide => KeyCode::NumpadDivide,
        WinitKeyCode::NumpadEnter => KeyCode::NumpadEnter,
        WinitKeyCode::NumpadEqual => KeyCode::NumpadEqual,
        WinitKeyCode::NumpadHash => KeyCode::NumpadHash,
        WinitKeyCode::NumpadMemoryAdd => KeyCode::NumpadMemoryAdd,
        WinitKeyCode::NumpadMemoryClear => KeyCode::NumpadMemoryClear,
        WinitKeyCode::NumpadMemoryRecall => KeyCode::NumpadMemoryRecall,
        WinitKeyCode::NumpadMemoryStore => KeyCode::NumpadMemoryStore,
        WinitKeyCode::NumpadMemorySubtract => KeyCode::NumpadMemorySubtract,
        WinitKeyCode::NumpadMultiply => KeyCode::NumpadMultiply,
        WinitKeyCode::NumpadParenLeft => KeyCode::NumpadParenLeft,
        WinitKeyCode::NumpadParenRight => KeyCode::NumpadParenRight,
        WinitKeyCode::NumpadStar => KeyCode::NumpadStar,
        WinitKeyCode::NumpadSubtract => KeyCode::NumpadSubtract,
        WinitKeyCode::Escape => KeyCode::Escape,
        WinitKeyCode::F1 => KeyCode::F1,
        WinitKeyCode::F2 => KeyCode::F2,
        WinitKeyCode::F3 => KeyCode::F3,
        WinitKeyCode::F4 => KeyCode::F4,
        WinitKeyCode::F5 => KeyCode::F5,
        WinitKeyCode::F6 => KeyCode::F6,
        WinitKeyCode::F7 => KeyCode::F7,
        WinitKeyCode::F8 => KeyCode::F8,
        WinitKeyCode::F9 => KeyCode::F9,
        WinitKeyCode::F10 => KeyCode::F10,
        WinitKeyCode::F11 => KeyCode::F11,
        WinitKeyCode::F12 => KeyCode::F12,
        WinitKeyCode::F13 => KeyCode::F13,
        WinitKeyCode::F14 => KeyCode::F14,
        WinitKeyCode::F15 => KeyCode::F15,
        WinitKeyCode::F16 => KeyCode::F16,
        WinitKeyCode::F17 => KeyCode::F17,
        WinitKeyCode::F18 => KeyCode::F18,
        WinitKeyCode::F19 => KeyCode::F19,
        WinitKeyCode::F20 => KeyCode::F20,
        WinitKeyCode::F21 => KeyCode::F21,
        WinitKeyCode::F22 => KeyCode::F22,
        WinitKeyCode::F23 => KeyCode::F23,
        WinitKeyCode::F24 => KeyCode::F24,
        WinitKeyCode::Fn => KeyCode::Fn,
        WinitKeyCode::FnLock => KeyCode::FnLock,
        WinitKeyCode::PrintScreen => KeyCode::PrintScreen,
        WinitKeyCode::ScrollLock => KeyCode::ScrollLock,
        WinitKeyCode::Pause => KeyCode::Pause,
        _ => KeyCode::Unknown,
    }
}
