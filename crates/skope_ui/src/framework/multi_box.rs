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
// MultiBoxExtender — 플러그인 확장 포인트
// ============================================================================

/// 멀티박스 확장 인터페이스 (UE5 FMultiBoxExtender)
///
/// named hook 으로 메뉴/툴바에 항목을 주입합니다.
/// 여러 익스텐더가 등록되면 `priority` 순으로 적용됩니다.
pub trait MultiBoxExtender: Send + Sync {
    /// 확장 대상 hook 이름 (예: "MainMenu.File", "Toolbar.Build")
    fn hook_name(&self) -> &str;

    /// 우선순위 (높을수록 먼저 적용, 기본 0)
    fn priority(&self) -> i32 { 0 }

    /// hook 위치에 주입할 엔트리 목록 반환
    fn extend(&self, existing: &[MultiBlockEntry]) -> Vec<MultiBlockEntry>;
}

// ============================================================================
// MultiBoxCustomization — 사용자 정의 툴바 레이아웃
// ============================================================================

/// 블록 가시성 오버라이드
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockVisibility {
    /// 기본 가시성 사용
    Default,
    /// 강제 표시
    Visible,
    /// 강제 숨김
    Hidden,
}

impl Default for BlockVisibility {
    fn default() -> Self { Self::Default }
}

/// 개별 블록 커스터마이징 규칙
#[derive(Debug, Clone)]
pub struct BlockCustomization {
    /// 대상 커맨드 ID (None이면 인덱스 기반)
    pub command_id: Option<CommandId>,
    /// 가시성
    pub visibility: BlockVisibility,
    /// 순서 가중치 (낮을수록 앞)
    pub order: i32,
}

/// 멀티박스 커스터마이징 (UE5 FMultiBoxCustomization)
///
/// 사용자 레이아웃 설정 (블록 숨기기, 순서 변경 등)을 저장합니다.
/// `MultiBoxBuilder::apply_customization()`으로 적용합니다.
#[derive(Debug, Clone, Default)]
pub struct MultiBoxCustomization {
    /// 커스터마이징 프로필 이름
    pub profile_name: String,
    /// 블록별 규칙
    rules: Vec<BlockCustomization>,
}

impl MultiBoxCustomization {
    pub fn new(profile_name: impl Into<String>) -> Self {
        Self {
            profile_name: profile_name.into(),
            rules: Vec::new(),
        }
    }

    /// 커맨드 ID에 대한 가시성 설정
    pub fn set_visibility(&mut self, cmd: CommandId, vis: BlockVisibility) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.command_id == Some(cmd)) {
            rule.visibility = vis;
        } else {
            self.rules.push(BlockCustomization {
                command_id: Some(cmd),
                visibility: vis,
                order: 0,
            });
        }
    }

    /// 커맨드 ID에 대한 순서 설정
    pub fn set_order(&mut self, cmd: CommandId, order: i32) {
        if let Some(rule) = self.rules.iter_mut().find(|r| r.command_id == Some(cmd)) {
            rule.order = order;
        } else {
            self.rules.push(BlockCustomization {
                command_id: Some(cmd),
                visibility: BlockVisibility::Default,
                order,
            });
        }
    }

    /// 규칙 조회
    pub fn get_rule(&self, cmd: CommandId) -> Option<&BlockCustomization> {
        self.rules.iter().find(|r| r.command_id == Some(cmd))
    }

    /// 규칙 목록 접근
    pub fn rules(&self) -> &[BlockCustomization] {
        &self.rules
    }

    /// 엔트리 목록에 커스터마이징 적용 (필터링 + 재정렬)
    pub fn apply(&self, entries: &mut Vec<MultiBlockEntry>) {
        // 가시성 필터링
        entries.retain(|entry| {
            if let Some(cmd_id) = entry.command_id {
                if let Some(rule) = self.get_rule(cmd_id) {
                    return rule.visibility != BlockVisibility::Hidden;
                }
            }
            true
        });

        // 순서 재정렬 (stable sort — 규칙 없는 항목은 원래 순서 유지)
        let rules = &self.rules;
        entries.sort_by(|a, b| {
            let order_a = a.command_id
                .and_then(|id| rules.iter().find(|r| r.command_id == Some(id)))
                .map_or(0, |r| r.order);
            let order_b = b.command_id
                .and_then(|id| rules.iter().find(|r| r.command_id == Some(id)))
                .map_or(0, |r| r.order);
            order_a.cmp(&order_b)
        });
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

    /// 익스텐더 적용 — hook_name이 일치하는 엔트리를 주입
    pub fn apply_extender(&mut self, extender: &dyn MultiBoxExtender, hook_name: &str) {
        if extender.hook_name() == hook_name {
            let new_entries = extender.extend(&self.entries);
            self.entries.extend(new_entries);
        }
    }

    /// 여러 익스텐더를 우선순위 순으로 적용
    pub fn apply_extenders(&mut self, extenders: &[&dyn MultiBoxExtender], hook_name: &str) {
        let mut sorted: Vec<&dyn MultiBoxExtender> = extenders
            .iter()
            .filter(|e| e.hook_name() == hook_name)
            .copied()
            .collect();
        sorted.sort_by(|a, b| b.priority().cmp(&a.priority()));
        for ext in sorted {
            let new_entries = ext.extend(&self.entries);
            self.entries.extend(new_entries);
        }
    }

    /// 커스터마이징 적용 (필터링 + 재정렬)
    pub fn apply_customization(&mut self, customization: &MultiBoxCustomization) {
        customization.apply(&mut self.entries);
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

    #[test]
    fn test_customization_visibility() {
        let mut custom = MultiBoxCustomization::new("test");
        custom.set_visibility(CommandId("file.save"), BlockVisibility::Hidden);

        let mut entries = vec![
            MultiBlockEntry::button(CommandId("file.new")).with_label("New"),
            MultiBlockEntry::button(CommandId("file.save")).with_label("Save"),
            MultiBlockEntry::button(CommandId("file.open")).with_label("Open"),
        ];

        custom.apply(&mut entries);
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.command_id != Some(CommandId("file.save"))));
    }

    #[test]
    fn test_customization_order() {
        let mut custom = MultiBoxCustomization::new("test");
        custom.set_order(CommandId("c"), -10); // 앞으로
        custom.set_order(CommandId("a"), 10);  // 뒤로

        let mut entries = vec![
            MultiBlockEntry::button(CommandId("a")).with_label("A"),
            MultiBlockEntry::button(CommandId("b")).with_label("B"),
            MultiBlockEntry::button(CommandId("c")).with_label("C"),
        ];

        custom.apply(&mut entries);
        assert_eq!(entries[0].command_id, Some(CommandId("c")));
        assert_eq!(entries[2].command_id, Some(CommandId("a")));
    }

    struct TestExtender {
        hook: String,
        priority: i32,
        label: String,
    }

    impl MultiBoxExtender for TestExtender {
        fn hook_name(&self) -> &str { &self.hook }
        fn priority(&self) -> i32 { self.priority }
        fn extend(&self, _existing: &[MultiBlockEntry]) -> Vec<MultiBlockEntry> {
            vec![MultiBlockEntry::button(CommandId("ext")).with_label(&self.label)]
        }
    }

    #[test]
    fn test_extender_apply() {
        let ext = TestExtender {
            hook: "Toolbar.Main".to_string(),
            priority: 0,
            label: "Extended".to_string(),
        };

        let mut builder = MultiBoxBuilder::new()
            .add_command(CommandId("file.new"));
        builder.apply_extender(&ext, "Toolbar.Main");
        assert_eq!(builder.entries().len(), 2);
    }

    #[test]
    fn test_extender_wrong_hook() {
        let ext = TestExtender {
            hook: "Toolbar.Other".to_string(),
            priority: 0,
            label: "Extended".to_string(),
        };

        let mut builder = MultiBoxBuilder::new()
            .add_command(CommandId("file.new"));
        builder.apply_extender(&ext, "Toolbar.Main");
        // hook 불일치 → 변경 없음
        assert_eq!(builder.entries().len(), 1);
    }

    #[test]
    fn test_block_visibility_default() {
        assert_eq!(BlockVisibility::default(), BlockVisibility::Default);
    }
}
