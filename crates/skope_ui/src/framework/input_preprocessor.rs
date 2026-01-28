//! Input Preprocessor Pipeline
//!
//! UE의 EInputPreProcessorType 패턴: Priority 기반 입력 처리 파이프라인
//! Overlay → Engine → Editor → Game 순서로 입력 이벤트를 처리

use glam::Vec2;
use winit::event::{ElementState, MouseButton};
use winit::keyboard::KeyCode;

/// 입력 처리 결과
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputProcessResult {
    /// 이벤트가 처리되었음 (하위 프로세서에 전달하지 않음)
    Handled,
    /// 이벤트가 처리되지 않음 (하위 프로세서에 전달)
    Unhandled,
}

/// 입력 처리 우선순위 (낮은 값이 먼저 처리)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputPriority {
    /// 오버레이 (모달, 디버그 등) — 최고 우선순위
    Overlay = 0,
    /// 엔진 레벨 (핫키, 시스템 단축키)
    Engine = 100,
    /// 에디터 레벨 (패널 단축키, 도킹 시스템)
    Editor = 200,
    /// 게임 레벨 (인게임 입력)
    Game = 300,
}

/// 입력 전처리기 트레이트
pub trait InputPreProcessor: Send + 'static {
    /// 이 프로세서의 우선순위
    fn priority(&self) -> InputPriority;

    /// 고유 이름 (디버그 + 제거용)
    fn name(&self) -> &str;

    /// 키보드 이벤트 처리
    fn process_key_event(&mut self, _key: KeyCode, _state: ElementState) -> InputProcessResult {
        InputProcessResult::Unhandled
    }

    /// 마우스 버튼 이벤트 처리
    fn process_mouse_event(&mut self, _button: MouseButton, _state: ElementState, _pos: Vec2) -> InputProcessResult {
        InputProcessResult::Unhandled
    }

    /// 마우스 휠 이벤트 처리
    fn process_mouse_wheel(&mut self, _delta: f32) -> InputProcessResult {
        InputProcessResult::Unhandled
    }
}

/// 입력 파이프라인 — 우선순위 순으로 프로세서를 실행
pub struct InputPipeline {
    processors: Vec<Box<dyn InputPreProcessor>>,
}

impl InputPipeline {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
        }
    }

    /// 프로세서 추가 (우선순위 순 자동 정렬)
    pub fn add(&mut self, processor: Box<dyn InputPreProcessor>) {
        let priority = processor.priority();
        let pos = self.processors
            .iter()
            .position(|p| p.priority() > priority)
            .unwrap_or(self.processors.len());
        self.processors.insert(pos, processor);
    }

    /// 이름으로 프로세서 제거
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(pos) = self.processors.iter().position(|p| p.name() == name) {
            self.processors.remove(pos);
            true
        } else {
            false
        }
    }

    /// 키보드 이벤트를 파이프라인에 통과
    pub fn process_key(&mut self, key: KeyCode, state: ElementState) -> InputProcessResult {
        for processor in &mut self.processors {
            if processor.process_key_event(key, state) == InputProcessResult::Handled {
                return InputProcessResult::Handled;
            }
        }
        InputProcessResult::Unhandled
    }

    /// 마우스 버튼 이벤트를 파이프라인에 통과
    pub fn process_mouse(&mut self, button: MouseButton, state: ElementState, pos: Vec2) -> InputProcessResult {
        for processor in &mut self.processors {
            if processor.process_mouse_event(button, state, pos) == InputProcessResult::Handled {
                return InputProcessResult::Handled;
            }
        }
        InputProcessResult::Unhandled
    }

    /// 마우스 휠 이벤트를 파이프라인에 통과
    pub fn process_wheel(&mut self, delta: f32) -> InputProcessResult {
        for processor in &mut self.processors {
            if processor.process_mouse_wheel(delta) == InputProcessResult::Handled {
                return InputProcessResult::Handled;
            }
        }
        InputProcessResult::Unhandled
    }
}

impl Default for InputPipeline {
    fn default() -> Self {
        Self::new()
    }
}
