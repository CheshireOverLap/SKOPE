//! Selection 시스템
//!
//! 에디터에서 오브젝트 선택 관리

use bevy_ecs::prelude::*;
use glam::{Mat4, Vec3};

use crate::editor::scene_viewer::Ray;
use crate::ecs_components::{GlobalTransform, MeshInstance, Transform};

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

/// AABB (Axis-Aligned Bounding Box)
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    /// 새 AABB 생성
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// 중심점
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// 크기
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// 단위 큐브 (기본 메시용)
    pub fn unit_cube() -> Self {
        Self {
            min: Vec3::splat(-0.5),
            max: Vec3::splat(0.5),
        }
    }

    /// 변환 적용 (World AABB 계산)
    pub fn transformed(&self, transform: Mat4) -> Self {
        // 8개 꼭짓점 변환 후 새 AABB 계산
        let corners = [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ];

        let mut new_min = Vec3::splat(f32::MAX);
        let mut new_max = Vec3::splat(f32::MIN);

        for corner in &corners {
            let transformed = transform.transform_point3(*corner);
            new_min = new_min.min(transformed);
            new_max = new_max.max(transformed);
        }

        Self {
            min: new_min,
            max: new_max,
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
pub fn raycast_scene(world: &mut World, ray: &Ray) -> Option<(Entity, f32)> {
    let mut closest: Option<(Entity, f32)> = None;

    // MeshInstance + GlobalTransform 가진 엔티티 검색
    let mut query = world.query::<(Entity, &MeshInstance, &GlobalTransform)>();

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
    add_to_selection: bool,
) -> bool {
    if let Some((entity, _distance)) = raycast_scene(world, ray) {
        if add_to_selection {
            selection.toggle(entity);
        } else {
            selection.select(entity);
        }
        log::info!("[Selection] Selected entity: {:?}", entity);
        true
    } else {
        if !add_to_selection {
            selection.clear();
            log::info!("[Selection] Cleared selection");
        }
        false
    }
}
