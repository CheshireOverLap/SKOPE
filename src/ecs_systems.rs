// SKOPE ECS Systems
// 모든 게임 로직을 ECS 시스템으로 분리

use bevy_ecs::prelude::*;

pub mod physics;
pub mod animation;
pub mod camera;
pub mod transform;
pub mod render_extract;
pub mod lighting;
pub mod scripting;
pub mod spells;
pub mod triggers;
pub mod ai;
pub mod inventory;
pub mod effects;
pub mod player;

// Re-exports
pub use physics::physics_step_system;
pub use crate::physics::{collect_collision_events_system, map_collision_to_entities_system};
pub use animation::{
    ai_animation_sync_system,
    animator_controller_update_system,
    animator_controller_render_system,
};
pub use camera::{camera_input_system, camera_extract_system};
pub use transform::transform_propagate_system;
pub use render_extract::{mesh_extract_system, skinned_mesh_extract_system};
pub use lighting::{lighting_extract_system, light_buffer_update_system};
pub use scripting::{entity_sync_system, debug_draw_sync_system};
pub use spells::{spell_process_system, effect_update_system};
pub use triggers::trigger_check_system;
pub use ai::{ai_state_machine_system, ai_movement_system};
pub use inventory::{item_pickup_system, item_use_system};
pub use effects::{
    flipbook_update_system,
    vat_update_system,
    particle_emitter_update_system,
    effect_extract_system,
    effect_despawn_system,
    effect_spawn_system,
    effect_time_update_system,
    effect_instance_system,
    effect_instance_cleanup_system,
    effect_lua_process_system,
    effect_callback_system,
};
pub use player::{
    player_input_system,
    player_movement_system,
    camera_follow_player_system,
};

// Magic Circle 시스템 re-exports
pub use skope_magic::{
    magic_circle_update_system,
    magic_circle_spawn_system,
    magic_circle_despawn_system,
    magic_circle_extract_system,
};

/// 시스템 실행 단계 정의
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SystemStage {
    /// 물리 시뮬레이션
    Physics,
    /// 애니메이션 업데이트
    Animation,
    /// Transform 계층 전파
    TransformPropagate,
    /// 입력 처리 (카메라 등)
    Input,
    /// 플레이어 시스템
    Player,
    /// 이펙트 시스템 (Flipbook, VAT, Particle)
    Effects,
    /// 렌더링 데이터 추출
    RenderExtract,
    /// 스크립팅
    Scripting,
    /// 스펠/마법 시스템
    Spells,
    /// 트리거 시스템
    Triggers,
    /// AI 시스템
    Ai,
    /// 인벤토리 시스템
    Inventory,
    /// 마법진 시스템
    MagicCircle,
}

/// Schedule에 모든 ECS 시스템 등록
pub fn configure_systems(schedule: &mut Schedule) {
    use crate::scripting::script_update_system;

    schedule
        // 물리 (순서: step → collect events → map to entities)
        .add_systems((
            physics_step_system,
            collect_collision_events_system,
            map_collision_to_entities_system,
        ).chain().in_set(SystemStage::Physics))
        // 애니메이션 (AnimatorController)
        .add_systems(
            animator_controller_update_system
            .in_set(SystemStage::Animation)
        )
        // Transform 전파
        .add_systems(transform_propagate_system.in_set(SystemStage::TransformPropagate))
        // 입력
        .add_systems(camera_input_system.in_set(SystemStage::Input))
        // 플레이어 시스템 (입력 → 이동 → 카메라 팔로우)
        .add_systems((
            player_input_system,
            player_movement_system,
            camera_follow_player_system,
        ).chain().in_set(SystemStage::Player))
        // 이펙트 시스템 (Flipbook, VAT, Particle 업데이트)
        .add_systems((
            effect_time_update_system,
            flipbook_update_system,
            vat_update_system,
            particle_emitter_update_system,
            effect_spawn_system,
            effect_despawn_system,
            effect_instance_system,
            effect_instance_cleanup_system,
            effect_lua_process_system,
            effect_callback_system,
        ).in_set(SystemStage::Effects))
        // 렌더 추출 - 메시 관련
        .add_systems((
            camera_extract_system,
            mesh_extract_system,
            skinned_mesh_extract_system,
            animator_controller_render_system,
        ).in_set(SystemStage::RenderExtract))
        // 렌더 추출 - 라이팅 및 이펙트
        .add_systems((
            lighting_extract_system,
            light_buffer_update_system,
            effect_extract_system,
            magic_circle_extract_system,
        ).in_set(SystemStage::RenderExtract))
        // 마법진 시스템 (스폰 → 업데이트 → 디스폰)
        .add_systems((
            magic_circle_spawn_system,
            magic_circle_update_system,
            magic_circle_despawn_system,
        ).chain().in_set(SystemStage::MagicCircle))
        // 스크립팅 (entity_sync → script_update → debug_draw_sync)
        .add_systems((
            entity_sync_system,
            script_update_system,
            debug_draw_sync_system,
        ).chain().in_set(SystemStage::Scripting))
        // 스펠 시스템 (spell_process → effect_update)
        .add_systems((
            spell_process_system,
            effect_update_system,
        ).chain().in_set(SystemStage::Spells))
        // 트리거 시스템
        .add_systems(trigger_check_system.in_set(SystemStage::Triggers))
        // AI 시스템 (상태 머신 → 이동 → AI-Animation 동기화)
        .add_systems((
            ai_state_machine_system,
            ai_movement_system,
            ai_animation_sync_system,
        ).chain().in_set(SystemStage::Ai))
        // 인벤토리 시스템 (픽업 → 사용)
        .add_systems((
            item_pickup_system,
            item_use_system,
        ).chain().in_set(SystemStage::Inventory))
        // 실행 순서 설정
        .configure_sets((
            SystemStage::Physics,
            SystemStage::Animation,
            SystemStage::TransformPropagate,
            SystemStage::Input,
            SystemStage::Player,  // 입력 후 플레이어 처리
            SystemStage::Ai,  // 플레이어 후 AI 처리
            SystemStage::Inventory,  // AI 후 인벤토리
            SystemStage::Scripting,
            SystemStage::Spells,
            SystemStage::Effects,  // 스펠 후 이펙트 처리
            SystemStage::MagicCircle,  // 이펙트 후 마법진 처리
            SystemStage::Triggers,
            SystemStage::RenderExtract,
        ).chain());
}
