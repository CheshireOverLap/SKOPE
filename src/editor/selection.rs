//! Selection 시스템
//!
//! 에디터에서 오브젝트 선택 관리

use skope_ecs::prelude::*;
use glam::Vec3;

use crate::editor::scene_viewer::Ray;
use crate::ecs_components::{GlobalTransform, Hidden, MeshInstance, Transform};

// Re-export from skope_editor crate
pub use skope_editor::{SelectionModifier, AABB};

/// 선택된 엔티티들
#[derive(Default)]
pub struct Selection {
    /// 선택된 엔티티 목록
    pub entities: Vec<Entity>,
}

impl Selection {
    /// 새 Selection 생성
    pub fn new() -> Self {
        Self::default()
    }

    /// 선택 비어있는지
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// 첫 번째 선택된 엔티티
    pub fn first(&self) -> Option<Entity> {
        self.entities.first().copied()
    }

    /// 선택 개수
    pub fn count(&self) -> usize {
        self.entities.len()
    }

    /// 엔티티가 선택되어 있는지
    pub fn contains(&self, entity: Entity) -> bool {
        self.entities.contains(&entity)
    }

    /// 선택 설정 (기존 선택 대체)
    pub fn set(&mut self, entities: Vec<Entity>) {
        self.entities = entities;
    }

    /// 단일 엔티티 선택
    pub fn select(&mut self, entity: Entity) {
        self.entities = vec![entity];
    }

    /// 선택에 추가 (Shift+클릭)
    pub fn add(&mut self, entity: Entity) {
        if !self.entities.contains(&entity) {
            self.entities.push(entity);
        }
    }

    /// 선택에서 제거
    pub fn remove(&mut self, entity: Entity) {
        self.entities.retain(|&e| e != entity);
    }

    /// 토글 (Ctrl+클릭)
    pub fn toggle(&mut self, entity: Entity) {
        if self.contains(entity) {
            self.remove(entity);
        } else {
            self.add(entity);
        }
    }

    /// 선택 해제
    pub fn clear(&mut self) {
        self.entities.clear();
    }

    /// 선택된 엔티티들의 중심점 계산
    pub fn center(&self, world: &World) -> Option<Vec3> {
        if self.entities.is_empty() {
            return None;
        }

        let mut sum = Vec3::ZERO;
        let mut count = 0;

        for &entity in &self.entities {
            if let Some(transform) = world.get::<Transform>(entity) {
                sum += transform.translation;
                count += 1;
            } else if let Some(global) = world.get::<GlobalTransform>(entity) {
                // GlobalTransform에서 위치 추출
                sum += global.0.w_axis.truncate();
                count += 1;
            }
        }

        if count > 0 {
            Some(sum / count as f32)
        } else {
            None
        }
    }
}


/// Ray-AABB 교차 검사
pub fn ray_aabb_intersection(ray: &Ray, aabb: &AABB) -> Option<f32> {
    let inv_dir = Vec3::new(
        if ray.direction.x.abs() > 0.0001 { 1.0 / ray.direction.x } else { f32::MAX * ray.direction.x.signum() },
        if ray.direction.y.abs() > 0.0001 { 1.0 / ray.direction.y } else { f32::MAX * ray.direction.y.signum() },
        if ray.direction.z.abs() > 0.0001 { 1.0 / ray.direction.z } else { f32::MAX * ray.direction.z.signum() },
    );

    let t1 = (aabb.min - ray.origin) * inv_dir;
    let t2 = (aabb.max - ray.origin) * inv_dir;

    let tmin = t1.min(t2);
    let tmax = t1.max(t2);

    let tmin_max = tmin.x.max(tmin.y).max(tmin.z);
    let tmax_min = tmax.x.min(tmax.y).min(tmax.z);

    if tmin_max <= tmax_min && tmax_min >= 0.0 {
        Some(if tmin_max > 0.0 { tmin_max } else { tmax_min })
    } else {
        None
    }
}

/// 씬에서 Ray와 교차하는 엔티티 찾기
/// Hidden 컴포넌트가 있는 엔티티는 제외
pub fn raycast_scene(world: &mut World, ray: &Ray) -> Option<(Entity, f32)> {
    let mut closest: Option<(Entity, f32)> = None;

    // MeshInstance + GlobalTransform 가진 엔티티 검색 (Hidden 제외)
    let query = world.query_filtered::<(Entity, &MeshInstance, &GlobalTransform), Without<Hidden>>();

    for (entity, _mesh, global_transform) in query.iter(world) {
        // 기본 AABB (단위 큐브) - 실제로는 메시별 AABB 사용해야 함
        let local_aabb = AABB::unit_cube();
        let world_aabb = local_aabb.transformed(global_transform.0);

        if let Some(t) = ray_aabb_intersection(ray, &world_aabb) {
            match closest {
                None => closest = Some((entity, t)),
                Some((_, closest_t)) if t < closest_t => closest = Some((entity, t)),
                _ => {}
            }
        }
    }

    closest
}

/// 클릭으로 엔티티 선택
pub fn pick_entity(
    world: &mut World,
    ray: &Ray,
    selection: &mut Selection,
    modifier: SelectionModifier,
) -> bool {
    if let Some((entity, _distance)) = raycast_scene(world, ray) {
        match modifier {
            SelectionModifier::Replace => selection.select(entity),
            SelectionModifier::Additive => selection.add(entity),
            SelectionModifier::Toggle => selection.toggle(entity),
        }
        log::debug!("[Selection] Picked entity: {:?} (mode: {:?})", entity, modifier);
        true
    } else {
        // 빈 공간 클릭
        match modifier {
            SelectionModifier::Replace => {
                selection.clear();
                log::debug!("[Selection] Cleared selection");
            }
            SelectionModifier::Additive | SelectionModifier::Toggle => {
                // 변경 없음
            }
        }
        false
    }
}
