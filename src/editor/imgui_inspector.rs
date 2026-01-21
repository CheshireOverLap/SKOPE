//! ImGui Inspector Panel
//!
//! 선택된 엔티티의 컴포넌트들을 편집
//! ECS World와 연결하여 실제 컴포넌트 데이터 표시/편집

use bevy_ecs::prelude::*;
use dear_imgui_rs::{Ui, TreeNodeFlags};
use glam::{Vec3, Quat};

use crate::ecs_components::{NodeName, Transform, Light, LightType, MeshInstance, Camera};

/// Inspector 패널 상태
pub struct ImGuiInspectorState {
    /// 마지막으로 표시한 엔티티
    pub last_entity: Option<Entity>,
    /// Transform 편집 버퍼
    pub transform_edit: TransformEdit,
    /// Light 편집 버퍼
    pub light_edit: LightEdit,
    /// Camera 편집 버퍼
    pub camera_edit: CameraEdit,
}

/// Transform 편집 버퍼
#[derive(Clone, Default)]
pub struct TransformEdit {
    pub position: [f32; 3],
    pub rotation: [f32; 3], // Euler degrees
    pub scale: [f32; 3],
}

/// Light 편집 버퍼
#[derive(Clone)]
pub struct LightEdit {
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
    pub spot_angle: f32, // degrees
}

impl Default for LightEdit {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 10.0,
            spot_angle: 45.0,
        }
    }
}

/// Camera 편집 버퍼
#[derive(Clone)]
pub struct CameraEdit {
    pub fov: f32, // degrees
    pub near: f32,
    pub far: f32,
}

impl Default for CameraEdit {
    fn default() -> Self {
        Self {
            fov: 60.0,
            near: 0.1,
            far: 1000.0,
        }
    }
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
            transform_edit: TransformEdit::default(),
            light_edit: LightEdit::default(),
            camera_edit: CameraEdit::default(),
        }
    }

    /// 엔티티 변경 시 편집 버퍼 초기화
    pub fn sync_from_entity(&mut self, world: &World, entity: Entity) {
        // Transform 동기화
        if let Some(transform) = world.get::<Transform>(entity) {
            self.transform_edit.position = transform.translation.into();
            let (yaw, pitch, roll) = transform.rotation.to_euler(glam::EulerRot::YXZ);
            self.transform_edit.rotation = [pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()];
            self.transform_edit.scale = transform.scale.into();
        }

        // Light 동기화
        if let Some(light) = world.get::<Light>(entity) {
            self.light_edit.color = light.color.into();
            self.light_edit.intensity = light.intensity;
            self.light_edit.range = light.range;
            self.light_edit.spot_angle = light.spot_angle.to_degrees();
        }

        // Camera 동기화
        if let Some(camera) = world.get::<Camera>(entity) {
            self.camera_edit.fov = camera.fov.to_degrees();
            self.camera_edit.near = camera.near;
            self.camera_edit.far = camera.far;
        }

        self.last_entity = Some(entity);
    }
}

/// Inspector 패널 액션
#[derive(Debug, Clone)]
pub enum InspectorAction {
    /// 아무 액션 없음
    None,
    /// Transform 변경
    TransformChanged {
        entity: Entity,
        position: Vec3,
        rotation: Quat,
        scale: Vec3,
    },
    /// Light 변경
    LightChanged {
        entity: Entity,
        color: Vec3,
        intensity: f32,
        range: f32,
        spot_angle: f32,
    },
    /// Camera 변경
    CameraChanged {
        entity: Entity,
        fov: f32,
        near: f32,
        far: f32,
    },
    /// 컴포넌트 제거
    RemoveComponent(Entity, &'static str),
}

/// Inspector 패널 렌더링 (ECS World 연결)
pub fn render_inspector_panel(
    ui: &Ui,
    world: &World,
    selected_entity: Option<Entity>,
    state: &mut ImGuiInspectorState,
) -> InspectorAction {
    let mut action = InspectorAction::None;

    ui.window("Inspector")
        .build(|| {
            ui.text("Details");
            ui.separator();

            match selected_entity {
                Some(entity) => {
                    // 엔티티가 변경되었으면 버퍼 동기화
                    if state.last_entity != Some(entity) {
                        state.sync_from_entity(world, entity);
                    }

                    // 엔티티 이름 표시
                    let name = world
                        .get::<NodeName>(entity)
                        .map(|n| n.0.clone())
                        .unwrap_or_else(|| format!("Entity {:?}", entity));

                    ui.text(&name);
                    ui.separator();

                    // Transform 컴포넌트 (편집 가능)
                    if world.get::<Transform>(entity).is_some() {
                        if let Some(new_action) = render_transform_component_editable(ui, entity, state) {
                            action = new_action;
                        }
                    }

                    // Light 컴포넌트 (편집 가능)
                    if world.get::<Light>(entity).is_some() {
                        if let Some(new_action) = render_light_component_editable(ui, world, entity, state) {
                            action = new_action;
                        }
                    }

                    // MeshInstance 컴포넌트 (읽기 전용)
                    if world.get::<MeshInstance>(entity).is_some() {
                        render_mesh_component(ui, world, entity);
                    }

                    // Camera 컴포넌트 (편집 가능)
                    if world.get::<Camera>(entity).is_some() {
                        if let Some(new_action) = render_camera_component_editable(ui, entity, state) {
                            action = new_action;
                        }
                    }
                }
                None => {
                    state.last_entity = None;
                    ui.text_colored([0.5, 0.5, 0.5, 1.0], "No entity selected");
                }
            }
        });

    action
}

/// Transform 컴포넌트 렌더링 (편집 가능)
fn render_transform_component_editable(
    ui: &Ui,
    entity: Entity,
    state: &mut ImGuiInspectorState,
) -> Option<InspectorAction> {
    let mut action = None;

    if ui.collapsing_header("Transform", TreeNodeFlags::DEFAULT_OPEN) {
        let mut changed = false;

        // Position
        ui.text("Position");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float3("##pos", &mut state.transform_edit.position).build() {
            changed = true;
        }

        // Rotation (Euler degrees)
        ui.text("Rotation");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float3("##rot", &mut state.transform_edit.rotation).build() {
            changed = true;
        }

        // Scale
        ui.text("Scale   ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float3("##scale", &mut state.transform_edit.scale).build() {
            changed = true;
        }

        if changed {
            // Euler to Quaternion 변환
            let [pitch, yaw, roll] = state.transform_edit.rotation;
            let rotation = Quat::from_euler(
                glam::EulerRot::YXZ,
                yaw.to_radians(),
                pitch.to_radians(),
                roll.to_radians(),
            );

            action = Some(InspectorAction::TransformChanged {
                entity,
                position: Vec3::from(state.transform_edit.position),
                rotation,
                scale: Vec3::from(state.transform_edit.scale),
            });
        }
    }

    action
}

/// Light 컴포넌트 렌더링 (편집 가능)
fn render_light_component_editable(
    ui: &Ui,
    world: &World,
    entity: Entity,
    state: &mut ImGuiInspectorState,
) -> Option<InspectorAction> {
    let mut action = None;

    if ui.collapsing_header("Light", TreeNodeFlags::DEFAULT_OPEN) {
        let light = world.get::<Light>(entity)?;
        let mut changed = false;

        // Light Type 표시 (읽기 전용)
        let type_str = match light.light_type {
            LightType::Point => "Point",
            LightType::Spot => "Spot",
            LightType::Sun => "Directional (Sun)",
            LightType::Area => "Area",
        };
        ui.text(format!("Type: {}", type_str));

        // Color (color picker)
        ui.text("Color   ");
        ui.same_line();
        if ui.color_edit3("##light_color", &mut state.light_edit.color) {
            changed = true;
        }

        // Intensity
        ui.text("Intensity");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.slider_f32("##intensity", &mut state.light_edit.intensity, 0.0, 100.0) {
            changed = true;
        }

        // Range (Point/Spot only)
        if !matches!(light.light_type, LightType::Sun) {
            ui.text("Range   ");
            ui.same_line();
            ui.set_next_item_width(-1.0);
            if ui.slider_f32("##range", &mut state.light_edit.range, 0.1, 100.0) {
                changed = true;
            }
        }

        // Spot Angle (Spot only)
        if matches!(light.light_type, LightType::Spot) {
            ui.text("Angle   ");
            ui.same_line();
            ui.set_next_item_width(-1.0);
            if ui.slider_f32("##spot_angle", &mut state.light_edit.spot_angle, 1.0, 179.0) {
                changed = true;
            }
        }

        // Cast Shadows (읽기 전용)
        ui.text(format!("Cast Shadows: {}", light.cast_shadows));

        if changed {
            action = Some(InspectorAction::LightChanged {
                entity,
                color: Vec3::from(state.light_edit.color),
                intensity: state.light_edit.intensity,
                range: state.light_edit.range,
                spot_angle: state.light_edit.spot_angle.to_radians(),
            });
        }
    }

    action
}

/// Mesh 컴포넌트 렌더링 (읽기 전용)
fn render_mesh_component(ui: &Ui, world: &World, entity: Entity) {
    if ui.collapsing_header("Mesh", TreeNodeFlags::DEFAULT_OPEN) {
        if let Some(mesh) = world.get::<MeshInstance>(entity) {
            ui.text(format!("Mesh Index: {}", mesh.mesh_index));
        }
    }
}

/// Camera 컴포넌트 렌더링 (편집 가능)
fn render_camera_component_editable(
    ui: &Ui,
    entity: Entity,
    state: &mut ImGuiInspectorState,
) -> Option<InspectorAction> {
    let mut action = None;

    if ui.collapsing_header("Camera", TreeNodeFlags::DEFAULT_OPEN) {
        let mut changed = false;

        // FOV
        ui.text("FOV     ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.slider_f32("##fov", &mut state.camera_edit.fov, 10.0, 120.0) {
            changed = true;
        }

        // Near
        ui.text("Near    ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float("##near", &mut state.camera_edit.near) {
            changed = true;
        }

        // Far
        ui.text("Far     ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float("##far", &mut state.camera_edit.far) {
            changed = true;
        }

        if changed {
            action = Some(InspectorAction::CameraChanged {
                entity,
                fov: state.camera_edit.fov.to_radians(),
                near: state.camera_edit.near,
                far: state.camera_edit.far,
            });
        }
    }

    action
}
