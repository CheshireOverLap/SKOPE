//! SKOPE Editor Module
//!
//! fyrox-ui + egui_dock 기반 에디터 구현
//!
//! 에디터 모듈은 개발 중이므로 dead_code 경고 허용

#![allow(dead_code)]

pub mod ui_renderer;
pub mod event_bridge;
pub mod scene_viewer;
pub mod gizmo;
pub mod selection;
pub mod command;
pub mod panels;
pub mod debug_viz;
pub mod spawn_menu;
pub mod clipboard;
pub mod docking;
pub mod ai_panel;
pub mod hierarchy_state;
pub mod i18n;
pub mod asset_browser;
pub mod inspector;
pub mod lua_inspector;
pub mod ui_editor;
pub mod ui_editor_window;
pub mod animation_timeline;

pub use docking::{FreeDockLayout, AiTabKind, EditorPlayState, MenuAction};
pub use animation_timeline::{AnimationTimelineState, TimelineAction};
pub use ui_editor_window::{UiEditorWindows, UiEditorWindow, CanvasResolution};
pub use ai_panel::AiPanelState;
pub use hierarchy_state::{HierarchyState, HierarchyAction};
pub use asset_browser::{AssetBrowserState, AssetBrowserAction};
pub use inspector::{InspectorState, InspectorAction};
pub use ui_editor::UiEditorState;
// i18n types: 외부 모듈에서 언어 설정 시 사용
#[allow(unused_imports)]
pub use i18n::{Language, TextKey, Translations};

/// 에디터 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorMode {
    /// 편집 모드 - EditorCamera 사용, Grid/Gizmo 표시
    #[default]
    Edit,
    /// 플레이 모드 - GameCamera 사용, Grid/Gizmo 숨김
    Play,
}

impl EditorMode {
    /// 모드 토글
    pub fn toggle(&mut self) {
        *self = match *self {
            EditorMode::Edit => EditorMode::Play,
            EditorMode::Play => EditorMode::Edit,
        };
    }

    /// 편집 모드인지
    pub fn is_edit(&self) -> bool {
        matches!(self, EditorMode::Edit)
    }

    /// 플레이 모드인지
    pub fn is_play(&self) -> bool {
        matches!(self, EditorMode::Play)
    }
}

// Legacy live_link (optional feature)
#[cfg(feature = "live_link")]
pub mod live_link;

use fyrox_ui::{
    UserInterface,
    UiNode,
    message::UiMessage,
};
use fyrox_core::pool::Handle;

use crate::editor::ui_renderer::FyroxUiRenderer;
use crate::editor::event_bridge::WinitEventBridge;

/// SKOPE 에디터 메인 구조체
pub struct Editor {
    /// fyrox-ui UserInterface
    pub ui: UserInterface,

    /// wgpu 기반 UI 렌더러
    pub renderer: FyroxUiRenderer,

    /// winit 이벤트 브릿지
    pub event_bridge: WinitEventBridge,

    /// 테스트용 버튼 핸들
    test_button: Handle<UiNode>,

    /// 테스트용 윈도우 핸들
    #[allow(dead_code)]
    test_window: Handle<UiNode>,
}

impl Editor {
    /// 새 에디터 인스턴스 생성
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        screen_size: (u32, u32),
    ) -> Self {
        // UI 인스턴스 생성
        let mut ui = UserInterface::new(fyrox_core::algebra::Vector2::new(
            screen_size.0 as f32,
            screen_size.1 as f32,
        ));

        // 테스트 UI 구성 (기본 숨김 - 필요시 F11로 토글)
        // 피드백: 시작 시 팝업이 뷰포트를 가리면 안됨
        let _ctx = &mut ui.build_ctx();

        // 테스트 버튼과 윈도우는 제거됨 (UX 피드백 반영)
        // - 시작 시 불필요한 UI 요소가 화면을 가리지 않도록
        // - 디버그 기능은 F11 또는 메뉴에서 접근하도록 변경 예정
        let test_button = Handle::NONE;
        let test_window = Handle::NONE;

        // wgpu 렌더러 생성
        let renderer = FyroxUiRenderer::new(device, queue, surface_format);

        // 이벤트 브릿지 생성
        let event_bridge = WinitEventBridge::new();

        Self {
            ui,
            renderer,
            event_bridge,
            test_button,
            test_window,
        }
    }

    /// 화면 크기 변경 처리
    pub fn resize(&mut self, width: u32, height: u32) {
        self.ui.set_screen_size(fyrox_core::algebra::Vector2::new(
            width as f32,
            height as f32,
        ));
    }

    /// 프레임 업데이트 (UI 갱신만, 메시지는 poll_messages로 별도 처리)
    pub fn update(&mut self, dt: f32) {
        // UI 업데이트
        self.ui.update(
            fyrox_core::algebra::Vector2::new(self.ui.screen_size().x, self.ui.screen_size().y),
            dt,
            &Default::default(),
        );
    }

    /// UI 메시지 폴링 - 외부에서 메시지 처리 가능
    pub fn poll_messages(&mut self) -> Vec<UiMessage> {
        let mut messages = Vec::new();
        while let Some(message) = self.ui.poll_message() {
            // 기본 Editor 메시지 처리 (테스트 버튼 등)
            self.handle_ui_message(&message);
            messages.push(message);
        }
        messages
    }

    /// UI 메시지 처리 (내부용)
    fn handle_ui_message(&mut self, message: &UiMessage) {
        // 버튼 클릭 처리
        if let Some(button_msg) = message.data::<fyrox_ui::button::ButtonMessage>() {
            if message.destination() == self.test_button {
                if let fyrox_ui::button::ButtonMessage::Click = button_msg {
                    log::info!("[Editor] Test button clicked!");
                }
            }
        }
    }

    /// UI 렌더링
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        screen_size: (u32, u32),
    ) {
        // wgpu로 렌더링 (update()에서 이미 draw() 호출됨)
        self.renderer.render(
            device,
            queue,
            encoder,
            target,
            &self.ui,
            screen_size,
        );
    }

    /// winit 이벤트 처리
    pub fn handle_window_event(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.event_bridge.translate_and_send(&mut self.ui, event)
    }
}
