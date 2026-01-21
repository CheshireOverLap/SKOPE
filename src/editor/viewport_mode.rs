//! Viewport Mode System
//!
//! Scene/Game 뷰포트 분리 및 입력 라우팅을 위한 모드 시스템입니다.

use egui::{Color32, Ui, Rect};
use serde::{Deserialize, Serialize};

/// 뷰포트 모드 (Scene vs Game)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ViewportMode {
    /// Scene 뷰 (에디터 카메라, 기즈모, 그리드 표시)
    #[default]
    Scene,
    /// Game 뷰 (게임 카메라, UI 표시, 입력 캡처)
    Game,
}

impl ViewportMode {
    /// 뷰포트 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::Scene => "Scene",
            Self::Game => "Game",
        }
    }

    /// 아이콘
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Scene => "🎬",
            Self::Game => "🎮",
        }
    }

    /// 토글
    pub fn toggle(&mut self) {
        *self = match self {
            Self::Scene => Self::Game,
            Self::Game => Self::Scene,
        };
    }
}

/// 뷰포트 툴바 상태
#[derive(Debug, Clone, Default)]
pub struct ViewportToolbar {
    /// 현재 모드
    pub mode: ViewportMode,
    /// 툴바 높이
    pub height: f32,
    /// 호버된 버튼
    pub hovered_button: Option<ToolbarButton>,
}

/// 툴바 버튼 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolbarButton {
    /// Scene/Game 모드 전환
    ModeToggle,
    /// 그리드 토글
    Grid,
    /// 기즈모 토글
    Gizmos,
    /// 와이어프레임 토글
    Wireframe,
    /// 조명 시각화 토글
    Lights,
    /// 콜라이더 시각화 토글
    Colliders,
    /// 화면비 선택
    AspectRatio,
    /// 해상도 선택
    Resolution,
    /// 통계 표시 토글
    Stats,
}

/// 뷰포트 툴바 액션
#[derive(Debug, Clone)]
pub enum ViewportToolbarAction {
    /// 모드 변경
    ChangeMode(ViewportMode),
    /// 그리드 토글
    ToggleGrid,
    /// 기즈모 토글
    ToggleGizmos,
    /// 와이어프레임 토글
    ToggleWireframe,
    /// 조명 시각화 토글
    ToggleLights,
    /// 콜라이더 시각화 토글
    ToggleColliders,
    /// 화면비 변경
    SetAspectRatio(AspectRatio),
    /// 해상도 변경
    SetResolution(u32, u32),
    /// 통계 표시 토글
    ToggleStats,
}

/// 화면비 프리셋
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AspectRatio {
    /// 자유 (뷰포트 맞춤)
    #[default]
    Free,
    /// 16:9
    Ratio16x9,
    /// 16:10
    Ratio16x10,
    /// 4:3
    Ratio4x3,
    /// 21:9 (울트라와이드)
    Ratio21x9,
    /// 1:1 (정사각형)
    Ratio1x1,
}

impl AspectRatio {
    /// 화면비 이름
    pub fn name(&self) -> &'static str {
        match self {
            Self::Free => "Free",
            Self::Ratio16x9 => "16:9",
            Self::Ratio16x10 => "16:10",
            Self::Ratio4x3 => "4:3",
            Self::Ratio21x9 => "21:9",
            Self::Ratio1x1 => "1:1",
        }
    }

    /// 화면비 값 (width / height)
    pub fn value(&self) -> Option<f32> {
        match self {
            Self::Free => None,
            Self::Ratio16x9 => Some(16.0 / 9.0),
            Self::Ratio16x10 => Some(16.0 / 10.0),
            Self::Ratio4x3 => Some(4.0 / 3.0),
            Self::Ratio21x9 => Some(21.0 / 9.0),
            Self::Ratio1x1 => Some(1.0),
        }
    }

    /// 모든 화면비 목록
    pub fn all() -> &'static [Self] {
        &[
            Self::Free,
            Self::Ratio16x9,
            Self::Ratio16x10,
            Self::Ratio4x3,
            Self::Ratio21x9,
            Self::Ratio1x1,
        ]
    }
}

/// 해상도 프리셋
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
    pub name: &'static str,
}

impl Resolution {
    pub const PRESETS: &'static [Self] = &[
        Self { width: 1920, height: 1080, name: "1080p" },
        Self { width: 2560, height: 1440, name: "1440p" },
        Self { width: 3840, height: 2160, name: "4K" },
        Self { width: 1280, height: 720, name: "720p" },
        Self { width: 1366, height: 768, name: "768p" },
        Self { width: 800, height: 600, name: "SVGA" },
    ];
}

impl ViewportToolbar {
    /// 새 ViewportToolbar 생성
    pub fn new() -> Self {
        Self {
            mode: ViewportMode::Scene,
            height: 28.0,
            hovered_button: None,
        }
    }

    /// 툴바 UI 렌더링
    pub fn ui(
        &mut self,
        ui: &mut Ui,
        show_grid: bool,
        show_gizmos: bool,
        show_wireframe: bool,
        show_lights: bool,
        show_colliders: bool,
        show_stats: bool,
        aspect_ratio: AspectRatio,
    ) -> Option<ViewportToolbarAction> {
        let mut action: Option<ViewportToolbarAction> = None;

        ui.horizontal(|ui| {
            ui.set_height(self.height);

            // Scene/Game 모드 토글
            let mode_btn_color = if self.mode == ViewportMode::Scene {
                Color32::from_rgb(60, 80, 120)
            } else {
                Color32::from_rgb(80, 60, 120)
            };

            let mode_btn = egui::Button::new(
                egui::RichText::new(format!("{} {}", self.mode.icon(), self.mode.name()))
                    .size(11.0)
                    .color(Color32::WHITE)
            )
            .fill(mode_btn_color)
            .corner_radius(4.0);

            if ui.add(mode_btn).clicked() {
                self.mode.toggle();
                action = Some(ViewportToolbarAction::ChangeMode(self.mode));
            }

            ui.separator();

            // Scene 모드 전용 버튼
            if self.mode == ViewportMode::Scene {
                // 그리드 토글
                if self.toolbar_toggle_button(ui, "Grid", show_grid).clicked() {
                    action = Some(ViewportToolbarAction::ToggleGrid);
                }

                // 기즈모 토글
                if self.toolbar_toggle_button(ui, "Gizmos", show_gizmos).clicked() {
                    action = Some(ViewportToolbarAction::ToggleGizmos);
                }

                // 와이어프레임 토글
                if self.toolbar_toggle_button(ui, "Wire", show_wireframe).clicked() {
                    action = Some(ViewportToolbarAction::ToggleWireframe);
                }

                ui.separator();

                // 시각화 토글
                if self.toolbar_toggle_button(ui, "💡", show_lights).clicked() {
                    action = Some(ViewportToolbarAction::ToggleLights);
                }

                if self.toolbar_toggle_button(ui, "📦", show_colliders).clicked() {
                    action = Some(ViewportToolbarAction::ToggleColliders);
                }
            }

            // Game 모드 전용 버튼
            if self.mode == ViewportMode::Game {
                // 화면비 드롭다운
                egui::ComboBox::from_id_salt("aspect_ratio")
                    .selected_text(aspect_ratio.name())
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for ratio in AspectRatio::all() {
                            if ui.selectable_label(*ratio == aspect_ratio, ratio.name()).clicked() {
                                action = Some(ViewportToolbarAction::SetAspectRatio(*ratio));
                            }
                        }
                    });

                ui.separator();

                // 해상도 드롭다운
                egui::ComboBox::from_id_salt("resolution")
                    .selected_text("Resolution")
                    .width(70.0)
                    .show_ui(ui, |ui| {
                        for res in Resolution::PRESETS {
                            if ui.selectable_label(false, res.name).clicked() {
                                action = Some(ViewportToolbarAction::SetResolution(res.width, res.height));
                            }
                        }
                    });
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // 통계 토글
                if self.toolbar_toggle_button(ui, "Stats", show_stats).clicked() {
                    action = Some(ViewportToolbarAction::ToggleStats);
                }
            });
        });

        action
    }

    /// 툴바 토글 버튼
    fn toolbar_toggle_button(&self, ui: &mut Ui, label: &str, is_active: bool) -> egui::Response {
        let bg_color = if is_active {
            Color32::from_rgb(60, 100, 80)
        } else {
            Color32::from_rgb(45, 45, 50)
        };

        let text_color = if is_active {
            Color32::WHITE
        } else {
            Color32::from_rgb(140, 140, 150)
        };

        ui.add(
            egui::Button::new(
                egui::RichText::new(label)
                    .size(10.0)
                    .color(text_color)
            )
            .fill(bg_color)
            .corner_radius(3.0)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewport_mode_toggle() {
        let mut mode = ViewportMode::Scene;
        mode.toggle();
        assert_eq!(mode, ViewportMode::Game);
        mode.toggle();
        assert_eq!(mode, ViewportMode::Scene);
    }

    #[test]
    fn test_aspect_ratio() {
        assert!(AspectRatio::Free.value().is_none());
        assert!((AspectRatio::Ratio16x9.value().unwrap() - 1.778).abs() < 0.01);
    }
}
