//! SKOPE Application Runner
//!
//! ECS 및 스크립팅 초기화 함수

use skope_ecs::prelude::*;

use crate::ecs_resources;
use crate::ecs_systems;
use crate::scripting;

/// ECS World 및 Schedule 초기화
pub fn init_ecs() -> (World, Schedule) {
    let mut world = World::new();
    let mut schedule = Schedule::default();

    // 기본 Resources 등록
    world.insert_resource(ecs_resources::Time::default());
    world.insert_resource(ecs_resources::KeyboardInput::default());
    world.insert_resource(ecs_resources::MouseInput::default());
    world.insert_resource(ecs_resources::GamePlayState::default());

    // Schedule에 systems 추가
    ecs_systems::configure_systems(&mut schedule);

    // RenderExtractedData 리소스 추가
    world.insert_resource(ecs_resources::RenderExtractedData::default());
    // Camera 시스템 리소스 등록
    world.insert_resource(ecs_resources::ActiveCameraShakes::default());
    world.insert_resource(ecs_resources::ViewTargetBlend::default());
    // Inventory 시스템 리소스 등록
    world.insert_resource(ecs_systems::inventory::ItemRegistry::new());
    world.init_resource::<skope_ecs::Events<ecs_systems::inventory::ItemUseEvent>>();

    // Effect 시스템 리소스 등록
    world.insert_resource(ecs_systems::effects::EffectAssets::default());

    // ComponentRegistry 초기화 + 컴포넌트 등록
    let mut registry = crate::scene::ComponentRegistry::default();
    crate::scene::registrations::register_all(&mut registry);
    world.insert_resource(registry);

    (world, schedule)
}

/// Lua 스크립팅 엔진 초기화
pub fn init_scripting(world: &mut World) {
    log::info!("=== Initializing Lua Scripting Engine ===");
    let script_engine = match scripting::ScriptEngine::new() {
        Ok(engine) => {
            if let Err(e) = engine.init_api() {
                log::error!("[Script] Failed to initialize API: {}", e);
            }
            log::info!("=== Lua scripting engine initialized");
            Some(engine)
        }
        Err(e) => {
            log::error!("[Script] Failed to create script engine: {}", e);
            None
        }
    };

    if let Some(engine) = script_engine {
        world.insert_non_send_resource(engine);
    }
}
