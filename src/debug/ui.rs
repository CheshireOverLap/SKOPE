//! Debug UI Bridge
//!
//! Re-exports from skope_debug_ui crate + local entity collection

// Re-export everything from the crate
pub use skope_debug_ui::*;

use skope_ecs::prelude::*;

/// ECS World에서 엔티티 정보 수집
pub fn collect_entity_info(world: &mut World) -> Vec<EntityInfo> {
    use crate::ecs_components::{
        Transform, NodeName, MeshInstance, Camera, CameraController,
        Health, Player, Weapon, Team, Velocity, BoxCollider, SphereCollider,
        ScriptComponent, EnemySpawner,
    };

    let mut entities = Vec::new();

    // 기본 컴포넌트 쿼리
    let query = world.query::<(
        Entity,
        Option<&NodeName>,
        Option<&Transform>,
        Option<&MeshInstance>,
        Option<&Camera>,
        Option<&CameraController>,
        Option<&Health>,
        Option<&Player>,
        Option<&Weapon>,
        Option<&Team>,
        Option<&Velocity>,
    )>();

    for (entity, name, transform, mesh, camera, camera_ctrl, health, player, weapon, team, velocity) in query.iter(world) {
        let entity_id = entity.to_bits();
        let entity_name = name
            .map(|n| n.0.clone())
            .unwrap_or_else(|| format!("Entity_{}", entity_id & 0xFFFF));

        let mut info = EntityInfo::new(entity_id, entity_name);

        // Transform 데이터
        if let Some(t) = transform {
            info.position = t.translation;
            info.rotation = t.rotation;
            info.scale = t.scale;
            info.components.push("Transform".to_string());
        }

        // 기본 컴포넌트 목록
        if mesh.is_some() {
            info.components.push("MeshInstance".to_string());
        }
        if camera.is_some() {
            info.components.push("Camera".to_string());
        }
        if camera_ctrl.is_some() {
            info.components.push("CameraController".to_string());
        }

        // 게임 컴포넌트 목록
        if let Some(h) = health {
            info.components.push(format!("Health({}/{})", h.current as i32, h.maximum as i32));
        }
        if player.is_some() {
            info.components.push("Player".to_string());
        }
        if let Some(w) = weapon {
            info.components.push(format!("Weapon({}/{})", w.ammo, w.max_ammo));
        }
        if let Some(t) = team {
            let team_str = match t {
                Team::Player => "Team(Player)",
                Team::Enemy => "Team(Enemy)",
                Team::Neutral => "Team(Neutral)",
            };
            info.components.push(team_str.to_string());
        }
        if velocity.is_some() {
            info.components.push("Velocity".to_string());
        }

        entities.push(info);
    }

    // 추가 컴포넌트 쿼리 (별도로 확인)
    let collider_query = world.query::<(Entity, Option<&BoxCollider>, Option<&SphereCollider>)>();
    let collider_map: std::collections::HashMap<u64, Vec<String>> = collider_query
        .iter(world)
        .filter_map(|(e, box_c, sphere_c)| {
            let mut components = Vec::new();
            if box_c.is_some() {
                components.push("BoxCollider".to_string());
            }
            if sphere_c.is_some() {
                components.push("SphereCollider".to_string());
            }
            if !components.is_empty() {
                Some((e.to_bits(), components))
            } else {
                None
            }
        })
        .collect();

    let script_query = world.query::<(Entity, Option<&ScriptComponent>, Option<&EnemySpawner>)>();
    let script_map: std::collections::HashMap<u64, Vec<String>> = script_query
        .iter(world)
        .filter_map(|(e, script, spawner)| {
            let mut components = Vec::new();
            if let Some(s) = script {
                components.push(format!("Script({})", s.script_path.split('/').next_back().unwrap_or(&s.script_path)));
            }
            if spawner.is_some() {
                components.push("EnemySpawner".to_string());
            }
            if !components.is_empty() {
                Some((e.to_bits(), components))
            } else {
                None
            }
        })
        .collect();

    // 추가 컴포넌트 병합
    for entity in &mut entities {
        if let Some(colliders) = collider_map.get(&entity.id) {
            entity.components.extend(colliders.clone());
        }
        if let Some(scripts) = script_map.get(&entity.id) {
            entity.components.extend(scripts.clone());
        }
    }

    // 이름순 정렬
    entities.sort_by(|a, b| a.name.cmp(&b.name));
    entities
}
