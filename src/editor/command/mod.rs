//! Command 시스템 (Undo/Redo)
//!
//! 에디터 작업을 되돌리거나 다시 실행할 수 있는 Command 패턴 구현

use bevy_ecs::prelude::*;
use bevy_hierarchy::prelude::*;
use glam::{Quat, Vec3};
use std::fmt::Debug;

use crate::ecs_components::{
    GlobalTransform, Light, LightType, MaterialHandle, MeshInstance, NodeName, Transform,
};
use crate::editor::spawn_menu::SpawnItem;

/// Command trait - 모든 에디터 커맨드가 구현해야 함
pub trait Command: Debug + Send + Sync {
    /// 커맨드 이름 (UI 표시용)
    fn name(&self) -> &str;

    /// 커맨드 실행
    fn execute(&mut self, world: &mut World);

    /// 커맨드 되돌리기
    fn undo(&mut self, world: &mut World);
}

/// 커맨드 스택 - Undo/Redo 관리
pub struct CommandStack {
    /// 실행된 커맨드 목록
    commands: Vec<Box<dyn Command>>,
    /// 현재 위치 (다음 redo할 커맨드 인덱스)
    current: usize,
    /// 최대 히스토리 크기
    max_history: usize,
}

impl Default for CommandStack {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandStack {
    /// 새 CommandStack 생성
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            current: 0,
            max_history: 100,
        }
    }

    /// 커맨드 실행 및 스택에 추가
    pub fn execute(&mut self, mut cmd: Box<dyn Command>, world: &mut World) {
        // 커맨드 실행
        cmd.execute(world);
        log::info!("[Command] Execute: {}", cmd.name());

        // 스택에 추가
        self.push_to_stack(cmd);
    }

    /// 이미 실행된 커맨드를 스택에 추가 (Gizmo 드래그 등)
    /// execute()를 호출하지 않고 스택에만 추가
    pub fn push_executed(&mut self, cmd: Box<dyn Command>) {
        log::info!("[Command] Push executed: {}", cmd.name());
        self.push_to_stack(cmd);
    }

    /// 내부: 커맨드를 스택에 추가
    fn push_to_stack(&mut self, cmd: Box<dyn Command>) {
        // 현재 위치 이후의 커맨드 제거 (redo 히스토리 삭제)
        self.commands.truncate(self.current);

        // 새 커맨드 추가
        self.commands.push(cmd);
        self.current += 1;

        // 히스토리 크기 제한
        if self.commands.len() > self.max_history {
            let remove_count = self.commands.len() - self.max_history;
            self.commands.drain(0..remove_count);
            self.current = self.current.saturating_sub(remove_count);
        }
    }

    /// Undo 가능 여부
    pub fn can_undo(&self) -> bool {
        self.current > 0
    }

    /// Redo 가능 여부
    pub fn can_redo(&self) -> bool {
        self.current < self.commands.len()
    }

    /// Undo 실행
    pub fn undo(&mut self, world: &mut World) -> bool {
        if !self.can_undo() {
            return false;
        }

        self.current -= 1;
        let cmd = &mut self.commands[self.current];
        log::info!("[Command] Undo: {}", cmd.name());
        cmd.undo(world);
        true
    }

    /// Redo 실행
    pub fn redo(&mut self, world: &mut World) -> bool {
        if !self.can_redo() {
            return false;
        }

        let cmd = &mut self.commands[self.current];
        log::info!("[Command] Redo: {}", cmd.name());
        cmd.execute(world);
        self.current += 1;
        true
    }

    /// 마지막 커맨드 이름 (Undo용)
    pub fn last_command_name(&self) -> Option<&str> {
        if self.current > 0 {
            Some(self.commands[self.current - 1].name())
        } else {
            None
        }
    }

    /// 다음 Redo 커맨드 이름
    pub fn next_redo_name(&self) -> Option<&str> {
        if self.current < self.commands.len() {
            Some(self.commands[self.current].name())
        } else {
            None
        }
    }

    /// 히스토리 초기화
    pub fn clear(&mut self) {
        self.commands.clear();
        self.current = 0;
    }
}

// ============ 기본 Command 구현 ============

/// 엔티티 이동 커맨드
#[derive(Debug)]
pub struct MoveCommand {
    /// 이동할 엔티티들
    entities: Vec<Entity>,
    /// 이동 오프셋
    delta: Vec3,
}

impl MoveCommand {
    /// 새 MoveCommand 생성
    pub fn new(entities: Vec<Entity>, delta: Vec3) -> Self {
        Self { entities, delta }
    }
}

impl Command for MoveCommand {
    fn name(&self) -> &str {
        "Move"
    }

    fn execute(&mut self, world: &mut World) {
        for &entity in &self.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation += self.delta;
            }
        }
    }

    fn undo(&mut self, world: &mut World) {
        for &entity in &self.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.translation -= self.delta;
            }
        }
    }
}

/// Transform 직접 설정 커맨드 (Inspector용)
#[derive(Debug)]
pub struct SetTransformCommand {
    /// 대상 엔티티
    entity: Entity,
    /// 이전 Transform
    old_transform: Transform,
    /// 새 Transform
    new_transform: Transform,
}

impl SetTransformCommand {
    /// 새 SetTransformCommand 생성
    pub fn new(entity: Entity, old_transform: Transform, new_transform: Transform) -> Self {
        Self {
            entity,
            old_transform,
            new_transform,
        }
    }
}

impl Command for SetTransformCommand {
    fn name(&self) -> &str {
        "Set Transform"
    }

    fn execute(&mut self, world: &mut World) {
        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            *transform = self.new_transform.clone();
        }
    }

    fn undo(&mut self, world: &mut World) {
        if let Some(mut transform) = world.get_mut::<Transform>(self.entity) {
            *transform = self.old_transform.clone();
        }
    }
}

/// 엔티티 회전 커맨드
#[derive(Debug)]
pub struct RotateCommand {
    /// 회전할 엔티티들
    entities: Vec<Entity>,
    /// 회전 쿼터니언 델타
    rotation_delta: Quat,
    /// 회전 중심점 (피벗)
    pivot: Vec3,
}

impl RotateCommand {
    /// 새 RotateCommand 생성
    pub fn new(entities: Vec<Entity>, rotation_delta: Quat, pivot: Vec3) -> Self {
        Self {
            entities,
            rotation_delta,
            pivot,
        }
    }
}

impl Command for RotateCommand {
    fn name(&self) -> &str {
        "Rotate"
    }

    fn execute(&mut self, world: &mut World) {
        for &entity in &self.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                // 피벗 중심 회전
                let offset = transform.translation - self.pivot;
                let rotated_offset = self.rotation_delta * offset;
                transform.translation = self.pivot + rotated_offset;

                // 오브젝트 자체 회전
                transform.rotation = self.rotation_delta * transform.rotation;
            }
        }
    }

    fn undo(&mut self, world: &mut World) {
        let inverse_rotation = self.rotation_delta.inverse();
        for &entity in &self.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                // 역회전
                transform.rotation = inverse_rotation * transform.rotation;

                // 피벗 중심 역이동
                let offset = transform.translation - self.pivot;
                let rotated_offset = inverse_rotation * offset;
                transform.translation = self.pivot + rotated_offset;
            }
        }
    }
}

/// 엔티티 스케일 커맨드
#[derive(Debug)]
pub struct ScaleCommand {
    /// 스케일할 엔티티들
    entities: Vec<Entity>,
    /// 스케일 팩터 (각 축별)
    scale_factor: Vec3,
    /// 스케일 중심점 (피벗)
    pivot: Vec3,
}

impl ScaleCommand {
    /// 새 ScaleCommand 생성
    pub fn new(entities: Vec<Entity>, scale_factor: Vec3, pivot: Vec3) -> Self {
        Self {
            entities,
            scale_factor,
            pivot,
        }
    }
}

impl Command for ScaleCommand {
    fn name(&self) -> &str {
        "Scale"
    }

    fn execute(&mut self, world: &mut World) {
        for &entity in &self.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                // 피벗 중심 스케일
                let offset = transform.translation - self.pivot;
                let scaled_offset = offset * self.scale_factor;
                transform.translation = self.pivot + scaled_offset;

                // 오브젝트 스케일 변경
                transform.scale *= self.scale_factor;
            }
        }
    }

    fn undo(&mut self, world: &mut World) {
        let inverse_scale = Vec3::ONE / self.scale_factor;
        for &entity in &self.entities {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                // 역스케일
                transform.scale *= inverse_scale;

                // 피벗 중심 역이동
                let offset = transform.translation - self.pivot;
                let scaled_offset = offset * inverse_scale;
                transform.translation = self.pivot + scaled_offset;
            }
        }
    }
}

// ============ Spawn Entity Command ============

/// 엔티티 생성 데이터
#[derive(Debug, Clone)]
pub struct SpawnData {
    /// 생성 위치
    pub position: Vec3,
    /// 생성 항목 타입
    pub spawn_item: SpawnItem,
    /// 메시 인덱스 (메시가 있는 경우)
    pub mesh_index: Option<usize>,
    /// 머티리얼 인덱스
    pub material_index: Option<usize>,
    /// 커스텀 이름 (None이면 spawn_item.entity_name() 사용)
    pub custom_name: Option<String>,
}

impl SpawnData {
    /// 새 SpawnData 생성
    pub fn new(spawn_item: SpawnItem, position: Vec3) -> Self {
        Self {
            position,
            spawn_item,
            mesh_index: None,
            material_index: None,
            custom_name: None,
        }
    }

    /// 메시 인덱스 설정
    pub fn with_mesh(mut self, mesh_index: usize) -> Self {
        self.mesh_index = Some(mesh_index);
        self
    }

    /// 머티리얼 인덱스 설정
    pub fn with_material(mut self, material_index: usize) -> Self {
        self.material_index = Some(material_index);
        self
    }

    /// 커스텀 이름 설정
    pub fn with_name(mut self, name: &str) -> Self {
        self.custom_name = Some(name.to_string());
        self
    }

    /// 엔티티 이름 반환 (커스텀 이름이 있으면 사용, 없으면 spawn_item 이름)
    pub fn entity_name(&self) -> &str {
        self.custom_name
            .as_deref()
            .unwrap_or_else(|| self.spawn_item.entity_name())
    }
}

/// 엔티티 생성 커맨드
#[derive(Debug)]
pub struct SpawnEntityCommand {
    /// 생성 데이터
    spawn_data: SpawnData,
    /// 생성된 엔티티 (undo용)
    spawned_entity: Option<Entity>,
}

impl SpawnEntityCommand {
    /// 새 SpawnEntityCommand 생성
    pub fn new(spawn_data: SpawnData) -> Self {
        Self {
            spawn_data,
            spawned_entity: None,
        }
    }

    /// 생성된 엔티티 반환
    pub fn spawned_entity(&self) -> Option<Entity> {
        self.spawned_entity
    }
}

impl Command for SpawnEntityCommand {
    fn name(&self) -> &str {
        "Spawn Entity"
    }

    fn execute(&mut self, world: &mut World) {
        let pos = self.spawn_data.position;
        let item = self.spawn_data.spawn_item;
        let entity_name = self.spawn_data.entity_name().to_string();

        // 기본 컴포넌트로 엔티티 생성
        let mut entity_cmd = world.spawn((
            Transform::from_translation(pos),
            GlobalTransform::default(),
            NodeName(entity_name.clone()),
        ));

        // 메시가 있는 경우
        if let Some(mesh_index) = self.spawn_data.mesh_index {
            entity_cmd.insert(MeshInstance { mesh_index });

            if let Some(material_index) = self.spawn_data.material_index {
                entity_cmd.insert(MaterialHandle { material_index });
            }
        }

        // 라이트인 경우
        match item {
            SpawnItem::PointLight => {
                entity_cmd.insert(Light {
                    light_type: LightType::Point,
                    color: Vec3::ONE,
                    intensity: 1.0,
                    range: 10.0,
                    spot_angle: 0.0,
                    cast_shadows: true,
                });
            }
            SpawnItem::SpotLight => {
                entity_cmd.insert(Light {
                    light_type: LightType::Spot,
                    color: Vec3::ONE,
                    intensity: 1.0,
                    range: 10.0,
                    spot_angle: 0.5, // ~30도
                    cast_shadows: true,
                });
            }
            SpawnItem::SunLight => {
                entity_cmd.insert(Light {
                    light_type: LightType::Sun,
                    color: Vec3::ONE,
                    intensity: 1.0,
                    range: 0.0, // Sun은 무한 범위
                    spot_angle: 0.0,
                    cast_shadows: true,
                });
            }
            _ => {}
        }

        self.spawned_entity = Some(entity_cmd.id());
        log::info!(
            "[SpawnEntityCommand] Spawned {} at {:?}",
            entity_name,
            pos
        );
    }

    fn undo(&mut self, world: &mut World) {
        if let Some(entity) = self.spawned_entity.take() {
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
                log::info!("[SpawnEntityCommand] Undo: despawned entity");
            }
        }
    }
}

// ============ Reparent Command ============

/// 엔티티 부모 변경 커맨드
#[derive(Debug)]
pub struct ReparentCommand {
    /// 대상 엔티티
    entity: Entity,
    /// 이전 부모 (None = 루트)
    old_parent: Option<Entity>,
    /// 새 부모 (None = 루트로 이동)
    new_parent: Option<Entity>,
}

impl ReparentCommand {
    /// 새 ReparentCommand 생성
    pub fn new(entity: Entity, old_parent: Option<Entity>, new_parent: Option<Entity>) -> Self {
        Self {
            entity,
            old_parent,
            new_parent,
        }
    }
}

impl Command for ReparentCommand {
    fn name(&self) -> &str {
        "Reparent"
    }

    fn execute(&mut self, world: &mut World) {
        // 유효성 검사
        if world.get_entity(self.entity).is_err() {
            log::warn!("[ReparentCommand] Entity {:?} not found", self.entity);
            return;
        }

        // 기존 부모에서 제거
        if self.old_parent.is_some() {
            world.entity_mut(self.entity).remove_parent();
        }

        // 새 부모에 추가
        if let Some(new_parent) = self.new_parent {
            if world.get_entity(new_parent).is_ok() {
                world.entity_mut(self.entity).set_parent(new_parent);
                log::info!(
                    "[ReparentCommand] {:?} → parent {:?}",
                    self.entity,
                    new_parent
                );
            } else {
                log::warn!("[ReparentCommand] New parent {:?} not found", new_parent);
            }
        } else {
            log::info!("[ReparentCommand] {:?} → root", self.entity);
        }
    }

    fn undo(&mut self, world: &mut World) {
        // 유효성 검사
        if world.get_entity(self.entity).is_err() {
            return;
        }

        // 새 부모에서 제거
        if self.new_parent.is_some() {
            world.entity_mut(self.entity).remove_parent();
        }

        // 이전 부모로 복원
        if let Some(old_parent) = self.old_parent {
            if world.get_entity(old_parent).is_ok() {
                world.entity_mut(self.entity).set_parent(old_parent);
                log::info!(
                    "[ReparentCommand] Undo: {:?} → parent {:?}",
                    self.entity,
                    old_parent
                );
            }
        } else {
            log::info!("[ReparentCommand] Undo: {:?} → root", self.entity);
        }
    }
}

// ============ Paste Command ============

/// 붙여넣기 커맨드 (Undo 지원)
#[derive(Debug)]
pub struct PasteCommand {
    /// 붙여넣기로 생성된 엔티티들
    pasted_entities: Vec<Entity>,
}

impl PasteCommand {
    /// 새 PasteCommand 생성
    pub fn new(pasted_entities: Vec<Entity>) -> Self {
        Self { pasted_entities }
    }

    /// 붙여넣기로 생성된 엔티티 목록
    #[allow(dead_code)]
    pub fn entities(&self) -> &[Entity] {
        &self.pasted_entities
    }
}

impl Command for PasteCommand {
    fn name(&self) -> &str {
        "Paste"
    }

    fn execute(&mut self, _world: &mut World) {
        // 붙여넣기는 Clipboard.paste_to()에서 이미 실행됨
        // 이 메서드는 Redo 시에만 호출되지만,
        // 엔티티가 이미 삭제된 후에는 재생성이 필요함
        // (현재는 Redo 미지원 - 추후 개선 가능)
        log::debug!("[PasteCommand] Execute called (already executed via Clipboard)");
    }

    fn undo(&mut self, world: &mut World) {
        // 붙여넣기로 생성된 엔티티들 삭제
        for &entity in &self.pasted_entities {
            if world.get_entity(entity).is_ok() {
                world.despawn(entity);
            }
        }
        log::info!(
            "[PasteCommand] Undo: despawned {} entities",
            self.pasted_entities.len()
        );
    }
}
