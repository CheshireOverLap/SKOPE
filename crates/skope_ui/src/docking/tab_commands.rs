//! TabCommands — 도킹 전용 키보드 단축키 (UE5 FTabCommands 대응)
//!
//! 탭/패널 조작을 위한 키보드 단축키 커맨드를 정의합니다.
//! 예: Ctrl+Tab (다음 탭), Ctrl+W (탭 닫기), Ctrl+Shift+T (탭 복원)

use std::collections::HashMap;

/// 도킹 커맨드 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DockCommand {
    /// 다음 탭으로 전환
    NextTab,
    /// 이전 탭으로 전환
    PrevTab,
    /// 현재 탭 닫기
    CloseTab,
    /// 마지막 닫은 탭 복원
    RestoreTab,
    /// 모든 탭 닫기
    CloseAllTabs,
    /// 오른쪽 탭 모두 닫기
    CloseTabsToRight,
    /// 사이드바로 이동
    MoveToSidebar,
    /// 사이드바에서 복원
    RestoreFromSidebar,
    /// 탭 분리 (새 윈도우)
    DetachTab,
    /// 특정 탭으로 이동 (1-9)
    GoToTab(u8),
    /// 마지막 활성 탭으로 복귀
    SwitchToLastTab,
}

/// 키 조합
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyBinding {
    /// 메인 키 (소문자 이름: "tab", "w", "t", "1"~"9" 등)
    pub key: String,
    /// Ctrl 키
    pub ctrl: bool,
    /// Shift 키
    pub shift: bool,
    /// Alt 키
    pub alt: bool,
}

impl KeyBinding {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            ctrl: false,
            shift: false,
            alt: false,
        }
    }

    pub fn ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    pub fn shift(mut self) -> Self {
        self.shift = true;
        self
    }

    pub fn alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// 사람이 읽을 수 있는 형태
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl { parts.push("Ctrl"); }
        if self.shift { parts.push("Shift"); }
        if self.alt { parts.push("Alt"); }
        parts.push(&self.key);
        parts.join("+")
    }

    /// 키 이벤트와 매칭되는지
    pub fn matches(&self, key: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        self.key == key && self.ctrl == ctrl && self.shift == shift && self.alt == alt
    }
}

/// 탭 커맨드 레지스트리
///
/// 도킹 전용 키보드 단축키를 관리합니다.
pub struct TabCommands {
    /// 커맨드별 키 바인딩
    bindings: HashMap<DockCommand, KeyBinding>,
    /// 키 → 커맨드 역매핑 (빠른 조회)
    reverse_map: HashMap<(String, bool, bool, bool), DockCommand>,
    /// 마지막 닫힌 탭 스택 (복원용)
    closed_tab_stack: Vec<ClosedTabRecord>,
    /// 마지막 활성 탭 ID (SwitchToLastTab용)
    last_active_tab_type: Option<String>,
}

/// 닫힌 탭 기록 (복원용)
#[derive(Debug, Clone)]
pub struct ClosedTabRecord {
    /// 탭 타입명 (스포너로 복원용)
    pub tab_type_name: String,
    /// 닫힌 시점의 인스턴스 ID
    pub instance_id: Option<String>,
}

impl Default for TabCommands {
    fn default() -> Self {
        let mut cmd = Self {
            bindings: HashMap::new(),
            reverse_map: HashMap::new(),
            closed_tab_stack: Vec::new(),
            last_active_tab_type: None,
        };
        cmd.register_defaults();
        cmd
    }
}

impl TabCommands {
    pub fn new() -> Self {
        Self::default()
    }

    /// 기본 키 바인딩 등록
    fn register_defaults(&mut self) {
        self.bind(DockCommand::NextTab, KeyBinding::new("Tab").ctrl());
        self.bind(DockCommand::PrevTab, KeyBinding::new("Tab").ctrl().shift());
        self.bind(DockCommand::CloseTab, KeyBinding::new("w").ctrl());
        self.bind(DockCommand::RestoreTab, KeyBinding::new("t").ctrl().shift());
        self.bind(DockCommand::CloseAllTabs, KeyBinding::new("w").ctrl().shift());
        self.bind(DockCommand::SwitchToLastTab, KeyBinding::new("Tab").alt());

        // Ctrl+1 ~ Ctrl+9
        for i in 1..=9u8 {
            self.bind(DockCommand::GoToTab(i), KeyBinding::new(i.to_string()).ctrl());
        }
    }

    /// 키 바인딩 등록/변경
    pub fn bind(&mut self, command: DockCommand, binding: KeyBinding) {
        let key = (binding.key.clone(), binding.ctrl, binding.shift, binding.alt);
        // 기존 바인딩 역맵에서 제거
        if let Some(old_binding) = self.bindings.get(&command) {
            let old_key = (old_binding.key.clone(), old_binding.ctrl, old_binding.shift, old_binding.alt);
            self.reverse_map.remove(&old_key);
        }
        self.reverse_map.insert(key, command);
        self.bindings.insert(command, binding);
    }

    /// 키 바인딩 해제
    pub fn unbind(&mut self, command: DockCommand) {
        if let Some(binding) = self.bindings.remove(&command) {
            let key = (binding.key.clone(), binding.ctrl, binding.shift, binding.alt);
            self.reverse_map.remove(&key);
        }
    }

    /// 키 이벤트에서 커맨드 조회
    pub fn command_for_key(&self, key: &str, ctrl: bool, shift: bool, alt: bool) -> Option<DockCommand> {
        let lookup = (key.to_string(), ctrl, shift, alt);
        self.reverse_map.get(&lookup).copied()
    }

    /// 커맨드의 키 바인딩 조회
    pub fn binding_for_command(&self, command: DockCommand) -> Option<&KeyBinding> {
        self.bindings.get(&command)
    }

    /// 커맨드의 키 바인딩 표시 텍스트
    pub fn binding_display(&self, command: DockCommand) -> Option<String> {
        self.bindings.get(&command).map(|b| b.display())
    }

    /// 닫힌 탭 기록 추가
    pub fn record_closed_tab(&mut self, tab_type_name: String, instance_id: Option<String>) {
        self.closed_tab_stack.push(ClosedTabRecord {
            tab_type_name,
            instance_id,
        });
        // 최대 20개 유지
        if self.closed_tab_stack.len() > 20 {
            self.closed_tab_stack.remove(0);
        }
    }

    /// 마지막 닫힌 탭 복원 정보 꺼내기
    pub fn pop_closed_tab(&mut self) -> Option<ClosedTabRecord> {
        self.closed_tab_stack.pop()
    }

    /// 닫힌 탭 기록 수
    pub fn closed_tab_count(&self) -> usize {
        self.closed_tab_stack.len()
    }

    /// 마지막 활성 탭 기록
    pub fn set_last_active(&mut self, tab_type: String) {
        self.last_active_tab_type = Some(tab_type);
    }

    /// 마지막 활성 탭 타입 조회
    pub fn last_active_tab_type(&self) -> Option<&str> {
        self.last_active_tab_type.as_deref()
    }

    /// 등록된 바인딩 수
    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bindings() {
        let cmd = TabCommands::new();
        // Ctrl+Tab → NextTab
        assert_eq!(cmd.command_for_key("Tab", true, false, false), Some(DockCommand::NextTab));
        // Ctrl+Shift+Tab → PrevTab
        assert_eq!(cmd.command_for_key("Tab", true, true, false), Some(DockCommand::PrevTab));
        // Ctrl+W → CloseTab
        assert_eq!(cmd.command_for_key("w", true, false, false), Some(DockCommand::CloseTab));
        // Ctrl+1 → GoToTab(1)
        assert_eq!(cmd.command_for_key("1", true, false, false), Some(DockCommand::GoToTab(1)));
    }

    #[test]
    fn test_custom_binding() {
        let mut cmd = TabCommands::new();
        cmd.bind(DockCommand::DetachTab, KeyBinding::new("d").ctrl().shift());
        assert_eq!(
            cmd.command_for_key("d", true, true, false),
            Some(DockCommand::DetachTab)
        );
    }

    #[test]
    fn test_unbind() {
        let mut cmd = TabCommands::new();
        cmd.unbind(DockCommand::CloseTab);
        assert_eq!(cmd.command_for_key("w", true, false, false), None);
    }

    #[test]
    fn test_binding_display() {
        let cmd = TabCommands::new();
        let display = cmd.binding_display(DockCommand::CloseTab);
        assert_eq!(display, Some("Ctrl+w".to_string()));
    }

    #[test]
    fn test_closed_tab_stack() {
        let mut cmd = TabCommands::new();
        cmd.record_closed_tab("Viewport".to_string(), None);
        cmd.record_closed_tab("Hierarchy".to_string(), Some("instance1".to_string()));

        assert_eq!(cmd.closed_tab_count(), 2);

        let restored = cmd.pop_closed_tab().unwrap();
        assert_eq!(restored.tab_type_name, "Hierarchy");
        assert_eq!(restored.instance_id, Some("instance1".to_string()));
        assert_eq!(cmd.closed_tab_count(), 1);
    }

    #[test]
    fn test_closed_tab_stack_limit() {
        let mut cmd = TabCommands::new();
        for i in 0..25 {
            cmd.record_closed_tab(format!("Tab{}", i), None);
        }
        assert_eq!(cmd.closed_tab_count(), 20);
    }

    #[test]
    fn test_last_active() {
        let mut cmd = TabCommands::new();
        assert!(cmd.last_active_tab_type().is_none());
        cmd.set_last_active("Viewport".to_string());
        assert_eq!(cmd.last_active_tab_type(), Some("Viewport"));
    }

    #[test]
    fn test_key_binding_matches() {
        let binding = KeyBinding::new("w").ctrl();
        assert!(binding.matches("w", true, false, false));
        assert!(!binding.matches("w", false, false, false));
        assert!(!binding.matches("q", true, false, false));
    }
}