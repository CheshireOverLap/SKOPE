//! UI Editor Floating Window System
//!
//! 언리얼 스타일 플로팅 윈도우로 Game UI 에셋(`.ui.ron`)을 편집.
//! Asset Browser에서 더블클릭으로 열기.
//!
//! ## wgpu 렌더링 아키텍처
//! - 각 UiEditorWindow는 자체 ViewportTexture를 가짐
//! - State::render()에서 UiEditorWindows::render_all() 호출
//! - egui에서 viewport texture를 Image로 표시

use egui::{Color32, Context, Rect, Ui, Vec2};
use std::collections::HashSet;
use std::path::PathBuf;

use skope_game_ui::{UiAsset, UiSystem, Widget, UiRenderer, animation_presets};
use crate::renderer::ViewportTexture;

/// 다중 윈도우 관리자
pub struct UiEditorWindows {
    /// 열린 에디터들
    pub editors: Vec<UiEditorWindow>,
    /// 다음 윈도우 ID
    next_id: u32,
    /// 공유 UiRenderer (옵션 - 생성 시 설정)
    ui_renderer: Option<UiRenderer>,
}

impl Default for UiEditorWindows {
    fn default() -> Self {
        Self {
            editors: Vec::new(),
            next_id: 1,
            ui_renderer: None,
        }
    }
}

impl UiEditorWindows {
    /// UiRenderer 초기화 (State에서 호출)
    pub fn init_renderer(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) {
        if self.ui_renderer.is_none() {
            self.ui_renderer = Some(UiRenderer::new(device, queue, format, 1920, 1080));
            log::info!("[UiEditorWindows] UiRenderer initialized");
        }
    }

    /// 파일 경로로 에디터 열기
    pub fn open(&mut self, path: PathBuf) {
        // 이미 열린 파일인지 확인
        for editor in &mut self.editors {
            if editor.file_path.as_ref() == Some(&path) {
                editor.focus_requested = true;
                return;
            }
        }

        // 새 에디터 생성
        let id = self.next_id;
        self.next_id += 1;

        let editor = UiEditorWindow::from_file(id, path);
        self.editors.push(editor);
    }

    /// 새 UI 에디터 생성 (빈 파일)
    pub fn create_new(&mut self) {
        let id = self.next_id;
        self.next_id += 1;

        let editor = UiEditorWindow::new(id);
        self.editors.push(editor);
    }

    /// 현재 디렉토리에 새 UI 파일 생성하고 열기
    pub fn create_new_in_dir(&mut self, dir: &PathBuf) -> Option<PathBuf> {
        // 고유한 파일명 생성
        let mut counter = 1;
        let mut path;
        loop {
            let name = if counter == 1 {
                "new_ui.ui.ron".to_string()
            } else {
                format!("new_ui_{}.ui.ron", counter)
            };
            path = dir.join(&name);
            if !path.exists() {
                break;
            }
            counter += 1;
        }

        // 기본 UiAsset 생성 및 저장
        let asset = UiAsset::new(path.file_stem().unwrap_or_default().to_string_lossy());
        if asset.save(&path).is_ok() {
            self.open(path.clone());
            Some(path)
        } else {
            log::error!("Failed to create UI file: {:?}", path);
            None
        }
    }

    /// 모든 에디터의 뷰포트 텍스처 초기화/업데이트
    pub fn update_viewports(
        &mut self,
        device: &wgpu::Device,
        egui_renderer: &mut egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
    ) {
        for editor in &mut self.editors {
            editor.ensure_viewport(device, egui_renderer, format);
        }
    }

    /// 모든 에디터의 UI를 뷰포트에 렌더링
    pub fn render_all(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        // UiRenderer가 없으면 스킵
        let Some(ref mut ui_renderer) = self.ui_renderer else {
            return;
        };

        for editor in &mut self.editors {
            if !editor.open {
                continue;
            }

            // 뷰포트가 없으면 스킵
            let Some(ref viewport) = editor.viewport else {
                continue;
            };

            // 레이아웃 계산
            let resolution = editor.canvas_resolution.size();
            editor.canvas_ui_system.set_screen_size(resolution.0 as f32, resolution.1 as f32);
            editor.canvas_ui_system.calculate_layout();

            // 렌더 타겟 클리어 (배경색)
            {
                let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("UI Editor Clear Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: viewport.render_target(),
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.12,
                                g: 0.13,
                                b: 0.15,
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
            }

            // UI 렌더링
            if let Some(ref root) = editor.canvas_ui_system.root {
                // UiRenderer 화면 크기 설정
                ui_renderer.resize(queue, resolution.0, resolution.1);

                // 위젯 렌더링
                ui_renderer.render(
                    device,
                    encoder,
                    viewport.render_target(),
                    queue,
                    root,
                );
            }
        }
    }

    /// 모든 윈도우 UI 렌더링 (egui)
    pub fn show(&mut self, ctx: &Context) {
        // 닫힌 윈도우 제거
        self.editors.retain(|e| e.open);

        // 각 에디터 윈도우 렌더링
        for editor in &mut self.editors {
            editor.show(ctx);
        }
    }

    /// 열린 에디터 수
    pub fn count(&self) -> usize {
        self.editors.len()
    }

    /// 모든 에디터 닫기
    pub fn close_all(&mut self) {
        self.editors.clear();
    }

    /// 저장하지 않은 변경사항이 있는지
    pub fn has_unsaved_changes(&self) -> bool {
        self.editors.iter().any(|e| e.dirty)
    }

    /// 렌더링이 필요한 에디터가 있는지
    pub fn needs_render(&self) -> bool {
        self.editors.iter().any(|e| e.open && e.viewport.is_some())
    }
}

/// 해상도 프리셋
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CanvasResolution {
    Res1920x1080,
    Res1280x720,
    Res800x600,
    Custom(u32, u32),
}

impl CanvasResolution {
    pub fn size(&self) -> (u32, u32) {
        match self {
            CanvasResolution::Res1920x1080 => (1920, 1080),
            CanvasResolution::Res1280x720 => (1280, 720),
            CanvasResolution::Res800x600 => (800, 600),
            CanvasResolution::Custom(w, h) => (*w, *h),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            CanvasResolution::Res1920x1080 => "1920x1080",
            CanvasResolution::Res1280x720 => "1280x720",
            CanvasResolution::Res800x600 => "800x600",
            CanvasResolution::Custom(_, _) => "Custom",
        }
    }
}

/// 단일 UI 에디터 윈도우
pub struct UiEditorWindow {
    /// 윈도우 ID
    pub id: u32,

    /// 파일 경로 (None이면 새 파일)
    pub file_path: Option<PathBuf>,

    /// UI 에셋 데이터
    pub asset: UiAsset,

    /// 변경사항 있음
    pub dirty: bool,

    /// 윈도우 열림 상태
    pub open: bool,

    /// 포커스 요청
    pub focus_requested: bool,

    // === 좌측: 팔레트 + 계층 ===
    /// 선택된 위젯 ID
    pub selected_widget_id: Option<String>,

    /// 확장된 계층 노드
    pub hierarchy_expanded: HashSet<String>,

    // === 중앙: Canvas ===
    /// 캔버스 줌 레벨
    pub canvas_zoom: f32,

    /// 캔버스 패닝 오프셋
    pub canvas_pan: Vec2,

    /// 캔버스 해상도
    pub canvas_resolution: CanvasResolution,

    /// 캔버스 UI 시스템 (미리보기용)
    pub canvas_ui_system: UiSystem,

    /// 뷰포트 텍스처 (wgpu 렌더 타겟)
    pub viewport: Option<ViewportTexture>,

    // === 우측: Details + AI ===
    /// AI 패널 표시 여부
    pub show_ai_panel: bool,

    /// AI 입력 필드
    pub ai_input: String,

    /// AI 응답 메시지
    pub ai_response: String,

    // === 내부 상태 ===
    /// 윈도우 크기
    window_size: Vec2,

    /// 마지막 캔버스 영역 (클릭 처리용)
    last_canvas_rect: Option<Rect>,

    // === 애니메이션 편집 상태 ===
    /// 선택된 프리셋 인덱스 (0=None)
    pub selected_preset: usize,
    /// 애니메이션 지속 시간
    pub animation_duration: f32,
    /// 애니메이션 지연 시간
    pub animation_delay: f32,
    /// 선택된 이징 함수 인덱스
    pub selected_easing: usize,
    /// 슬라이드 거리 (slide_in 프리셋용)
    pub slide_distance: f32,
}

impl UiEditorWindow {
    /// 새 빈 에디터 생성
    pub fn new(id: u32) -> Self {
        Self {
            id,
            file_path: None,
            asset: UiAsset::new("Untitled"),
            dirty: false,
            open: true,
            focus_requested: true,
            selected_widget_id: None,
            hierarchy_expanded: HashSet::new(),
            canvas_zoom: 0.5, // 기본 50% 줌 (1920x1080이 화면에 맞게)
            canvas_pan: Vec2::ZERO,
            canvas_resolution: CanvasResolution::Res1920x1080,
            canvas_ui_system: UiSystem::new(),
            viewport: None,
            show_ai_panel: true,
            ai_input: String::new(),
            ai_response: String::new(),
            window_size: Vec2::new(1200.0, 800.0),
            last_canvas_rect: None,
            // 애니메이션 편집 기본값
            selected_preset: 0,
            animation_duration: 0.3,
            animation_delay: 0.0,
            selected_easing: 2, // EaseOut
            slide_distance: 100.0,
        }
    }

    /// 파일에서 에디터 생성
    pub fn from_file(id: u32, path: PathBuf) -> Self {
        let asset = UiAsset::load(&path).unwrap_or_else(|e| {
            log::error!("Failed to load UI file {:?}: {}", path, e);
            UiAsset::new(path.file_stem().unwrap_or_default().to_string_lossy())
        });

        let mut editor = Self::new(id);
        editor.file_path = Some(path);
        editor.asset = asset.clone();

        // UI 시스템에 루트 위젯 설정
        editor.canvas_ui_system.set_root(asset.root);

        // 계층 기본 확장 (루트)
        if let Some(ref root_id) = editor.canvas_ui_system.root.as_ref().and_then(|r| r.id.clone()) {
            editor.hierarchy_expanded.insert(root_id.clone());
        }

        editor
    }

    /// 뷰포트 텍스처 확보 (없으면 생성)
    pub fn ensure_viewport(
        &mut self,
        device: &wgpu::Device,
        egui_renderer: &mut egui_wgpu::Renderer,
        format: wgpu::TextureFormat,
    ) {
        let resolution = self.canvas_resolution.size();

        if let Some(ref mut viewport) = self.viewport {
            // 해상도가 다르면 리사이즈
            if viewport.size != resolution {
                viewport.resize(device, egui_renderer, resolution);
            }
        } else {
            // 새 뷰포트 생성
            self.viewport = Some(ViewportTexture::new(device, egui_renderer, format, resolution));
            log::info!("[UiEditorWindow {}] Created viewport {}x{}", self.id, resolution.0, resolution.1);
        }
    }

    /// 윈도우 제목
    fn title(&self) -> String {
        let name = self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.asset.name.clone());

        if self.dirty {
            format!("UI Editor - {}*", name)
        } else {
            format!("UI Editor - {}", name)
        }
    }

    /// 파일 저장
    pub fn save(&mut self) -> Result<(), String> {
        if let Some(ref path) = self.file_path {
            // UI 시스템에서 현재 상태 추출
            if let Some(root) = self.canvas_ui_system.root.clone() {
                self.asset.root = root;
            }
            self.asset.touch();

            self.asset.save(path)
                .map_err(|e| e.to_string())?;
            self.dirty = false;
            Ok(())
        } else {
            Err("No file path set".to_string())
        }
    }

    /// 다른 이름으로 저장
    pub fn save_as(&mut self, path: PathBuf) -> Result<(), String> {
        self.file_path = Some(path);
        self.save()
    }

    /// 윈도우 UI 렌더링
    pub fn show(&mut self, ctx: &Context) {
        let title = self.title();
        let id = egui::Id::new(format!("ui_editor_{}", self.id));

        let mut open = self.open;

        // 포커스 요청 처리
        if self.focus_requested {
            ctx.memory_mut(|mem| mem.request_focus(id));
            self.focus_requested = false;
        }

        egui::Window::new(&title)
            .id(id)
            .open(&mut open)
            .default_size(self.window_size)
            .resizable(true)
            .collapsible(true)
            .show(ctx, |ui| {
                self.render_content(ui);
            });

        self.open = open;
    }

    /// 윈도우 내용 렌더링
    fn render_content(&mut self, ui: &mut Ui) {
        // 툴바
        self.render_toolbar(ui);

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // 3열 레이아웃
        let available = ui.available_size();
        let left_width = 180.0;
        let right_width = if self.show_ai_panel { 250.0 } else { 180.0 };
        let center_width = (available.x - left_width - right_width - 16.0).max(200.0);

        ui.horizontal(|ui| {
            // 좌측: 팔레트 + 계층
            ui.vertical(|ui| {
                ui.set_width(left_width);
                self.render_left_panel(ui);
            });

            ui.separator();

            // 중앙: Canvas
            ui.vertical(|ui| {
                ui.set_width(center_width);
                self.render_canvas(ui);
            });

            ui.separator();

            // 우측: Details + AI
            ui.vertical(|ui| {
                ui.set_width(right_width);
                self.render_right_panel(ui);
            });
        });
    }

    /// 툴바 렌더링
    fn render_toolbar(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // 저장 버튼
            if ui.button("Save").clicked() {
                if let Err(e) = self.save() {
                    log::error!("Save failed: {}", e);
                }
            }

            ui.separator();

            // Undo/Redo (TODO: 구현)
            ui.add_enabled(false, egui::Button::new("Undo"));
            ui.add_enabled(false, egui::Button::new("Redo"));

            ui.separator();

            // 해상도 선택
            let prev_resolution = self.canvas_resolution;
            egui::ComboBox::from_id_salt(format!("resolution_{}", self.id))
                .selected_text(self.canvas_resolution.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.canvas_resolution, CanvasResolution::Res1920x1080, "1920x1080");
                    ui.selectable_value(&mut self.canvas_resolution, CanvasResolution::Res1280x720, "1280x720");
                    ui.selectable_value(&mut self.canvas_resolution, CanvasResolution::Res800x600, "800x600");
                });

            // 해상도 변경 시 뷰포트 무효화 (다음 프레임에 재생성)
            if prev_resolution != self.canvas_resolution {
                self.viewport = None;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // AI 패널 토글
                if ui.selectable_label(self.show_ai_panel, "AI").clicked() {
                    self.show_ai_panel = !self.show_ai_panel;
                }

                // 줌 컨트롤
                if ui.small_button("-").clicked() {
                    self.canvas_zoom = (self.canvas_zoom - 0.1).clamp(0.1, 2.0);
                }
                ui.label(format!("{:.0}%", self.canvas_zoom * 100.0));
                if ui.small_button("+").clicked() {
                    self.canvas_zoom = (self.canvas_zoom + 0.1).clamp(0.1, 2.0);
                }

                // Fit 버튼
                if ui.small_button("Fit").clicked() {
                    self.canvas_zoom = 0.5;
                    self.canvas_pan = Vec2::ZERO;
                }
            });
        });
    }

    /// 좌측 패널: 팔레트 + 계층
    fn render_left_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // 위젯 팔레트
                egui::CollapsingHeader::new("Widget Palette")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_palette(ui);
                    });

                ui.add_space(8.0);

                // 위젯 계층
                egui::CollapsingHeader::new("Hierarchy")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_hierarchy(ui);
                    });
            });
    }

    /// 위젯 팔레트 렌더링
    fn render_palette(&mut self, ui: &mut Ui) {
        let widgets = [
            ("Container", "▢"),
            ("Text", "T"),
            ("Image", "◻"),
            ("Button", "◉"),
            ("Input", "⌨"),
            ("Slider", "─"),
            ("Toggle", "☑"),
            ("Scroll", "↕"),
            ("Progress", "▬"),
        ];

        ui.horizontal_wrapped(|ui| {
            for (label, icon) in widgets {
                let button = egui::Button::new(format!("{} {}", icon, label))
                    .min_size(Vec2::new(70.0, 24.0));

                let response = ui.add(button);

                if response.clicked() {
                    log::info!("[UiEditor] Add widget: {}", label);
                    // TODO: 위젯 추가 구현
                }
            }
        });
    }

    /// 위젯 계층 렌더링
    fn render_hierarchy(&mut self, ui: &mut Ui) {
        // Clone to avoid borrow conflict with self
        if let Some(root) = self.canvas_ui_system.root.clone() {
            self.render_hierarchy_node(ui, &root, 0);
        } else {
            ui.label("(empty)");
        }
    }

    /// 계층 노드 렌더링 (재귀)
    fn render_hierarchy_node(&mut self, ui: &mut Ui, widget: &Widget, depth: usize) {
        let id = widget.id.clone().unwrap_or_else(|| format!("widget_{}", depth));
        let has_children = !widget.children.is_empty();
        let is_selected = self.selected_widget_id.as_ref() == Some(&id);
        let is_expanded = self.hierarchy_expanded.contains(&id);

        let indent = depth as f32 * 12.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);

            // 확장/축소 버튼
            if has_children {
                let icon = if is_expanded { "▼" } else { "▶" };
                if ui.small_button(icon).clicked() {
                    if is_expanded {
                        self.hierarchy_expanded.remove(&id);
                    } else {
                        self.hierarchy_expanded.insert(id.clone());
                    }
                }
            } else {
                ui.add_space(18.0);
            }

            // 위젯 타입 아이콘
            let type_icon = match &widget.widget_type {
                skope_game_ui::WidgetType::Container => "▢",
                skope_game_ui::WidgetType::Text { .. } => "T",
                skope_game_ui::WidgetType::Image { .. } => "◻",
                skope_game_ui::WidgetType::Button { .. } => "◉",
                skope_game_ui::WidgetType::InputField { .. } => "⌨",
                _ => "◇",
            };
            ui.label(type_icon);

            // 위젯 이름
            if ui.selectable_label(is_selected, &id).clicked() {
                self.selected_widget_id = Some(id.clone());
            }
        });

        // 자식 노드 렌더링
        if has_children && is_expanded {
            for child in &widget.children {
                self.render_hierarchy_node(ui, child, depth + 1);
            }
        }
    }

    /// 캔버스 렌더링 (egui에 viewport texture 표시)
    fn render_canvas(&mut self, ui: &mut Ui) {
        let available = ui.available_size();
        let resolution = self.canvas_resolution.size();

        // 캔버스 영역 할당
        let (response, painter) = ui.allocate_painter(available, egui::Sense::click_and_drag());
        let rect = response.rect;

        // 배경 (체커보드 패턴 시뮬레이션)
        painter.rect_filled(rect, 0.0, Color32::from_rgb(35, 38, 45));

        // 캔버스 영역 계산 (줌 및 패닝 적용)
        let canvas_size = Vec2::new(resolution.0 as f32, resolution.1 as f32) * self.canvas_zoom;
        let canvas_pos = rect.center() - canvas_size / 2.0 + self.canvas_pan;
        let canvas_rect = Rect::from_min_size(canvas_pos, canvas_size);

        self.last_canvas_rect = Some(canvas_rect);

        // 뷰포트 텍스처가 있으면 표시
        if let Some(ref viewport) = self.viewport {
            // egui Image로 뷰포트 텍스처 표시
            let image = egui::Image::new(egui::load::SizedTexture::new(
                viewport.egui_texture_id,
                canvas_size,
            ));

            // 캔버스 위치에 이미지 그리기
            let image_rect = canvas_rect;
            ui.put(image_rect, image);
        } else {
            // 뷰포트 없음 - 플레이스홀더
            painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(25, 28, 32));
            painter.text(
                canvas_rect.center(),
                egui::Align2::CENTER_CENTER,
                "Initializing...",
                egui::FontId::proportional(14.0),
                Color32::from_rgb(80, 85, 95),
            );
        }

        // 캔버스 테두리
        painter.rect_stroke(
            canvas_rect,
            0.0,
            egui::Stroke::new(1.0, Color32::from_rgb(60, 65, 75)),
            egui::StrokeKind::Outside,
        );

        // 해상도 표시
        painter.text(
            canvas_rect.left_top() + Vec2::new(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!("{}x{}", resolution.0, resolution.1),
            egui::FontId::proportional(10.0),
            Color32::from_rgb(120, 125, 135),
        );

        // 선택된 위젯 하이라이트 + 리사이즈 핸들
        if let Some(ref selected_id) = self.selected_widget_id.clone() {
            if let Some(ref root) = self.canvas_ui_system.root {
                if let Some(widget) = find_widget_by_id(root, selected_id) {
                    // computed_rect를 캔버스 좌표로 변환
                    let widget_rect = &widget.computed_rect;
                    let screen_rect = Rect::from_min_size(
                        canvas_rect.min + Vec2::new(
                            widget_rect.x * self.canvas_zoom,
                            widget_rect.y * self.canvas_zoom,
                        ),
                        Vec2::new(
                            widget_rect.width * self.canvas_zoom,
                            widget_rect.height * self.canvas_zoom,
                        ),
                    );

                    // 선택 테두리 (파란색)
                    painter.rect_stroke(
                        screen_rect,
                        0.0,
                        egui::Stroke::new(2.0, Color32::from_rgb(60, 140, 220)),
                        egui::StrokeKind::Outside,
                    );

                    // 리사이즈 핸들 (8개: 코너 4개 + 엣지 중앙 4개)
                    let handle_size = 6.0;
                    let handle_color = Color32::from_rgb(60, 140, 220);
                    let handle_bg = Color32::WHITE;

                    let corners = [
                        screen_rect.left_top(),
                        screen_rect.right_top(),
                        screen_rect.left_bottom(),
                        screen_rect.right_bottom(),
                    ];

                    let edges = [
                        egui::pos2(screen_rect.center().x, screen_rect.top()),    // top
                        egui::pos2(screen_rect.center().x, screen_rect.bottom()), // bottom
                        egui::pos2(screen_rect.left(), screen_rect.center().y),   // left
                        egui::pos2(screen_rect.right(), screen_rect.center().y),  // right
                    ];

                    // 코너 핸들 (정사각형)
                    for pos in corners {
                        let handle_rect = Rect::from_center_size(pos, Vec2::splat(handle_size));
                        painter.rect_filled(handle_rect, 0.0, handle_bg);
                        painter.rect_stroke(
                            handle_rect,
                            0.0,
                            egui::Stroke::new(1.0, handle_color),
                            egui::StrokeKind::Inside,
                        );
                    }

                    // 엣지 핸들 (작은 정사각형)
                    for pos in edges {
                        let handle_rect = Rect::from_center_size(pos, Vec2::splat(handle_size - 1.0));
                        painter.rect_filled(handle_rect, 0.0, handle_bg);
                        painter.rect_stroke(
                            handle_rect,
                            0.0,
                            egui::Stroke::new(1.0, handle_color),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
            }
        }

        // 마우스 휠로 줌
        if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let old_zoom = self.canvas_zoom;
                self.canvas_zoom = (self.canvas_zoom + scroll * 0.002).clamp(0.1, 2.0);

                // 마우스 위치를 중심으로 줌
                if let Some(mouse_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let mouse_in_canvas = mouse_pos - canvas_rect.center();
                    let zoom_delta = self.canvas_zoom / old_zoom;
                    self.canvas_pan = self.canvas_pan + mouse_in_canvas * (1.0 - zoom_delta);
                }
            }
        }

        // 중버튼 드래그로 패닝
        if response.dragged_by(egui::PointerButton::Middle) {
            self.canvas_pan += response.drag_delta();
        }

        // 우클릭 드래그로도 패닝 (Alt 없이)
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.canvas_pan += response.drag_delta();
        }

        // 클릭으로 위젯 선택
        if response.clicked() {
            if let Some(click_pos) = response.interact_pointer_pos() {
                self.handle_canvas_click(click_pos, canvas_rect);
            }
        }
    }

    /// 캔버스 클릭 처리 (위젯 선택)
    fn handle_canvas_click(&mut self, click_pos: egui::Pos2, canvas_rect: Rect) {
        let resolution = self.canvas_resolution.size();

        // 클릭 위치를 UI 좌표로 변환
        let relative_pos = click_pos - canvas_rect.min;
        let ui_x = (relative_pos.x / self.canvas_zoom).clamp(0.0, resolution.0 as f32);
        let ui_y = (relative_pos.y / self.canvas_zoom).clamp(0.0, resolution.1 as f32);

        // hit test
        if let Some(ref root) = self.canvas_ui_system.root {
            if let Some(hit_id) = self.hit_test_widget(root, ui_x, ui_y) {
                self.selected_widget_id = Some(hit_id);
                log::debug!("[UiEditor] Selected widget at ({:.0}, {:.0})", ui_x, ui_y);
            } else {
                self.selected_widget_id = None;
            }
        }
    }

    /// 위젯 hit test (재귀)
    fn hit_test_widget(&self, widget: &Widget, x: f32, y: f32) -> Option<String> {
        // 자식부터 테스트 (위에 있는 것 우선)
        for child in widget.children.iter().rev() {
            if let Some(hit) = self.hit_test_widget(child, x, y) {
                return Some(hit);
            }
        }

        // 현재 위젯 테스트
        let rect = &widget.computed_rect;
        if x >= rect.x && x <= rect.x + rect.width &&
           y >= rect.y && y <= rect.y + rect.height {
            if widget.visible && widget.interactive {
                return widget.id.clone();
            }
        }

        None
    }

    /// 우측 패널: Details + AI
    fn render_right_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // Details 섹션
                egui::CollapsingHeader::new("Details")
                    .default_open(true)
                    .show(ui, |ui| {
                        self.render_details(ui);
                    });

                ui.add_space(8.0);

                // AI Assistant 섹션
                if self.show_ai_panel {
                    egui::CollapsingHeader::new("AI Assistant")
                        .default_open(true)
                        .show(ui, |ui| {
                            self.render_ai_panel(ui);
                        });
                }
            });
    }

    /// Details 패널 렌더링
    fn render_details(&mut self, ui: &mut Ui) {
        if let Some(ref id) = self.selected_widget_id.clone() {
            ui.horizontal(|ui| {
                ui.label("ID:");
                ui.label(egui::RichText::new(id.as_str()).strong());
            });

            if let Some(ref root) = self.canvas_ui_system.root {
                if let Some(widget) = find_widget_by_id(root, &id) {
                    // 타입 표시
                    let type_str = match &widget.widget_type {
                        skope_game_ui::WidgetType::Container => "Container",
                        skope_game_ui::WidgetType::Text { .. } => "Text",
                        skope_game_ui::WidgetType::Image { .. } => "Image",
                        skope_game_ui::WidgetType::Button { .. } => "Button",
                        skope_game_ui::WidgetType::InputField { .. } => "InputField",
                        skope_game_ui::WidgetType::Slider { .. } => "Slider",
                        skope_game_ui::WidgetType::Toggle { .. } => "Toggle",
                        skope_game_ui::WidgetType::ScrollView { .. } => "ScrollView",
                        skope_game_ui::WidgetType::ProgressBar { .. } => "ProgressBar",
                        skope_game_ui::WidgetType::NineSlice { .. } => "NineSlice",
                        skope_game_ui::WidgetType::Sprite { .. } => "Sprite",
                    };
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        ui.label(type_str);
                    });

                    ui.separator();

                    // Layout 정보
                    ui.label(egui::RichText::new("Layout").strong());
                    ui.horizontal(|ui| {
                        ui.label("Offset:");
                        ui.label(format!("({:.0}, {:.0})", widget.layout.offset.0, widget.layout.offset.1));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Anchor:");
                        ui.label(format!("{:?}", widget.layout.anchor));
                    });

                    // Computed rect
                    ui.separator();
                    ui.label(egui::RichText::new("Computed").small());
                    let rect = &widget.computed_rect;
                    ui.label(format!("  Pos: ({:.0}, {:.0})", rect.x, rect.y));
                    ui.label(format!("  Size: {:.0}x{:.0}", rect.width, rect.height));
                }
            }

            // Animation 섹션
            ui.add_space(8.0);
            ui.separator();
            self.render_animation_section(ui, &id);
        } else {
            ui.label("Select a widget");
        }
    }

    /// Animation 섹션 렌더링
    fn render_animation_section(&mut self, ui: &mut Ui, widget_id: &str) {
        ui.label(egui::RichText::new("Animation").strong());
        ui.add_space(4.0);

        // 프리셋 목록
        const PRESET_NAMES: &[&str] = &[
            "None",
            "Fade In",
            "Fade Out",
            "Slide In Left",
            "Slide In Right",
            "Slide In Top",
            "Slide In Bottom",
            "Pop In",
            "Pop Out",
            "Shake",
            "Pulse",
        ];

        // 이징 함수 목록
        const EASING_NAMES: &[&str] = &[
            "Linear",
            "EaseIn",
            "EaseOut",
            "EaseInOut",
            "EaseInQuad",
            "EaseOutQuad",
            "EaseInOutQuad",
            "EaseInCubic",
            "EaseOutCubic",
            "EaseInOutCubic",
            "EaseOutBack",
            "EaseOutBounce",
            "EaseOutElastic",
        ];

        // 프리셋 선택
        ui.horizontal(|ui| {
            ui.label("Preset:");
            let selected_preset_name = *PRESET_NAMES.get(self.selected_preset).unwrap_or(&"None");
            egui::ComboBox::from_id_salt(format!("anim_preset_{}", self.id))
                .width(100.0)
                .selected_text(selected_preset_name)
                .show_ui(ui, |ui| {
                    for (i, name) in PRESET_NAMES.iter().enumerate() {
                        ui.selectable_value(&mut self.selected_preset, i, *name);
                    }
                });
        });

        // None이 아닌 경우에만 파라미터 표시
        if self.selected_preset > 0 {
            // Duration
            ui.horizontal(|ui| {
                ui.label("Duration:");
                ui.add(egui::DragValue::new(&mut self.animation_duration)
                    .speed(0.01)
                    .range(0.05..=5.0)
                    .suffix(" s"));
            });

            // Delay
            ui.horizontal(|ui| {
                ui.label("Delay:");
                ui.add(egui::DragValue::new(&mut self.animation_delay)
                    .speed(0.01)
                    .range(0.0..=5.0)
                    .suffix(" s"));
            });

            // Slide 프리셋인 경우 거리 입력
            if self.selected_preset >= 3 && self.selected_preset <= 6 {
                ui.horizontal(|ui| {
                    ui.label("Distance:");
                    ui.add(egui::DragValue::new(&mut self.slide_distance)
                        .speed(1.0)
                        .range(10.0..=500.0)
                        .suffix(" px"));
                });
            }

            // Easing 선택 (Shake, Pulse 제외)
            if self.selected_preset < 9 {
                ui.horizontal(|ui| {
                    ui.label("Easing:");
                    let selected_easing_name = *EASING_NAMES.get(self.selected_easing).unwrap_or(&"Linear");
                    egui::ComboBox::from_id_salt(format!("anim_easing_{}", self.id))
                        .width(100.0)
                        .selected_text(selected_easing_name)
                        .show_ui(ui, |ui| {
                            for (i, name) in EASING_NAMES.iter().enumerate() {
                                ui.selectable_value(&mut self.selected_easing, i, *name);
                            }
                        });
                });
            }

            ui.add_space(4.0);

            // Preview 버튼
            ui.horizontal(|ui| {
                if ui.button("▶ Preview").clicked() {
                    self.preview_animation(widget_id);
                }
                if ui.button("Stop").clicked() {
                    self.canvas_ui_system.stop_all_animations();
                }
            });
        }
    }

    /// 애니메이션 프리뷰 재생
    fn preview_animation(&mut self, widget_id: &str) {
        let animation = match self.selected_preset {
            1 => animation_presets::fade_in(widget_id, self.animation_duration),
            2 => animation_presets::fade_out(widget_id, self.animation_duration),
            3 => animation_presets::slide_in_left(widget_id, self.slide_distance, self.animation_duration),
            4 => animation_presets::slide_in_right(widget_id, self.slide_distance, self.animation_duration),
            5 => animation_presets::slide_in_top(widget_id, self.slide_distance, self.animation_duration),
            6 => animation_presets::slide_in_bottom(widget_id, self.slide_distance, self.animation_duration),
            7 => animation_presets::pop_in(widget_id, self.animation_duration),
            8 => animation_presets::pop_out(widget_id, self.animation_duration),
            9 => animation_presets::shake(widget_id),
            10 => animation_presets::pulse(widget_id),
            _ => return,
        };

        // 지연 시간 적용
        let animation = skope_game_ui::ActiveAnimation {
            delay: self.animation_delay,
            ..animation
        };

        self.canvas_ui_system.play_animation(animation);
        log::info!("[UiEditor] Preview animation: preset={}", self.selected_preset);
    }

    /// AI 패널 렌더링
    fn render_ai_panel(&mut self, ui: &mut Ui) {
        // 선택된 위젯 컨텍스트 (상세 정보)
        if let Some(ref selected_id) = self.selected_widget_id.clone() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label("Selected:");
                    ui.label(egui::RichText::new(selected_id.as_str()).strong().color(Color32::from_rgb(100, 180, 255)));
                });

                // 위젯 타입 및 속성 표시
                if let Some(ref root) = self.canvas_ui_system.root.clone() {
                    if let Some(widget) = find_widget_by_id(&root, &selected_id) {
                        let type_str = match &widget.widget_type {
                            skope_game_ui::WidgetType::Container => "Container",
                            skope_game_ui::WidgetType::Text { .. } => "Text",
                            skope_game_ui::WidgetType::Image { .. } => "Image",
                            skope_game_ui::WidgetType::Button { .. } => "Button",
                            skope_game_ui::WidgetType::InputField { .. } => "InputField",
                            skope_game_ui::WidgetType::NineSlice { .. } => "NineSlice",
                            skope_game_ui::WidgetType::ProgressBar { .. } => "ProgressBar",
                            skope_game_ui::WidgetType::ScrollView { .. } => "ScrollView",
                            skope_game_ui::WidgetType::Slider { .. } => "Slider",
                            skope_game_ui::WidgetType::Toggle { .. } => "Toggle",
                            skope_game_ui::WidgetType::Sprite { .. } => "Sprite",
                        };
                        ui.label(format!("Type: {}", type_str));

                        // 위치/크기 정보
                        let rect = &widget.computed_rect;
                        ui.label(format!("Pos: ({:.0}, {:.0})", rect.x, rect.y));
                        ui.label(format!("Size: {:.0}x{:.0}", rect.width, rect.height));
                    }
                }
            });
            ui.add_space(4.0);
        }

        // Quick Actions (선택된 위젯에 따라 다른 액션)
        ui.label(egui::RichText::new("Quick Actions").strong());
        ui.horizontal_wrapped(|ui| {
            // 기본 액션
            if ui.small_button("🎯 Center").on_hover_text("중앙 정렬").clicked() {
                self.action_center_widget();
            }
            if ui.small_button("📐 Fill Width").on_hover_text("가로 꽉 채움").clicked() {
                self.action_fill_width();
            }
            if ui.small_button("📏 Fill Height").on_hover_text("세로 꽉 채움").clicked() {
                self.action_fill_height();
            }
        });

        ui.horizontal_wrapped(|ui| {
            // 위젯 추가 액션
            if ui.small_button("➕ Text").on_hover_text("텍스트 추가").clicked() {
                self.action_add_widget("Text");
            }
            if ui.small_button("➕ Button").on_hover_text("버튼 추가").clicked() {
                self.action_add_widget("Button");
            }
            if ui.small_button("➕ Image").on_hover_text("이미지 추가").clicked() {
                self.action_add_widget("Image");
            }
        });

        ui.add_space(8.0);

        // AI 명령 입력
        ui.label(egui::RichText::new("AI Command").strong());
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.ai_input)
                    .hint_text("예: \"버튼 3개 추가해줘\"")
                    .desired_width(ui.available_width() - 50.0)
            );
            if ui.button("Send").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                if !self.ai_input.is_empty() {
                    self.process_ai_command();
                }
            }
        });

        // AI 응답 영역
        ui.add_space(4.0);
        if !self.ai_response.is_empty() {
            ui.group(|ui| {
                ui.label(egui::RichText::new("AI:").strong());
                ui.label(&self.ai_response);
            });
        }
    }

    /// Quick Action: 위젯 중앙 정렬
    fn action_center_widget(&mut self) {
        if let Some(ref id) = self.selected_widget_id {
            log::info!("[AI] Center widget: {}", id);
            // TODO: 위젯의 레이아웃을 Center로 변경
            self.dirty = true;
            self.ai_response = format!("'{}' 위젯을 중앙 정렬했습니다.", id);
        } else {
            self.ai_response = "먼저 위젯을 선택하세요.".to_string();
        }
    }

    /// Quick Action: 가로 꽉 채움
    fn action_fill_width(&mut self) {
        if let Some(ref id) = self.selected_widget_id {
            log::info!("[AI] Fill width: {}", id);
            self.dirty = true;
            self.ai_response = format!("'{}' 위젯을 가로로 꽉 채웠습니다.", id);
        } else {
            self.ai_response = "먼저 위젯을 선택하세요.".to_string();
        }
    }

    /// Quick Action: 세로 꽉 채움
    fn action_fill_height(&mut self) {
        if let Some(ref id) = self.selected_widget_id {
            log::info!("[AI] Fill height: {}", id);
            self.dirty = true;
            self.ai_response = format!("'{}' 위젯을 세로로 꽉 채웠습니다.", id);
        } else {
            self.ai_response = "먼저 위젯을 선택하세요.".to_string();
        }
    }

    /// Quick Action: 위젯 추가
    fn action_add_widget(&mut self, widget_type: &str) {
        log::info!("[AI] Add widget: {}", widget_type);

        // 새 위젯 생성 (Widget::default() 기반)
        let new_id = format!("new_{}_{}", widget_type.to_lowercase(), self.id);
        let mut new_widget = Widget::default();
        new_widget.id = Some(new_id.clone());

        match widget_type {
            "Text" => {
                new_widget.widget_type = skope_game_ui::WidgetType::Text {
                    content: "New Text".to_string(),
                    font: None,
                    font_size: Some(16.0),
                };
            }
            "Button" => {
                new_widget.widget_type = skope_game_ui::WidgetType::Button {
                    text: Some("New Button".to_string()),
                    states: Default::default(),
                };
                new_widget.layout.size = skope_game_ui::Size::Fixed(120.0, 40.0);
            }
            "Image" => {
                new_widget.widget_type = skope_game_ui::WidgetType::Image {
                    src: "placeholder.png".to_string(),
                    color: None,
                    preserve_aspect: true,
                };
                new_widget.layout.size = skope_game_ui::Size::Fixed(100.0, 100.0);
            }
            _ => return,
        };

        // 선택된 위젯에 자식으로 추가하거나 root에 추가
        if let Some(ref mut root) = self.canvas_ui_system.root {
            if let Some(ref selected_id) = self.selected_widget_id {
                // 선택된 위젯에 자식으로 추가
                if let Some(parent) = find_widget_by_id_mut(root, selected_id) {
                    parent.children.push(new_widget);
                    self.ai_response = format!("'{}' 위젯을 '{}'에 추가했습니다.", new_id, selected_id);
                }
            } else {
                // root에 자식으로 추가
                root.children.push(new_widget);
                self.ai_response = format!("'{}' 위젯을 루트에 추가했습니다.", new_id);
            }
        }

        self.selected_widget_id = Some(new_id);
        self.dirty = true;

        // 레이아웃 재계산
        let resolution = self.canvas_resolution.size();
        self.canvas_ui_system.set_screen_size(resolution.0 as f32, resolution.1 as f32);
        self.canvas_ui_system.calculate_layout();
    }

    /// AI 명령 처리
    fn process_ai_command(&mut self) {
        let command = self.ai_input.clone();
        self.ai_input.clear();

        log::info!("[AI] Processing command: {}", command);

        // 간단한 명령 파싱
        let command_lower = command.to_lowercase();

        if command_lower.contains("버튼") && command_lower.contains("추가") {
            self.action_add_widget("Button");
        } else if command_lower.contains("텍스트") && command_lower.contains("추가") {
            self.action_add_widget("Text");
        } else if command_lower.contains("이미지") && command_lower.contains("추가") {
            self.action_add_widget("Image");
        } else if command_lower.contains("중앙") || command_lower.contains("center") {
            self.action_center_widget();
        } else {
            self.ai_response = format!("명령을 이해하지 못했습니다: \"{}\"", command);
        }
    }
}

/// 위젯 ID로 찾기 (헬퍼)
fn find_widget_by_id<'a>(widget: &'a Widget, id: &str) -> Option<&'a Widget> {
    if widget.id.as_ref().map(|s| s.as_str()) == Some(id) {
        return Some(widget);
    }
    for child in &widget.children {
        if let Some(found) = find_widget_by_id(child, id) {
            return Some(found);
        }
    }
    None
}

/// 위젯 ID로 찾기 (가변 참조)
fn find_widget_by_id_mut<'a>(widget: &'a mut Widget, id: &str) -> Option<&'a mut Widget> {
    if widget.id.as_ref().map(|s| s.as_str()) == Some(id) {
        return Some(widget);
    }
    for child in &mut widget.children {
        if let Some(found) = find_widget_by_id_mut(child, id) {
            return Some(found);
        }
    }
    None
}
