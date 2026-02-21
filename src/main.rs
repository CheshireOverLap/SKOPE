//! SKOPE Engine - Main Entry Point
//!
//! SlateApp 기반 멀티 윈도우 아키텍처

use skope_gltf as gltf_loader;
mod ecs_components;
mod ecs_resources;
mod ecs_systems;
mod assets;
mod skope_data;
mod scene;
mod physics;

mod renderer;
mod debug;
mod scripting;
mod audio;
mod shaders;
use skope_effects as particles;
mod prefab;
mod editor;
mod material;
mod texture;
mod app;
mod game;
mod paths;
use app::{EngineHandler, init_ecs, init_scripting};

fn main() {
    // 로그 시스템 초기화
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    // ECS 초기화
    let (mut world, schedule) = init_ecs();

    // Debug UI 초기화
    let debug_ui = debug::ui::DebugUi::new();

    // Lua 스크립팅 초기화
    init_scripting(&mut world);

    // EngineHandler 생성
    let handler = EngineHandler::new(
        world,
        schedule,
        debug_ui,
    );

    // 폰트 데이터 로드
    let fonts_dir = std::path::Path::new(paths::engine::FONTS);
    let font_path = fonts_dir.join("NotoSansKR-Regular.ttf");
    let font_data = std::fs::read(&font_path).unwrap_or_else(|e| {
        log::warn!("[Main] Failed to load font {:?}: {}", font_path, e);
        Vec::new()
    });

    // 모노스페이스 폰트 체인 로드 (JetBrains Mono → D2Coding 폴백)
    let mut mono_chain: Vec<Vec<u8>> = Vec::new();
    for name in &["JetBrainsMono-Regular.ttf", "D2Coding-Regular.ttf"] {
        let path = fonts_dir.join(name);
        match std::fs::read(&path) {
            Ok(data) => {
                log::info!("[Main] Loaded monospace font: {}", name);
                mono_chain.push(data);
            }
            Err(_) => {
                log::info!("[Main] Monospace font not found: {:?} (optional)", path);
            }
        }
    }

    // 윈도우 아이콘 로드
    let window_icon = load_window_icon_data();

    // GPU features/limits (엔진 요구사항)
    let required_features = wgpu::Features::TEXTURE_BINDING_ARRAY
        | wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
        | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;

    let mut required_limits = wgpu::Limits::default();
    required_limits.max_sampled_textures_per_shader_stage = 4096;
    required_limits.max_storage_textures_per_shader_stage = 4096;
    required_limits.max_storage_buffers_per_shader_stage = 16; // MaterialEval Group2 needs 10 storage buffers
    required_limits.max_binding_array_elements_per_shader_stage = 4096;
    required_limits.max_binding_array_sampler_elements_per_shader_stage = 16;

    // 아이콘 매니저 — engine_assets/icons/ 스캔 및 프리로드 목록 생성
    let icon_manager = editor::icons::IconManager::new(paths::engine::ICONS);
    let preload_icons = icon_manager.build_preload_list();

    // SlateApp 설정
    let mut config = skope_ui::application::SlateAppConfig::new("SKOPE Engine")
        .with_size(1440, 810)
        .with_font(font_data)
        .with_clear_color(0.12, 0.12, 0.14, 1.0)
        .with_required_features(required_features)
        .with_required_limits(required_limits)
        .with_decorations(false)  // 커스텀 타이틀바
        .with_resizable(true)
        .with_icon_base_path(paths::engine::ICONS)
        .with_preload_icons(preload_icons);

    // 모노스페이스 폰트 체인 등록
    if !mono_chain.is_empty() {
        config = config.with_font_chain(skope_ui::core::FontFamily::Monospace, mono_chain);
    }

    if let Some((rgba, w, h)) = window_icon {
        config = config.with_window_icon(rgba, w, h);
    }

    // SlateApp + EngineHandler 실행
    let slate_app = skope_ui::application::SlateApp::new(config, handler);
    slate_app.run().unwrap();
}

/// 윈도우 아이콘 RGBA 데이터 로드
fn load_window_icon_data() -> Option<(Vec<u8>, u32, u32)> {
    let icon_path = std::path::Path::new(paths::engine::ICONS).join("skope_logo.png");
    if !icon_path.exists() {
        log::warn!("[Window] Icon not found: {:?}", icon_path);
        return None;
    }

    match image::open(&icon_path) {
        Ok(img) => {
            let resized = img.resize(64, 64, image::imageops::FilterType::Lanczos3);
            let rgba = resized.to_rgba8();
            let (width, height) = rgba.dimensions();
            log::info!("[Window] Loaded SKOPE logo as window icon ({}x{})", width, height);
            Some((rgba.into_raw(), width, height))
        }
        Err(e) => {
            log::warn!("[Window] Failed to load icon: {}", e);
            None
        }
    }
}
