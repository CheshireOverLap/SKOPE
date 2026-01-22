//! ImGui Inspector Panel
//!
//! 선택된 엔티티의 컴포넌트들을 편집
//! ECS World와 연결하여 실제 컴포넌트 데이터 표시/편집

use bevy_ecs::prelude::*;
use dear_imgui_rs::{Ui, TreeNodeFlags};
use glam::{Vec3, Quat};

use crate::ecs_components::{
    NodeName, Transform, Light, LightType, MeshInstance, Camera,
    BoxCollider, SphereCollider, MaterialHandle,
};
use crate::material::MaterialRegistry;

/// Inspector 패널 상태
pub struct ImGuiInspectorState {
    /// 마지막으로 표시한 엔티티
    pub last_entity: Option<Entity>,
    /// Entity 이름 편집 버퍼
    pub name_edit: String,
    pub name_editing: bool,
    /// Transform 편집 버퍼
    pub transform_edit: TransformEdit,
    /// Light 편집 버퍼
    pub light_edit: LightEdit,
    /// Camera 편집 버퍼
    pub camera_edit: CameraEdit,
    /// BoxCollider 편집 버퍼
    pub box_collider_edit: BoxColliderEdit,
    /// SphereCollider 편집 버퍼
    pub sphere_collider_edit: SphereColliderEdit,
    /// Material 편집 버퍼
    pub material_edit: MaterialEdit,
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
    pub cast_shadows: bool,
}

impl Default for LightEdit {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
            range: 10.0,
            spot_angle: 45.0,
            cast_shadows: true,
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

/// BoxCollider 편집 버퍼
#[derive(Clone, Default)]
pub struct BoxColliderEdit {
    pub half_extents: [f32; 3],
    pub offset: [f32; 3],
}

/// SphereCollider 편집 버퍼
#[derive(Clone, Default)]
pub struct SphereColliderEdit {
    pub radius: f32,
    pub offset: [f32; 3],
}

/// Material 편집 버퍼
#[derive(Clone)]
pub struct MaterialEdit {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_strength: f32,
    pub normal_scale: f32,
    pub material_name: String,
}

impl Default for MaterialEdit {
    fn default() -> Self {
        Self {
            base_color: [1.0, 1.0, 1.0, 1.0],
            metallic: 0.0,
            roughness: 0.5,
            emissive_strength: 0.0,
            normal_scale: 1.0,
            material_name: String::new(),
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
            name_edit: String::new(),
            name_editing: false,
            transform_edit: TransformEdit::default(),
            light_edit: LightEdit::default(),
            camera_edit: CameraEdit::default(),
            box_collider_edit: BoxColliderEdit::default(),
            sphere_collider_edit: SphereColliderEdit::default(),
            material_edit: MaterialEdit::default(),
        }
    }

    /// 엔티티 변경 시 편집 버퍼 초기화
    pub fn sync_from_entity(&mut self, world: &World, entity: Entity) {
        // Name 동기화
        if let Some(name) = world.get::<NodeName>(entity) {
            self.name_edit = name.0.clone();
        } else {
            self.name_edit = format!("Entity {:?}", entity);
        }
        self.name_editing = false;

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
            self.light_edit.cast_shadows = light.cast_shadows;
        }

        // Camera 동기화
        if let Some(camera) = world.get::<Camera>(entity) {
            self.camera_edit.fov = camera.fov.to_degrees();
            self.camera_edit.near = camera.near;
            self.camera_edit.far = camera.far;
        }

        // BoxCollider 동기화
        if let Some(collider) = world.get::<BoxCollider>(entity) {
            self.box_collider_edit.half_extents = collider.half_extents.into();
            self.box_collider_edit.offset = collider.offset.into();
        }

        // SphereCollider 동기화
        if let Some(collider) = world.get::<SphereCollider>(entity) {
            self.sphere_collider_edit.radius = collider.radius;
            self.sphere_collider_edit.offset = collider.offset.into();
        }

        // Material 동기화 (MaterialHandle에서 실제 MaterialRegistry 조회 필요)
        if let Some(mat_handle) = world.get::<MaterialHandle>(entity) {
            if let Some(registry) = world.get_resource::<MaterialRegistry>() {
                if let Some(entry) = registry.get_by_index(mat_handle.material_index) {
                    self.material_edit.base_color = entry.def.base_color;
                    self.material_edit.metallic = entry.def.metallic;
                    self.material_edit.roughness = entry.def.roughness;
                    self.material_edit.emissive_strength = entry.def.emissive_strength;
                    self.material_edit.normal_scale = entry.def.normal_scale;
                    self.material_edit.material_name = entry.def.name.clone();
                }
            }
        }

        self.last_entity = Some(entity);
    }
}

/// Inspector 패널 액션
#[derive(Debug, Clone)]
pub enum InspectorAction {
    /// 아무 액션 없음
    None,
    /// 엔티티 이름 변경
    RenameEntity(Entity, String),
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
        cast_shadows: bool,
    },
    /// Camera 변경
    CameraChanged {
        entity: Entity,
        fov: f32,
        near: f32,
        far: f32,
    },
    /// BoxCollider 변경
    BoxColliderChanged {
        entity: Entity,
        half_extents: Vec3,
        offset: Vec3,
    },
    /// SphereCollider 변경
    SphereColliderChanged {
        entity: Entity,
        radius: f32,
        offset: Vec3,
    },
    /// Material 변경
    MaterialChanged {
        material_index: usize,
        base_color: [f32; 4],
        metallic: f32,
        roughness: f32,
        emissive_strength: f32,
        normal_scale: f32,
    },
    /// Material 저장
    SaveMaterial(usize),
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
            match selected_entity {
                Some(entity) => {
                    // 엔티티가 변경되었으면 버퍼 동기화
                    if state.last_entity != Some(entity) {
                        state.sync_from_entity(world, entity);
                    }

                    // 엔티티 이름 편집
                    if let Some(name_action) = render_entity_header(ui, entity, state) {
                        action = name_action;
                    }

                    ui.separator();

                    // Transform 컴포넌트 (편집 가능)
                    if world.get::<Transform>(entity).is_some() {
                        if let Some(new_action) = render_transform_component(ui, entity, state) {
                            action = new_action;
                        }
                    }

                    // Light 컴포넌트 (편집 가능)
                    if world.get::<Light>(entity).is_some() {
                        if let Some(new_action) = render_light_component(ui, world, entity, state) {
                            action = new_action;
                        }
                    }

                    // Camera 컴포넌트 (편집 가능)
                    if world.get::<Camera>(entity).is_some() {
                        if let Some(new_action) = render_camera_component(ui, entity, state) {
                            action = new_action;
                        }
                    }

                    // MeshInstance 컴포넌트 (읽기 전용)
                    if world.get::<MeshInstance>(entity).is_some() {
                        render_mesh_component(ui, world, entity);
                    }

                    // BoxCollider 컴포넌트 (편집 가능)
                    if world.get::<BoxCollider>(entity).is_some() {
                        if let Some(new_action) = render_box_collider_component(ui, entity, state) {
                            action = new_action;
                        }
                    }

                    // SphereCollider 컴포넌트 (편집 가능)
                    if world.get::<SphereCollider>(entity).is_some() {
                        if let Some(new_action) = render_sphere_collider_component(ui, entity, state) {
                            action = new_action;
                        }
                    }

                    // Material 컴포넌트 (편집 가능)
                    if let Some(mat_handle) = world.get::<MaterialHandle>(entity) {
                        if let Some(new_action) = render_material_component(ui, world, mat_handle.material_index, state) {
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

/// 엔티티 헤더 (이름 편집 가능)
fn render_entity_header(
    ui: &Ui,
    entity: Entity,
    state: &mut ImGuiInspectorState,
) -> Option<InspectorAction> {
    let mut action = None;

    // 이름 편집 모드
    if state.name_editing {
        ui.set_next_item_width(-60.0);
        let enter_pressed = ui.input_text("##name_edit", &mut state.name_edit)
            .enter_returns_true(true)
            .build();

        ui.same_line();
        if ui.button("OK") || enter_pressed {
            action = Some(InspectorAction::RenameEntity(entity, state.name_edit.clone()));
            state.name_editing = false;
        }
        ui.same_line();
        if ui.button("X") {
            state.name_editing = false;
        }
    } else {
        // 이름 표시 + 편집 버튼
        ui.text(&state.name_edit);
        ui.same_line();
        if ui.small_button("Edit") {
            state.name_editing = true;
        }
    }

    action
}

/// Transform 컴포넌트 렌더링 (편집 가능)
fn render_transform_component(
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
fn render_light_component(
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

        // Cast Shadows
        if ui.checkbox("Cast Shadows", &mut state.light_edit.cast_shadows) {
            changed = true;
        }

        if changed {
            action = Some(InspectorAction::LightChanged {
                entity,
                color: Vec3::from(state.light_edit.color),
                intensity: state.light_edit.intensity,
                range: state.light_edit.range,
                spot_angle: state.light_edit.spot_angle.to_radians(),
                cast_shadows: state.light_edit.cast_shadows,
            });
        }
    }

    action
}

/// Camera 컴포넌트 렌더링 (편집 가능)
fn render_camera_component(
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

/// Mesh 컴포넌트 렌더링 (읽기 전용)
fn render_mesh_component(ui: &Ui, world: &World, entity: Entity) {
    if ui.collapsing_header("Mesh", TreeNodeFlags::DEFAULT_OPEN) {
        if let Some(mesh) = world.get::<MeshInstance>(entity) {
            ui.text(format!("Mesh Index: {}", mesh.mesh_index));
        }
    }
}

/// BoxCollider 컴포넌트 렌더링 (편집 가능)
fn render_box_collider_component(
    ui: &Ui,
    entity: Entity,
    state: &mut ImGuiInspectorState,
) -> Option<InspectorAction> {
    let mut action = None;

    if ui.collapsing_header("Box Collider", TreeNodeFlags::DEFAULT_OPEN) {
        let mut changed = false;

        // Half Extents
        ui.text("Size    ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float3("##box_size", &mut state.box_collider_edit.half_extents).build() {
            changed = true;
        }

        // Offset
        ui.text("Offset  ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float3("##box_offset", &mut state.box_collider_edit.offset).build() {
            changed = true;
        }

        if changed {
            action = Some(InspectorAction::BoxColliderChanged {
                entity,
                half_extents: Vec3::from(state.box_collider_edit.half_extents),
                offset: Vec3::from(state.box_collider_edit.offset),
            });
        }
    }

    action
}

/// SphereCollider 컴포넌트 렌더링 (편집 가능)
fn render_sphere_collider_component(
    ui: &Ui,
    entity: Entity,
    state: &mut ImGuiInspectorState,
) -> Option<InspectorAction> {
    let mut action = None;

    if ui.collapsing_header("Sphere Collider", TreeNodeFlags::DEFAULT_OPEN) {
        let mut changed = false;

        // Radius
        ui.text("Radius  ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float("##sphere_radius", &mut state.sphere_collider_edit.radius) {
            changed = true;
        }

        // Offset
        ui.text("Offset  ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.input_float3("##sphere_offset", &mut state.sphere_collider_edit.offset).build() {
            changed = true;
        }

        if changed {
            action = Some(InspectorAction::SphereColliderChanged {
                entity,
                radius: state.sphere_collider_edit.radius,
                offset: Vec3::from(state.sphere_collider_edit.offset),
            });
        }
    }

    action
}

/// Material 컴포넌트 렌더링 (편집 가능)
fn render_material_component(
    ui: &Ui,
    world: &World,
    material_index: usize,
    state: &mut ImGuiInspectorState,
) -> Option<InspectorAction> {
    let mut action = None;

    if ui.collapsing_header("Material", TreeNodeFlags::DEFAULT_OPEN) {
        let registry = world.get_resource::<MaterialRegistry>()?;
        let entry = registry.get_by_index(material_index)?;
        let mut changed = false;

        // Material Name
        ui.text(format!("Name: {}", &entry.def.name));
        ui.text(format!("Index: {}", material_index));

        ui.separator();

        // Base Color
        ui.text("Base Color");
        ui.same_line();
        if ui.color_edit4("##mat_color", &mut state.material_edit.base_color) {
            changed = true;
        }

        // Metallic
        ui.text("Metallic  ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.slider_f32("##metallic", &mut state.material_edit.metallic, 0.0, 1.0) {
            changed = true;
        }

        // Roughness
        ui.text("Roughness ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.slider_f32("##roughness", &mut state.material_edit.roughness, 0.0, 1.0) {
            changed = true;
        }

        // Emissive Strength
        ui.text("Emissive  ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.slider_f32("##emissive", &mut state.material_edit.emissive_strength, 0.0, 10.0) {
            changed = true;
        }

        // Normal Scale
        ui.text("Normal    ");
        ui.same_line();
        ui.set_next_item_width(-1.0);
        if ui.slider_f32("##normal_scale", &mut state.material_edit.normal_scale, 0.0, 2.0) {
            changed = true;
        }

        if changed {
            action = Some(InspectorAction::MaterialChanged {
                material_index,
                base_color: state.material_edit.base_color,
                metallic: state.material_edit.metallic,
                roughness: state.material_edit.roughness,
                emissive_strength: state.material_edit.emissive_strength,
                normal_scale: state.material_edit.normal_scale,
            });
        }

        // Save button (only for RON materials)
        if entry.can_save() {
            ui.separator();
            if ui.button("Save Material") {
                action = Some(InspectorAction::SaveMaterial(material_index));
            }
        }
    }

    action
}
