//! UICommandList — 단축키/커맨드 시스템
//!
//! UE의 FUICommandList / FInputBindingManager 참고 (단순화 버전)

use std::collections::HashMap;
use winit::keyboard::KeyCode;

/// 입력 조합 (키 + 모디파이어)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InputChord {
    pub key: KeyCode,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl InputChord {
    pub fn new(key: KeyCode) -> Self {
        Self { key, ctrl: false, shift: false, alt: false }
    }

    pub fn ctrl(mut self) -> Self { self.ctrl = true; self }
    pub fn shift(mut self) -> Self { self.shift = true; self }
    pub fn alt(mut self) -> Self { self.alt = true; self }
}

/// 커맨드 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub &'static str);

/// 커맨드 정보
pub struct UICommandInfo {
    pub id: CommandId,
    pub label: &'static str,
    pub description: &'static str,
    pub default_chord: Option<InputChord>,
}

/// 커맨드 액션
pub struct UIAction {
    pub execute: Box<dyn FnMut()>,
    pub can_execute: Option<Box<dyn Fn() -> bool>>,
}

/// 커맨드 리스트 — 단축키 바인딩 + 실행
pub struct UICommandList {
    commands: Vec<(UICommandInfo, UIAction)>,
    chord_index: HashMap<InputChord, usize>,
}

impl UICommandList {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            chord_index: HashMap::new(),
        }
    }

    /// 커맨드 등록
    pub fn map_action(&mut self, info: UICommandInfo, action: UIAction) {
        let idx = self.commands.len();
        if let Some(ref chord) = info.default_chord {
            self.chord_index.insert(chord.clone(), idx);
        }
        self.commands.push((info, action));
    }

    /// 키 이벤트 처리 — 매칭되는 커맨드가 있으면 실행하고 true 반환
    pub fn process_key_event(
        &mut self,
        key: KeyCode,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> bool {
        let chord = InputChord { key, ctrl, shift, alt };
        if let Some(&idx) = self.chord_index.get(&chord) {
            let (_, action) = &mut self.commands[idx];
            let can = action.can_execute.as_ref().map_or(true, |f| f());
            if can {
                (action.execute)();
                return true;
            }
        }
        false
    }

    /// 등록된 커맨드 수
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
