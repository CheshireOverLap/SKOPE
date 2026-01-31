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

/// 반복 모드 (UE의 EUIActionRepeatMode)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UIActionRepeatMode {
    /// 반복 비활성화 (기본)
    #[default]
    RepeatDisabled,
    /// 키를 누르고 있으면 반복 실행
    RepeatEnabled,
}

// ============================================================================
// SlateIcon — 커맨드 아이콘 (UE의 FSlateIcon)
// ============================================================================

/// 커맨드에 연결된 아이콘 정보
///
/// 스타일 셋 기반 아이콘 참조. UE의 `FSlateIcon`에 해당.
#[derive(Debug, Clone)]
pub struct SlateIcon {
    /// 스타일 셋 이름 (예: "EditorStyle")
    pub style_set: String,
    /// 브러시 이름 (예: "Icons.Save")
    pub brush_name: String,
    /// 작은 아이콘 브러시 이름 (선택적)
    pub small_icon: Option<String>,
}

impl SlateIcon {
    pub fn new(style_set: impl Into<String>, brush_name: impl Into<String>) -> Self {
        Self {
            style_set: style_set.into(),
            brush_name: brush_name.into(),
            small_icon: None,
        }
    }

    pub fn with_small_icon(mut self, small_icon: impl Into<String>) -> Self {
        self.small_icon = Some(small_icon.into());
        self
    }
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
    /// 반복 모드
    pub repeat_mode: UIActionRepeatMode,
    /// 아이콘 (선택적)
    pub icon: Option<SlateIcon>,
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
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: None,
        }
    }

    /// 아이콘 설정
    pub fn with_icon(mut self, icon: SlateIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 반복 모드 설정
    pub fn with_repeat_mode(mut self, mode: UIActionRepeatMode) -> Self {
        self.repeat_mode = mode;
        self
    }

    /// 설명 설정
    pub fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// 바인딩 컨텍스트 설정
    pub fn with_context(mut self, context: BindingContextId) -> Self {
        self.binding_context = context;
        self
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

    /// 모든 오버라이드 제거
    pub fn clear_all_overrides(&mut self) {
        self.overrides.clear();
    }

    /// 키 충돌 감지 — 같은 코드를 사용하는 커맨드 쌍 찾기
    ///
    /// `commands`의 모든 커맨드 쌍에서 동일 chord를 사용하는 경우를 반환합니다.
    /// 오버라이드도 고려합니다.
    pub fn detect_conflicts(&self, commands: &UICommandList) -> Vec<ConflictInfo> {
        let mut chord_map: HashMap<InputChord, Vec<CommandId>> = HashMap::new();

        for (info, _) in commands.commands() {
            // 오버라이드 적용된 chord 또는 기본 chord 사용
            let effective_chord = self.overrides.get(&info.id)
                .cloned()
                .or_else(|| info.default_chord.clone());

            if let Some(chord) = effective_chord {
                chord_map.entry(chord).or_default().push(info.id);
            }

            // 보조 단축키도 체크
            if let Some(ref chord) = info.secondary_chord {
                chord_map.entry(chord.clone()).or_default().push(info.id);
            }
        }

        chord_map.into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .map(|(chord, commands)| ConflictInfo { chord, commands })
            .collect()
    }

    /// 오버라이드를 JSON으로 직렬화
    pub fn save_bindings_json(&self) -> String {
        let entries: Vec<(String, String)> = self.overrides.iter()
            .map(|(id, chord)| (id.0.to_string(), chord.display_text()))
            .collect();
        serde_json::to_string_pretty(&entries).unwrap_or_default()
    }

    /// JSON에서 오버라이드 복원
    ///
    /// JSON 형식: `[["CommandId", "Ctrl+S"], ...]`
    /// 파싱 실패한 항목은 무시합니다.
    pub fn load_bindings_json(&mut self, json: &str) -> usize {
        let entries: Vec<(String, String)> = match serde_json::from_str(json) {
            Ok(v) => v,
            Err(_) => return 0,
        };

        let mut loaded = 0;
        for (id_str, chord_str) in &entries {
            if let Some(chord) = parse_chord_display_text(chord_str) {
                // CommandId는 &'static str이므로 런타임 문자열은 leak로 변환
                // (설정 파일 로드는 앱 수명과 동일하므로 안전)
                let id_static: &'static str = Box::leak(id_str.clone().into_boxed_str());
                self.overrides.insert(CommandId(id_static), chord);
                loaded += 1;
            }
        }
        loaded
    }
}

/// 키 충돌 정보
#[derive(Debug)]
pub struct ConflictInfo {
    /// 충돌하는 코드
    pub chord: InputChord,
    /// 같은 코드를 사용하는 커맨드들
    pub commands: Vec<CommandId>,
}

/// 테스트용 비-싱글톤 InputBindingManager 생성
#[cfg(test)]
#[allow(non_snake_case)]
pub fn InputBindingManager_test_new() -> InputBindingManager {
    InputBindingManager::new()
}

/// 디스플레이 텍스트에서 InputChord 파싱 (예: "Ctrl+Shift+S")
fn parse_chord_display_text(text: &str) -> Option<InputChord> {
    let parts: Vec<&str> = text.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut key_name = "";

    for part in &parts {
        match *part {
            "Ctrl" | "\u{2318}" => ctrl = true,
            "Shift" | "\u{21e7}" => shift = true,
            "Alt" | "\u{2325}" => alt = true,
            other => key_name = other,
        }
    }

    let key = parse_key_name(key_name)?;
    Some(InputChord { key, ctrl, shift, alt })
}

/// 키 이름에서 KeyCode 파싱
fn parse_key_name(name: &str) -> Option<KeyCode> {
    match name {
        "A" => Some(KeyCode::A), "B" => Some(KeyCode::B), "C" => Some(KeyCode::C),
        "D" => Some(KeyCode::D), "E" => Some(KeyCode::E), "F" => Some(KeyCode::F),
        "G" => Some(KeyCode::G), "H" => Some(KeyCode::H), "I" => Some(KeyCode::I),
        "J" => Some(KeyCode::J), "K" => Some(KeyCode::K), "L" => Some(KeyCode::L),
        "M" => Some(KeyCode::M), "N" => Some(KeyCode::N), "O" => Some(KeyCode::O),
        "P" => Some(KeyCode::P), "Q" => Some(KeyCode::Q), "R" => Some(KeyCode::R),
        "S" => Some(KeyCode::S), "T" => Some(KeyCode::T), "U" => Some(KeyCode::U),
        "V" => Some(KeyCode::V), "W" => Some(KeyCode::W), "X" => Some(KeyCode::X),
        "Y" => Some(KeyCode::Y), "Z" => Some(KeyCode::Z),
        "0" => Some(KeyCode::Key0), "1" => Some(KeyCode::Key1), "2" => Some(KeyCode::Key2),
        "3" => Some(KeyCode::Key3), "4" => Some(KeyCode::Key4), "5" => Some(KeyCode::Key5),
        "6" => Some(KeyCode::Key6), "7" => Some(KeyCode::Key7), "8" => Some(KeyCode::Key8),
        "9" => Some(KeyCode::Key9),
        "F1" => Some(KeyCode::F1), "F2" => Some(KeyCode::F2), "F3" => Some(KeyCode::F3),
        "F4" => Some(KeyCode::F4), "F5" => Some(KeyCode::F5), "F6" => Some(KeyCode::F6),
        "F7" => Some(KeyCode::F7), "F8" => Some(KeyCode::F8), "F9" => Some(KeyCode::F9),
        "F10" => Some(KeyCode::F10), "F11" => Some(KeyCode::F11), "F12" => Some(KeyCode::F12),
        "Esc" => Some(KeyCode::Escape), "Tab" => Some(KeyCode::Tab),
        "Backspace" => Some(KeyCode::Backspace), "Enter" => Some(KeyCode::Enter),
        "Space" => Some(KeyCode::Space), "Del" => Some(KeyCode::Delete),
        "Ins" => Some(KeyCode::Insert), "Home" => Some(KeyCode::Home),
        "End" => Some(KeyCode::End), "PgUp" => Some(KeyCode::PageUp),
        "PgDn" => Some(KeyCode::PageDown),
        "\u{2190}" => Some(KeyCode::Left), "\u{2192}" => Some(KeyCode::Right),
        "\u{2191}" => Some(KeyCode::Up), "\u{2193}" => Some(KeyCode::Down),
        _ => None,
    }
}
