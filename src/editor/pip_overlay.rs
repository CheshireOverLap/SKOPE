//! Picture-in-Picture (PiP) Overlay System
//!
//! Scene 뷰 안에서 Game 뷰 미리보기를 제공합니다.
//!
//! ## 기능 레벨
//! - Basic: 고정 위치/크기, 텍스처 표시
//! - Interactive: 드래그/리사이즈, 클릭 전환
//! - Advanced: 멀티 PiP, 직접 조작, 소스 선택

use egui::{Color32, Pos2, Rect, Response, Sense, Stroke, TextureId, Ui, Vec2};
use serde::{Deserialize, Serialize};

/// PiP 오버레이 위치
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum PipPosition {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
    /// 사용자 정의 위치 (0.0~1.0 정규화 좌표)
    Custom { x: f32, y: f32 },
}

impl PipPosition {
    /// 다음 위치로 순환 (Shift+P용)
    pub fn cycle(&self) -> Self {
        match self {
            Self::TopRight => Self::BottomRight,
            Self::BottomRight => Self::BottomLeft,
            Self::BottomLeft => Self::TopLeft,
            Self::TopLeft => Self::TopRight,
            Self::Custom { .. } => Self::TopRight,
        }
    }

    /// 위치 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::TopLeft => "Top Left",
            Self::TopRight => "Top Right",
            Self::BottomLeft => "Bottom Left",
            Self::BottomRight => "Bottom Right",
            Self::Custom { .. } => "Custom",
        }
    }

    /// 모든 프리셋 위치
    pub fn presets() -> &'static [Self] {
        &[Self::TopLeft, Self::TopRight, Self::BottomLeft, Self::BottomRight]
    }
}

/// PiP 오버레이 크기
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum PipSize {
    /// 부모의 15%
    Small,
    /// 부모의 25%
    #[default]
    Medium,
    /// 부모의 35%
    Large,
    /// 사용자 정의 크기 (픽셀)
    Custom { width: f32, height: f32 },
}

impl PipSize {
    /// 부모 크기 대비 비율
    pub fn ratio(&self) -> f32 {
        match self {
            Self::Small => 0.15,
            Self::Medium => 0.25,
            Self::Large => 0.35,
            Self::Custom { .. } => 0.25, // 기본값
        }
    }

    /// 실제 픽셀 크기 계산
    pub fn to_pixels(&self, parent_size: Vec2) -> Vec2 {
        match self {
            Self::Small => parent_size * 0.15,
            Self::Medium => parent_size * 0.25,
            Self::Large => parent_size * 0.35,
            Self::Custom { width, height } => Vec2::new(*width, *height),
        }
    }

    /// 크기 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::Small => "Small (15%)",
            Self::Medium => "Medium (25%)",
            Self::Large => "Large (35%)",
            Self::Custom { .. } => "Custom",
        }
    }

    /// 모든 프리셋 크기
    pub fn presets() -> &'static [Self] {
        &[Self::Small, Self::Medium, Self::Large]
    }
}

/// PiP 소스 타입 (Advanced)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PipSource {
    /// Game 뷰 (기본)
    #[default]
    Game,
    /// Scene 뷰 (Game 탭에서 Scene 미리보기)
    Scene,
    /// 특정 카메라 (인덱스)
    Camera(usize),
}

impl PipSource {
    /// 소스 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::Game => "Game View",
            Self::Scene => "Scene View",
            Self::Camera(_) => "Custom Camera",
        }
    }
}

/// PiP 오버레이 상태
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipOverlay {
    /// 활성화 여부
    pub enabled: bool,
    /// 위치
    pub position: PipPosition,
    /// 크기
    pub size: PipSize,
    /// 투명도 (0.0 ~ 1.0)
    pub opacity: f32,
    /// 테두리 표시
    pub show_border: bool,
    /// 헤더 표시
    pub show_header: bool,
    /// 소스 (Advanced)
    pub source: PipSource,
    /// 줌 레벨 (1.0 = 100%)
    pub zoom: f32,

    // Interactive 상태 (직렬화 제외)
    #[serde(skip)]
    pub dragging: bool,
    #[serde(skip)]
    pub drag_offset: Vec2,
    #[serde(skip)]
    pub resizing: bool,
    #[serde(skip)]
    pub hovered: bool,
}

impl Default for PipOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            position: PipPosition::TopRight,
            size: PipSize::Medium,
            opacity: 0.9,
            show_border: true,
            show_header: true,
            source: PipSource::Game,
            zoom: 1.0,
            dragging: false,
            drag_offset: Vec2::ZERO,
            resizing: false,
            hovered: false,
        }
    }
}

impl PipOverlay {
    /// 새 PipOverlay 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 토글
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        log::info!("[PiP] Toggled: {}", self.enabled);
    }

    /// 위치 순환
    pub fn cycle_position(&mut self) {
        self.position = self.position.cycle();
        log::info!("[PiP] Position: {:?}", self.position);
    }

    /// 줌 설정
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(0.5, 2.0);
    }
}

/// PiP 액션 (외부로 전달)
#[derive(Debug, Clone)]
pub enum PipAction {
    /// 없음
    None,
    /// 탭 전환 요청 (더블클릭)
    SwitchToTab,
    /// 닫기
    Close,
}

/// PiP 렌더러
pub struct PipRenderer;

impl PipRenderer {
    /// PiP 오버레이 렌더링
    ///
    /// # Arguments
    /// * `ui` - egui UI 컨텍스트
    /// * `pip` - PiP 설정 (mut: 드래그/리사이즈 상태 업데이트)
    /// * `texture_id` - 표시할 텍스처 (Game 또는 Scene)
    /// * `viewport_rect` - 부모 뷰포트 영역
    ///
    /// # Returns
    /// PipAction - 발생한 액션
    pub fn render(
        ui: &mut Ui,
        pip: &mut PipOverlay,
        texture_id: Option<TextureId>,
        viewport_rect: Rect,
    ) -> PipAction {
        if !pip.enabled {
            return PipAction::None;
        }

        let Some(texture_id) = texture_id else {
            return PipAction::None;
        };

        let mut action = PipAction::None;

        // PiP 영역 계산
        let pip_size = pip.size.to_pixels(viewport_rect.size());
        let pip_rect = Self::calculate_pip_rect(pip, viewport_rect, pip_size);

        // 헤더 높이
        let header_height = if pip.show_header { 20.0 } else { 0.0 };
        let content_rect = Rect::from_min_size(
            pip_rect.min + Vec2::new(0.0, header_height),
            pip_rect.size() - Vec2::new(0.0, header_height),
        );

        // 배경 프레임
        let alpha = (pip.opacity * 255.0) as u8;
        let bg_color = Color32::from_rgba_unmultiplied(20, 20, 25, alpha);
        let border_color = if pip.hovered {
            Color32::from_rgb(255, 165, 0) // SKOPE 오렌지 (호버)
        } else {
            Color32::from_rgba_unmultiplied(255, 165, 0, 180) // SKOPE 오렌지 (기본)
        };

        // 그림자
        let shadow_rect = pip_rect.translate(Vec2::new(2.0, 2.0));
        ui.painter().rect_filled(
            shadow_rect,
            4.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 80),
        );

        // 배경
        ui.painter().rect_filled(pip_rect, 4.0, bg_color);

        // 테두리
        if pip.show_border {
            ui.painter().rect_stroke(
                pip_rect,
                4.0,
                Stroke::new(2.0, border_color),
                egui::StrokeKind::Inside,
            );
        }

        // 헤더
        if pip.show_header {
            let header_rect = Rect::from_min_size(
                pip_rect.min,
                Vec2::new(pip_rect.width(), header_height),
            );

            // 헤더 배경 (위쪽만 둥글게)
            ui.painter().rect_filled(
                header_rect,
                4.0, // 전체 둥글기 사용 (egui 버전 호환)
                Color32::from_rgba_unmultiplied(40, 40, 50, alpha),
            );

            // 헤더 텍스트
            let source_icon = match pip.source {
                PipSource::Game => "🎮",
                PipSource::Scene => "🎬",
                PipSource::Camera(_) => "📷",
            };
            let header_text = format!("{} {}", source_icon, pip.source.name());

            ui.painter().text(
                header_rect.left_center() + Vec2::new(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &header_text,
                egui::FontId::proportional(11.0),
                Color32::from_rgba_unmultiplied(220, 220, 230, alpha),
            );

            // 닫기 버튼
            let close_btn_rect = Rect::from_min_size(
                Pos2::new(header_rect.right() - 18.0, header_rect.min.y + 2.0),
                Vec2::splat(16.0),
            );
            let close_response = ui.allocate_rect(close_btn_rect, Sense::click());

            let close_color = if close_response.hovered() {
                Color32::from_rgb(255, 100, 100)
            } else {
                Color32::from_rgba_unmultiplied(180, 180, 190, alpha)
            };

            ui.painter().text(
                close_btn_rect.center(),
                egui::Align2::CENTER_CENTER,
                "✕",
                egui::FontId::proportional(12.0),
                close_color,
            );

            if close_response.clicked() {
                pip.enabled = false;
                action = PipAction::Close;
            }

            // 헤더 드래그 처리
            let drag_response = ui.allocate_rect(
                Rect::from_min_max(header_rect.min, close_btn_rect.min),
                Sense::drag(),
            );

            if drag_response.drag_started() {
                pip.dragging = true;
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    pip.drag_offset = pointer_pos - pip_rect.min;
                }
            }

            if drag_response.dragged() && pip.dragging {
                if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                    let new_pos = pointer_pos - pip.drag_offset;
                    // 정규화 좌표로 변환
                    let norm_x = (new_pos.x - viewport_rect.min.x) / viewport_rect.width();
                    let norm_y = (new_pos.y - viewport_rect.min.y) / viewport_rect.height();
                    pip.position = PipPosition::Custom {
                        x: norm_x.clamp(0.0, 1.0 - pip_size.x / viewport_rect.width()),
                        y: norm_y.clamp(0.0, 1.0 - pip_size.y / viewport_rect.height()),
                    };
                }
            }

            if drag_response.drag_stopped() {
                pip.dragging = false;
            }
        }

        // 콘텐츠 (텍스처)
        ui.painter().image(
            texture_id,
            content_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
        );

        // 리사이즈 핸들 (우하단)
        let resize_handle_size = 12.0;
        let resize_rect = Rect::from_min_size(
            Pos2::new(
                pip_rect.right() - resize_handle_size,
                pip_rect.bottom() - resize_handle_size,
            ),
            Vec2::splat(resize_handle_size),
        );

        let resize_response = ui.allocate_rect(resize_rect, Sense::drag());

        // 리사이즈 핸들 시각화
        if resize_response.hovered() || pip.resizing {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);

            // 리사이즈 핸들 삼각형
            let points = [
                resize_rect.right_bottom(),
                resize_rect.right_bottom() - Vec2::new(resize_handle_size, 0.0),
                resize_rect.right_bottom() - Vec2::new(0.0, resize_handle_size),
            ];
            ui.painter().add(egui::Shape::convex_polygon(
                points.to_vec(),
                Color32::from_rgb(255, 165, 0),
                Stroke::NONE,
            ));
        }

        if resize_response.drag_started() {
            pip.resizing = true;
        }

        if resize_response.dragged() && pip.resizing {
            if let Some(pointer_pos) = ui.input(|i| i.pointer.hover_pos()) {
                let new_width = (pointer_pos.x - pip_rect.min.x).max(100.0);
                let new_height = (pointer_pos.y - pip_rect.min.y).max(80.0);
                pip.size = PipSize::Custom {
                    width: new_width,
                    height: new_height,
                };
            }
        }

        if resize_response.drag_stopped() {
            pip.resizing = false;
        }

        // 더블클릭 → 탭 전환
        let content_response = ui.allocate_rect(content_rect, Sense::click());
        pip.hovered = content_response.hovered() || resize_response.hovered();

        if content_response.double_clicked() {
            action = PipAction::SwitchToTab;
            log::info!("[PiP] Double-click: Switch to tab");
        }

        action
    }

    /// PiP 영역 계산
    fn calculate_pip_rect(pip: &PipOverlay, viewport_rect: Rect, pip_size: Vec2) -> Rect {
        let padding = 10.0;

        let pos = match pip.position {
            PipPosition::TopLeft => Pos2::new(
                viewport_rect.min.x + padding,
                viewport_rect.min.y + padding,
            ),
            PipPosition::TopRight => Pos2::new(
                viewport_rect.max.x - pip_size.x - padding,
                viewport_rect.min.y + padding,
            ),
            PipPosition::BottomLeft => Pos2::new(
                viewport_rect.min.x + padding,
                viewport_rect.max.y - pip_size.y - padding,
            ),
            PipPosition::BottomRight => Pos2::new(
                viewport_rect.max.x - pip_size.x - padding,
                viewport_rect.max.y - pip_size.y - padding,
            ),
            PipPosition::Custom { x, y } => Pos2::new(
                viewport_rect.min.x + x * viewport_rect.width(),
                viewport_rect.min.y + y * viewport_rect.height(),
            ),
        };

        Rect::from_min_size(pos, pip_size)
    }
}

/// PiP 설정 UI (툴바 또는 설정 패널용)
pub fn render_pip_settings(ui: &mut Ui, pip: &mut PipOverlay) {
    ui.horizontal(|ui| {
        // 활성화 토글
        let pip_label = if pip.enabled { "🎮 PiP ●" } else { "🎮 PiP" };
        if ui.selectable_label(pip.enabled, pip_label).clicked() {
            pip.toggle();
        }

        if pip.enabled {
            ui.separator();

            // 위치 선택
            egui::ComboBox::from_id_salt("pip_position")
                .selected_text(pip.position.name())
                .show_ui(ui, |ui| {
                    for pos in PipPosition::presets() {
                        ui.selectable_value(&mut pip.position, *pos, pos.name());
                    }
                });

            // 크기 선택
            egui::ComboBox::from_id_salt("pip_size")
                .selected_text(pip.size.name())
                .show_ui(ui, |ui| {
                    for size in PipSize::presets() {
                        ui.selectable_value(&mut pip.size, *size, size.name());
                    }
                });

            // 투명도 슬라이더
            ui.add(
                egui::Slider::new(&mut pip.opacity, 0.3..=1.0)
                    .text("Opacity")
                    .show_value(false),
            );
        }
    });
}

// ============================================================================
// Keyboard Shortcuts Handler
// ============================================================================

/// PiP 키보드 단축키 결과
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipShortcutResult {
    /// 아무 일도 없음
    None,
    /// PiP 토글됨
    Toggled,
    /// 위치 순환됨
    PositionCycled,
    /// 줌 변경됨
    ZoomChanged,
}

/// PiP 키보드 단축키 핸들러
///
/// Scene/Game 뷰에서 키보드 입력을 처리합니다.
///
/// ## 단축키
/// - `P`: PiP 토글
/// - `Shift+P`: 위치 순환
/// - `Ctrl+P`: 줌 100%로 리셋
/// - `Alt+1~4`: 위치 프리셋 선택
pub struct PipShortcutHandler;

impl PipShortcutHandler {
    /// 키보드 입력 처리
    ///
    /// # Arguments
    /// * `ui` - egui UI (입력 읽기용)
    /// * `pip` - PiP 설정
    ///
    /// # Returns
    /// 처리된 단축키 결과
    pub fn handle_input(ui: &Ui, pip: &mut PipOverlay) -> PipShortcutResult {
        let modifiers = ui.input(|i| i.modifiers);

        // P 키 체크
        let p_pressed = ui.input(|i| i.key_pressed(egui::Key::P));

        if p_pressed {
            if modifiers.shift {
                // Shift+P: 위치 순환
                pip.cycle_position();
                return PipShortcutResult::PositionCycled;
            } else if modifiers.ctrl || modifiers.command {
                // Ctrl+P: 줌 리셋
                pip.zoom = 1.0;
                log::info!("[PiP] Zoom reset to 100%");
                return PipShortcutResult::ZoomChanged;
            } else if !modifiers.alt {
                // P: 토글 (Alt 없이)
                pip.toggle();
                return PipShortcutResult::Toggled;
            }
        }

        // Alt+1~4: 위치 프리셋
        if modifiers.alt {
            if ui.input(|i| i.key_pressed(egui::Key::Num1)) {
                pip.position = PipPosition::TopLeft;
                return PipShortcutResult::PositionCycled;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Num2)) {
                pip.position = PipPosition::TopRight;
                return PipShortcutResult::PositionCycled;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Num3)) {
                pip.position = PipPosition::BottomLeft;
                return PipShortcutResult::PositionCycled;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Num4)) {
                pip.position = PipPosition::BottomRight;
                return PipShortcutResult::PositionCycled;
            }
        }

        // +/-: 줌 조절 (PiP가 활성화된 경우)
        if pip.enabled {
            if ui.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
                pip.zoom = (pip.zoom + 0.1).min(2.0);
                return PipShortcutResult::ZoomChanged;
            }
            if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
                pip.zoom = (pip.zoom - 0.1).max(0.5);
                return PipShortcutResult::ZoomChanged;
            }
        }

        PipShortcutResult::None
    }

    /// 단축키 힌트 텍스트 (툴팁용)
    pub fn shortcut_hints() -> &'static str {
        "P: Toggle PiP\nShift+P: Cycle position\nCtrl+P: Reset zoom\nAlt+1~4: Position presets\n+/-: Zoom"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pip_position_cycle() {
        let mut pos = PipPosition::TopRight;
        pos = pos.cycle();
        assert_eq!(pos, PipPosition::BottomRight);
        pos = pos.cycle();
        assert_eq!(pos, PipPosition::BottomLeft);
        pos = pos.cycle();
        assert_eq!(pos, PipPosition::TopLeft);
        pos = pos.cycle();
        assert_eq!(pos, PipPosition::TopRight);
    }

    #[test]
    fn test_pip_size_to_pixels() {
        let parent = Vec2::new(1000.0, 800.0);

        assert_eq!(PipSize::Small.to_pixels(parent), Vec2::new(150.0, 120.0));
        assert_eq!(PipSize::Medium.to_pixels(parent), Vec2::new(250.0, 200.0));
        assert_eq!(PipSize::Large.to_pixels(parent), Vec2::new(350.0, 280.0));
    }

    #[test]
    fn test_pip_overlay_default() {
        let pip = PipOverlay::default();
        assert!(!pip.enabled);
        assert_eq!(pip.position, PipPosition::TopRight);
        assert_eq!(pip.size, PipSize::Medium);
        assert!((pip.opacity - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_pip_shortcut_result() {
        // 단축키 결과 enum 테스트
        assert_ne!(PipShortcutResult::None, PipShortcutResult::Toggled);
        assert_ne!(PipShortcutResult::PositionCycled, PipShortcutResult::ZoomChanged);
    }

    #[test]
    fn test_pip_zoom_clamp() {
        let mut pip = PipOverlay::default();

        pip.set_zoom(0.1);
        assert!((pip.zoom - 0.5).abs() < 0.001); // 최소 0.5

        pip.set_zoom(5.0);
        assert!((pip.zoom - 2.0).abs() < 0.001); // 최대 2.0

        pip.set_zoom(1.5);
        assert!((pip.zoom - 1.5).abs() < 0.001); // 범위 내
    }
}
