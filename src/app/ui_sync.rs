//! UI Synchronization Helpers
//!
//! 반복되는 UI 동기화 패턴을 추출한 헬퍼 메서드들
//! sync_hierarchy, sync_inspector는 main.rs에서 사용 중

#![allow(dead_code)]

use crate::App;
use bevy_ecs::entity::Entity;

impl App {
    /// Hierarchy 패널 재구성
    /// 엔티티 추가/삭제/이동 후 호출
    pub fn sync_hierarchy(&mut self) {
        if let Some(ref mut hierarchy) = self.hierarchy_panel {
            if let Some(ref mut editor) = self.fyrox_editor {
                hierarchy.rebuild(&mut self.world, &mut editor.ui);
            }
        }
    }

    /// Inspector 패널 동기화
    /// 선택 변경 후 호출
    pub fn sync_inspector(&mut self) {
        if let (Some(ref mut inspector), Some(ref editor)) =
            (&mut self.inspector_panel, &self.fyrox_editor)
        {
            inspector.sync_from_world(&self.world, &editor.ui);
        }
    }

    /// Hierarchy에서 선택 상태 동기화
    pub fn sync_hierarchy_selection(&mut self) {
        if let Some(ref hierarchy) = self.hierarchy_panel {
            if let Some(ref scene_viewer) = self.scene_viewer {
                if let Some(ref editor) = self.fyrox_editor {
                    hierarchy.sync_selection(&scene_viewer.selection, &editor.ui);
                }
            }
        }
    }

    /// Selection 업데이트 및 Gizmo 갱신
    pub fn update_selection(&mut self, entities: Vec<Entity>) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            scene_viewer.selection.entities = entities;
            scene_viewer.update_gizmo_from_selection(&self.world);
        }
    }

    /// Selection에 엔티티 추가 (Shift+클릭)
    pub fn add_to_selection(&mut self, entity: Entity) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            if !scene_viewer.selection.entities.contains(&entity) {
                scene_viewer.selection.entities.push(entity);
            }
            scene_viewer.update_gizmo_from_selection(&self.world);
        }
    }

    /// Selection에서 엔티티 토글 (Ctrl+클릭)
    pub fn toggle_selection(&mut self, entity: Entity) {
        if let Some(ref mut scene_viewer) = self.scene_viewer {
            if let Some(pos) = scene_viewer.selection.entities.iter().position(|&e| e == entity) {
                scene_viewer.selection.entities.remove(pos);
            } else {
                scene_viewer.selection.entities.push(entity);
            }
            scene_viewer.update_gizmo_from_selection(&self.world);
        }
    }

    /// 전체 UI 패널 동기화 (선택 변경 후)
    pub fn sync_all_panels(&mut self) {
        self.sync_hierarchy();
        self.sync_hierarchy_selection();
        self.sync_inspector();
    }

    /// 현재 선택된 엔티티 목록 가져오기
    pub fn get_selection(&self) -> Vec<Entity> {
        self.scene_viewer
            .as_ref()
            .map(|sv| sv.selection.entities.clone())
            .unwrap_or_default()
    }

    /// 선택이 비어있는지 확인
    pub fn has_selection(&self) -> bool {
        self.scene_viewer
            .as_ref()
            .map(|sv| !sv.selection.entities.is_empty())
            .unwrap_or(false)
    }

    /// Edit 모드인지 확인
    pub fn is_edit_mode(&self) -> bool {
        self.editor_mode.is_edit()
    }
}
