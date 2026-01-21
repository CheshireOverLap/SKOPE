//! ImGui Inspector Panel
//!
//! 선택된 엔티티의 컴포넌트들을 편집
//! ECS World와 연결하여 실제 컴포넌트 데이터 표시/편집

use bevy_ecs::prelude::*;
use dear_imgui_rs::{Ui, TreeNodeFlags};

#[allow(unused_imports)]
use glam::{Vec3, Quat};

use crate::ecs_components::{NodeName, Transform, Light, LightType, MeshInstance, Camera};

/// Inspector 패널 상태
pub struct ImGuiInspectorState {
    /// 마지막으로 표시한 엔티티
    pub last_entity: Option<Entity>,
}

impl Default for ImGuiInspectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl ImGuiInspectorState {
    pub fn new() -> Self {
        Self {
            last_entity: None,
        }
    }
}

/// Inspector 패널 액션
#[derive(Debug, Clone)]
pub enum InspectorAction {
    /// 아무 액션 없음
    None,
    /// Transform 변경
    TransformChanged(Entity),
    /// Light 변경
    LightChanged(Entity),
    /// 컴포넌트 제거
    RemoveComponent(Entity, &'static str),
}

/// Inspector 패널 렌더링 (ECS World 연결)
pub fn render_inspector_panel(
    ui: &Ui,
    world: &World,
    selected_entity: Option<Entity>,
    _state: &mut ImGuiInspectorState,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    ui.window("Inspector")
        .build(|| {
            ui.text("Details");
            ui.separator();

            match selected_entity {
                Some(entity) => {
                    // 엔티티 이름 표시
                    let name = world
                        .get::<NodeName>(entity)
                        .map(|n| n.0.clone())
                        .unwrap_or_else(|| format!("Entity {:?}", entity));

                    ui.text(&name);
                    ui.separator();

                    // Transform 컴포넌트 (읽기 전용)
                    if world.get::<Transform>(entity).is_some() {
                        render_transform_component_readonly(ui, world, entity);
                    }

                    // Light 컴포넌트 (읽기 전용)
                    if world.get::<Light>(entity).is_some() {
                        render_light_component_readonly(ui, world, entity);
                    }

                    // MeshInstance 컴포넌트
                    if world.get::<MeshInstance>(entity).is_some() {
                        render_mesh_component(ui, world, entity);
                    }

                    // Camera 컴포넌트
                    if world.get::<Camera>(entity).is_some() {
                        render_camera_component(ui, world, entity);
                    }
                }
                None => {
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], "No entity selected");
                }
            }
        });

    action
}

/// Transform 컴포넌트 렌더링 (읽기 전용)
fn render_transform_component_readonly(ui: &Ui, world: &World, entity: Entity) {
    if ui.collapsing_header("Transform", TreeNodeFlags::DEFAULT_OPEN) {
        if let Some(transform) = world.get::<Transform>(entity) {
            // Position
            ui.text(format!("Position: ({:.2}, {:.2}, {:.2})",
                transform.translation.x, transform.translation.y, transform.translation.z));

            // Rotation (Euler degrees로 표시)
            let (yaw, pitch, roll) = transform.rotation.to_euler(glam::EulerRot::YXZ);
            ui.text(format!("Rotation: ({:.1}, {:.1}, {:.1})",
                pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()));

            // Scale
            ui.text(format!("Scale: ({:.2}, {:.2}, {:.2})",
                transform.scale.x, transform.scale.y, transform.scale.z));
        }
    }
}

/// Light 컴포넌트 렌더링 (읽기 전용)
fn render_light_component_readonly(ui: &Ui, world: &World, entity: Entity) {
    if ui.collapsing_header("Light", TreeNodeFlags::DEFAULT_OPEN) {
        if let Some(light) = world.get::<Light>(entity) {
            // Light Type 표시
            let type_str = match light.light_type {
                LightType::Point => "Point",
                LightType::Spot => "Spot",
                LightType::Sun => "Directional (Sun)",
                LightType::Area => "Area",
            };
            ui.text(format!("Type: {}", type_str));

            // Color
            ui.text(format!("Color: ({:.2}, {:.2}, {:.2})",
                light.color.x, light.color.y, light.color.z));

            // Intensity
            ui.text(format!("Intensity: {:.2}", light.intensity));

            // Range (Point/Spot only)
            if !matches!(light.light_type, LightType::Sun) {
                ui.text(format!("Range: {:.2}", light.range));
            }

            // Spot Angle (Spot only)
            if matches!(light.light_type, LightType::Spot) {
                ui.text(format!("Spot Angle: {:.1}", light.spot_angle.to_degrees()));
            }

            // Cast Shadows
            ui.text(format!("Cast Shadows: {}", light.cast_shadows));
        }
    }
}

/// Mesh 컴포넌트 렌더링 (읽기 전용)
fn render_mesh_component(ui: &Ui, world: &World, entity: Entity) {
    if ui.collapsing_header("Mesh", TreeNodeFlags::DEFAULT_OPEN) {
        if let Some(mesh) = world.get::<MeshInstance>(entity) {
            ui.text(format!("Mesh Index: {}", mesh.mesh_index));
        }
    }
}

/// Camera 컴포넌트 렌더링 (읽기 전용)
fn render_camera_component(ui: &Ui, world: &World, entity: Entity) {
    if ui.collapsing_header("Camera", TreeNodeFlags::DEFAULT_OPEN) {
        if let Some(camera) = world.get::<Camera>(entity) {
            ui.text(format!("FOV: {:.1}", camera.fov.to_degrees()));
            ui.text(format!("Near: {:.3}", camera.near));
            ui.text(format!("Far: {:.1}", camera.far));
        }
    }
}
