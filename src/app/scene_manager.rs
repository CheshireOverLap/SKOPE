//! Scene Manager
//!
//! 씬 저장/로드 관련 로직

#![allow(dead_code)]

use std::path::Path;
use bevy_ecs::prelude::*;
use glam;

use crate::App;
use crate::ecs_components;
use crate::skope_data;
use crate::editor;

impl App {
    /// 현재 씬을 .skope 파일로 저장
    pub fn save_current_scene(&mut self) {
        use skope_data::{Scene, SceneEntity, Vec3 as SkopeVec3, ComponentData};

        let mut entities = Vec::new();

        // World에서 NodeName + Transform 가진 엔티티 추출
        let mut query = self.world.query::<(
            Entity,
            &ecs_components::NodeName,
            &ecs_components::Transform,
            Option<&ecs_components::MeshInstance>,
        )>();

        for (_entity, name, transform, mesh_opt) in query.iter(&self.world) {
            // MeshInstance가 있으면 StaticProp으로, 없으면 기본 컴포넌트로
            let component = if let Some(mesh) = mesh_opt {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: Some(format!("mesh_{}", mesh.mesh_index)),
                }
            } else {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: None,
                }
            };

            // Quaternion을 Euler로 변환
            let (x, y, z) = transform.rotation.to_euler(glam::EulerRot::XYZ);

            entities.push(SceneEntity {
                name: name.0.clone(),
                position: SkopeVec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ),
                rotation: SkopeVec3::new(x, y, z),
                scale: SkopeVec3::new(
                    transform.scale.x,
                    transform.scale.y,
                    transform.scale.z,
                ),
                component,
            });
        }

        let scene = Scene { entities };

        // 파일로 저장
        let path = "levels/Scene_saved.skope";
        match scene.to_file(path) {
            Ok(_) => log::info!("[Editor] Scene saved to {} ({} entities)", path, scene.entities.len()),
            Err(e) => log::error!("[Editor] Failed to save scene: {}", e),
        }
    }

    /// 새 씬 생성
    pub fn new_scene(&mut self) {
        log::info!("[Editor] Creating new scene...");

        // 기존 엔티티 모두 삭제 (루트 제외)
        let mut to_despawn = Vec::new();
        {
            let mut query = self.world.query::<(Entity, &ecs_components::NodeName)>();
            for (entity, _name) in query.iter(&self.world) {
                to_despawn.push(entity);
            }
        }

        for entity in to_despawn {
            if let Ok(entity_mut) = self.world.get_entity_mut(entity) {
                entity_mut.despawn();
            }
        }

        // 씬 경로 초기화
        self.dock_layout.current_scene_path = None;
        self.dock_layout.scene_dirty = false;

        log::info!("[Editor] New scene created");
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
            self.load_scene_from_path(&path);
            self.dock_layout.current_scene_path = Some(path);
            self.dock_layout.scene_dirty = false;
        }
    }

    /// 씬 저장 (현재 경로 있으면 그대로, 없으면 Save As)
    pub fn save_scene(&mut self) {
        if let Some(path) = &self.dock_layout.current_scene_path.clone() {
            self.save_scene_to_path(path);
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
            self.save_scene_to_path(&path);
            self.dock_layout.current_scene_path = Some(path);
            self.dock_layout.scene_dirty = false;
        }
    }

    /// 지정 경로에 씬 저장
    pub fn save_scene_to_path(&mut self, path: &Path) {
        use skope_data::{Scene, SceneEntity, Vec3 as SkopeVec3, ComponentData};

        let mut entities = Vec::new();

        // World에서 엔티티 추출
        let mut query = self.world.query::<(
            Entity,
            &ecs_components::NodeName,
            &ecs_components::Transform,
            Option<&ecs_components::MeshInstance>,
            Option<&ecs_components::Light>,
        )>();

        for (_entity, name, transform, mesh_opt, light_opt) in query.iter(&self.world) {
            let component = if let Some(light) = light_opt {
                ComponentData::Light {
                    light_type: match light.light_type {
                        ecs_components::LightType::Sun => skope_data::LightType::Sun,
                        ecs_components::LightType::Point => skope_data::LightType::Point,
                        ecs_components::LightType::Spot => skope_data::LightType::Spot,
                        _ => skope_data::LightType::Point,  // 기타 (Area 등) → Point로 대체
                    },
                    light_energy: light.intensity,
                    light_color: (light.color.x, light.color.y, light.color.z),
                }
            } else if let Some(mesh) = mesh_opt {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: Some(format!("mesh_{}", mesh.mesh_index)),
                }
            } else {
                ComponentData::StaticProp {
                    has_collision: false,
                    mesh: None,
                }
            };

            let (rx, ry, rz) = transform.rotation.to_euler(glam::EulerRot::XYZ);

            entities.push(SceneEntity {
                name: name.0.clone(),
                position: SkopeVec3::new(
                    transform.translation.x,
                    transform.translation.y,
                    transform.translation.z,
                ),
                rotation: SkopeVec3::new(rx, ry, rz),
                scale: SkopeVec3::new(
                    transform.scale.x,
                    transform.scale.y,
                    transform.scale.z,
                ),
                component,
            });
        }

        let scene = Scene { entities };

        match scene.to_file(path) {
            Ok(_) => {
                log::info!("[Editor] Scene saved to {:?} ({} entities)", path, scene.entities.len());
                self.dock_layout.scene_dirty = false;
            }
            Err(e) => log::error!("[Editor] Failed to save scene: {}", e),
        }
    }

    /// 지정 경로에서 씬 로드
    pub fn load_scene_from_path(&mut self, path: &Path) {
        use skope_data::Scene;

        match Scene::from_file(path) {
            Ok(scene) => {
                // 기존 엔티티 삭제
                self.new_scene();

                // 새 엔티티 스폰
                for entity_data in &scene.entities {
                    let translation = glam::Vec3::new(
                        entity_data.position.x,
                        entity_data.position.y,
                        entity_data.position.z,
                    );
                    let rotation = glam::Quat::from_euler(
                        glam::EulerRot::XYZ,
                        entity_data.rotation.x,
                        entity_data.rotation.y,
                        entity_data.rotation.z,
                    );
                    let scale = glam::Vec3::new(
                        entity_data.scale.x,
                        entity_data.scale.y,
                        entity_data.scale.z,
                    );

                    let transform = ecs_components::Transform {
                        translation,
                        rotation,
                        scale,
                    };

                    // 기본 엔티티 스폰
                    let mut entity_cmd = self.world.spawn((
                        ecs_components::NodeName(entity_data.name.clone()),
                        transform,
                        ecs_components::GlobalTransform::default(),
                    ));

                    // 컴포넌트에 따라 추가
                    match &entity_data.component {
                        skope_data::ComponentData::StaticProp { mesh, .. } => {
                            if let Some(mesh_name) = mesh {
                                // mesh_0, mesh_1 형식에서 인덱스 추출
                                if let Some(idx_str) = mesh_name.strip_prefix("mesh_") {
                                    if let Ok(idx) = idx_str.parse::<usize>() {
                                        entity_cmd.insert(ecs_components::MeshInstance {
                                            mesh_index: idx,
                                        });
                                    }
                                }
                            }
                        }
                        skope_data::ComponentData::Light { light_type, light_energy, light_color } => {
                            let lt = match light_type {
                                skope_data::LightType::Sun => ecs_components::LightType::Sun,
                                skope_data::LightType::Point => ecs_components::LightType::Point,
                                skope_data::LightType::Spot => ecs_components::LightType::Spot,
                                skope_data::LightType::Area => ecs_components::LightType::Point,  // Area → Point 대체
                            };
                            entity_cmd.insert(ecs_components::Light {
                                light_type: lt,
                                color: glam::Vec3::new(light_color.0, light_color.1, light_color.2),
                                intensity: *light_energy,
                                range: 10.0,
                                spot_angle: 45.0,
                                cast_shadows: true,
                            });
                        }
                        _ => {}
                    }
                }

                log::info!("[Editor] Scene loaded from {:?} ({} entities)", path, scene.entities.len());
            }
            Err(e) => {
                log::error!("[Editor] Failed to load scene: {}", e);
            }
        }
    }

    /// 메뉴 액션 처리
    pub fn handle_menu_action(&mut self, action: editor::MenuAction, event_loop: &winit::event_loop::ActiveEventLoop) {
        match action {
            editor::MenuAction::NewScene => {
                self.new_scene();
            }
            editor::MenuAction::OpenScene => {
                self.open_scene_dialog();
            }
            editor::MenuAction::SaveScene => {
                self.save_scene();
            }
            editor::MenuAction::SaveSceneAs => {
                self.save_scene_as_dialog();
            }
            editor::MenuAction::Quit => {
                log::info!("[Menu] Quit requested");
                event_loop.exit();
            }
            // GameObject 액션은 state.rs에서 처리됨
            editor::MenuAction::CreateEmpty |
            editor::MenuAction::Create3DObject(_) |
            editor::MenuAction::CreateLight(_) |
            editor::MenuAction::CreateCamera => {}
        }
    }
}
