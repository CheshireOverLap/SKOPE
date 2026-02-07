//! GenericCommands — 표준 편집 커맨드 프리셋
//!
//! UE 참조: `FGenericCommands` (Cut/Copy/Paste/Undo/Redo/SelectAll/Delete)
//!
//! 에디터 전역에서 공통으로 사용되는 표준 커맨드 정의.
//! 실제 실행 로직은 각 위젯/컨텍스트에서 `UIAction`으로 바인딩합니다.

use crate::event::KeyCode;
use super::{
    CommandId, UICommandInfo, InputChord, UIActionType,
    UIActionRepeatMode, SlateIcon, CONTEXT_GLOBAL,
};

/// 표준 커맨드 ID 상수
pub mod commands {
    use super::CommandId;

    pub const CUT: CommandId = CommandId("GenericCommands.Cut");
    pub const COPY: CommandId = CommandId("GenericCommands.Copy");
    pub const PASTE: CommandId = CommandId("GenericCommands.Paste");
    pub const UNDO: CommandId = CommandId("GenericCommands.Undo");
    pub const REDO: CommandId = CommandId("GenericCommands.Redo");
    pub const SELECT_ALL: CommandId = CommandId("GenericCommands.SelectAll");
    pub const DELETE: CommandId = CommandId("GenericCommands.Delete");
    pub const DUPLICATE: CommandId = CommandId("GenericCommands.Duplicate");
    pub const FIND: CommandId = CommandId("GenericCommands.Find");
    pub const SAVE: CommandId = CommandId("GenericCommands.Save");
}

/// 표준 편집 커맨드 프리셋 생성기
///
/// `UICommandInfo`만 제공하며, 실제 `UIAction`은 컨텍스트별로 바인딩합니다.
pub struct GenericCommands;

impl GenericCommands {
    /// Cut (Ctrl+X)
    pub fn cut() -> UICommandInfo {
        UICommandInfo {
            id: commands::CUT,
            label: "Cut",
            description: "Cut the selection to clipboard",
            default_chord: Some(InputChord::new(KeyCode::X).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Cut")),
        }
    }

    /// Copy (Ctrl+C)
    pub fn copy() -> UICommandInfo {
        UICommandInfo {
            id: commands::COPY,
            label: "Copy",
            description: "Copy the selection to clipboard",
            default_chord: Some(InputChord::new(KeyCode::C).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Copy")),
        }
    }

    /// Paste (Ctrl+V)
    pub fn paste() -> UICommandInfo {
        UICommandInfo {
            id: commands::PASTE,
            label: "Paste",
            description: "Paste from clipboard",
            default_chord: Some(InputChord::new(KeyCode::V).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Paste")),
        }
    }

    /// Undo (Ctrl+Z)
    pub fn undo() -> UICommandInfo {
        UICommandInfo {
            id: commands::UNDO,
            label: "Undo",
            description: "Undo the last action",
            default_chord: Some(InputChord::new(KeyCode::Z).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatEnabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Undo")),
        }
    }

    /// Redo (Ctrl+Y / Ctrl+Shift+Z)
    pub fn redo() -> UICommandInfo {
        UICommandInfo {
            id: commands::REDO,
            label: "Redo",
            description: "Redo the last undone action",
            default_chord: Some(InputChord::new(KeyCode::Y).ctrl()),
            secondary_chord: Some(InputChord::new(KeyCode::Z).ctrl().shift()),
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatEnabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Redo")),
        }
    }

    /// Select All (Ctrl+A)
    pub fn select_all() -> UICommandInfo {
        UICommandInfo {
            id: commands::SELECT_ALL,
            label: "Select All",
            description: "Select all items",
            default_chord: Some(InputChord::new(KeyCode::A).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: None,
        }
    }

    /// Delete (Del)
    pub fn delete() -> UICommandInfo {
        UICommandInfo {
            id: commands::DELETE,
            label: "Delete",
            description: "Delete the selection",
            default_chord: Some(InputChord::new(KeyCode::Delete)),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatEnabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Delete")),
        }
    }

    /// Duplicate (Ctrl+D)
    pub fn duplicate() -> UICommandInfo {
        UICommandInfo {
            id: commands::DUPLICATE,
            label: "Duplicate",
            description: "Duplicate the selection",
            default_chord: Some(InputChord::new(KeyCode::D).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: None,
        }
    }

    /// Find (Ctrl+F)
    pub fn find() -> UICommandInfo {
        UICommandInfo {
            id: commands::FIND,
            label: "Find",
            description: "Open search",
            default_chord: Some(InputChord::new(KeyCode::F).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Find")),
        }
    }

    /// Save (Ctrl+S)
    pub fn save() -> UICommandInfo {
        UICommandInfo {
            id: commands::SAVE,
            label: "Save",
            description: "Save the current file",
            default_chord: Some(InputChord::new(KeyCode::S).ctrl()),
            secondary_chord: None,
            binding_context: CONTEXT_GLOBAL,
            action_type: UIActionType::Button,
            repeat_mode: UIActionRepeatMode::RepeatDisabled,
            icon: Some(SlateIcon::new("EditorStyle", "Icons.Save")),
        }
    }

    /// 모든 표준 커맨드 정보 반환
    pub fn all() -> Vec<UICommandInfo> {
        vec![
            Self::cut(), Self::copy(), Self::paste(),
            Self::undo(), Self::redo(),
            Self::select_all(), Self::delete(),
            Self::duplicate(), Self::find(), Self::save(),
        ]
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_commands_count() {
        let all = GenericCommands::all();
        assert_eq!(all.len(), 10);
    }

    #[test]
    fn test_generic_commands_ids() {
        assert_eq!(commands::CUT.0, "GenericCommands.Cut");
        assert_eq!(commands::SAVE.0, "GenericCommands.Save");
    }

    #[test]
    fn test_cut_has_ctrl_x() {
        let cut = GenericCommands::cut();
        let chord = cut.default_chord.unwrap();
        assert_eq!(chord.key, KeyCode::X);
        assert!(chord.ctrl);
        assert!(!chord.shift);
        assert!(!chord.alt);
        assert_eq!(chord.display_text(), "Ctrl+X");
    }

    #[test]
    fn test_redo_has_secondary_chord() {
        let redo = GenericCommands::redo();
        assert!(redo.secondary_chord.is_some());
        let secondary = redo.secondary_chord.unwrap();
        assert_eq!(secondary.key, KeyCode::Z);
        assert!(secondary.ctrl);
        assert!(secondary.shift);
    }

    #[test]
    fn test_undo_repeat_enabled() {
        let undo = GenericCommands::undo();
        assert_eq!(undo.repeat_mode, UIActionRepeatMode::RepeatEnabled);
    }

    #[test]
    fn test_save_has_icon() {
        let save = GenericCommands::save();
        assert!(save.icon.is_some());
        let icon = save.icon.unwrap();
        assert_eq!(icon.brush_name, "Icons.Save");
    }

    #[test]
    fn test_conflict_detection() {
        use super::super::{UIAction, UICommandList, InputBindingManager};

        let mut list = UICommandList::new();

        // 같은 Ctrl+S를 사용하는 두 커맨드 등록
        let save1 = UICommandInfo::simple(
            CommandId("Test.Save1"), "Save1",
            Some(InputChord::new(KeyCode::S).ctrl()),
        );
        let save2 = UICommandInfo::simple(
            CommandId("Test.Save2"), "Save2",
            Some(InputChord::new(KeyCode::S).ctrl()),
        );

        list.map_action(save1, UIAction::simple(|| {}));
        list.map_action(save2, UIAction::simple(|| {}));

        let mgr = InputBindingManager::instance().lock().unwrap();
        let conflicts = mgr.detect_conflicts(&list);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].commands.len(), 2);
    }

    #[test]
    fn test_json_round_trip() {
        // 새 인스턴스 생성 (싱글톤 말고)
        let mut mgr = super::super::InputBindingManager_test_new();

        mgr.set_override(commands::SAVE, InputChord::new(KeyCode::S).ctrl().shift());
        mgr.set_override(commands::FIND, InputChord::new(KeyCode::G).ctrl());

        let json = mgr.save_bindings_json();
        assert!(!json.is_empty());

        let mut mgr2 = super::super::InputBindingManager_test_new();
        let loaded = mgr2.load_bindings_json(&json);
        assert_eq!(loaded, 2);
    }
}
