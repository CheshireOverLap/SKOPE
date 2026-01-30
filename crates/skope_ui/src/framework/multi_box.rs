//! MultiBox Builder — 메뉴/툴바 통합 빌더 (UE5 FMultiBoxBuilder)
//!
//! CommandId 기반으로 메뉴 아이템과 툴바 버튼을 선언적으로 구성합니다.
//! UICommandList에서 label, shortcut 등을 자동 조회합니다.

use std::sync::{Arc, Mutex};

use super::command::{CommandId, UICommandList};
use crate::widget::{MenuItem, MenuItemType};

// ============================================================================
// MultiBlockType
// ============================================================================

/// 멀티블록 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiBlockType {
    /// 일반 버튼
    Button,
    /// 토글 버튼
    Toggle,
    /// 체크박스
    Check,
    /// 라디오
    Radio,
    /// 구분선
    Separator,
    /// 서브메뉴
    SubMenu,
    /// 커스텀 위젯
    Widget,
    /// 섹션 헤더
    Heading,
}

// ============================================================================
// MultiBlockEntry
// ============================================================================

/// 멀티블록 엔트리 (메뉴/툴바 항목 하나)
pub struct MultiBlockEntry {
    /// 블록 타입
    pub block_type: MultiBlockType,
    /// 연결된 커맨드 ID
    pub command_id: Option<CommandId>,
    /// 레이블 (None이면 커맨드에서 가져옴)
    pub label: Option<String>,
    /// 아이콘 (유니코드 또는 경로)
    pub icon: Option<String>,
    /// 툴팁
    pub tooltip: Option<String>,
    /// 서브메뉴 자식 항목들
    pub children: Vec<MultiBlockEntry>,
    /// 실행 콜백 (커맨드가 없을 때)
    pub on_execute: Option<Box<dyn Fn() + Send + Sync>>,
    /// 섹션 이름 (그룹핑용)
    pub section: Option<String>,
}

impl MultiBlockEntry {
    /// 버튼 엔트리 (커맨드 기반)
    pub fn button(cmd: CommandId) -> Self {
        Self {
            block_type: MultiBlockType::Button,
            command_id: Some(cmd),
            label: None,
            icon: None,
            tooltip: None,
            children: Vec::new(),
            on_execute: None,
            section: None,
        }
    }

    /// 토글 엔트리
    pub fn toggle(cmd: CommandId) -> Self {
        Self {
            block_type: MultiBlockType::Toggle,
            command_id: Some(cmd),
            label: None,
            icon: None,
            tooltip: None,
            children: Vec::new(),
            on_execute: None,
            section: None,
        }
    }

    /// 체크 엔트리
    pub fn check(cmd: CommandId) -> Self {
        Self {
            block_type: MultiBlockType::Check,
            command_id: Some(cmd),
            label: None,
            icon: None,
            tooltip: None,
            children: Vec::new(),
            on_execute: None,
            section: None,
        }
    }

    /// 구분선
    pub fn separator() -> Self {
        Self {
            block_type: MultiBlockType::Separator,
            command_id: None,
            label: None,
            icon: None,
            tooltip: None,
            children: Vec::new(),
            on_execute: None,
            section: None,
        }
    }

    /// 서브메뉴
    pub fn submenu(label: impl Into<String>) -> Self {
        Self {
            block_type: MultiBlockType::SubMenu,
            command_id: None,
            label: Some(label.into()),
            icon: None,
            tooltip: None,
            children: Vec::new(),
            on_execute: None,
            section: None,
        }
    }

    /// 섹션 헤더
    pub fn heading(label: impl Into<String>) -> Self {
        Self {
            block_type: MultiBlockType::Heading,
            command_id: None,
            label: Some(label.into()),
            icon: None,
            tooltip: None,
            children: Vec::new(),
            on_execute: None,
            section: None,
        }
    }

    // Builder 메서드

    /// 레이블 설정 (커맨드 레이블 오버라이드)
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 아이콘 설정
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 툴팁 설정
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// 섹션 설정
    pub fn with_section(mut self, section: impl Into<String>) -> Self {
        self.section = Some(section.into());
        self
    }

    /// 자식 추가 (서브메뉴용)
    pub fn add_child(mut self, child: MultiBlockEntry) -> Self {
        self.children.push(child);
        self
    }

    /// 실행 콜백 설정 (커맨드 없이 직접 콜백)
    pub fn with_action(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_execute = Some(Box::new(f));
        self
    }
}

// ============================================================================
// MultiBoxBuilder
// ============================================================================

/// MultiBox 빌더 (UE5 FMultiBoxBuilder)
///
/// 커맨드 리스트와 엔트리 목록으로 메뉴/툴바를 생성합니다.
pub struct MultiBoxBuilder {
    entries: Vec<MultiBlockEntry>,
    command_list: Option<Arc<Mutex<UICommandList>>>,
}

impl MultiBoxBuilder {
    /// 새 빌더 생성
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            command_list: None,
        }
    }

    /// 커맨드 리스트 연결
    pub fn with_command_list(mut self, list: Arc<Mutex<UICommandList>>) -> Self {
        self.command_list = Some(list);
        self
    }

    /// 엔트리 추가
    pub fn add(mut self, entry: MultiBlockEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// 커맨드 ID로 버튼 추가 (편의 메서드)
    pub fn add_command(self, id: CommandId) -> Self {
        self.add(MultiBlockEntry::button(id))
    }

    /// 구분선 추가 (편의 메서드)
    pub fn add_separator(self) -> Self {
        self.add(MultiBlockEntry::separator())
    }

    /// 섹션 헤더 추가 (편의 메서드)
    pub fn add_heading(self, label: impl Into<String>) -> Self {
        self.add(MultiBlockEntry::heading(label))
    }

    /// 엔트리 목록 접근
    pub fn entries(&self) -> &[MultiBlockEntry] {
        &self.entries
    }

    /// 엔트리 목록 소유권 이동
    pub fn into_entries(self) -> Vec<MultiBlockEntry> {
        self.entries
    }

    /// MenuItem 목록으로 변환
    pub fn build_menu_items(&self) -> Vec<MenuItem> {
        self.resolve_menu_items(&self.entries)
    }

    /// 엔트리 → MenuItem 변환 (재귀)
    fn resolve_menu_items(&self, entries: &[MultiBlockEntry]) -> Vec<MenuItem> {
        let cmd_list = self.command_list.as_ref().and_then(|l| l.lock().ok());

        entries.iter().map(|entry| {
            match entry.block_type {
                MultiBlockType::Separator => MenuItem::separator(),
                MultiBlockType::Heading => {
                    MenuItem::header(entry.label.as_deref().unwrap_or(""))
                }
                MultiBlockType::SubMenu => {
                    let label = entry.label.as_deref().unwrap_or("Submenu");
                    let children = self.resolve_menu_items(&entry.children);
                    MenuItem::new(label).submenu(children)
                }
                MultiBlockType::Button | MultiBlockType::Toggle |
                MultiBlockType::Check | MultiBlockType::Radio |
                MultiBlockType::Widget => {
                    // 커맨드에서 정보 조회
                    let (label, shortcut) = if let (Some(cmd_id), Some(ref list)) =
                        (entry.command_id, &cmd_list)
                    {
                        if let Some((info, _)) = list.find_command(cmd_id) {
                            let lbl = entry.label.clone()
                                .unwrap_or_else(|| info.label.to_string());
                            let sc = info.default_chord.as_ref()
                                .map(|c| c.display_text());
                            (lbl, sc)
                        } else {
                            (entry.label.clone().unwrap_or_default(), None)
                        }
                    } else {
                        (entry.label.clone().unwrap_or_default(), None)
                    };

                    let mut item = MenuItem::new(label);
                    if let Some(sc) = shortcut {
                        item.shortcut = Some(sc);
                    }
                    if let Some(ref icon) = entry.icon {
                        item.icon = Some(icon.clone());
                    }

                    // 아이템 타입 매핑
                    match entry.block_type {
                        MultiBlockType::Check => {
                            item.item_type = MenuItemType::Check;
                        }
                        MultiBlockType::Radio => {
                            item.item_type = MenuItemType::Radio;
                        }
                        _ => {}
                    }

                    item
                }
            }
        }).collect()
    }
}

impl Default for MultiBoxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_basic() {
        let builder = MultiBoxBuilder::new()
            .add(MultiBlockEntry::heading("File"))
            .add_command(CommandId("file.new"))
            .add_command(CommandId("file.open"))
            .add_separator()
            .add_command(CommandId("file.save"));

        assert_eq!(builder.entries().len(), 5);
        assert_eq!(builder.entries()[0].block_type, MultiBlockType::Heading);
        assert_eq!(builder.entries()[1].block_type, MultiBlockType::Button);
        assert_eq!(builder.entries()[3].block_type, MultiBlockType::Separator);
    }

    #[test]
    fn test_submenu() {
        let entry = MultiBlockEntry::submenu("Edit")
            .add_child(MultiBlockEntry::button(CommandId("edit.undo")))
            .add_child(MultiBlockEntry::button(CommandId("edit.redo")));

        assert_eq!(entry.block_type, MultiBlockType::SubMenu);
        assert_eq!(entry.children.len(), 2);
        assert_eq!(entry.label.as_deref(), Some("Edit"));
    }

    #[test]
    fn test_entry_builder() {
        let entry = MultiBlockEntry::button(CommandId("test"))
            .with_label("Test Label")
            .with_icon("🔧")
            .with_tooltip("This is a test")
            .with_section("Tools");

        assert_eq!(entry.label.as_deref(), Some("Test Label"));
        assert_eq!(entry.icon.as_deref(), Some("🔧"));
        assert_eq!(entry.tooltip.as_deref(), Some("This is a test"));
        assert_eq!(entry.section.as_deref(), Some("Tools"));
    }

    #[test]
    fn test_build_menu_items_no_command_list() {
        let builder = MultiBoxBuilder::new()
            .add(MultiBlockEntry::heading("Section"))
            .add(MultiBlockEntry::button(CommandId("test")).with_label("Test"))
            .add_separator();

        let items = builder.build_menu_items();
        assert_eq!(items.len(), 3);
    }
}
