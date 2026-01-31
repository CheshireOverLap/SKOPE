//! Editor Clipboard
//!
//! Copy/Paste 시스템을 위한 클립보드 버퍼

use bevy_ecs::prelude::*;
use glam::Vec3;

use crate::ecs_components::{
    GlobalTransform, Light, MaterialHandle, MeshInstance, NodeName, Transform,
};

/// 클립보드에 저장할 엔티티 데이터
#[derive(Debug, Clone)]
pub struct ClipboardEntity {
    pub name: String,
    pub transform: Transform,
    pub mesh_instance: Option<MeshInstance>,
    pub material_handle: Option<MaterialHandle>,
    pub light: Option<Light>,
}

/// 클립보드 버퍼
#[derive(Default)]
pub struct Clipboard {
    /// 복사된 엔티티들
    pub entities: Vec<ClipboardEntity>,
    /// 복사 시 중심점 (붙여넣기 오프셋 계산용)
    pub pivot: Option<Vec3>,
}

impl Clipboard {
    /// 새 클립보드 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 클립보드가 비어있는지 확인
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// 클립보드 비우기
    pub fn clear(&mut self) {
        self.entities.clear();
        self.pivot = None;
    }

    /// 선택된 엔티티들을 클립보드에 복사
    pub fn copy_from(&mut self, world: &World, entities: &[Entity]) {
        self.clear();

        let mut position_sum = Vec3::ZERO;
        let mut count = 0;

        for &entity in entities {
            if let Some(transform) = world.get::<Transform>(entity) {
                let name = world
                    .get::<NodeName>(entity)
                    .map(|n| n.0.clone())
                    .unwrap_or_else(|| format!("Entity_{:?}", entity));

                self.entities.push(ClipboardEntity {
                    name,
                    transform: transform.clone(),
                    mesh_instance: world.get::<MeshInstance>(entity).cloned(),
                    material_handle: world.get::<MaterialHandle>(entity).cloned(),
                    light: world.get::<Light>(entity).cloned(),
                });

                position_sum += transform.translation;
                count += 1;
            }
        }

        // 중심점 계산
        self.pivot = if count > 0 {
            Some(position_sum / count as f32)
        } else {
            None
        };

        log::info!(
            "[Clipboard] Copied {} entities (pivot: {:?})",
            self.entities.len(),
            self.pivot
        );
    }

    /// 클립보드에서 월드로 붙여넣기
    /// 반환: 생성된 엔티티들
    pub fn paste_to(&self, world: &mut World, target_pos: Vec3) -> Vec<Entity> {
        if self.is_empty() {
            log::warn!("[Clipboard] Paste failed: clipboard is empty");
            return Vec::new();
        }

        let mut pasted = Vec::new();

        // 오프셋 계산: 타겟 위치 - 복사 시 중심점
        let offset = self
            .pivot
            .map(|pivot| target_pos - pivot)
            .unwrap_or(target_pos);

        for (i, clip) in self.entities.iter().enumerate() {
            // 새 위치 계산
            let mut new_transform = clip.transform.clone();
            new_transform.translation += offset;

            // 이름에 _paste 접미사 추가
            let new_name = format!("{}_paste{}", clip.name, i);
            let new_pos = new_transform.translation;

            // 기본 컴포넌트로 엔티티 생성
            let mut cmd = world.spawn((
                new_transform,
                GlobalTransform::default(),
                NodeName(new_name.clone()),
            ));

            // 옵션 컴포넌트 추가
            if let Some(mi) = &clip.mesh_instance {
                cmd.insert(mi.clone());
            }
            if let Some(mh) = &clip.material_handle {
                cmd.insert(mh.clone());
            }
            if let Some(light) = &clip.light {
                cmd.insert(light.clone());
            }

            let new_entity = cmd.id();
            pasted.push(new_entity);

            log::debug!("[Clipboard] Pasted entity: {} at {:?}", new_name, new_pos);
        }

        log::info!(
            "[Clipboard] Pasted {} entities at offset {:?}",
            pasted.len(),
            offset
        );

        pasted
    }

    /// 클립보드 내용 개수
    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.entities.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_empty() {
        let clipboard = Clipboard::new();
        assert!(clipboard.is_empty());
        assert_eq!(clipboard.count(), 0);
    }

    #[test]
    fn test_clipboard_clear() {
        let mut clipboard = Clipboard::new();
        clipboard.entities.push(ClipboardEntity {
            name: "Test".to_string(),
            transform: Transform::default(),
            mesh_instance: None,
            material_handle: None,
            light: None,
        });
        clipboard.pivot = Some(Vec3::ZERO);

        assert!(!clipboard.is_empty());
        clipboard.clear();
        assert!(clipboard.is_empty());
        assert!(clipboard.pivot.is_none());
    }
}
