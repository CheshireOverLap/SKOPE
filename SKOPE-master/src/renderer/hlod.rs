// SKOPE Engine - Hierarchical LOD (HLOD) System
//
// 원거리 오브젝트 그룹을 하나의 병합된 메시로 렌더링하는 시스템
// 설계 문서 Section 7.1 HLOD 구현
//
// Reference: SKOPE Engine Rendering Pipeline v1.1 Design Doc

use glam::{Vec3, Mat4};
use std::collections::HashMap;

use super::lod::{BoundingSphere, LodSelector, LodConfig, LodSelection};

/// HLOD 설정
#[derive(Debug, Clone)]
pub struct HlodConfig {
    /// HLOD 활성화 거리 (이 거리보다 멀면 HLOD 사용)
    pub activation_distance: f32,

    /// 최대 HLOD 레벨
    pub max_levels: u32,

    /// 레벨별 거리 배율 (각 레벨은 이전 레벨의 이 배율만큼 멀리서 활성화)
    pub distance_multiplier: f32,

    /// 클러스터당 최대 오브젝트 수
    pub max_objects_per_cluster: u32,

    /// 최소 클러스터 크기 (미터)
    pub min_cluster_size: f32,

    /// Dithered transition 사용
    pub use_dithered_transition: bool,

    /// LOD bias (품질 조정)
    pub lod_bias: f32,
}

impl Default for HlodConfig {
    fn default() -> Self {
        Self {
            activation_distance: 100.0,
            max_levels: 3,
            distance_multiplier: 3.0,
            max_objects_per_cluster: 64,
            min_cluster_size: 10.0,
            use_dithered_transition: true,
            lod_bias: 1.0,
        }
    }
}

/// HLOD 노드 (트리 구조)
#[derive(Debug, Clone)]
pub struct HlodNode {
    /// 노드 ID
    pub id: u32,

    /// 바운딩 볼륨
    pub bounds: BoundingSphere,

    /// 부모 노드 (None = 루트)
    pub parent: Option<u32>,

    /// 자식 노드들
    pub children: Vec<u32>,

    /// 이 노드가 나타내는 원본 오브젝트들 (리프 노드만 해당)
    pub source_objects: Vec<u32>,

    /// 병합된 메시 인덱스 (HLOD 레벨용)
    pub merged_mesh_idx: Option<usize>,

    /// HLOD 레벨 (0 = 원본, 1+ = 병합된 HLOD)
    pub level: u32,

    /// 삼각형 수 (통계용)
    pub triangle_count: u32,

    /// 병합된 텍스처 아틀라스 인덱스
    pub atlas_idx: Option<u32>,
}

impl HlodNode {
    pub fn new_leaf(id: u32, bounds: BoundingSphere, source_objects: Vec<u32>, triangle_count: u32) -> Self {
        Self {
            id,
            bounds,
            parent: None,
            children: Vec::new(),
            source_objects,
            merged_mesh_idx: None,
            level: 0,
            triangle_count,
            atlas_idx: None,
        }
    }

    pub fn new_cluster(id: u32, bounds: BoundingSphere, children: Vec<u32>, level: u32) -> Self {
        Self {
            id,
            bounds,
            parent: None,
            children,
            source_objects: Vec::new(),
            merged_mesh_idx: None,
            level,
            triangle_count: 0,
            atlas_idx: None,
        }
    }

    /// 리프 노드인지 확인
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// HLOD 클러스터 (공간 분할 단위)
#[derive(Debug, Clone)]
pub struct HlodCluster {
    /// 클러스터 ID
    pub id: u32,

    /// 바운딩 볼륨
    pub bounds: BoundingSphere,

    /// 포함된 노드들
    pub nodes: Vec<u32>,

    /// HLOD 레벨별 메시 인덱스
    /// level 0 = 개별 렌더링 (노드별)
    /// level 1+ = 병합된 HLOD 메시
    pub level_meshes: Vec<Option<usize>>,

    /// 현재 활성 레벨
    pub active_level: u32,

    /// 전환 중인지 여부
    pub is_transitioning: bool,

    /// 전환 진행도 (0.0 ~ 1.0)
    pub transition_progress: f32,
}

impl HlodCluster {
    pub fn new(id: u32, bounds: BoundingSphere, nodes: Vec<u32>, max_levels: u32) -> Self {
        let level_meshes = vec![None; max_levels as usize + 1];
        Self {
            id,
            bounds,
            nodes,
            level_meshes,
            active_level: 0,
            is_transitioning: false,
            transition_progress: 0.0,
        }
    }
}

/// HLOD 선택 결과
#[derive(Debug, Clone, Copy)]
pub struct HlodSelection {
    /// 선택된 HLOD 레벨
    pub level: u32,

    /// 사용할 메시 인덱스
    pub mesh_idx: Option<usize>,

    /// 전환 중인 경우 다음 레벨
    pub next_level: Option<u32>,

    /// 전환 진행도
    pub transition_factor: f32,

    /// 개별 오브젝트 렌더링 여부 (level 0)
    pub render_individual: bool,
}

/// HLOD 통계
#[derive(Debug, Default, Clone)]
pub struct HlodStats {
    /// 총 클러스터 수
    pub total_clusters: u32,

    /// 활성 클러스터 수 (화면에 보이는)
    pub active_clusters: u32,

    /// HLOD 레벨별 클러스터 수
    pub clusters_per_level: [u32; 4],

    /// 절약된 드로우 콜 수
    pub draw_calls_saved: u32,

    /// 절약된 삼각형 수
    pub triangles_saved: u64,

    /// 전환 중인 클러스터 수
    pub transitioning_clusters: u32,
}

impl HlodStats {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// HLOD 시스템
pub struct HlodSystem {
    /// 설정
    pub config: HlodConfig,

    /// 모든 노드
    nodes: HashMap<u32, HlodNode>,

    /// 모든 클러스터
    clusters: HashMap<u32, HlodCluster>,

    /// 다음 노드 ID
    next_node_id: u32,

    /// 다음 클러스터 ID
    next_cluster_id: u32,

    /// LOD 선택기 (기존 시스템 재사용)
    lod_selector: LodSelector,

    /// 통계
    pub stats: HlodStats,

    /// 빌드 완료 여부
    is_built: bool,
}

impl HlodSystem {
    pub fn new(config: HlodConfig) -> Self {
        let lod_config = LodConfig {
            thresholds: [0.3, 0.1, 0.03, 0.01],
            transition_zone: 0.1,
            bias: config.lod_bias,
        };

        Self {
            config,
            nodes: HashMap::new(),
            clusters: HashMap::new(),
            next_node_id: 0,
            next_cluster_id: 0,
            lod_selector: LodSelector::new(lod_config),
            stats: HlodStats::default(),
            is_built: false,
        }
    }

    /// 새 노드 추가 (원본 오브젝트)
    pub fn add_object(&mut self, bounds: BoundingSphere, triangle_count: u32) -> u32 {
        let id = self.next_node_id;
        self.next_node_id += 1;

        let node = HlodNode::new_leaf(id, bounds, vec![id], triangle_count);
        self.nodes.insert(id, node);

        id
    }

    /// 여러 오브젝트 일괄 추가
    pub fn add_objects(&mut self, objects: &[(BoundingSphere, u32)]) -> Vec<u32> {
        objects.iter().map(|(bounds, tri_count)| {
            self.add_object(*bounds, *tri_count)
        }).collect()
    }

    /// HLOD 트리 빌드
    ///
    /// 공간 분할 후 계층적 클러스터 생성
    pub fn build(&mut self) {
        if self.nodes.is_empty() {
            return;
        }

        // 1. 전체 바운딩 볼륨 계산
        let world_bounds = self.calculate_world_bounds();

        // 2. 옥트리 기반 공간 분할
        let leaf_nodes: Vec<u32> = self.nodes.keys().copied().collect();
        self.build_octree(world_bounds, &leaf_nodes, 0);

        // 3. 클러스터 생성
        self.generate_clusters();

        self.is_built = true;
        self.stats.total_clusters = self.clusters.len() as u32;

        log::info!(
            "[HLOD] Built {} clusters from {} objects",
            self.clusters.len(),
            self.nodes.len()
        );
    }

    /// 전체 바운딩 볼륨 계산
    fn calculate_world_bounds(&self) -> BoundingSphere {
        if self.nodes.is_empty() {
            return BoundingSphere::new(Vec3::ZERO, 0.0);
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for node in self.nodes.values() {
            let node_min = node.bounds.center - Vec3::splat(node.bounds.radius);
            let node_max = node.bounds.center + Vec3::splat(node.bounds.radius);
            min = min.min(node_min);
            max = max.max(node_max);
        }

        BoundingSphere::from_aabb(min, max)
    }

    /// 옥트리 기반 공간 분할
    fn build_octree(&mut self, bounds: BoundingSphere, node_ids: &[u32], depth: u32) {
        // 최대 깊이 또는 최소 노드 수에 도달하면 중단
        if depth >= self.config.max_levels || node_ids.len() <= self.config.max_objects_per_cluster as usize {
            return;
        }

        // 8개 자식 영역으로 분할
        let half_radius = bounds.radius * 0.5;
        let offsets = [
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new( 1.0, -1.0, -1.0),
            Vec3::new(-1.0,  1.0, -1.0),
            Vec3::new( 1.0,  1.0, -1.0),
            Vec3::new(-1.0, -1.0,  1.0),
            Vec3::new( 1.0, -1.0,  1.0),
            Vec3::new(-1.0,  1.0,  1.0),
            Vec3::new( 1.0,  1.0,  1.0),
        ];

        for offset in offsets {
            let child_center = bounds.center + offset * half_radius;
            let child_bounds = BoundingSphere::new(child_center, half_radius);

            // 이 영역에 속하는 노드들 찾기
            let child_nodes: Vec<u32> = node_ids.iter()
                .filter(|&&id| {
                    if let Some(node) = self.nodes.get(&id) {
                        Self::bounds_intersect(&child_bounds, &node.bounds)
                    } else {
                        false
                    }
                })
                .copied()
                .collect();

            if !child_nodes.is_empty() {
                self.build_octree(child_bounds, &child_nodes, depth + 1);
            }
        }
    }

    /// 바운딩 볼륨 교차 테스트
    fn bounds_intersect(a: &BoundingSphere, b: &BoundingSphere) -> bool {
        let dist = (a.center - b.center).length();
        dist < (a.radius + b.radius)
    }

    /// 클러스터 생성
    fn generate_clusters(&mut self) {
        // 간단한 구현: 모든 리프 노드를 거리 기반으로 클러스터링
        let leaf_ids: Vec<u32> = self.nodes.values()
            .filter(|n| n.is_leaf())
            .map(|n| n.id)
            .collect();

        if leaf_ids.is_empty() {
            return;
        }

        // Greedy 클러스터링
        let mut assigned: std::collections::HashSet<u32> = std::collections::HashSet::new();
        let max_per_cluster = self.config.max_objects_per_cluster as usize;

        for &seed_id in &leaf_ids {
            if assigned.contains(&seed_id) {
                continue;
            }

            let seed_node = match self.nodes.get(&seed_id) {
                Some(n) => n,
                None => continue,
            };

            // 가까운 노드들 모으기
            let mut cluster_nodes = vec![seed_id];
            assigned.insert(seed_id);

            for &candidate_id in &leaf_ids {
                if assigned.contains(&candidate_id) {
                    continue;
                }

                if cluster_nodes.len() >= max_per_cluster {
                    break;
                }

                if let Some(candidate) = self.nodes.get(&candidate_id) {
                    let dist = (seed_node.bounds.center - candidate.bounds.center).length();
                    if dist < self.config.min_cluster_size * 2.0 {
                        cluster_nodes.push(candidate_id);
                        assigned.insert(candidate_id);
                    }
                }
            }

            // 클러스터 바운딩 계산
            let cluster_bounds = self.calculate_cluster_bounds(&cluster_nodes);

            // 클러스터 생성
            let cluster_id = self.next_cluster_id;
            self.next_cluster_id += 1;

            let cluster = HlodCluster::new(
                cluster_id,
                cluster_bounds,
                cluster_nodes,
                self.config.max_levels,
            );
            self.clusters.insert(cluster_id, cluster);
        }
    }

    /// 클러스터 바운딩 계산
    fn calculate_cluster_bounds(&self, node_ids: &[u32]) -> BoundingSphere {
        if node_ids.is_empty() {
            return BoundingSphere::new(Vec3::ZERO, 0.0);
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for &id in node_ids {
            if let Some(node) = self.nodes.get(&id) {
                let node_min = node.bounds.center - Vec3::splat(node.bounds.radius);
                let node_max = node.bounds.center + Vec3::splat(node.bounds.radius);
                min = min.min(node_min);
                max = max.max(node_max);
            }
        }

        BoundingSphere::from_aabb(min, max)
    }

    /// 클러스터에 병합 메시 설정
    pub fn set_cluster_mesh(&mut self, cluster_id: u32, level: u32, mesh_idx: usize) {
        if let Some(cluster) = self.clusters.get_mut(&cluster_id) {
            if (level as usize) < cluster.level_meshes.len() {
                cluster.level_meshes[level as usize] = Some(mesh_idx);
            }
        }
    }

    /// 프레임 업데이트 - HLOD 선택 및 전환
    pub fn update(
        &mut self,
        camera_pos: Vec3,
        _proj: Mat4,
        _screen_height: f32,
        delta_time: f32,
    ) {
        self.stats.reset();
        self.stats.total_clusters = self.clusters.len() as u32;

        // Config 값 미리 복사 (borrow checker 우회)
        let activation_distance = self.config.activation_distance;
        let distance_multiplier = self.config.distance_multiplier;
        let max_levels = self.config.max_levels;

        for cluster in self.clusters.values_mut() {
            // 거리 계산
            let distance = (cluster.bounds.center - camera_pos).length();

            // 레벨 결정 (인라인)
            let target_level = Self::calculate_level_inline(
                distance, activation_distance, distance_multiplier, max_levels
            );

            // 전환 처리
            if cluster.active_level != target_level {
                cluster.is_transitioning = true;
                cluster.transition_progress += delta_time * 2.0; // 0.5초 전환

                if cluster.transition_progress >= 1.0 {
                    cluster.active_level = target_level;
                    cluster.is_transitioning = false;
                    cluster.transition_progress = 0.0;
                }

                self.stats.transitioning_clusters += 1;
            }

            // 통계 업데이트
            let level_idx = (cluster.active_level as usize).min(3);
            self.stats.clusters_per_level[level_idx] += 1;

            if cluster.active_level > 0 {
                self.stats.active_clusters += 1;
                // 절약된 드로우 콜: (개별 오브젝트 수 - 1)
                if cluster.nodes.len() > 1 {
                    self.stats.draw_calls_saved += (cluster.nodes.len() - 1) as u32;
                }
            }
        }
    }

    /// HLOD 레벨 계산
    fn calculate_hlod_level(&self, distance: f32) -> u32 {
        Self::calculate_level_inline(
            distance,
            self.config.activation_distance,
            self.config.distance_multiplier,
            self.config.max_levels,
        )
    }

    /// HLOD 레벨 계산 (인라인, borrow checker 우회용)
    fn calculate_level_inline(
        distance: f32,
        activation_distance: f32,
        distance_multiplier: f32,
        max_levels: u32,
    ) -> u32 {
        for level in 0..max_levels {
            let threshold = activation_distance * distance_multiplier.powi(level as i32);
            if distance < threshold {
                return level;
            }
        }

        max_levels
    }

    /// 클러스터의 현재 렌더 정보 조회
    pub fn get_cluster_render_info(&self, cluster_id: u32) -> Option<HlodSelection> {
        let cluster = self.clusters.get(&cluster_id)?;

        let level = cluster.active_level;
        let mesh_idx = cluster.level_meshes.get(level as usize).copied().flatten();

        let (next_level, transition_factor) = if cluster.is_transitioning {
            let next = if level < self.config.max_levels {
                level + 1
            } else {
                level.saturating_sub(1)
            };
            (Some(next), cluster.transition_progress)
        } else {
            (None, 0.0)
        };

        Some(HlodSelection {
            level,
            mesh_idx,
            next_level,
            transition_factor,
            render_individual: level == 0,
        })
    }

    /// 모든 활성 클러스터 렌더 정보 수집
    pub fn collect_render_info(&self, camera_pos: Vec3, _proj: Mat4) -> Vec<(u32, HlodSelection)> {
        let mut results = Vec::new();

        for (&id, cluster) in &self.clusters {
            // Frustum culling은 상위 레이어에서 처리 가정
            // 여기서는 거리 기반 필터링만
            let distance = (cluster.bounds.center - camera_pos).length();

            // 너무 먼 클러스터는 완전히 스킵
            if distance > self.config.activation_distance * self.config.distance_multiplier.powi(4) {
                continue;
            }

            if let Some(selection) = self.get_cluster_render_info(id) {
                results.push((id, selection));
            }
        }

        results
    }

    /// 클러스터 내 원본 오브젝트 ID 조회
    pub fn get_cluster_objects(&self, cluster_id: u32) -> Option<&[u32]> {
        self.clusters.get(&cluster_id).map(|c| c.nodes.as_slice())
    }

    /// 노드 조회
    pub fn get_node(&self, node_id: u32) -> Option<&HlodNode> {
        self.nodes.get(&node_id)
    }

    /// 클러스터 조회
    pub fn get_cluster(&self, cluster_id: u32) -> Option<&HlodCluster> {
        self.clusters.get(&cluster_id)
    }

    /// 모든 클러스터 ID
    pub fn cluster_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.clusters.keys().copied()
    }

    /// 빌드 상태
    pub fn is_built(&self) -> bool {
        self.is_built
    }

    /// 통계 조회
    pub fn stats(&self) -> &HlodStats {
        &self.stats
    }

    /// 설정 변경
    pub fn set_config(&mut self, config: HlodConfig) {
        let lod_bias = config.lod_bias;
        self.config = config;
        self.lod_selector = LodSelector::new(LodConfig {
            thresholds: [0.3, 0.1, 0.03, 0.01],
            transition_zone: 0.1,
            bias: lod_bias,
        });
    }

    /// 초기화
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.clusters.clear();
        self.next_node_id = 0;
        self.next_cluster_id = 0;
        self.is_built = false;
        self.stats.reset();
    }
}

/// Impostor 생성을 위한 유틸리티
pub mod impostor {
    use glam::Vec3;

    /// Impostor 캡처 방향 (구면 샘플링)
    pub fn octahedral_directions(subdivision: u32) -> Vec<Vec3> {
        let mut directions = Vec::new();
        let steps = subdivision as i32;

        for y in -steps..=steps {
            for x in -steps..=steps {
                let fx = x as f32 / steps as f32;
                let fy = y as f32 / steps as f32;

                // Octahedral mapping
                let mut dir = Vec3::new(fx, 0.0, fy);
                dir.y = 1.0 - dir.x.abs() - dir.z.abs();

                if dir.y < 0.0 {
                    let old_x = dir.x;
                    dir.x = (1.0 - dir.z.abs()) * old_x.signum();
                    dir.z = (1.0 - old_x.abs()) * dir.z.signum();
                }

                if dir.length_squared() > 0.0001 {
                    directions.push(dir.normalize());
                }
            }
        }

        directions
    }

    /// Billboard 정점 생성
    pub fn billboard_vertices(center: Vec3, size: f32, camera_right: Vec3, camera_up: Vec3) -> [Vec3; 4] {
        let half = size * 0.5;
        [
            center - camera_right * half - camera_up * half,
            center + camera_right * half - camera_up * half,
            center + camera_right * half + camera_up * half,
            center - camera_right * half + camera_up * half,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hlod_build() {
        let mut hlod = HlodSystem::new(HlodConfig::default());

        // 오브젝트 추가
        for i in 0..10 {
            let pos = Vec3::new(i as f32 * 5.0, 0.0, 0.0);
            hlod.add_object(BoundingSphere::new(pos, 1.0), 100);
        }

        hlod.build();

        assert!(hlod.is_built());
        assert!(hlod.clusters.len() > 0);
    }

    #[test]
    fn test_hlod_level_selection() {
        let hlod = HlodSystem::new(HlodConfig {
            activation_distance: 100.0,
            distance_multiplier: 2.0,
            ..Default::default()
        });

        // 가까운 거리 = level 0
        assert_eq!(hlod.calculate_hlod_level(50.0), 0);

        // 중간 거리 = level 1
        assert_eq!(hlod.calculate_hlod_level(150.0), 1);

        // 먼 거리 = level 2
        assert_eq!(hlod.calculate_hlod_level(350.0), 2);
    }
}
