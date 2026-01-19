//! Command Queue - 에디터 명령 큐
//!
//! UI 레이어에서 발생하는 명령을 안전하게 큐잉하고
//! 프레임 시작 시 순차적으로 처리합니다.

use std::collections::VecDeque;
use bevy_ecs::entity::Entity;
use egui::ViewportId;

/// 뷰포트 액션 (플로팅 윈도우 제어)
#[derive(Debug, Clone)]
pub enum ViewportAction {
    /// 타이틀 변경
    SetTitle(String),
    /// 최소화
    Minimize,
    /// 최대화/복원
    Maximize,
    /// 위치 변경
    SetPosition(i32, i32),
    /// 크기 변경
    SetSize(u32, u32),
    /// 포커스
    Focus,
    /// 닫기
    Close,
}

/// 에디터 명령
#[derive(Debug, Clone)]
pub enum EditorCommand {
    /// 엔티티 선택
    SelectEntity(Option<Entity>),

    /// 뷰포트 명령
    Viewport(ViewportId, ViewportAction),

    /// 플레이 모드 전환
    SetPlayMode(crate::editor::EditorPlayState),

    /// 씬 저장
    SaveScene,

    /// 씬 로드
    LoadScene(String),

    /// Undo
    Undo,

    /// Redo
    Redo,
}

/// 명령 큐
///
/// FIFO 순서로 명령을 저장하고 처리합니다.
/// 프레임 시작 시 drain()으로 모든 명령을 가져와 처리합니다.
#[derive(Debug, Default)]
pub struct CommandQueue {
    commands: VecDeque<EditorCommand>,
}

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
