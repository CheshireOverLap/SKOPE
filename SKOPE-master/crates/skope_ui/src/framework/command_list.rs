//! CommandList — 커맨드 리스트 스택 및 축소 버튼 지원
//!
//! UE Slate의 FCommandListGroup 확장 기능.

use std::collections::HashMap;

/// 커맨드 리스트 엔트리
#[derive(Debug, Clone)]
pub struct CommandListEntry {
    pub command_name: String,
    pub label: String,
    pub description: String,
    pub icon_name: Option<String>,
    pub shortcut: Option<String>,
    pub is_enabled: bool,
    pub is_visible: bool,
    pub is_checked: Option<bool>,
}

impl CommandListEntry {
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            command_name: name.into(),
            label: label.into(),
            description: String::new(),
            icon_name: None,
            shortcut: None,
            is_enabled: true,
            is_visible: true,
            is_checked: None,
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon_name = Some(icon.into());
        self
    }

    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn with_checked(mut self, checked: bool) -> Self {
        self.is_checked = Some(checked);
        self
    }
}

/// 커맨드 리스트 — 여러 커맨드를 그룹으로 관리
#[derive(Debug, Clone)]
pub struct CommandListGroup {
    name: String,
    entries: Vec<CommandListEntry>,
    separators: Vec<usize>,
}

impl CommandListGroup {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entries: Vec::new(),
            separators: Vec::new(),
        }
    }

    pub fn add(&mut self, entry: CommandListEntry) {
        self.entries.push(entry);
    }

    pub fn add_separator(&mut self) {
        self.separators.push(self.entries.len());
    }

    pub fn is_separator_at(&self, index: usize) -> bool {
        self.separators.contains(&index)
    }

    pub fn entries(&self) -> &[CommandListEntry] { &self.entries }
    pub fn name(&self) -> &str { &self.name }
    pub fn count(&self) -> usize { self.entries.len() }

    /// 보이는 항목만 필터
    pub fn visible_entries(&self) -> Vec<&CommandListEntry> {
        self.entries.iter().filter(|e| e.is_visible).collect()
    }

    /// 활성 항목만 필터
    pub fn enabled_entries(&self) -> Vec<&CommandListEntry> {
        self.entries.iter().filter(|e| e.is_enabled).collect()
    }
}

/// 커맨드 리스트 스택 — 컨텍스트별 커맨드 리스트 중첩
pub struct CommandListStack {
    stack: Vec<CommandListGroup>,
}

impl CommandListStack {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn push(&mut self, list: CommandListGroup) {
        self.stack.push(list);
    }

    pub fn pop(&mut self) -> Option<CommandListGroup> {
        self.stack.pop()
    }

    pub fn top(&self) -> Option<&CommandListGroup> {
        self.stack.last()
    }

    /// 전체 스택의 커맨드를 병합 (상위 우선)
    pub fn merged_commands(&self) -> Vec<&CommandListEntry> {
        let mut seen = HashMap::new();
        let mut result = Vec::new();
        // 스택 위에서부터 (최근 것 우선)
        for list in self.stack.iter().rev() {
            for entry in &list.entries {
                if !seen.contains_key(&entry.command_name) {
                    seen.insert(entry.command_name.clone(), true);
                    result.push(entry);
                }
            }
        }
        result
    }

    pub fn depth(&self) -> usize { self.stack.len() }
    pub fn is_empty(&self) -> bool { self.stack.is_empty() }
}

/// 축소 버튼 — 공간 부족 시 커맨드를 드롭다운으로 축소
#[derive(Debug, Clone)]
pub struct CollapsedButtonInfo {
    pub label: String,
    pub icon_name: Option<String>,
    pub collapsed_entries: Vec<String>,
    pub is_open: bool,
}

impl CollapsedButtonInfo {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon_name: None,
            collapsed_entries: Vec::new(),
            is_open: false,
        }
    }

    pub fn add_entry(&mut self, command_name: impl Into<String>) {
        self.collapsed_entries.push(command_name.into());
    }

    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    pub fn entry_count(&self) -> usize { self.collapsed_entries.len() }
}

/// 툴바 축소 관리자
pub struct ToolbarCollapseManager {
    available_width: f32,
    button_widths: Vec<(String, f32)>,
    collapse_threshold: f32,
}

impl ToolbarCollapseManager {
    pub fn new(available_width: f32) -> Self {
        Self {
            available_width,
            button_widths: Vec::new(),
            collapse_threshold: 0.0,
        }
    }

    pub fn set_available_width(&mut self, width: f32) {
        self.available_width = width;
    }

    pub fn add_button(&mut self, name: impl Into<String>, width: f32) {
        self.button_widths.push((name.into(), width));
    }

    pub fn clear_buttons(&mut self) {
        self.button_widths.clear();
    }

    /// 축소가 필요한 버튼 목록 계산
    pub fn compute_collapsed(&self) -> Vec<String> {
        let mut total = 0.0f32;
        let mut collapsed = Vec::new();
        // 뒤에서부터 축소
        for (name, width) in &self.button_widths {
            total += width;
        }
        if total <= self.available_width {
            return collapsed; // 축소 불필요
        }
        // 뒤에서부터 제거
        let mut running = total;
        for (name, width) in self.button_widths.iter().rev() {
            if running <= self.available_width {
                break;
            }
            collapsed.push(name.clone());
            running -= width;
        }
        collapsed.reverse();
        collapsed
    }

    pub fn button_count(&self) -> usize { self.button_widths.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_list_entry() {
        let entry = CommandListEntry::new("copy", "Copy")
            .with_shortcut("Ctrl+C")
            .with_icon("copy_icon");
        assert_eq!(entry.command_name, "copy");
        assert_eq!(entry.shortcut, Some("Ctrl+C".into()));
    }

    #[test]
    fn test_command_list() {
        let mut list = CommandListGroup::new("edit");
        list.add(CommandListEntry::new("cut", "Cut"));
        list.add(CommandListEntry::new("copy", "Copy"));
        list.add_separator();
        list.add(CommandListEntry::new("paste", "Paste"));
        assert_eq!(list.count(), 3);
        assert!(list.is_separator_at(2));
    }

    #[test]
    fn test_command_list_visibility() {
        let mut list = CommandListGroup::new("file");
        list.add(CommandListEntry::new("new", "New"));
        let mut hidden = CommandListEntry::new("secret", "Secret");
        hidden.is_visible = false;
        list.add(hidden);
        assert_eq!(list.visible_entries().len(), 1);
    }

    #[test]
    fn test_command_list_stack() {
        let mut stack = CommandListStack::new();
        let mut global = CommandListGroup::new("global");
        global.add(CommandListEntry::new("save", "Save"));
        global.add(CommandListEntry::new("open", "Open"));

        let mut local = CommandListGroup::new("local");
        local.add(CommandListEntry::new("save", "Save (Local)")); // override
        local.add(CommandListEntry::new("format", "Format"));

        stack.push(global);
        stack.push(local);

        let merged = stack.merged_commands();
        assert_eq!(merged.len(), 3); // save(local), format, open
    }

    #[test]
    fn test_collapsed_button() {
        let mut btn = CollapsedButtonInfo::new("More");
        btn.add_entry("action1");
        btn.add_entry("action2");
        assert_eq!(btn.entry_count(), 2);
        assert!(!btn.is_open);
        btn.toggle();
        assert!(btn.is_open);
    }

    #[test]
    fn test_toolbar_collapse_all_fit() {
        let mut mgr = ToolbarCollapseManager::new(300.0);
        mgr.add_button("A", 80.0);
        mgr.add_button("B", 80.0);
        mgr.add_button("C", 80.0);
        let collapsed = mgr.compute_collapsed();
        assert!(collapsed.is_empty()); // 240 < 300
    }

    #[test]
    fn test_toolbar_collapse_overflow() {
        let mut mgr = ToolbarCollapseManager::new(200.0);
        mgr.add_button("A", 80.0);
        mgr.add_button("B", 80.0);
        mgr.add_button("C", 80.0);
        let collapsed = mgr.compute_collapsed();
        assert!(!collapsed.is_empty()); // 240 > 200
        assert!(collapsed.contains(&"C".to_string()));
    }
}
