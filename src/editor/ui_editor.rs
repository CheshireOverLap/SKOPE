//! UI Editor Panel - Game UI 편집 도구
//!
//! skope_game_ui의 위젯 트리를 시각적으로 편집

use egui::{Color32, Ui, RichText, ScrollArea};
use skope_game_ui::{Widget, WidgetType, UiSystem};

/// UI Editor 상태
pub struct UiEditorState {
    /// 선택된 위젯 ID
    pub selected_widget_id: Option<String>,
    /// 위젯 트리 펼침 상태
    pub expanded_widgets: std::collections::HashSet<String>,
    /// 편집 모드 (Hierarchy, Canvas, Both)
    pub edit_mode: UiEditMode,
    /// 새 위젯 생성 메뉴 열림 상태
    pub show_create_menu: bool,
}

/// 편집 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiEditMode {
    #[default]
    Hierarchy,
    Canvas,
    Split,
}

impl Default for UiEditorState {
    fn default() -> Self {
        Self {
            selected_widget_id: None,
            expanded_widgets: std::collections::HashSet::new(),
            edit_mode: UiEditMode::Hierarchy,
            show_create_menu: false,
        }
    }
}

impl UiEditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// UI Editor 패널 렌더링
    pub fn ui(&mut self, ui: &mut Ui, mut game_ui: Option<&mut UiSystem>) {
        // 상단 툴바
        ui.horizontal(|ui| {
            ui.label(RichText::new("UI Editor").strong());
            ui.separator();

            // 모드 선택
            ui.selectable_value(&mut self.edit_mode, UiEditMode::Hierarchy, "Hierarchy");
            ui.selectable_value(&mut self.edit_mode, UiEditMode::Canvas, "Canvas");
            ui.selectable_value(&mut self.edit_mode, UiEditMode::Split, "Split");

            ui.separator();

            // 새 위젯 버튼
            if ui.button("+ Widget").clicked() {
                self.show_create_menu = !self.show_create_menu;
            }
        });

        ui.separator();

        // 새 위젯 생성 메뉴
        if self.show_create_menu {
            self.create_widget_menu(ui);
        }

        match self.edit_mode {
            UiEditMode::Hierarchy => {
                self.hierarchy_view(ui, game_ui.as_deref_mut());
            }
            UiEditMode::Canvas => {
                self.canvas_view(ui, game_ui.as_deref_mut());
            }
            UiEditMode::Split => {
                // 좌우 분할
                ui.columns(2, |cols| {
                    self.hierarchy_view(&mut cols[0], game_ui);
                    self.widget_inspector(&mut cols[1], None);
                });
            }
        }
    }

    /// 위젯 생성 메뉴
    fn create_widget_menu(&mut self, ui: &mut Ui) {
        egui::Frame::popup(ui.style())
            .show(ui, |ui| {
                ui.set_min_width(150.0);
                ui.label(RichText::new("Create Widget").size(11.0).color(Color32::GRAY));
                ui.separator();

                let widget_types = [
                    ("Container", "🗂"),
                    ("Text", "T"),
                    ("Button", "🔘"),
                    ("Image", "🖼"),
                    ("ProgressBar", "▰"),
                    ("InputField", "📝"),
                    ("ScrollView", "📜"),
                    ("Slider", "━"),
                    ("Toggle", "☑"),
                ];

                for (name, icon) in widget_types {
                    if ui.button(format!("{} {}", icon, name)).clicked() {
                        log::info!("[UI Editor] Create {} widget", name);
                        self.show_create_menu = false;
                        // TODO: 실제 위젯 생성
                    }
                }
            });
    }

    /// Hierarchy 뷰 (위젯 트리)
    fn hierarchy_view(&mut self, ui: &mut Ui, game_ui: Option<&mut UiSystem>) {
        let available_height = ui.available_height();
        let tree_height = available_height * 0.5;
        let inspector_height = available_height * 0.5;

        // 위젯 트리와 Inspector를 분리해서 game_ui 참조 문제 해결
        let has_ui = game_ui.is_some();
        let _root_exists = game_ui.as_ref().map(|g| g.root.is_some()).unwrap_or(false);

        // 위젯 정보 미리 수집
        let widget_info = if let Some(ref selected_id) = self.selected_widget_id {
            game_ui.as_ref().and_then(|g| g.get_widget(selected_id)).map(|w| {
                WidgetInfo {
                    id: w.id.clone(),
                    widget_type: widget_type_name(&w.widget_type).to_string(),
                    visible: w.visible,
                    interactive: w.interactive,
                    draggable: w.draggable,
                    drop_target: w.drop_target,
                    anchor: format!("{:?}", w.layout.anchor),
                    offset: w.layout.offset,
                    size: format!("{:?}", w.layout.size),
                    tooltip: w.tooltip.clone(),
                }
            })
        } else {
            None
        };

        // 위젯 트리
        ui.group(|ui| {
            ui.set_min_height(tree_height);
            ui.label(RichText::new("Widget Tree").size(11.0).color(Color32::from_rgb(150, 160, 180)));
            ui.separator();

            ScrollArea::vertical()
                .id_salt("ui_tree_scroll")
                .max_height(tree_height - 30.0)
                .show(ui, |ui| {
                    if has_ui {
                        if let Some(ref game_ui) = game_ui {
                            if let Some(ref root) = game_ui.root {
                                self.render_widget_tree(ui, root, 0);
                            } else {
                                ui.label(RichText::new("No UI loaded").size(10.0).color(Color32::GRAY));
                                ui.label(RichText::new("Load a .ron UI file or create widgets").size(9.0).color(Color32::from_rgb(100, 100, 110)));
                            }
                        }
                    } else {
                        ui.label(RichText::new("Game UI not available").size(10.0).color(Color32::GRAY));
                    }
                });
        });

        // 선택된 위젯 Inspector (미리 수집한 정보 사용)
        ui.group(|ui| {
            ui.set_min_height(inspector_height);
            self.widget_inspector_with_info(ui, widget_info);
        });
    }

    /// 위젯 트리 항목 렌더링 (재귀)
    fn render_widget_tree(&mut self, ui: &mut Ui, widget: &Widget, depth: usize) {
        let id = widget.id.clone().unwrap_or_else(|| format!("unnamed_{}", depth));
        let has_children = !widget.children.is_empty();

        let is_selected = self.selected_widget_id.as_ref() == Some(&id);
        let is_expanded = self.expanded_widgets.contains(&id);

        // 들여쓰기
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 16.0);

            // 펼침/접기 버튼
            if has_children {
                let arrow = if is_expanded { "▼" } else { "▶" };
                if ui.small_button(arrow).clicked() {
                    if is_expanded {
                        self.expanded_widgets.remove(&id);
                    } else {
                        self.expanded_widgets.insert(id.clone());
                    }
                }
            } else {
                ui.add_space(18.0); // 버튼 공간 유지
            }

            // 위젯 타입 아이콘
            let icon = widget_type_icon(&widget.widget_type);
            ui.label(RichText::new(icon).size(12.0).color(Color32::from_rgb(150, 180, 200)));

            // 위젯 이름 (클릭으로 선택)
            let label = if widget.id.is_some() {
                id.clone()
            } else {
                format!("({})", widget_type_name(&widget.widget_type))
            };

            let response = ui.selectable_label(is_selected, RichText::new(&label).size(11.0));
            if response.clicked() {
                self.selected_widget_id = Some(id.clone());
            }

            // 가시성 표시
            if !widget.visible {
                ui.label(RichText::new("👁").size(9.0).color(Color32::from_rgb(80, 80, 90)));
            }
        });

        // 자식 위젯들 (펼침 상태일 때만)
        if has_children && is_expanded {
            for child in &widget.children {
                self.render_widget_tree(ui, child, depth + 1);
            }
        }
    }

    /// 위젯 Inspector
    fn widget_inspector(&mut self, ui: &mut Ui, game_ui: Option<&mut UiSystem>) {
        ui.label(RichText::new("Inspector").size(11.0).color(Color32::from_rgb(150, 160, 180)));
        ui.separator();

        let Some(ref selected_id) = self.selected_widget_id else {
            ui.label(RichText::new("No widget selected").size(10.0).color(Color32::GRAY));
            return;
        };

        // 위젯 찾기 (immutable 참조로)
        let widget_info = if let Some(game_ui) = &game_ui {
            game_ui.get_widget(selected_id).map(|w| {
                // 필요한 정보만 복사
                WidgetInfo {
                    id: w.id.clone(),
                    widget_type: widget_type_name(&w.widget_type).to_string(),
                    visible: w.visible,
                    interactive: w.interactive,
                    draggable: w.draggable,
                    drop_target: w.drop_target,
                    anchor: format!("{:?}", w.layout.anchor),
                    offset: w.layout.offset,
                    size: format!("{:?}", w.layout.size),
                    tooltip: w.tooltip.clone(),
                }
            })
        } else {
            None
        };

        let Some(info) = widget_info else {
            ui.label(RichText::new("Widget not found").size(10.0).color(Color32::from_rgb(200, 100, 100)));
            return;
        };

        ScrollArea::vertical()
            .id_salt("widget_inspector_scroll")
            .show(ui, |ui| {
                // ID 섹션
                ui.collapsing("Identity", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("ID:");
                        ui.label(RichText::new(info.id.as_deref().unwrap_or("(none)")).monospace());
                    });
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        ui.label(RichText::new(&info.widget_type).color(Color32::from_rgb(150, 200, 150)));
                    });
                });

                // 상태 섹션
                ui.collapsing("State", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Visible:");
                        ui.label(if info.visible { "✓" } else { "✗" });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Interactive:");
                        ui.label(if info.interactive { "✓" } else { "✗" });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Draggable:");
                        ui.label(if info.draggable { "✓" } else { "✗" });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Drop Target:");
                        ui.label(if info.drop_target { "✓" } else { "✗" });
                    });
                });

                // 레이아웃 섹션
                ui.collapsing("Layout", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Anchor:");
                        ui.label(RichText::new(&info.anchor).monospace().size(10.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Offset:");
                        ui.label(RichText::new(format!("({:.1}, {:.1})", info.offset.0, info.offset.1)).monospace().size(10.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Size:");
                        ui.label(RichText::new(&info.size).monospace().size(10.0));
                    });
                });

                // 툴팁 섹션
                if let Some(ref tooltip) = info.tooltip {
                    ui.collapsing("Tooltip", |ui| {
                        ui.label(RichText::new(tooltip).size(10.0));
                    });
                }
            });
    }

    /// 위젯 Inspector (미리 수집된 정보 사용)
    fn widget_inspector_with_info(&mut self, ui: &mut Ui, widget_info: Option<WidgetInfo>) {
        ui.label(RichText::new("Inspector").size(11.0).color(Color32::from_rgb(150, 160, 180)));
        ui.separator();

        let Some(ref _selected_id) = self.selected_widget_id else {
            ui.label(RichText::new("No widget selected").size(10.0).color(Color32::GRAY));
            return;
        };

        let Some(info) = widget_info else {
            ui.label(RichText::new("Widget not found").size(10.0).color(Color32::from_rgb(200, 100, 100)));
            return;
        };

        ScrollArea::vertical()
            .id_salt("widget_inspector_scroll2")
            .show(ui, |ui| {
                // ID 섹션
                ui.collapsing("Identity", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("ID:");
                        ui.label(RichText::new(info.id.as_deref().unwrap_or("(none)")).monospace());
                    });
                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        ui.label(RichText::new(&info.widget_type).color(Color32::from_rgb(150, 200, 150)));
                    });
                });

                // 상태 섹션
                ui.collapsing("State", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Visible:");
                        ui.label(if info.visible { "✓" } else { "✗" });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Interactive:");
                        ui.label(if info.interactive { "✓" } else { "✗" });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Draggable:");
                        ui.label(if info.draggable { "✓" } else { "✗" });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Drop Target:");
                        ui.label(if info.drop_target { "✓" } else { "✗" });
                    });
                });

                // 레이아웃 섹션
                ui.collapsing("Layout", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Anchor:");
                        ui.label(RichText::new(&info.anchor).monospace().size(10.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Offset:");
                        ui.label(RichText::new(format!("({:.1}, {:.1})", info.offset.0, info.offset.1)).monospace().size(10.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Size:");
                        ui.label(RichText::new(&info.size).monospace().size(10.0));
                    });
                });

                // 툴팁 섹션
                if let Some(ref tooltip) = info.tooltip {
                    ui.collapsing("Tooltip", |ui| {
                        ui.label(RichText::new(tooltip).size(10.0));
                    });
                }
            });
    }

    /// Canvas 뷰 (시각적 미리보기 - 추후 구현)
    fn canvas_view(&mut self, ui: &mut Ui, _game_ui: Option<&mut UiSystem>) {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.label(RichText::new("🎨 Canvas View").size(16.0).color(Color32::from_rgb(100, 120, 150)));
            ui.add_space(10.0);
            ui.label(RichText::new("Visual UI editing coming soon...").size(11.0).color(Color32::GRAY));
            ui.add_space(20.0);
            ui.label(RichText::new("Use Hierarchy mode for now").size(10.0).color(Color32::from_rgb(80, 100, 120)));
        });
    }
}

/// 위젯 정보 (Inspector 표시용)
struct WidgetInfo {
    id: Option<String>,
    widget_type: String,
    visible: bool,
    interactive: bool,
    draggable: bool,
    drop_target: bool,
    anchor: String,
    offset: (f32, f32),
    size: String,
    tooltip: Option<String>,
}

/// 위젯 타입 아이콘
fn widget_type_icon(wt: &WidgetType) -> &'static str {
    match wt {
        WidgetType::Container => "🗂",
        WidgetType::Text { .. } => "T",
        WidgetType::Image { .. } => "🖼",
        WidgetType::NineSlice { .. } => "🔲",
        WidgetType::Button { .. } => "🔘",
        WidgetType::ProgressBar { .. } => "▰",
        WidgetType::ScrollView { .. } => "📜",
        WidgetType::InputField { .. } => "📝",
        WidgetType::Slider { .. } => "━",
        WidgetType::Toggle { .. } => "☑",
        WidgetType::Sprite { .. } => "🎴",
    }
}

/// 위젯 타입 이름
fn widget_type_name(wt: &WidgetType) -> &'static str {
    match wt {
        WidgetType::Container => "Container",
        WidgetType::Text { .. } => "Text",
        WidgetType::Image { .. } => "Image",
        WidgetType::NineSlice { .. } => "NineSlice",
        WidgetType::Button { .. } => "Button",
        WidgetType::ProgressBar { .. } => "ProgressBar",
        WidgetType::ScrollView { .. } => "ScrollView",
        WidgetType::InputField { .. } => "InputField",
        WidgetType::Slider { .. } => "Slider",
        WidgetType::Toggle { .. } => "Toggle",
        WidgetType::Sprite { .. } => "Sprite",
    }
}
