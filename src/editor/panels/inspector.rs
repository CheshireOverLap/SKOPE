//! Inspector 패널
//!
//! 선택된 엔티티의 컴포넌트 속성을 표시/편집
//! - Transform 편집 (Position, Rotation, Scale)
//! - 값 변경 시 Command 생성 (Undo/Redo 지원)

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use fyrox_core::algebra::Vector3;
use fyrox_core::pool::Handle;
use fyrox_ui::{
    grid::{Column, GridBuilder, Row},
    message::{MessageDirection, UiMessage},
    scroll_viewer::ScrollViewerBuilder,
    stack_panel::StackPanelBuilder,
    text::TextBuilder,
    vec::{Vec3EditorBuilder, Vec3EditorMessage},
    widget::WidgetBuilder,
    window::{WindowBuilder, WindowTitle},
    Orientation, Thickness, UiNode, UserInterface,
};
use glam::{EulerRot, Quat, Vec3};

use crate::ecs_components::{NodeName, Transform};
use crate::editor::command::{CommandStack, SetTransformCommand};

/// Inspector 패널
pub struct InspectorPanel {
    /// 윈도우 핸들
    pub window: Handle<UiNode>,
    /// 현재 표시 중인 엔티티
    current_entity: Option<Entity>,

    /// 엔티티 이름 표시
    name_text: Handle<UiNode>,
    /// 부모 정보 표시
    parent_text: Handle<UiNode>,
    /// 자식 정보 표시
    children_text: Handle<UiNode>,

    /// Position 에디터
    position_editor: Handle<UiNode>,
    /// Rotation 에디터 (Euler degrees)
    rotation_editor: Handle<UiNode>,
    /// Scale 에디터
    scale_editor: Handle<UiNode>,

    /// World→UI 동기화 중 플래그 (무한 루프 방지)
    is_updating_from_world: bool,
}

impl InspectorPanel {
    /// 새 Inspector 패널 생성
    pub fn new(ui: &mut UserInterface) -> Self {
        let ctx = &mut ui.build_ctx();

        // 엔티티 이름 표시
        let name_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_height(24.0),
        )
        .with_text("No Selection")
        .build(ctx);

        // 부모 정보 표시
        let parent_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness {
                    left: 5.0,
                    top: 0.0,
                    right: 5.0,
                    bottom: 2.0,
                })
                .with_height(18.0),
        )
        .with_text("Parent: None")
        .build(ctx);

        // 자식 정보 표시
        let children_text = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness {
                    left: 5.0,
                    top: 0.0,
                    right: 5.0,
                    bottom: 5.0,
                })
                .with_height(18.0),
        )
        .with_text("Children: 0")
        .build(ctx);

        // Transform 섹션 헤더
        let transform_header = TextBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness {
                    left: 5.0,
                    top: 10.0,
                    right: 5.0,
                    bottom: 2.0,
                })
                .with_height(20.0),
        )
        .with_text("Transform")
        .build(ctx);

        // Position 라벨 + 에디터
        let position_label = TextBuilder::new(
            WidgetBuilder::new()
                .on_row(0)
                .on_column(0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Position")
        .build(ctx);

        let position_editor = Vec3EditorBuilder::<f32>::new(
            WidgetBuilder::new()
                .on_row(0)
                .on_column(1)
                .with_height(22.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_value(Vector3::new(0.0, 0.0, 0.0))
        .with_step(Vector3::new(0.1, 0.1, 0.1))
        .with_precision(3)
        .build(ctx);

        // Rotation 라벨 + 에디터
        let rotation_label = TextBuilder::new(
            WidgetBuilder::new()
                .on_row(1)
                .on_column(0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Rotation")
        .build(ctx);

        let rotation_editor = Vec3EditorBuilder::<f32>::new(
            WidgetBuilder::new()
                .on_row(1)
                .on_column(1)
                .with_height(22.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_value(Vector3::new(0.0, 0.0, 0.0))
        .with_step(Vector3::new(1.0, 1.0, 1.0)) // 1도 단위
        .with_precision(1)
        .build(ctx);

        // Scale 라벨 + 에디터
        let scale_label = TextBuilder::new(
            WidgetBuilder::new()
                .on_row(2)
                .on_column(0)
                .with_margin(Thickness::uniform(4.0)),
        )
        .with_text("Scale")
        .build(ctx);

        let scale_editor = Vec3EditorBuilder::<f32>::new(
            WidgetBuilder::new()
                .on_row(2)
                .on_column(1)
                .with_height(22.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_value(Vector3::new(1.0, 1.0, 1.0))
        .with_step(Vector3::new(0.1, 0.1, 0.1))
        .with_precision(3)
        .build(ctx);

        // Transform Grid (Label | Editor)
        let transform_grid = GridBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(5.0))
                .with_child(position_label)
                .with_child(position_editor)
                .with_child(rotation_label)
                .with_child(rotation_editor)
                .with_child(scale_label)
                .with_child(scale_editor),
        )
        .add_column(Column::strict(70.0))
        .add_column(Column::stretch())
        .add_row(Row::strict(26.0))
        .add_row(Row::strict(26.0))
        .add_row(Row::strict(26.0))
        .build(ctx);

        // 컨텐츠 스택
        let content = StackPanelBuilder::new(
            WidgetBuilder::new()
                .with_child(name_text)
                .with_child(parent_text)
                .with_child(children_text)
                .with_child(transform_header)
                .with_child(transform_grid),
        )
        .with_orientation(Orientation::Vertical)
        .build(ctx);

        // 스크롤 뷰어
        let scroll_viewer = ScrollViewerBuilder::new(WidgetBuilder::new())
            .with_content(content)
            .build(ctx);

        // 윈도우
        let window = WindowBuilder::new(
            WidgetBuilder::new()
                .with_width(300.0)
                .with_height(350.0)
                .with_desired_position(fyrox_core::algebra::Vector2::new(980.0, 50.0)),
        )
        .with_title(WindowTitle::text("Inspector"))
        .with_content(scroll_viewer)
        .build(ctx);

        Self {
            window,
            current_entity: None,
            name_text,
            parent_text,
            children_text,
            position_editor,
            rotation_editor,
            scale_editor,
            is_updating_from_world: false,
        }
    }

    /// 선택 변경 시 호출
    pub fn set_entity(&mut self, entity: Option<Entity>, world: &World, ui: &UserInterface) {
        self.current_entity = entity;
        self.sync_from_world(world, ui);
    }

    /// World에서 UI로 값 동기화
    pub fn sync_from_world(&mut self, world: &World, ui: &UserInterface) {
        self.is_updating_from_world = true;

        if let Some(entity) = self.current_entity {
            // 이름 업데이트
            let name = if let Some(node_name) = world.get::<NodeName>(entity) {
                node_name.0.clone()
            } else {
                format!("Entity {:?}", entity)
            };

            ui.send(
                self.name_text,
                fyrox_ui::text::TextMessage::Text(name),
            );

            // 부모 정보 업데이트
            let parent_info = if let Some(parent) = world.get::<Parent>(entity) {
                let parent_entity = parent.get();
                let parent_name = world
                    .get::<NodeName>(parent_entity)
                    .map(|n| n.0.clone())
                    .unwrap_or_else(|| format!("{:?}", parent_entity));
                format!("Parent: {}", parent_name)
            } else {
                "Parent: None (Root)".to_string()
            };
            ui.send(
                self.parent_text,
                fyrox_ui::text::TextMessage::Text(parent_info),
            );

            // 자식 정보 업데이트
            let children_info = if let Some(children) = world.get::<Children>(entity) {
                let count = children.len();
                if count <= 3 {
                    let names: Vec<String> = children
                        .iter()
                        .filter_map(|&child| {
                            world.get::<NodeName>(child).map(|n| n.0.clone())
                        })
                        .collect();
                    if names.is_empty() {
                        format!("Children: {}", count)
                    } else {
                        format!("Children: {}", names.join(", "))
                    }
                } else {
                    format!("Children: {} items", count)
                }
            } else {
                "Children: 0".to_string()
            };
            ui.send(
                self.children_text,
                fyrox_ui::text::TextMessage::Text(children_info),
            );

            // Transform 업데이트
            if let Some(transform) = world.get::<Transform>(entity) {
                // Position
                let pos = transform.translation;
                ui.send(
                    self.position_editor,
                    Vec3EditorMessage::Value(Vector3::new(pos.x, pos.y, pos.z)),
                );

                // Rotation (Quat → Euler degrees)
                let (x, y, z) = transform.rotation.to_euler(EulerRot::XYZ);
                ui.send(
                    self.rotation_editor,
                    Vec3EditorMessage::Value(Vector3::new(
                        x.to_degrees(),
                        y.to_degrees(),
                        z.to_degrees(),
                    )),
                );

                // Scale
                let scale = transform.scale;
                ui.send(
                    self.scale_editor,
                    Vec3EditorMessage::Value(Vector3::new(scale.x, scale.y, scale.z)),
                );
            }
        } else {
            // 선택 없음
            ui.send(
                self.name_text,
                fyrox_ui::text::TextMessage::Text("No Selection".to_string()),
            );
            ui.send(
                self.parent_text,
                fyrox_ui::text::TextMessage::Text("Parent: -".to_string()),
            );
            ui.send(
                self.children_text,
                fyrox_ui::text::TextMessage::Text("Children: -".to_string()),
            );

            // 에디터 초기화
            ui.send(
                self.position_editor,
                Vec3EditorMessage::Value(Vector3::new(0.0, 0.0, 0.0)),
            );
            ui.send(
                self.rotation_editor,
                Vec3EditorMessage::Value(Vector3::new(0.0, 0.0, 0.0)),
            );
            ui.send(
                self.scale_editor,
                Vec3EditorMessage::Value(Vector3::new(1.0, 1.0, 1.0)),
            );
        }

        self.is_updating_from_world = false;
    }

    /// UI 메시지 처리
    /// 반환값: Transform이 변경되었는지 여부
    pub fn handle_message(
        &mut self,
        message: &UiMessage,
        world: &mut World,
        commands: &mut CommandStack,
    ) -> bool {
        // World→UI 동기화 중이면 무시 (무한 루프 방지)
        if self.is_updating_from_world {
            return false;
        }

        // Vec3Editor 값 변경 감지
        if let Some(Vec3EditorMessage::Value(new_value)) =
            message.data::<Vec3EditorMessage<f32>>()
        {
            // FromWidget 메시지만 처리 (사용자 입력)
            if message.direction() == MessageDirection::FromWidget {
                let entity = match self.current_entity {
                    Some(e) => e,
                    None => return false,
                };

                let old_transform = match world.get::<Transform>(entity) {
                    Some(t) => t.clone(),
                    None => return false,
                };

                let mut new_transform = old_transform.clone();

                // 어떤 에디터에서 온 메시지인지 확인
                if message.destination() == self.position_editor {
                    new_transform.translation =
                        Vec3::new(new_value.x, new_value.y, new_value.z);
                    log::info!(
                        "[Inspector] Position changed: ({:.2}, {:.2}, {:.2})",
                        new_value.x, new_value.y, new_value.z
                    );
                } else if message.destination() == self.rotation_editor {
                    // Euler degrees → Quat
                    let euler_rad = Vec3::new(
                        new_value.x.to_radians(),
                        new_value.y.to_radians(),
                        new_value.z.to_radians(),
                    );
                    new_transform.rotation =
                        Quat::from_euler(EulerRot::XYZ, euler_rad.x, euler_rad.y, euler_rad.z);
                    log::info!(
                        "[Inspector] Rotation changed: ({:.1}°, {:.1}°, {:.1}°)",
                        new_value.x, new_value.y, new_value.z
                    );
                } else if message.destination() == self.scale_editor {
                    new_transform.scale = Vec3::new(new_value.x, new_value.y, new_value.z);
                    log::info!(
                        "[Inspector] Scale changed: ({:.2}, {:.2}, {:.2})",
                        new_value.x, new_value.y, new_value.z
                    );
                } else {
                    return false;
                }

                // Command 실행 (Undo/Redo 지원)
                commands.execute(
                    Box::new(SetTransformCommand::new(entity, old_transform, new_transform)),
                    world,
                );

                return true;
            }
        }

        false
    }
}
