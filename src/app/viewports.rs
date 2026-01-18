//! Viewport Registry for Multi-Window Support
//!
//! OS 네이티브 플로팅 윈도우 관리를 위한 ViewportRegistry

use std::collections::HashMap;
use std::sync::Arc;
use egui::ViewportId;
use winit::window::{Window, WindowId};
use crate::editor::docking::Tab;

/// 플로팅 윈도우 데이터
pub struct ViewportData {
    /// egui ViewportId
    pub viewport_id: ViewportId,
    /// winit Window (OS 네이티브 윈도우)
    pub window: Arc<Window>,
    /// wgpu Surface (렌더링 타겟)
    pub surface: wgpu::Surface<'static>,
    /// Surface 설정
    pub config: wgpu::SurfaceConfiguration,
    /// egui_winit State (이벤트 처리)
    pub egui_state: egui_winit::State,
    /// 윈도우 크기 (physical pixels)
    pub size: (u32, u32),
    /// 이 윈도우에 표시되는 탭
    pub tab: Tab,
    /// 윈도우 제목
    pub title: String,
    /// 첫 렌더링이 필요한지 (흰 화면 방지용)
    pub needs_initial_render: bool,
}

impl ViewportData {
    /// 새 ViewportData 생성
    pub fn new(
        viewport_id: ViewportId,
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        egui_state: egui_winit::State,
        tab: Tab,
    ) -> Self {
        let size = window.inner_size();
        let title = format!("SKOPE - {}", tab.title());

        Self {
            viewport_id,
            window,
            surface,
            config,
            egui_state,
            size: (size.width, size.height),
            tab,
            title,
            needs_initial_render: true, // 생성 시 첫 렌더링 필요
        }
    }

    /// Surface 리사이즈
    pub fn resize(&mut self, device: &wgpu::Device, new_size: (u32, u32)) {
        if new_size.0 > 0 && new_size.1 > 0 {
            self.size = new_size;
            self.config.width = new_size.0;
            self.config.height = new_size.1;
            self.surface.configure(device, &self.config);
        }
    }
}

/// 플로팅 윈도우 레지스트리
pub struct ViewportRegistry {
    /// ViewportId → ViewportData 매핑
    pub viewports: HashMap<ViewportId, ViewportData>,
    /// WindowId → ViewportId 매핑 (이벤트 라우팅용)
    pub window_to_viewport: HashMap<WindowId, ViewportId>,
    /// 닫힌 윈도우 큐 (다음 프레임에서 정리)
    pub pending_close: Vec<ViewportId>,
    /// 뷰포트 ID 카운터 (고유 ID 생성용)
    viewport_counter: u64,
}

impl ViewportRegistry {
    /// 새 레지스트리 생성
    pub fn new() -> Self {
        Self {
            viewports: HashMap::new(),
            window_to_viewport: HashMap::new(),
            pending_close: Vec::new(),
            viewport_counter: 1, // 0은 ROOT 예약
        }
    }

    /// 다음 고유 ViewportId 생성
    pub fn next_viewport_id(&mut self) -> ViewportId {
        let id = self.viewport_counter;
        self.viewport_counter += 1;
        ViewportId::from_hash_of(format!("floating_viewport_{}", id))
    }

    /// 플로팅 윈도우 등록
    pub fn register(&mut self, data: ViewportData) {
        let window_id = data.window.id();
        let viewport_id = data.viewport_id;

        log::info!(
            "[ViewportRegistry] Registered floating window: {:?} -> {:?} (tab: {:?})",
            window_id,
            viewport_id,
            data.tab
        );

        self.window_to_viewport.insert(window_id, viewport_id);
        self.viewports.insert(viewport_id, data);
    }

    /// WindowId로 ViewportId 조회
    pub fn get_viewport_id(&self, window_id: WindowId) -> Option<ViewportId> {
        self.window_to_viewport.get(&window_id).copied()
    }

    /// ViewportId로 ViewportData 조회
    pub fn get(&self, viewport_id: ViewportId) -> Option<&ViewportData> {
        self.viewports.get(&viewport_id)
    }

    /// ViewportId로 ViewportData 가변 조회
    pub fn get_mut(&mut self, viewport_id: ViewportId) -> Option<&mut ViewportData> {
        self.viewports.get_mut(&viewport_id)
    }

    /// WindowId로 ViewportData 조회
    pub fn get_by_window(&self, window_id: WindowId) -> Option<&ViewportData> {
        self.window_to_viewport
            .get(&window_id)
            .and_then(|vid| self.viewports.get(vid))
    }

    /// WindowId로 ViewportData 가변 조회
    pub fn get_mut_by_window(&mut self, window_id: WindowId) -> Option<&mut ViewportData> {
        if let Some(vid) = self.window_to_viewport.get(&window_id).copied() {
            self.viewports.get_mut(&vid)
        } else {
            None
        }
    }

    /// 플로팅 윈도우 닫기 예약
    pub fn schedule_close(&mut self, viewport_id: ViewportId) {
        if !self.pending_close.contains(&viewport_id) {
            self.pending_close.push(viewport_id);
            log::info!("[ViewportRegistry] Scheduled close for viewport: {:?}", viewport_id);
        }
    }

    /// WindowId로 닫기 예약
    pub fn schedule_close_by_window(&mut self, window_id: WindowId) {
        if let Some(viewport_id) = self.window_to_viewport.get(&window_id).copied() {
            self.schedule_close(viewport_id);
        }
    }

    /// 예약된 닫기 처리 및 닫힌 탭들 반환 (메인으로 복귀용)
    pub fn process_pending_closes(&mut self) -> Vec<Tab> {
        let mut closed_tabs = Vec::new();

        for viewport_id in self.pending_close.drain(..) {
            if let Some(data) = self.viewports.remove(&viewport_id) {
                let window_id = data.window.id();
                self.window_to_viewport.remove(&window_id);
                closed_tabs.push(data.tab);

                log::info!(
                    "[ViewportRegistry] Closed floating window: {:?} (tab: {:?})",
                    viewport_id,
                    data.tab
                );
            }
        }

        closed_tabs
    }

    /// 특정 탭이 플로팅 중인지 확인
    pub fn is_tab_floating(&self, tab: Tab) -> bool {
        self.viewports.values().any(|v| v.tab == tab)
    }

    /// 특정 탭의 ViewportId 조회
    pub fn get_viewport_for_tab(&self, tab: Tab) -> Option<ViewportId> {
        self.viewports
            .iter()
            .find(|(_, v)| v.tab == tab)
            .map(|(id, _)| *id)
    }

    /// 모든 플로팅 탭 목록
    pub fn floating_tabs(&self) -> Vec<Tab> {
        self.viewports.values().map(|v| v.tab).collect()
    }

    /// 플로팅 윈도우 개수
    pub fn count(&self) -> usize {
        self.viewports.len()
    }

    /// 모든 WindowId 반환 (이벤트 루프용)
    pub fn all_window_ids(&self) -> Vec<WindowId> {
        self.window_to_viewport.keys().copied().collect()
    }
}

impl Default for ViewportRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 플로팅 윈도우 생성 요청 (비동기 생성을 위한 중간 구조체)
pub struct FloatingWindowRequest {
    /// 분리할 탭
    pub tab: Tab,
    /// 초기 위치 (화면 좌표)
    pub position: Option<(i32, i32)>,
    /// 초기 크기
    pub size: (u32, u32),
}

impl FloatingWindowRequest {
    pub fn new(tab: Tab) -> Self {
        Self {
            tab,
            position: None,
            size: (400, 300),
        }
    }

    pub fn with_position(mut self, x: i32, y: i32) -> Self {
        self.position = Some((x, y));
        self
    }

    pub fn with_size(mut self, width: u32, height: u32) -> Self {
        self.size = (width, height);
        self
    }
}
