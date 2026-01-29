//! UICommandList — 단축키/커맨드 시스템
//!
//! UE의 FUICommandList / FInputBindingManager / FBindingContext 참고
//! - BindingContext 계층 구조
//! - UIAction에 IsChecked/IsVisible 델리게이트
//! - Primary/Secondary 코드 지원
//! - 계층적 커맨드 리스트 (parent 버블링)

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use crate::event::KeyCode;

// ============================================================================
// BindingContext — UE의 FBindingContext
// ============================================================================

/// 바인딩 컨텍스트 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingContextId(pub &'static str);

/// 바인딩 컨텍스트 (계층 구조)
#[derive(Debug, Clone)]
pub struct BindingContext {
    pub id: BindingContextId,
    pub description: &'static str,
    pub parent: Option<BindingContextId>,
}

impl BindingContext {
    pub fn new(id: BindingContextId, description: &'static str) -> Self {
        Self { id, description, parent: None }
    }

    pub fn with_parent(mut self, parent: BindingContextId) -> Self {
        self.parent = Some(parent);
        self
    }
}

/// 글로벌 컨텍스트 (루트)
pub const CONTEXT_GLOBAL: BindingContextId = BindingContextId("Global");
/// 에디터 컨텍스트
pub const CONTEXT_EDITOR: BindingContextId = BindingContextId("Editor");
/// 뷰포트 컨텍스트
pub const CONTEXT_VIEWPORT: BindingContextId = BindingContextId("Viewport");

// ============================================================================
// InputChord — 입력 조합
// ============================================================================

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

    /// 디스플레이 텍스트 생성 (예: "Ctrl+Shift+S")
    pub fn display_text(&self) -> String {
        let mut parts = Vec::new();

        #[cfg(target_os = "macos")]
        {
            if self.ctrl { parts.push("⌘"); }
            if self.shift { parts.push("⇧"); }
            if self.alt { parts.push("⌥"); }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if self.ctrl { parts.push("Ctrl"); }
            if self.shift { parts.push("Shift"); }
            if self.alt { parts.push("Alt"); }
        }

        parts.push(key_display_name(self.key));
        parts.join("+")
    }
}

/// 키 이름 반환
fn key_display_name(key: KeyCode) -> &'static str {
    match key {
        KeyCode::A => "A", KeyCode::B => "B", KeyCode::C => "C",
        KeyCode::D => "D", KeyCode::E => "E", KeyCode::F => "F",
        KeyCode::G => "G", KeyCode::H => "H", KeyCode::I => "I",
        KeyCode::J => "J", KeyCode::K => "K", KeyCode::L => "L",
        KeyCode::M => "M", KeyCode::N => "N", KeyCode::O => "O",
        KeyCode::P => "P", KeyCode::Q => "Q", KeyCode::R => "R",
        KeyCode::S => "S", KeyCode::T => "T", KeyCode::U => "U",
        KeyCode::V => "V", KeyCode::W => "W", KeyCode::X => "X",
        KeyCode::Y => "Y", KeyCode::Z => "Z",
        KeyCode::Key0 => "0", KeyCode::Key1 => "1", KeyCode::Key2 => "2",
        KeyCode::Key3 => "3", KeyCode::Key4 => "4", KeyCode::Key5 => "5",
        KeyCode::Key6 => "6", KeyCode::Key7 => "7", KeyCode::Key8 => "8",
        KeyCode::Key9 => "9",
        KeyCode::F1 => "F1", KeyCode::F2 => "F2", KeyCode::F3 => "F3",
        KeyCode::F4 => "F4", KeyCode::F5 => "F5", KeyCode::F6 => "F6",
        KeyCode::F7 => "F7", KeyCode::F8 => "F8", KeyCode::F9 => "F9",
        KeyCode::F10 => "F10", KeyCode::F11 => "F11", KeyCode::F12 => "F12",
        KeyCode::Escape => "Esc", KeyCode::Tab => "Tab",
        KeyCode::CapsLock => "CapsLock", KeyCode::Backspace => "Backspace",
        KeyCode::Enter => "Enter", KeyCode::Space => "Space",
        KeyCode::Left => "←", KeyCode::Right => "→",
        KeyCode::Up => "↑", KeyCode::Down => "↓",
        KeyCode::Insert => "Ins", KeyCode::Delete => "Del",
        KeyCode::Home => "Home", KeyCode::End => "End",
        KeyCode::PageUp => "PgUp", KeyCode::PageDown => "PgDn",
        _ => "?",
    }
}

// ============================================================================
// UIActionType — 액션 유형
// ============================================================================

/// 커맨드 액션 유형 (UE의 EUIActionType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UIActionType {
    /// 일반 버튼 (실행 후 끝)
    #[default]
    Button,
    /// 토글 (체크 상태 전환)
    Toggle,
    /// 라디오 (그룹 내 하나만 선택)
    Radio,
}

// ============================================================================
// CommandId
// ============================================================================

/// 커맨드 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommandId(pub &'static str);

// ============================================================================
// UICommandInfo — 커맨드 정보
// ============================================================================

/// 커맨드 정보
pub struct UICommandInfo {
    pub id: CommandId,
    pub label: &'static str,
    pub description: &'static str,
    pub default_chord: Option<InputChord>,
    /// 보조 단축키 (UE의 SecondaryInputChord)
    pub secondary_chord: Option<InputChord>,
    /// 바인딩 컨텍스트
    pub binding_context: BindingContextId,
    /// 액션 유형
    pub action_type: UIActionType,
}

impl UICommandInfo {
    /// 간단한 커맨드 생성
    pub fn simple(id: CommandId, label: &'static str, chord: Option<InputChord>) -> Self {
        Self {
            id,
            label,
            description: "",
            default_chord: chord,
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
        }
    }

    /// 단축키 디스플레이 텍스트 (Primary 기준)
    pub fn shortcut_text(&self) -> Option<String> {
        self.default_chord.as_ref().map(|c| c.display_text())
    }
}

// ============================================================================
// UIAction — 커맨드 액션 (UE의 FUIAction 확장)
// ============================================================================

/// 커맨드 액션
pub struct UIAction {
    pub execute: Box<dyn FnMut() + Send + Sync>,
    pub can_execute: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    /// 체크 상태 (토글/라디오용)
    pub is_checked: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    /// 가시성 상태
    pub is_visible: Option<Box<dyn Fn() -> bool + Send + Sync>>,
}

impl UIAction {
    /// 단순 실행 액션
    pub fn simple(f: impl FnMut() + Send + Sync + 'static) -> Self {
        Self {
            execute: Box::new(f),
            can_execute: None,
            is_checked: None,
            is_visible: None,
        }
    }

    /// can_execute 추가
    pub fn with_can_execute(mut self, f: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        self.can_execute = Some(Box::new(f));
        self
    }

    /// is_checked 추가
    pub fn with_is_checked(mut self, f: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        self.is_checked = Some(Box::new(f));
        self
    }

    /// is_visible 추가
    pub fn with_is_visible(mut self, f: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        self.is_visible = Some(Box::new(f));
        self
    }
}

// ============================================================================
// UICommandList — 커맨드 리스트 (계층적)
// ============================================================================

/// 커맨드 리스트 — 단축키 바인딩 + 실행
/// parent로 버블링 가능 (UE의 FUICommandList 계층 구조)
pub struct UICommandList {
    commands: Vec<(UICommandInfo, UIAction)>,
    chord_index: HashMap<InputChord, usize>,
    /// 부모 커맨드 리스트 (매칭 실패 시 버블링)
    parent: Option<Arc<Mutex<UICommandList>>>,
}

impl UICommandList {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            chord_index: HashMap::new(),
            parent: None,
        }
    }

    /// 부모 커맨드 리스트 설정
    pub fn with_parent(mut self, parent: Arc<Mutex<UICommandList>>) -> Self {
        self.parent = Some(parent);
        self
    }

    /// 부모 설정
    pub fn set_parent(&mut self, parent: Arc<Mutex<UICommandList>>) {
        self.parent = Some(parent);
    }

    /// 커맨드 등록
    pub fn map_action(&mut self, info: UICommandInfo, action: UIAction) {
        let idx = self.commands.len();
        if let Some(ref chord) = info.default_chord {
            self.chord_index.insert(chord.clone(), idx);
        }
        if let Some(ref chord) = info.secondary_chord {
            self.chord_index.insert(chord.clone(), idx);
        }
        self.commands.push((info, action));
    }

    /// 키 이벤트 처리 — 매칭되는 커맨드가 있으면 실행하고 true 반환
    /// 매칭 실패 시 parent로 버블링
    pub fn process_key_event(
        &mut self,
        key: KeyCode,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> bool {
        let chord = InputChord { key, ctrl, shift, alt };

        // 자기 자신에서 먼저 검색
        if let Some(&idx) = self.chord_index.get(&chord) {
            let (_, action) = &mut self.commands[idx];

            // is_visible 체크
            let visible = action.is_visible.as_ref().map_or(true, |f| f());
            if !visible {
                // 보이지 않으면 parent로 버블링
            } else {
                let can = action.can_execute.as_ref().map_or(true, |f| f());
                if can {
                    (action.execute)();
                    return true;
                }
            }
        }

        // parent 버블링
        if let Some(ref parent) = self.parent {
            if let Ok(mut parent) = parent.lock() {
                return parent.process_key_event(key, ctrl, shift, alt);
            }
        }

        false
    }

    /// ID로 커맨드 찾기
    pub fn find_command(&self, id: CommandId) -> Option<&(UICommandInfo, UIAction)> {
        self.commands.iter().find(|(info, _)| info.id == id)
    }

    /// 등록된 커맨드 이터레이터
    pub fn commands(&self) -> &[(UICommandInfo, UIAction)] {
        &self.commands
    }

    /// 등록된 커맨드 수
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

// ============================================================================
// InputBindingManager — 글로벌 바인딩 매니저 (UE의 FInputBindingManager)
// ============================================================================

/// 글로벌 입력 바인딩 매니저 (싱글톤)
pub struct InputBindingManager {
    /// 등록된 바인딩 컨텍스트
    contexts: HashMap<BindingContextId, BindingContext>,
    /// 커스텀 코드 오버라이드 (사용자가 키 리바인딩 시)
    overrides: HashMap<CommandId, InputChord>,
}

impl InputBindingManager {
    fn new() -> Self {
        let mut mgr = Self {
            contexts: HashMap::new(),
            overrides: HashMap::new(),
        };
        // 기본 컨텍스트 등록
        mgr.register_context(BindingContext::new(CONTEXT_GLOBAL, "Global context"));
        mgr.register_context(
            BindingContext::new(CONTEXT_EDITOR, "Editor context")
                .with_parent(CONTEXT_GLOBAL),
        );
        mgr.register_context(
            BindingContext::new(CONTEXT_VIEWPORT, "Viewport context")
                .with_parent(CONTEXT_EDITOR),
        );
        mgr
    }

    /// 싱글톤 인스턴스
    pub fn instance() -> &'static Mutex<InputBindingManager> {
        static INSTANCE: OnceLock<Mutex<InputBindingManager>> = OnceLock::new();
        INSTANCE.get_or_init(|| Mutex::new(InputBindingManager::new()))
    }

    /// 컨텍스트 등록
    pub fn register_context(&mut self, context: BindingContext) {
        self.contexts.insert(context.id, context);
    }

    /// 컨텍스트 조회
    pub fn get_context(&self, id: BindingContextId) -> Option<&BindingContext> {
        self.contexts.get(&id)
    }

    /// 컨텍스트가 다른 컨텍스트의 자손인지 확인
    pub fn is_descendant_of(&self, child: BindingContextId, ancestor: BindingContextId) -> bool {
        let mut current = Some(child);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.contexts.get(&id).and_then(|ctx| ctx.parent);
        }
        false
    }

    /// 커스텀 키바인딩 오버라이드 설정
    pub fn set_override(&mut self, command: CommandId, chord: InputChord) {
        self.overrides.insert(command, chord);
    }

    /// 커스텀 오버라이드 조회
    pub fn get_override(&self, command: CommandId) -> Option<&InputChord> {
        self.overrides.get(&command)
    }

    /// 오버라이드 제거
    pub fn clear_override(&mut self, command: CommandId) {
        self.overrides.remove(&command);
    }
}
