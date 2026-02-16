//! Scene Manager
//!
//! 씬 저장/로드 관련 로직 — 독립 함수 + App thin wrapper

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use skope_ecs::prelude::*;

use crate::App;
use crate::ecs_components;
use crate::paths;

// ============ Standalone Functions ============
// EngineHandler, App 등 어디서든 &mut World만 있으면 사용 가능

/// 지정 경로에 씬 저장. 성공 시 true 반환.
/// ComponentRegistry를 사용하여 등록된 모든 컴포넌트를 자동 직렬화.
pub fn save_scene_to_path(world: &mut World, path: &Path) -> bool {
    match crate::scene::save_to_file(world, path) {
        Ok(()) => {
            log::info!("[Scene] Saved to {:?}", path);
            true
        }
        Err(e) => {
            log::error!("[Scene] Save failed: {}", e);
            false
        }
    }
}

/// 지정 경로에서 씬 로드. 성공 시 true 반환.
/// ComponentRegistry를 사용하여 등록된 모든 컴포넌트를 자동 역직렬화.
pub fn load_scene_from_path(world: &mut World, path: &Path) -> bool {
    clear_scene(world);
    match crate::scene::load_from_file(world, path) {
        Ok(()) => {
            crate::skope_data::process_pending_colliders(world);
            log::info!("[Scene] Loaded from {:?}", path);
            true
        }
        Err(e) => {
            log::error!("[Scene] Load failed: {}", e);
            false
        }
    }
}

/// 새 씬 생성 (기존 엔티티 모두 삭제 + 기본 엔티티 스폰)
pub fn new_scene(world: &mut World) {
    log::info!("[Editor] Creating new scene...");
    clear_scene(world);
    spawn_default_scene_entities(world);
    log::info!("[Editor] New scene created");
}

/// 기존 씬 엔티티 모두 삭제 (NodeName 가진 엔티티)
fn clear_scene(world: &mut World) {
    let to_despawn: Vec<Entity> = {
        let query = world.query::<(Entity, &ecs_components::NodeName)>();
        query.iter(world).map(|(e, _)| e).collect()
    };

    for entity in to_despawn {
        if let Ok(entity_mut) = world.get_entity_mut(entity) {
            entity_mut.despawn();
        }
    }
}

/// dirty 시 저장 확인 다이얼로그. false 반환 = 호출자가 작업 중단해야 함
pub fn prompt_save_if_dirty(dirty: bool, current_path: &Option<PathBuf>, world: &mut World) -> bool {
    if !dirty {
        return true;
    }

    let result = rfd::MessageDialog::new()
        .set_title("Unsaved Changes")
        .set_description("Scene has unsaved changes. Save before continuing?")
        .set_buttons(rfd::MessageButtons::YesNoCancel)
        .show();

    match result {
        rfd::MessageDialogResult::Yes => {
            // 저장 후 진행
            if let Some(path) = current_path {
                save_scene_to_path(world, path)
            } else {
                // Save As 다이얼로그
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("SKOPE Scene", &["skope"])
                    .set_title("Save Scene As")
                    .set_file_name("untitled.skope")
                    .save_file()
                {
                    save_scene_to_path(world, &path)
                } else {
                    false // 취소됨
                }
            }
        }
        rfd::MessageDialogResult::No => {
            true // 저장하지 않고 진행
        }
        _ => {
            false // Cancel - 작업 중단
        }
    }
}

/// 빈 씬 기본 엔티티 스폰 (Sun Light)
fn spawn_default_scene_entities(world: &mut World) {
    // 45° 아래, 45° 옆에서 비추는 Sun Light
    let sun_rotation = glam::Quat::from_euler(
        glam::EulerRot::XYZ,
        -std::f32::consts::FRAC_PI_4,  // -45° X (위에서 아래로)
        std::f32::consts::FRAC_PI_4,   // 45° Y
        0.0,
    );

    world.spawn((
        ecs_components::NodeName("Directional Light".into()),
        ecs_components::Transform {
            translation: glam::Vec3::ZERO,
            rotation: sun_rotation,
            scale: glam::Vec3::ONE,
        },
        ecs_components::GlobalTransform::default(),
        ecs_components::Light::sun(2.0, glam::Vec3::new(1.0, 0.98, 0.95)),
    ));
}

// ============ impl App — thin wrappers (backward compatibility) ============

impl App {
    /// 현재 씬을 .skope 파일로 저장 (레거시 경로)
    pub fn save_current_scene(&mut self) {
        let path = format!("{}/Scene_saved.skope", paths::game::LEVELS);
        save_scene_to_path(&mut self.world, Path::new(&path));
    }

    /// 새 씬 생성
    pub fn new_scene(&mut self) {
        new_scene(&mut self.world);
        self.current_scene_path = None;
        self.scene_dirty = false;
    }

    /// 씬 열기 대화상자
    pub fn open_scene_dialog(&mut self) {
        log::info!("[Editor] Opening scene dialog...");

        let file = rfd::FileDialog::new()
            .add_filter("SKOPE Scene", &["skope"])
            .add_filter("All Files", &["*"])
            .set_title("Open Scene")
            .pick_file();

        if let Some(path) = file {
            log::info!("[Editor] Selected: {:?}", path);
            if load_scene_from_path(&mut self.world, &path) {
                self.current_scene_path = Some(path);
                self.scene_dirty = false;
            }
        }
    }

    /// 씬 저장 (현재 경로 있으면 그대로, 없으면 Save As)
    pub fn save_scene(&mut self) {
        if let Some(path) = &self.current_scene_path.clone() {
            if save_scene_to_path(&mut self.world, path) {
                self.scene_dirty = false;
            }
        } else {
            self.save_scene_as_dialog();
        }
    }

    /// 다른 이름으로 저장 대화상자
    pub fn save_scene_as_dialog(&mut self) {
        log::info!("[Editor] Save As dialog...");

        let file = rfd::FileDialog::new()
            .add_filter("SKOPE Scene", &["skope"])
            .set_title("Save Scene As")
            .set_file_name("untitled.skope")
            .save_file();

        if let Some(path) = file {
            log::info!("[Editor] Saving to: {:?}", path);
            if save_scene_to_path(&mut self.world, &path) {
                self.current_scene_path = Some(path);
                self.scene_dirty = false;
            }
        }
    }

    /// 지정 경로에 씬 저장
    pub fn save_scene_to_path(&mut self, path: &Path) {
        if save_scene_to_path(&mut self.world, path) {
            self.scene_dirty = false;
        }
    }

    /// 지정 경로에서 씬 로드
    pub fn load_scene_from_path(&mut self, path: &Path) {
        load_scene_from_path(&mut self.world, path);
    }
}
