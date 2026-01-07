//! Asset Browser (유니티 스타일)
//!
//! - 리스트/그리드 뷰 전환
//! - 슬라이더로 아이콘 크기 조절
//! - 드래그 앤 드롭 지원
//! - 더블클릭으로 에셋 열기

use egui::{Color32, Response, Sense, Ui, Vec2};
use std::path::PathBuf;

/// Asset Browser 액션 (UI에서 반환)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetBrowserAction {
    /// 아무 액션 없음
    None,
    /// 파일 열기 (더블클릭)
    OpenFile(PathBuf),
    /// 새 UI Layout 생성
    CreateUiLayout,
    /// 새 폴더 생성
    CreateFolder,
    /// 폴더 이동 (더블클릭 on directory)
    NavigateTo(PathBuf),
}

/// 뷰 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    List,
    Grid,
}

/// Asset Browser 상태
#[derive(Debug, Clone)]
pub struct AssetBrowserState {
    /// 현재 디렉토리
    pub current_dir: PathBuf,
    /// 아이콘 크기 (16 ~ 128)
    pub icon_size: f32,
    /// 선택된 에셋
    pub selected: Option<PathBuf>,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        Self {
            current_dir: PathBuf::from("assets/models"),
            icon_size: 64.0,
            selected: None,
        }
    }
}

impl AssetBrowserState {
    /// 뷰 모드 결정 (아이콘 크기 기반)
    pub fn view_mode(&self) -> ViewMode {
        if self.icon_size <= 20.0 {
            ViewMode::List
        } else {
            ViewMode::Grid
        }
    }

    /// Asset Browser UI - 액션 반환
    pub fn ui(&mut self, ui: &mut Ui) -> AssetBrowserAction {
        let mut action = AssetBrowserAction::None;

        // 상단: 경로 표시 + 버튼들 + 슬라이더
        ui.horizontal(|ui| {
            // 상위 폴더 버튼
            if ui.small_button("⬆").on_hover_text("상위 폴더").clicked() {
                if let Some(parent) = self.current_dir.parent() {
                    action = AssetBrowserAction::NavigateTo(parent.to_path_buf());
                }
            }

            // Breadcrumb 경로
            ui.label(format!("📁 {}", self.current_dir.display()));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // 아이콘 크기 슬라이더
                ui.spacing_mut().slider_width = 80.0;
                ui.add(
                    egui::Slider::new(&mut self.icon_size, 16.0..=96.0)
                        .show_value(false)
                        .trailing_fill(true)
                );

                // Create 메뉴
                ui.menu_button("➕ Create", |ui| {
                    if ui.button("📐 UI Layout").clicked() {
                        action = AssetBrowserAction::CreateUiLayout;
                        ui.close_menu();
                    }
                    if ui.button("📁 Folder").clicked() {
                        action = AssetBrowserAction::CreateFolder;
                        ui.close_menu();
                    }
                });
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // 파일 목록
        let view_mode = self.view_mode();
        let list_action = egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                match view_mode {
                    ViewMode::List => self.render_list_view(ui),
                    ViewMode::Grid => self.render_grid_view(ui),
                }
            })
            .inner;

        // 파일 목록에서 반환된 액션 우선
        if list_action != AssetBrowserAction::None {
            action = list_action;
        }

        // NavigateTo 액션 즉시 처리
        if let AssetBrowserAction::NavigateTo(ref path) = action {
            self.current_dir = path.clone();
            return AssetBrowserAction::None; // 내부 처리 완료
        }

        action
    }

    /// 리스트 뷰 렌더링
    fn render_list_view(&mut self, ui: &mut Ui) -> AssetBrowserAction {
        let entries = self.get_entries();
        let mut action = AssetBrowserAction::None;

        if entries.is_empty() {
            Self::empty_state(ui, "No assets found");
            return action;
        }

        for entry in entries {
            let is_selected = self.selected.as_ref() == Some(&entry.path);
            let response = self.render_list_item(ui, &entry, is_selected);

            // 클릭 - 선택
            if response.clicked() {
                self.selected = Some(entry.path.clone());
            }

            // 더블클릭 - 열기/탐색
            if response.double_clicked() {
                if entry.is_dir {
                    action = AssetBrowserAction::NavigateTo(entry.path.clone());
                } else {
                    action = AssetBrowserAction::OpenFile(entry.path.clone());
                }
            }

            // 드래그 앤 드롭
            if entry.is_model || entry.is_ui_layout {
                response.dnd_set_drag_payload(entry.path.to_string_lossy().to_string());
            }
        }

        action
    }

    /// 그리드 뷰 렌더링
    fn render_grid_view(&mut self, ui: &mut Ui) -> AssetBrowserAction {
        let entries = self.get_entries();
        let mut action = AssetBrowserAction::None;

        if entries.is_empty() {
            Self::empty_state(ui, "No assets found");
            return action;
        }

        let icon_size = self.icon_size;
        let spacing = 8.0;
        let item_width = icon_size + spacing * 2.0;
        let available_width = ui.available_width();
        let columns = ((available_width / item_width).floor() as usize).max(1);

        egui::Grid::new("asset_grid")
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for (i, entry) in entries.iter().enumerate() {
                    let is_selected = self.selected.as_ref() == Some(&entry.path);
                    let response = self.render_grid_item(ui, entry, is_selected, icon_size);

                    // 클릭 - 선택
                    if response.clicked() {
                        self.selected = Some(entry.path.clone());
                    }

                    // 더블클릭 - 열기/탐색
                    if response.double_clicked() {
                        if entry.is_dir {
                            action = AssetBrowserAction::NavigateTo(entry.path.clone());
                        } else {
                            action = AssetBrowserAction::OpenFile(entry.path.clone());
                        }
                    }

                    // 드래그 앤 드롭
                    if entry.is_model || entry.is_ui_layout {
                        response.dnd_set_drag_payload(entry.path.to_string_lossy().to_string());
                    }

                    if (i + 1) % columns == 0 {
                        ui.end_row();
                    }
                }
            });

        action
    }

    /// 리스트 아이템 렌더링
    fn render_list_item(&self, ui: &mut Ui, entry: &AssetEntry, selected: bool) -> Response {
        let height = 22.0;
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            Sense::click_and_drag(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // 배경 (선택/호버)
            let bg_color = if selected {
                Color32::from_rgb(50, 80, 120)
            } else if response.hovered() {
                Color32::from_rgb(45, 48, 55)
            } else {
                Color32::TRANSPARENT
            };

            if bg_color != Color32::TRANSPARENT {
                painter.rect_filled(rect, 2.0, bg_color);
            }

            // 아이콘
            let icon = entry.icon();
            let icon_color = entry.icon_color();
            painter.text(
                rect.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(14.0),
                icon_color,
            );

            // 파일명
            painter.text(
                rect.left_center() + egui::vec2(28.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &entry.name,
                egui::FontId::proportional(13.0),
                Color32::from_rgb(200, 205, 215),
            );

            // 파일 크기
            if let Some(size) = entry.size_str() {
                painter.text(
                    rect.right_center() - egui::vec2(8.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    size,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(100, 105, 115),
                );
            }
        }

        response
    }

    /// 그리드 아이템 렌더링
    fn render_grid_item(&self, ui: &mut Ui, entry: &AssetEntry, selected: bool, icon_size: f32) -> Response {
        let padding = 6.0;
        let text_height = 16.0;
        let total_height = icon_size + padding * 2.0 + text_height;
        let total_width = icon_size + padding * 2.0;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(total_width, total_height),
            Sense::click_and_drag(),
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            // 배경
            let bg_color = if selected {
                Color32::from_rgb(50, 80, 120)
            } else if response.hovered() {
                Color32::from_rgb(45, 48, 55)
            } else {
                Color32::TRANSPARENT
            };

            if bg_color != Color32::TRANSPARENT {
                painter.rect_filled(rect, 4.0, bg_color);
            }

            // 아이콘 영역
            let icon_rect = egui::Rect::from_min_size(
                rect.min + egui::vec2(padding, padding),
                Vec2::splat(icon_size),
            );

            // 아이콘 배경
            painter.rect_filled(icon_rect, 4.0, Color32::from_rgb(55, 58, 65));

            // 아이콘
            let icon = entry.icon();
            let icon_color = entry.icon_color();
            let icon_font_size = (icon_size * 0.5).clamp(16.0, 48.0);
            painter.text(
                icon_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(icon_font_size),
                icon_color,
            );

            // 파일명 (하단, 잘림 처리)
            let text_rect = egui::Rect::from_min_size(
                egui::pos2(rect.min.x, icon_rect.max.y + 2.0),
                Vec2::new(total_width, text_height),
            );

            let truncated_name = Self::truncate_text(&entry.name, total_width - 4.0, 11.0);
            painter.text(
                text_rect.center(),
                egui::Align2::CENTER_CENTER,
                truncated_name,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(180, 185, 195),
            );
        }

        response
    }

    /// 텍스트 잘라내기
    fn truncate_text(text: &str, max_width: f32, font_size: f32) -> String {
        let char_width = font_size * 0.6;
        let max_chars = (max_width / char_width) as usize;

        if text.len() <= max_chars {
            text.to_string()
        } else if max_chars > 3 {
            format!("{}...", &text[..max_chars - 3])
        } else {
            "...".to_string()
        }
    }

    /// 빈 상태 표시
    fn empty_state(ui: &mut Ui, message: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                egui::RichText::new("📂")
                    .size(32.0)
                    .color(Color32::from_rgb(80, 85, 95)),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(message)
                    .size(13.0)
                    .color(Color32::from_rgb(100, 105, 115)),
            );
        });
    }

    /// 파일 목록 가져오기
    fn get_entries(&self) -> Vec<AssetEntry> {
        let mut entries = Vec::new();

        if let Ok(read_dir) = std::fs::read_dir(&self.current_dir) {
            for entry in read_dir.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    let path = entry.path();
                    let is_dir = path.is_dir();
                    let is_model = name.ends_with(".glb") || name.ends_with(".gltf");
                    let is_ui_layout = name.ends_with(".ui.ron");
                    let size = if is_dir {
                        None
                    } else {
                        entry.metadata().ok().map(|m| m.len())
                    };

                    entries.push(AssetEntry {
                        name: name.to_string(),
                        path,
                        is_dir,
                        is_model,
                        is_ui_layout,
                        size,
                    });
                }
            }
        }

        // 정렬: 폴더 먼저, 그 다음 이름순
        entries.sort_by(|a, b| {
            match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        entries
    }
}

/// 에셋 항목
struct AssetEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    is_model: bool,
    is_ui_layout: bool,
    size: Option<u64>,
}

impl AssetEntry {
    fn icon(&self) -> &'static str {
        if self.is_dir {
            "📁"
        } else if self.is_ui_layout {
            "📐"  // UI Layout
        } else if self.is_model {
            "🎮"
        } else if self.name.ends_with(".png") || self.name.ends_with(".jpg") {
            "🖼"
        } else if self.name.ends_with(".lua") || self.name.ends_with(".rs") {
            "📜"
        } else {
            "📄"
        }
    }

    fn icon_color(&self) -> Color32 {
        if self.is_dir {
            Color32::from_rgb(220, 180, 100)
        } else if self.is_ui_layout {
            Color32::from_rgb(180, 140, 220)  // 보라색 - UI Layout
        } else if self.is_model {
            Color32::from_rgb(100, 180, 220)
        } else {
            Color32::from_rgb(150, 155, 165)
        }
    }

    fn size_str(&self) -> Option<String> {
        self.size.map(|s| {
            if s < 1024 {
                format!("{} B", s)
            } else if s < 1024 * 1024 {
                format!("{:.1} KB", s as f64 / 1024.0)
            } else {
                format!("{:.1} MB", s as f64 / (1024.0 * 1024.0))
            }
        })
    }
}
