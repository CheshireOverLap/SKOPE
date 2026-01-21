//! AI Diff Command (Undo/Redo 통합)
//!
//! AI가 제안한 변경 사항을 적용하고 되돌릴 수 있습니다.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::ecs_components::Transform;
use crate::editor::command::Command;

use super::types::{AIDiffSession, DiffChange, DiffChangeId, DiffChangeType, DiffValue, EntityModification};

// ============================================================================
// Apply Diff Command
// ============================================================================

/// AI Diff 적용 커맨드
///
/// 선택된 변경 사항들을 월드에 적용하고, Undo를 위한 이전 상태를 저장합니다.
#[derive(Debug)]
pub struct ApplyDiffCommand {
    /// 세션 ID
    pub session_id: u64,
    /// 적용할 변경 ID 목록
    pub change_ids: Vec<DiffChangeId>,
    /// 변경 사항 (execute 시 참조)
    pub changes: Vec<DiffChange>,
    /// Undo용 이전 상태
    previous_states: Vec<PreviousState>,
    /// 적용된 변경 수
    applied_count: usize,
}

/// Undo용 이전 상태
#[derive(Debug, Clone)]
enum PreviousState {
    /// Entity 변경 이전 상태
    EntityModification {
        entity: Entity,
        component_type: String,
        field_path: String,
        old_value: DiffValue,
    },
    /// 추가된 컴포넌트 (Undo 시 제거)
    AddedComponent {
        entity: Entity,
        component_type: String,
    },
    /// 제거된 컴포넌트 (Undo 시 복원)
    RemovedComponent {
        entity: Entity,
        component_type: String,
        values: HashMap<String, DiffValue>,
    },
}

impl ApplyDiffCommand {
    /// 새 ApplyDiffCommand 생성
    pub fn new(session: &AIDiffSession, change_ids: Vec<DiffChangeId>) -> Self {
        // 선택된 변경 사항만 복사
        let changes: Vec<DiffChange> = session
            .changes
            .iter()
            .filter(|c| change_ids.contains(&c.id))
            .cloned()
            .collect();

        Self {
            session_id: session.id.0,
            change_ids,
            changes,
            previous_states: Vec::new(),
            applied_count: 0,
        }
    }

    /// 승인된 모든 변경 적용
    pub fn from_accepted(session: &AIDiffSession) -> Self {
        let change_ids: Vec<DiffChangeId> = session
            .changes
            .iter()
            .filter(|c| c.is_accepted() == Some(true))
            .map(|c| c.id)
            .collect();

        Self::new(session, change_ids)
    }

    /// Transform 필드 값 적용
    fn apply_transform_field(
        transform: &mut Transform,
        field_path: &str,
        value: &DiffValue,
    ) -> Option<DiffValue> {
        let old_value = match field_path {
            "translation.x" => {
                let old = DiffValue::Float(transform.translation.x as f64);
                if let DiffValue::Float(v) = value {
                    transform.translation.x = *v as f32;
                }
                Some(old)
            }
            "translation.y" => {
                let old = DiffValue::Float(transform.translation.y as f64);
                if let DiffValue::Float(v) = value {
                    transform.translation.y = *v as f32;
                }
                Some(old)
            }
            "translation.z" => {
                let old = DiffValue::Float(transform.translation.z as f64);
                if let DiffValue::Float(v) = value {
                    transform.translation.z = *v as f32;
                }
                Some(old)
            }
            "translation" => {
                let old = DiffValue::Vec3 {
                    x: transform.translation.x,
                    y: transform.translation.y,
                    z: transform.translation.z,
                };
                if let DiffValue::Vec3 { x, y, z } = value {
                    transform.translation.x = *x;
                    transform.translation.y = *y;
                    transform.translation.z = *z;
                }
                Some(old)
            }
            "scale.x" => {
                let old = DiffValue::Float(transform.scale.x as f64);
                if let DiffValue::Float(v) = value {
                    transform.scale.x = *v as f32;
                }
                Some(old)
            }
            "scale.y" => {
                let old = DiffValue::Float(transform.scale.y as f64);
                if let DiffValue::Float(v) = value {
                    transform.scale.y = *v as f32;
                }
                Some(old)
            }
            "scale.z" => {
                let old = DiffValue::Float(transform.scale.z as f64);
                if let DiffValue::Float(v) = value {
                    transform.scale.z = *v as f32;
                }
                Some(old)
            }
            "scale" => {
                let old = DiffValue::Vec3 {
                    x: transform.scale.x,
                    y: transform.scale.y,
                    z: transform.scale.z,
                };
                if let DiffValue::Vec3 { x, y, z } = value {
                    transform.scale.x = *x;
                    transform.scale.y = *y;
                    transform.scale.z = *z;
                }
                Some(old)
            }
            "rotation" => {
                let old = DiffValue::Vec4 {
                    x: transform.rotation.x,
                    y: transform.rotation.y,
                    z: transform.rotation.z,
                    w: transform.rotation.w,
                };
                if let DiffValue::Vec4 { x, y, z, w } = value {
                    transform.rotation.x = *x;
                    transform.rotation.y = *y;
                    transform.rotation.z = *z;
                    transform.rotation.w = *w;
                }
                Some(old)
            }
            _ => None,
        };
        old_value
    }
}

impl Command for ApplyDiffCommand {
    fn name(&self) -> &str {
        "Apply AI Changes"
    }

    fn execute(&mut self, world: &mut World) {
        self.previous_states.clear();
        self.applied_count = 0;

        // 먼저 모든 변경사항의 필요 데이터를 추출 (borrow 문제 회피)
        let mut entity_mods: Vec<(Entity, Vec<EntityModification>)> = Vec::new();
        let mut add_components: Vec<(Entity, String, HashMap<String, DiffValue>)> = Vec::new();
        let mut remove_components: Vec<(Entity, String)> = Vec::new();

        for change in &self.changes {
            match &change.change_type {
                DiffChangeType::Entity {
                    entity,
                    modifications,
                    ..
                } => {
                    entity_mods.push((*entity, modifications.clone()));
                }
                DiffChangeType::AddComponent {
                    entity,
                    component_type,
                    initial_values,
                    ..
                } => {
                    add_components.push((*entity, component_type.clone(), initial_values.clone()));
                }
                DiffChangeType::RemoveComponent {
                    entity,
                    component_type,
                    ..
                } => {
                    remove_components.push((*entity, component_type.clone()));
                }
                DiffChangeType::Code { .. } => {
                    log::warn!("[ApplyDiffCommand] Code changes not yet implemented");
                }
                DiffChangeType::CreateAsset { .. } => {
                    log::warn!("[ApplyDiffCommand] Asset creation not yet implemented");
                }
            }
        }

        // Entity 수정 적용
        for (entity, modifications) in entity_mods {
            for modification in modifications {
                if modification.component_type == "Transform" {
                    if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                        if let Some(old_value) = Self::apply_transform_field(
                            &mut transform,
                            &modification.field_path,
                            &modification.new_value,
                        ) {
                            self.previous_states.push(PreviousState::EntityModification {
                                entity,
                                component_type: modification.component_type.clone(),
                                field_path: modification.field_path.clone(),
                                old_value,
                            });
                            self.applied_count += 1;
                        }
                    }
                }
            }
        }

        // 컴포넌트 추가
        for (entity, component_type, initial_values) in add_components {
            if component_type == "Transform" {
                if world.get::<Transform>(entity).is_none() {
                    let mut transform = Transform::default();

                    for (field, value) in &initial_values {
                        Self::apply_transform_field(&mut transform, field, value);
                    }

                    world.entity_mut(entity).insert(transform);

                    self.previous_states.push(PreviousState::AddedComponent {
                        entity,
                        component_type: component_type.clone(),
                    });
                    self.applied_count += 1;
                }
            }
        }

        // 컴포넌트 제거
        for (entity, component_type) in remove_components {
            if component_type == "Transform" {
                if let Some(transform) = world.get::<Transform>(entity) {
                    let mut values = HashMap::new();
                    values.insert(
                        "translation".to_string(),
                        DiffValue::Vec3 {
                            x: transform.translation.x,
                            y: transform.translation.y,
                            z: transform.translation.z,
                        },
                    );
                    values.insert(
                        "scale".to_string(),
                        DiffValue::Vec3 {
                            x: transform.scale.x,
                            y: transform.scale.y,
                            z: transform.scale.z,
                        },
                    );
                    values.insert(
                        "rotation".to_string(),
                        DiffValue::Vec4 {
                            x: transform.rotation.x,
                            y: transform.rotation.y,
                            z: transform.rotation.z,
                            w: transform.rotation.w,
                        },
                    );

                    self.previous_states.push(PreviousState::RemovedComponent {
                        entity,
                        component_type: component_type.clone(),
                        values,
                    });

                    world.entity_mut(entity).remove::<Transform>();
                    self.applied_count += 1;
                }
            }
        }

        log::info!(
            "[ApplyDiffCommand] Applied {} changes from session {}",
            self.applied_count,
            self.session_id
        );
    }

    fn undo(&mut self, world: &mut World) {
        // 역순으로 복원
        for state in self.previous_states.iter().rev() {
            match state {
                PreviousState::EntityModification {
                    entity,
                    component_type,
                    field_path,
                    old_value,
                } => {
                    if component_type == "Transform" {
                        if let Some(mut transform) = world.get_mut::<Transform>(*entity) {
                            Self::apply_transform_field(&mut transform, field_path, old_value);
                        }
                    }
                }
                PreviousState::AddedComponent {
                    entity,
                    component_type,
                } => {
                    // 추가된 컴포넌트 제거
                    if component_type == "Transform" {
                        world.entity_mut(*entity).remove::<Transform>();
                    }
                }
                PreviousState::RemovedComponent {
                    entity,
                    component_type,
                    values,
                } => {
                    // 제거된 컴포넌트 복원
                    if component_type == "Transform" {
                        let mut transform = Transform::default();
                        for (field, value) in values {
                            Self::apply_transform_field(&mut transform, field, value);
                        }
                        world.entity_mut(*entity).insert(transform);
                    }
                }
            }
        }

        log::info!(
            "[ApplyDiffCommand] Undo: restored {} states",
            self.previous_states.len()
        );
    }
}

// ============================================================================
// Diff Transaction (여러 변경을 하나로 묶기)
// ============================================================================

/// Diff 트랜잭션
///
/// 여러 AI 변경을 하나의 Undo 단위로 묶습니다.
#[derive(Debug)]
pub struct DiffTransaction {
    /// 트랜잭션 이름
    pub name: String,
    /// 포함된 커맨드들
    commands: Vec<ApplyDiffCommand>,
}

impl DiffTransaction {
    /// 새 트랜잭션 시작
    pub fn begin(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            commands: Vec::new(),
        }
    }

    /// 커맨드 추가
    pub fn add(&mut self, command: ApplyDiffCommand) {
        self.commands.push(command);
    }

    /// 트랜잭션이 비어있는지
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// 포함된 커맨드 수
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
}

impl Command for DiffTransaction {
    fn name(&self) -> &str {
        &self.name
    }

    fn execute(&mut self, world: &mut World) {
        for cmd in &mut self.commands {
            cmd.execute(world);
        }
        log::info!(
            "[DiffTransaction] Executed '{}' with {} commands",
            self.name,
            self.commands.len()
        );
    }

    fn undo(&mut self, world: &mut World) {
        // 역순으로 undo
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(world);
        }
        log::info!(
            "[DiffTransaction] Undo '{}' ({} commands)",
            self.name,
            self.commands.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::ai_diff::types::*;

    #[test]
    fn test_apply_diff_command_creation() {
        let mut session = AIDiffSession::new("Test");
        session.add_change(DiffChange::new(
            DiffChangeType::Entity {
                entity: Entity::from_raw(1),
                entity_name: "Test".to_string(),
                modifications: vec![EntityModification {
                    component_type: "Transform".to_string(),
                    field_path: "translation.x".to_string(),
                    old_value: DiffValue::Float(0.0),
                    new_value: DiffValue::Float(10.0),
                }],
            },
            "Move X",
        ));

        session.changes[0].set_accepted(true);

        let cmd = ApplyDiffCommand::from_accepted(&session);
        assert_eq!(cmd.changes.len(), 1);
    }

    #[test]
    fn test_diff_transaction() {
        let mut tx = DiffTransaction::begin("Test Transaction");
        assert!(tx.is_empty());
        assert_eq!(tx.command_count(), 0);
    }
}
