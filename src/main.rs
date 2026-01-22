//! SKOPE Engine - Main Entry Point
//!
//! 모든 로직은 app 모듈로 분리됨

use winit::event_loop::{ControlFlow, EventLoop};

use skope_gltf as gltf_loader;
mod ecs_components;
mod ecs_resources;
mod ecs_systems;
mod assets;
mod skope_data;
mod physics;
use skope_hair as hair;
mod renderer;
mod debug;
use skope_game_ui as ui;
mod scripting;
mod audio;
mod shaders;
use skope_effects as particles;
mod prefab;
mod sprite;
mod editor;
mod material;
mod texture;
mod app;
mod game;
mod paths;
mod splash;

use app::{App, init_ecs, init_game_ui, init_scripting};

fn main() {
    // 로그 시스템 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    // ECS 초기화
    let (mut world, schedule) = init_ecs();

    // Debug UI 초기화
    let debug_ui = debug::ui::DebugUi::new();

    // Game UI 초기화
    let (game_ui, ui_hot_reloader) = init_game_ui();

    // Lua 스크립팅 초기화
    init_scripting(&mut world);

    // App 생성 및 실행
    let mut app = App::new(
        world,
        schedule,
        debug_ui,
        game_ui,
        ui_hot_reloader,
    );

    event_loop.run_app(&mut app).unwrap();
}
