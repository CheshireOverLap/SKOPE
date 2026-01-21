//! Command Registry - 에디터 명령 레지스트리
//!
//! 모든 에디터 명령을 등록하고 검색할 수 있는 중앙 레지스트리입니다.
//! Command Palette에서 명령을 검색하고 실행하는데 사용됩니다.

use std::collections::HashMap;

use crate::app::EditorCommand;

/// 키보드 수정자 (Ctrl, Shift, Alt)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers { ctrl: false, shift: false, alt: false };
    pub const CTRL: Modifiers = Modifiers { ctrl: true, shift: false, alt: false };
    pub const SHIFT: Modifiers = Modifiers { ctrl: false, shift: true, alt: false };
    pub const ALT: Modifiers = Modifiers { ctrl: false, shift: false, alt: true };
    pub const CTRL_SHIFT: Modifiers = Modifiers { ctrl: true, shift: true, alt: false };
    pub const CTRL_ALT: Modifiers = Modifiers { ctrl: true, shift: false, alt: true };

    /// 문자열로 변환 (예: "Ctrl+Shift")
    pub fn to_string(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        parts.join("+")
    }
}

/// 키보드 단축키
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyboardShortcut {
    pub modifiers: Modifiers,
    pub key: egui::Key,
}

impl KeyboardShortcut {
    /// 새 단축키 생성
    pub fn new(modifiers: Modifiers, key: egui::Key) -> Self {
        Self { modifiers, key }
    }

    /// Ctrl + 키
    pub fn ctrl(key: egui::Key) -> Self {
        Self::new(Modifiers::CTRL, key)
    }

    /// Shift + 키
    pub fn shift(key: egui::Key) -> Self {
        Self::new(Modifiers::SHIFT, key)
    }

    /// Ctrl+Shift + 키
    pub fn ctrl_shift(key: egui::Key) -> Self {
        Self::new(Modifiers::CTRL_SHIFT, key)
    }

    /// 문자열로 변환 (예: "Ctrl+S")
    pub fn to_display_string(&self) -> String {
        let mod_str = self.modifiers.to_string();
        let key_str = format!("{:?}", self.key);

        if mod_str.is_empty() {
            key_str
        } else {
            format!("{}+{}", mod_str, key_str)
        }
    }
}

/// 명령 카테고리
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandCategory {
    File,
    Edit,
    View,
    GameObject,
    Component,
    Window,
    Help,
    AI,
    Debug,
}

impl CommandCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            CommandCategory::File => "File",
            CommandCategory::Edit => "Edit",
            CommandCategory::View => "View",
            CommandCategory::GameObject => "GameObject",
            CommandCategory::Component => "Component",
            CommandCategory::Window => "Window",
            CommandCategory::Help => "Help",
            CommandCategory::AI => "AI",
            CommandCategory::Debug => "Debug",
        }
    }

    pub fn all() -> &'static [CommandCategory] {
        &[
            CommandCategory::File,
            CommandCategory::Edit,
            CommandCategory::View,
            CommandCategory::GameObject,
            CommandCategory::Component,
            CommandCategory::Window,
            CommandCategory::Help,
            CommandCategory::AI,
            CommandCategory::Debug,
        ]
    }
}

/// 등록된 명령
#[derive(Clone)]
pub struct RegisteredCommand {
    /// 고유 ID (예: "file.save")
    pub id: String,
    /// 표시 라벨 (예: "Save")
    pub label: String,
    /// 카테고리
    pub category: CommandCategory,
    /// 단축키 (옵션)
    pub shortcut: Option<KeyboardShortcut>,
    /// 검색용 키워드
    pub keywords: Vec<String>,
    /// 아이콘 (옵션)
    pub icon: Option<String>,
    /// 명령 생성 함수
    action: fn() -> EditorCommand,
    /// 활성화 조건 (옵션) - true면 활성화
    pub enabled: bool,
}

impl RegisteredCommand {
    /// 새 명령 생성
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        category: CommandCategory,
        action: fn() -> EditorCommand,
    ) -> Self {
        let label_str = label.into();
        Self {
            id: id.into(),
            keywords: vec![label_str.to_lowercase()],
            label: label_str,
            category,
            shortcut: None,
            icon: None,
            action,
            enabled: true,
        }
    }

    /// 단축키 추가
    pub fn with_shortcut(mut self, shortcut: KeyboardShortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// 키워드 추가
    pub fn with_keywords(mut self, keywords: Vec<&str>) -> Self {
        self.keywords.extend(keywords.into_iter().map(|s| s.to_lowercase()));
        self
    }

    /// 아이콘 추가
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 명령 실행
    pub fn execute(&self) -> EditorCommand {
        (self.action)()
    }

    /// 검색어와 일치하는지 확인 (퍼지 매칭)
    pub fn matches(&self, query: &str) -> f32 {
        let query_lower = query.to_lowercase();

        // 정확히 일치하면 최고 점수
        if self.label.to_lowercase() == query_lower {
            return 1.0;
        }

        // ID 매칭
        if self.id.to_lowercase().contains(&query_lower) {
            return 0.9;
        }

        // 라벨 접두사 매칭
        if self.label.to_lowercase().starts_with(&query_lower) {
            return 0.8;
        }

        // 라벨 포함 매칭
        if self.label.to_lowercase().contains(&query_lower) {
            return 0.7;
        }

        // 키워드 매칭
        for keyword in &self.keywords {
            if keyword.contains(&query_lower) {
                return 0.6;
            }
        }

        // 카테고리 매칭
        if self.category.display_name().to_lowercase().contains(&query_lower) {
            return 0.3;
        }

        0.0
    }
}

impl std::fmt::Debug for RegisteredCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredCommand")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("category", &self.category)
            .field("shortcut", &self.shortcut)
            .finish()
    }
}

/// 명령 레지스트리
#[derive(Default)]
pub struct CommandRegistry {
    /// ID → 명령 매핑
    commands: HashMap<String, RegisteredCommand>,
    /// 단축키 → ID 매핑
    shortcut_map: HashMap<KeyboardShortcut, String>,
    /// 카테고리별 명령 ID 목록
    category_map: HashMap<CommandCategory, Vec<String>>,
}

impl CommandRegistry {
    /// 새 레지스트리 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 기본 명령들이 등록된 레지스트리 생성
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        register_default_commands(&mut registry);
        registry
    }

    /// 명령 등록
    pub fn register(&mut self, command: RegisteredCommand) {
        let id = command.id.clone();
        let category = command.category;

        // 단축키 등록
        if let Some(ref shortcut) = command.shortcut {
            self.shortcut_map.insert(shortcut.clone(), id.clone());
        }

        // 카테고리 맵 업데이트
        self.category_map
            .entry(category)
            .or_default()
            .push(id.clone());

        // 명령 등록
        self.commands.insert(id, command);
    }

    /// ID로 명령 조회
    pub fn get(&self, id: &str) -> Option<&RegisteredCommand> {
        self.commands.get(id)
    }

    /// 단축키로 명령 조회
    pub fn get_by_shortcut(&self, shortcut: &KeyboardShortcut) -> Option<&RegisteredCommand> {
        self.shortcut_map
            .get(shortcut)
            .and_then(|id| self.commands.get(id))
    }

    /// 검색 (퍼지 매칭)
    pub fn search(&self, query: &str) -> Vec<&RegisteredCommand> {
        if query.is_empty() {
            // 빈 쿼리면 모든 명령 반환 (카테고리 순)
            return self.all_commands();
        }

        let mut results: Vec<_> = self.commands
            .values()
            .filter(|cmd| cmd.enabled)
            .map(|cmd| (cmd, cmd.matches(query)))
            .filter(|(_, score)| *score > 0.0)
            .collect();

        // 점수 내림차순 정렬
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        results.into_iter().map(|(cmd, _)| cmd).collect()
    }

    /// 카테고리별 명령 조회
    pub fn get_by_category(&self, category: CommandCategory) -> Vec<&RegisteredCommand> {
        self.category_map
            .get(&category)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.commands.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 모든 명령 반환 (카테고리 순)
    pub fn all_commands(&self) -> Vec<&RegisteredCommand> {
        CommandCategory::all()
            .iter()
            .flat_map(|cat| self.get_by_category(*cat))
            .collect()
    }

    /// 명령 수
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl std::fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandRegistry")
            .field("command_count", &self.commands.len())
            .finish()
    }
}

/// 기본 명령 등록
pub fn register_default_commands(registry: &mut CommandRegistry) {
    use egui::Key;

    // === File ===
    registry.register(
        RegisteredCommand::new("file.save", "Save Scene", CommandCategory::File, || EditorCommand::SaveScene)
            .with_shortcut(KeyboardShortcut::ctrl(Key::S))
            .with_keywords(vec!["save", "scene", "저장"])
            .with_icon("💾")
    );

    registry.register(
        RegisteredCommand::new("file.load", "Open Scene", CommandCategory::File, || EditorCommand::LoadScene(String::new()))
            .with_shortcut(KeyboardShortcut::ctrl(Key::O))
            .with_keywords(vec!["open", "load", "scene", "열기"])
            .with_icon("📂")
    );

    // === Edit ===
    registry.register(
        RegisteredCommand::new("edit.undo", "Undo", CommandCategory::Edit, || EditorCommand::Undo)
            .with_shortcut(KeyboardShortcut::ctrl(Key::Z))
            .with_keywords(vec!["undo", "취소", "되돌리기"])
            .with_icon("↩")
    );

    registry.register(
        RegisteredCommand::new("edit.redo", "Redo", CommandCategory::Edit, || EditorCommand::Redo)
            .with_shortcut(KeyboardShortcut::ctrl_shift(Key::Z))
            .with_keywords(vec!["redo", "다시실행"])
            .with_icon("↪")
    );

    registry.register(
        RegisteredCommand::new("edit.deselect", "Deselect All", CommandCategory::Edit, || EditorCommand::SelectEntity(None))
            .with_shortcut(KeyboardShortcut::new(Modifiers::NONE, Key::Escape))
            .with_keywords(vec!["deselect", "clear", "selection", "선택해제"])
    );

    // === View ===
    registry.register(
        RegisteredCommand::new("view.play", "Play", CommandCategory::View, || {
            EditorCommand::SetPlayMode(crate::editor::EditorPlayState::Playing)
        })
            .with_shortcut(KeyboardShortcut::new(Modifiers::NONE, Key::F5))
            .with_keywords(vec!["play", "run", "start", "실행"])
            .with_icon("▶")
    );

    registry.register(
        RegisteredCommand::new("view.pause", "Pause", CommandCategory::View, || {
            EditorCommand::SetPlayMode(crate::editor::EditorPlayState::Paused)
        })
            .with_shortcut(KeyboardShortcut::shift(Key::F5))
            .with_keywords(vec!["pause", "stop", "일시정지"])
            .with_icon("⏸")
    );

    registry.register(
        RegisteredCommand::new("view.stop", "Stop", CommandCategory::View, || {
            EditorCommand::SetPlayMode(crate::editor::EditorPlayState::Edit)
        })
            .with_shortcut(KeyboardShortcut::ctrl_shift(Key::F5))
            .with_keywords(vec!["stop", "edit", "정지", "편집"])
            .with_icon("⏹")
    );

    log::info!("[CommandRegistry] Registered {} default commands", registry.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_registry() {
        let registry = CommandRegistry::with_defaults();
        assert!(registry.len() > 0);

        // ID로 검색
        let save_cmd = registry.get("file.save");
        assert!(save_cmd.is_some());
        assert_eq!(save_cmd.unwrap().label, "Save Scene");

        // 단축키로 검색
        let shortcut = KeyboardShortcut::ctrl(egui::Key::S);
        let cmd = registry.get_by_shortcut(&shortcut);
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().id, "file.save");
    }

    #[test]
    fn test_fuzzy_search() {
        let registry = CommandRegistry::with_defaults();

        // "save" 검색
        let results = registry.search("save");
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "file.save");

        // "undo" 검색
        let results = registry.search("undo");
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "edit.undo");
    }

    #[test]
    fn test_category_search() {
        let registry = CommandRegistry::with_defaults();

        let file_commands = registry.get_by_category(CommandCategory::File);
        assert!(!file_commands.is_empty());

        for cmd in file_commands {
            assert_eq!(cmd.category, CommandCategory::File);
        }
    }
}
