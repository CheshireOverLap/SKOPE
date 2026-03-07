//! TextUndoManager — 텍스트 편집 실행 취소/다시 실행 관리자
//!
//! UE5.7 FTextEditHelper / FUndoState 패턴 매칭.
//! 텍스트 편집 위젯에 내장하여 Ctrl+Z/Y 지원.

use std::collections::VecDeque;

/// 실행 취소 스냅샷 — UE5.7 FUndoState
#[derive(Debug, Clone)]
struct UndoState {
    /// 텍스트 내용
    text: String,
    /// 커서 위치 (바이트 인덱스)
    cursor_pos: usize,
    /// 선택 시작 (None이면 선택 없음)
    selection_start: Option<usize>,
}

/// 텍스트 편집 Undo/Redo 관리자 — UE5.7 FTextEditHelper
///
/// 스냅샷 기반 undo/redo 스택.
/// 텍스트 변경 전 `save_state()`를 호출하고,
/// Ctrl+Z로 `undo()`, Ctrl+Y로 `redo()`를 호출합니다.
pub struct TextUndoManager {
    /// Undo 스택 (과거 상태) — VecDeque로 O(1) pop_front
    undo_stack: VecDeque<UndoState>,
    /// Redo 스택 (미래 상태)
    redo_stack: Vec<UndoState>,
    /// 최대 기록 수
    max_history: usize,
}

impl TextUndoManager {
    /// 새 관리자 생성 (최대 100 기록)
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            max_history: 100,
        }
    }

    /// 현재 상태 저장 — 텍스트 변경 전 호출
    ///
    /// Redo 스택은 새 편집이 시작되면 비워집니다.
    pub fn save_state(&mut self, text: &str, cursor_pos: usize, selection_start: Option<usize>) {
        // 중복 저장 방지
        if let Some(last) = self.undo_stack.back() {
            if last.text == text && last.cursor_pos == cursor_pos {
                return;
            }
        }

        self.undo_stack.push_back(UndoState {
            text: text.to_string(),
            cursor_pos,
            selection_start,
        });

        // 최대 기록 수 초과 시 오래된 것 제거 — O(1) pop_front
        if self.undo_stack.len() > self.max_history {
            self.undo_stack.pop_front();
        }

        // 새 편집 → Redo 스택 비우기
        self.redo_stack.clear();
    }

    /// Undo — 이전 상태로 복원
    ///
    /// 현재 상태를 Redo 스택에 저장하고, Undo 스택에서 꺼냅니다.
    /// 반환: (텍스트, 커서 위치, 선택 시작) 또는 None
    pub fn undo(&mut self, current_text: &str, current_cursor: usize, current_selection: Option<usize>)
        -> Option<(String, usize, Option<usize>)>
    {
        let state = self.undo_stack.pop_back()?;

        // 현재 상태를 Redo에 저장
        self.redo_stack.push(UndoState {
            text: current_text.to_string(),
            cursor_pos: current_cursor,
            selection_start: current_selection,
        });

        Some((state.text, state.cursor_pos, state.selection_start))
    }

    /// Redo — 실행 취소한 상태를 다시 적용
    ///
    /// 현재 상태를 Undo 스택에 저장하고, Redo 스택에서 꺼냅니다.
    pub fn redo(&mut self, current_text: &str, current_cursor: usize, current_selection: Option<usize>)
        -> Option<(String, usize, Option<usize>)>
    {
        let state = self.redo_stack.pop()?;

        // 현재 상태를 Undo에 저장
        self.undo_stack.push_back(UndoState {
            text: current_text.to_string(),
            cursor_pos: current_cursor,
            selection_start: current_selection,
        });

        Some((state.text, state.cursor_pos, state.selection_start))
    }

    /// Undo 가능 여부
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Redo 가능 여부
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// 기록 초기화
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for TextUndoManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Clipboard — 플랫폼 클립보드 인터페이스
// ============================================================================

/// 클립보드 인터페이스 — UE5.7 FPlatformApplicationMisc::ClipboardCopy/Paste
///
/// 현재는 내부 버퍼로 동작하며, 추후 OS 클립보드 연동 가능.
pub struct Clipboard {
    /// 내부 클립보드 버퍼
    buffer: String,
}

impl Clipboard {
    /// 새 클립보드
    pub fn new() -> Self {
        Self { buffer: String::new() }
    }

    /// 텍스트 복사 — UE5.7 ClipboardCopy
    pub fn copy(&mut self, text: &str) {
        self.buffer = text.to_string();
    }

    /// 텍스트 붙여넣기 — UE5.7 ClipboardPaste
    pub fn paste(&self) -> &str {
        &self.buffer
    }

    /// 클립보드에 내용이 있는지
    pub fn has_content(&self) -> bool {
        !self.buffer.is_empty()
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

/// 전역 클립보드 접근 (thread-safe)
///
/// UE5.7 FPlatformApplicationMisc 패턴.
/// 추후 OS 클립보드 통합 시 이 함수만 수정하면 됩니다.
pub fn clipboard() -> &'static std::sync::Mutex<Clipboard> {
    use std::sync::{Mutex, OnceLock};
    static CLIPBOARD: OnceLock<Mutex<Clipboard>> = OnceLock::new();
    CLIPBOARD.get_or_init(|| Mutex::new(Clipboard::new()))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_redo() {
        let mut mgr = TextUndoManager::new();

        // 초기 상태 저장
        mgr.save_state("Hello", 5, None);
        mgr.save_state("Hello World", 11, None);

        // Undo
        let result = mgr.undo("Hello World!", 12, None);
        assert!(result.is_some());
        let (text, cursor, _) = result.unwrap();
        assert_eq!(text, "Hello World");
        assert_eq!(cursor, 11);

        // Redo
        let result = mgr.redo(&text, cursor, None);
        assert!(result.is_some());
        let (text2, cursor2, _) = result.unwrap();
        assert_eq!(text2, "Hello World!");
        assert_eq!(cursor2, 12);
    }

    #[test]
    fn test_undo_clears_redo() {
        let mut mgr = TextUndoManager::new();

        mgr.save_state("A", 1, None);
        mgr.save_state("AB", 2, None);

        // Undo
        let _ = mgr.undo("ABC", 3, None);
        assert!(mgr.can_redo());

        // 새 편집 → Redo 소멸
        mgr.save_state("AX", 2, None);
        assert!(!mgr.can_redo());
    }

    #[test]
    fn test_clipboard() {
        let cb = clipboard();
        {
            let mut c = cb.lock().unwrap();
            c.copy("test data");
        }
        {
            let c = cb.lock().unwrap();
            assert_eq!(c.paste(), "test data");
        }
    }
}
