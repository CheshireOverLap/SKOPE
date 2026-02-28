//! Command Queue - 에디터 명령 큐
//!
//! UI 레이어에서 발생하는 명령을 안전하게 큐잉하고
//! 프레임 시작 시 순차적으로 처리합니다.

use std::collections::VecDeque;
use skope_ecs::Entity;
use crate::editor::EditorMode;

/// 에디터 명령
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum EditorCommand {
    /// 엔티티 선택
    SelectEntity(Option<Entity>),

    /// 플레이 모드 전환
    SetPlayMode(EditorMode),

    /// 씬 저장
    SaveScene,

    /// 씬 로드
    LoadScene(String),

    /// Undo
    Undo,

    /// Redo
    Redo,

    /// 선택된 엔티티 삭제
    DeleteSelected,

    /// 선택 복제
    Duplicate,

    /// 복사
    Copy,

    /// 잘라내기
    Cut,

    /// 붙여넣기
    Paste,
}

/// 명령 큐
///
/// FIFO 순서로 명령을 저장하고 처리합니다.
/// 프레임 시작 시 drain()으로 모든 명령을 가져와 처리합니다.
#[derive(Debug, Default)]
pub struct CommandQueue {
    commands: VecDeque<EditorCommand>,
}

#[allow(dead_code)]
impl CommandQueue {
    /// 새 CommandQueue 생성
    pub fn new() -> Self {
        Self {
            commands: VecDeque::new(),
        }
    }

    /// 명령 추가
    pub fn push(&mut self, cmd: EditorCommand) {
        self.commands.push_back(cmd);
    }

    /// 모든 명령 가져오기 (큐 비움)
    pub fn drain(&mut self) -> impl Iterator<Item = EditorCommand> + '_ {
        self.commands.drain(..)
    }

    /// 큐가 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// 큐에 있는 명령 수
    pub fn len(&self) -> usize {
        self.commands.len()
    }
}
