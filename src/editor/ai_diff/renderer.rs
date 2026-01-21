//! AI Diff View 렌더러
//!
//! Git Diff 스타일로 변경 사항을 시각화합니다.

use egui::{Color32, RichText, Stroke, Ui};

use super::types::*;

/// Diff View 상태
#[derive(Debug, Default)]
pub struct DiffViewState {
    /// 현재 세션
    pub session: Option<AIDiffSession>,
    /// 스크롤 위치
    pub scroll_offset: f32,
    /// 접힌 변경 ID 목록
    pub collapsed_changes: std::collections::HashSet<DiffChangeId>,
    /// 라인 번호 표시 여부
    pub show_line_numbers: bool,
}

impl DiffViewState {
    /// 새 상태 생성
    pub fn new() -> Self {
        Self {
            session: None,
            scroll_offset: 0.0,
            collapsed_changes: std::collections::HashSet::new(),
            show_line_numbers: true,
        }
    }

    /// 세션 설정
    pub fn set_session(&mut self, session: AIDiffSession) {
        self.session = Some(session);
        self.collapsed_changes.clear();
        self.scroll_offset = 0.0;
    }

    /// 세션 닫기
    pub fn close(&mut self) {
        self.session = None;
    }

    /// 세션 있는지 확인
    pub fn has_session(&self) -> bool {
        self.session.is_some()
    }
}

/// Diff View 렌더링
pub fn render_diff_view(
    ui: &mut Ui,
    state: &mut DiffViewState,
) -> DiffViewAction {
    // 세션 없으면 빈 상태 표시
    if state.session.is_none() {
        ui.centered_and_justified(|ui| {
            ui.label("AI Diff가 없습니다");
        });
        return DiffViewAction::None;
    }

    let mut action = DiffViewAction::None;

    // 세션 정보 먼저 읽기 (불변 참조)
    let (changes_count, session_status, explanation) = {
        let session = state.session.as_ref().unwrap();
        (session.changes.len(), session.status, session.explanation.clone())
    };

    // 헤더
    ui.horizontal(|ui| {
        ui.label(RichText::new("🤖").size(16.0));
        ui.strong(format!(
            "AI가 {}개의 변경을 제안합니다",
            changes_count
        ));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 상태 표시
            let status_text = match session_status {
                DiffSessionStatus::Pending => ("⏳ Pending", Color32::YELLOW),
                DiffSessionStatus::Partial => ("◐ Partial", Color32::LIGHT_BLUE),
                DiffSessionStatus::Accepted => ("✓ Accepted", Color32::GREEN),
                DiffSessionStatus::Rejected => ("✗ Rejected", Color32::RED),
            };
            ui.colored_label(status_text.1, status_text.0);
        });
    });

    // AI 설명
    if !explanation.is_empty() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("💡").size(12.0));
            ui.label(
                RichText::new(&explanation)
                    .size(12.0)
                    .color(Color32::from_rgb(180, 180, 190)),
            );
        });
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // 변경 목록 - 인덱스로 접근하여 차용 문제 회피
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for i in 0..changes_count {
                let change_action = render_change_by_index(ui, state, i);
                if !matches!(change_action, DiffViewAction::None) {
                    action = change_action;
                }
                ui.add_space(8.0);
            }
        });

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    // 하단 액션 버튼
    ui.horizontal(|ui| {
        let accept_btn = egui::Button::new(
            RichText::new("✓ Accept All")
                .color(Color32::WHITE),
        )
        .fill(Color32::from_rgb(40, 120, 40));

        if ui.add(accept_btn).clicked() {
            action = DiffViewAction::AcceptAll;
        }

        let reject_btn = egui::Button::new(
            RichText::new("✗ Reject All")
                .color(Color32::WHITE),
        )
        .fill(Color32::from_rgb(120, 40, 40));

        if ui.add(reject_btn).clicked() {
            action = DiffViewAction::RejectAll;
        }

        ui.separator();

        let apply_btn = egui::Button::new("Apply Selected");
        if ui.add(apply_btn).clicked() {
            action = DiffViewAction::ApplySelected;
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Close").clicked() {
                action = DiffViewAction::Close;
            }
        });
    });

    // 액션 처리
    match &action {
        DiffViewAction::AcceptAll => {
            if let Some(s) = &mut state.session {
                s.accept_all();
            }
        }
        DiffViewAction::RejectAll => {
            if let Some(s) = &mut state.session {
                s.reject_all();
            }
        }
        DiffViewAction::AcceptChange(id) => {
            if let Some(s) = &mut state.session {
                if let Some(change) = s.changes.iter_mut().find(|c| c.id == *id) {
                    change.set_accepted(true);
                }
                s.update_status();
            }
        }
        DiffViewAction::RejectChange(id) => {
            if let Some(s) = &mut state.session {
                if let Some(change) = s.changes.iter_mut().find(|c| c.id == *id) {
                    change.set_accepted(false);
                }
                s.update_status();
            }
        }
        DiffViewAction::Close => {
            state.close();
        }
        _ => {}
    }

    action
}

/// 인덱스로 변경 렌더링 (차용 문제 회피)
fn render_change_by_index(
    ui: &mut Ui,
    state: &mut DiffViewState,
    index: usize,
) -> DiffViewAction {
    let mut action = DiffViewAction::None;

    // 필요한 데이터 먼저 추출
    let (change_id, is_collapsed, accepted, description, change_type_info) = {
        let session = state.session.as_ref().unwrap();
        let change = &session.changes[index];
        let is_collapsed = state.collapsed_changes.contains(&change.id);

        let type_info = match &change.change_type {
            DiffChangeType::Entity { entity_name, modifications, .. } => {
                ChangeTypeInfo::Entity {
                    entity_name: entity_name.clone(),
                    modifications: modifications.clone(),
                }
            }
            DiffChangeType::AddComponent { entity_name, component_type, initial_values, .. } => {
                ChangeTypeInfo::AddComponent {
                    entity_name: entity_name.clone(),
                    component_type: component_type.clone(),
                    initial_values: initial_values.clone(),
                }
            }
            DiffChangeType::RemoveComponent { entity_name, component_type, .. } => {
                ChangeTypeInfo::RemoveComponent {
                    entity_name: entity_name.clone(),
                    component_type: component_type.clone(),
                }
            }
            DiffChangeType::Code { file_path, hunks, .. } => {
                ChangeTypeInfo::Code {
                    file_path: file_path.display().to_string(),
                    hunks: hunks.clone(),
                }
            }
            DiffChangeType::CreateAsset { asset_path, asset_type, .. } => {
                ChangeTypeInfo::CreateAsset {
                    asset_path: asset_path.display().to_string(),
                    asset_type: asset_type.clone(),
                }
            }
        };

        (change.id, is_collapsed, change.accepted, change.description.clone(), type_info)
    };

    // 프레임 색상 (승인 상태에 따라)
    let frame_color = match accepted {
        Some(true) => Color32::from_rgba_unmultiplied(40, 100, 40, 50),
        Some(false) => Color32::from_rgba_unmultiplied(100, 40, 40, 50),
        None => Color32::from_rgb(25, 25, 30),
    };

    let border_color = match accepted {
        Some(true) => Color32::from_rgb(60, 140, 60),
        Some(false) => Color32::from_rgb(140, 60, 60),
        None => Color32::from_rgb(60, 60, 70),
    };

    egui::Frame::new()
        .fill(frame_color)
        .stroke(Stroke::new(1.0, border_color))
        .corner_radius(4.0)
        .inner_margin(8.0)
        .show(ui, |ui| {
            // 헤더
            ui.horizontal(|ui| {
                // 접기/펼치기
                let arrow = if is_collapsed { "▶" } else { "▼" };
                if ui.small_button(arrow).clicked() {
                    if is_collapsed {
                        state.collapsed_changes.remove(&change_id);
                    } else {
                        state.collapsed_changes.insert(change_id);
                    }
                }

                // 아이콘 + 제목
                let (icon, title) = match &change_type_info {
                    ChangeTypeInfo::Entity { entity_name, .. } => {
                        ("🎮", format!("Entity: {}", entity_name))
                    }
                    ChangeTypeInfo::AddComponent { entity_name, component_type, .. } => {
                        ("➕", format!("{} + {}", entity_name, component_type))
                    }
                    ChangeTypeInfo::RemoveComponent { entity_name, component_type, .. } => {
                        ("➖", format!("{} - {}", entity_name, component_type))
                    }
                    ChangeTypeInfo::Code { file_path, .. } => {
                        ("📄", file_path.clone())
                    }
                    ChangeTypeInfo::CreateAsset { asset_path, .. } => {
                        ("📦", asset_path.clone())
                    }
                };

                ui.label(icon);
                ui.strong(&title);

                // 설명
                if !description.is_empty() {
                    ui.weak(format!("- {}", description));
                }

                // 승인/거부 버튼
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match accepted {
                        None => {
                            if ui.small_button("✓").clicked() {
                                action = DiffViewAction::AcceptChange(change_id);
                            }
                            if ui.small_button("✗").clicked() {
                                action = DiffViewAction::RejectChange(change_id);
                            }
                        }
                        Some(true) => {
                            ui.colored_label(Color32::GREEN, "✓ Accepted");
                            if ui.small_button("Undo").clicked() {
                                // Undo - 세션 변경
                                if let Some(s) = &mut state.session {
                                    if let Some(c) = s.changes.iter_mut().find(|c| c.id == change_id) {
                                        c.accepted = None;
                                    }
                                }
                            }
                        }
                        Some(false) => {
                            ui.colored_label(Color32::RED, "✗ Rejected");
                            if ui.small_button("Undo").clicked() {
                                // Undo - 세션 변경
                                if let Some(s) = &mut state.session {
                                    if let Some(c) = s.changes.iter_mut().find(|c| c.id == change_id) {
                                        c.accepted = None;
                                    }
                                }
                            }
                        }
                    }
                });
            });

            // 내용 (접히지 않았을 때)
            if !is_collapsed {
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);

                match &change_type_info {
                    ChangeTypeInfo::Entity { modifications, .. } => {
                        render_entity_modifications(ui, modifications);
                    }
                    ChangeTypeInfo::AddComponent { component_type, initial_values, .. } => {
                        render_component_add(ui, component_type, initial_values);
                    }
                    ChangeTypeInfo::RemoveComponent { component_type, .. } => {
                        ui.colored_label(
                            Color32::from_rgb(255, 100, 100),
                            format!("- Remove {} component", component_type),
                        );
                    }
                    ChangeTypeInfo::Code { hunks, .. } => {
                        render_code_diff(ui, hunks, state.show_line_numbers);
                    }
                    ChangeTypeInfo::CreateAsset { asset_type, .. } => {
                        ui.colored_label(
                            Color32::from_rgb(100, 255, 100),
                            format!("+ Create new {} asset", asset_type),
                        );
                    }
                }
            }
        });

    action
}

/// 변경 타입 정보 (렌더링용)
#[derive(Clone)]
enum ChangeTypeInfo {
    Entity {
        entity_name: String,
        modifications: Vec<EntityModification>,
    },
    AddComponent {
        entity_name: String,
        component_type: String,
        initial_values: std::collections::HashMap<String, DiffValue>,
    },
    RemoveComponent {
        entity_name: String,
        component_type: String,
    },
    Code {
        file_path: String,
        hunks: Vec<DiffHunk>,
    },
    CreateAsset {
        asset_path: String,
        asset_type: String,
    },
}

/// 엔티티 수정 렌더링
fn render_entity_modifications(ui: &mut Ui, modifications: &[EntityModification]) {
    for modification in modifications {
        ui.horizontal(|ui| {
            ui.colored_label(Color32::YELLOW, "~");
            ui.label(format!(
                "{}.{}",
                modification.component_type, modification.field_path
            ));
        });

        ui.horizontal(|ui| {
            ui.label("  ");
            ui.colored_label(
                Color32::from_rgb(255, 100, 100),
                format!("- {}", modification.old_value),
            );
        });

        ui.horizontal(|ui| {
            ui.label("  ");
            ui.colored_label(
                Color32::from_rgb(100, 255, 100),
                format!("+ {}", modification.new_value),
            );
        });
    }
}

/// 컴포넌트 추가 렌더링
fn render_component_add(
    ui: &mut Ui,
    component_type: &str,
    initial_values: &std::collections::HashMap<String, DiffValue>,
) {
    ui.colored_label(
        Color32::from_rgb(100, 255, 100),
        format!("+ Add {} component", component_type),
    );

    for (key, value) in initial_values {
        ui.horizontal(|ui| {
            ui.label("  ");
            ui.weak(format!("{}: {}", key, value));
        });
    }
}

/// 코드 Diff 렌더링
fn render_code_diff(ui: &mut Ui, hunks: &[DiffHunk], show_line_numbers: bool) {
    let font = egui::FontId::monospace(12.0);

    for hunk in hunks {
        // 청크 헤더
        ui.colored_label(
            Color32::from_rgb(100, 149, 237),
            format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, hunk.old_count, hunk.new_start, hunk.new_count
            ),
        );

        for line in &hunk.lines {
            let (bg_color, text_color) = match line.line_type {
                DiffLineType::Context => (
                    Color32::TRANSPARENT,
                    Color32::from_rgb(180, 180, 180),
                ),
                DiffLineType::Addition => (
                    Color32::from_rgba_unmultiplied(0, 100, 0, 50),
                    Color32::from_rgb(100, 255, 100),
                ),
                DiffLineType::Deletion => (
                    Color32::from_rgba_unmultiplied(100, 0, 0, 50),
                    Color32::from_rgb(255, 100, 100),
                ),
            };

            ui.horizontal(|ui| {
                // 라인 번호
                if show_line_numbers {
                    let old_num = line
                        .old_line_number
                        .map(|n| format!("{:4}", n))
                        .unwrap_or_else(|| "    ".to_string());
                    let new_num = line
                        .new_line_number
                        .map(|n| format!("{:4}", n))
                        .unwrap_or_else(|| "    ".to_string());

                    ui.colored_label(Color32::DARK_GRAY, format!("{} {}", old_num, new_num));
                }

                // 라인 내용
                let prefix = line.line_type.prefix();
                let line_text = format!("{} {}", prefix, line.content);

                egui::Frame::new()
                    .fill(bg_color)
                    .show(ui, |ui| {
                        ui.label(RichText::new(&line_text).font(font.clone()).color(text_color));
                    });
            });
        }
    }
}

/// Diff View 윈도우로 표시
pub fn render_diff_view_window(
    ctx: &egui::Context,
    state: &mut DiffViewState,
) -> DiffViewAction {
    if !state.has_session() {
        return DiffViewAction::None;
    }

    let mut action = DiffViewAction::None;

    egui::Window::new("AI Diff View")
        .default_size([600.0, 500.0])
        .resizable(true)
        .collapsible(true)
        .show(ctx, |ui| {
            action = render_diff_view(ui, state);
        });

    action
}
