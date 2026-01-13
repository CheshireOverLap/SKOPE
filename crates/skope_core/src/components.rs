//! ECS Components for SKOPE Engine
//!
//! Core components shared across all engine modules.

use bevy_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};

// ============ Transform Components ============

/// Local transform component (position, rotation, scale)
#[derive(Component, Debug, Clone)]
pub struct Transform {
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl Transform {
    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            translation,
            ..Default::default()
        }
    }

    pub fn from_rotation(rotation: Quat) -> Self {
        Self {
            rotation,
            ..Default::default()
        }
    }

    pub fn from_scale(scale: Vec3) -> Self {
        Self {
            scale,
            ..Default::default()
        }
    }

    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

/// Global transform component (world space matrix)
#[derive(Component, Debug, Clone)]
pub struct GlobalTransform(pub Mat4);

impl Default for GlobalTransform {
    fn default() -> Self {
        Self(Mat4::IDENTITY)
    }
}

impl GlobalTransform {
    /// Get the translation (position) from the global transform
    pub fn translation(&self) -> Vec3 {
        self.0.w_axis.truncate()
    }
}

// ============ Rendering Components ============

/// Mesh instance component - references MeshAssets
#[derive(Component, Debug, Clone)]
pub struct MeshInstance {
    pub mesh_index: usize,
}

/// Material handle component - references MaterialAssets
#[derive(Component, Debug, Clone)]
pub struct MaterialHandle {
    pub material_index: usize,
}

// ============ Camera Components ============

/// Camera component
#[derive(Component, Debug, Clone)]
pub struct Camera {
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub is_active: bool,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            fov: 45.0_f32.to_radians(),
            near: 0.1,
            far: 100.0,
            is_active: true,
        }
    }
}

/// FPS-style camera controller
#[derive(Component, Debug, Clone)]
pub struct CameraController {
    pub yaw: f32,
    pub pitch: f32,
    pub move_speed: f32,
    pub sensitivity: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: -0.3,
            move_speed: 5.0,
            sensitivity: 0.003,
        }
    }
}

// ============ Skeletal Mesh Components ============

/// 스킨드 메시 인스턴스 - SkinnedMeshAssets 참조
#[derive(Component, Debug, Clone)]
pub struct SkinnedMeshInstance {
    pub skinned_mesh_index: usize,
    pub skeleton_entity: Entity,
}

/// 스켈레톤 컴포넌트 - 본 트리의 루트
#[derive(Component, Debug, Clone)]
pub struct Skeleton {
    /// 모델 이름 (SkinnedModelRegistry의 모델 이름)
    pub model_name: String,
    pub skin_index: usize,
    pub joint_entities: Vec<Entity>,
}

/// 본(조인트) 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Joint {
    pub joint_index: usize,
    pub inverse_bind_matrix: Mat4,
}

/// 본 매트릭스 버퍼 (GPU 업로드용)
#[derive(Component, Debug)]
pub struct JointMatrices {
    pub matrices: Vec<Mat4>,
}

impl Default for JointMatrices {
    fn default() -> Self {
        Self {
            matrices: Vec::new(),
        }
    }
}

// ============ Physics Components ============

/// Velocity component for physics movement
#[derive(Component, Debug, Clone, Default)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

impl Velocity {
    pub fn new(linear: Vec3, angular: Vec3) -> Self {
        Self { linear, angular }
    }

    pub fn from_linear(linear: Vec3) -> Self {
        Self { linear, angular: Vec3::ZERO }
    }
}

/// Simple AABB Collider component
#[derive(Component, Debug, Clone)]
pub struct BoxCollider {
    pub half_extents: Vec3,
    pub offset: Vec3,
}

impl Default for BoxCollider {
    fn default() -> Self {
        Self {
            half_extents: Vec3::ONE * 0.5,
            offset: Vec3::ZERO,
        }
    }
}

impl BoxCollider {
    pub fn new(half_extents: Vec3) -> Self {
        Self { half_extents, offset: Vec3::ZERO }
    }

    pub fn with_offset(half_extents: Vec3, offset: Vec3) -> Self {
        Self { half_extents, offset }
    }
}

/// Sphere Collider component
#[derive(Component, Debug, Clone)]
pub struct SphereCollider {
    pub radius: f32,
    pub offset: Vec3,
}

impl Default for SphereCollider {
    fn default() -> Self {
        Self { radius: 0.5, offset: Vec3::ZERO }
    }
}

impl SphereCollider {
    pub fn new(radius: f32) -> Self {
        Self { radius, offset: Vec3::ZERO }
    }
}

/// Rigid body type
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigidBodyType {
    Static,
    Dynamic,
    Kinematic,
}

impl Default for RigidBodyType {
    fn default() -> Self {
        Self::Static
    }
}

// ============ Gameplay Components ============

/// Player marker component
#[derive(Component, Debug, Clone, Default)]
pub struct Player {
    pub player_id: u32,
}

impl Player {
    pub fn new(player_id: u32) -> Self {
        Self { player_id }
    }
}

/// Health component for damageable entities
#[derive(Component, Debug, Clone)]
pub struct Health {
    pub current: f32,
    pub maximum: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self { current: 100.0, maximum: 100.0 }
    }
}

impl Health {
    pub fn new(max: f32) -> Self {
        Self { current: max, maximum: max }
    }

    pub fn take_damage(&mut self, amount: f32) {
        self.current = (self.current - amount).max(0.0);
    }

    pub fn heal(&mut self, amount: f32) {
        self.current = (self.current + amount).min(self.maximum);
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn percentage(&self) -> f32 {
        if self.maximum > 0.0 { self.current / self.maximum } else { 0.0 }
    }
}

/// Enemy spawner component
#[derive(Component, Debug, Clone)]
pub struct EnemySpawner {
    pub enemy_prefab: String,
    pub spawn_interval: f32,
    pub spawn_radius: f32,
    pub max_enemies: u32,
    pub current_count: u32,
    pub time_since_spawn: f32,
    pub respawn_enabled: bool,
}

impl Default for EnemySpawner {
    fn default() -> Self {
        Self {
            enemy_prefab: "default_enemy".to_string(),
            spawn_interval: 5.0,
            spawn_radius: 10.0,
            max_enemies: 5,
            current_count: 0,
            time_since_spawn: 0.0,
            respawn_enabled: true,
        }
    }
}

/// Weapon component
#[derive(Component, Debug, Clone)]
pub struct Weapon {
    pub damage: f32,
    pub fire_rate: f32,
    pub range: f32,
    pub ammo: u32,
    pub max_ammo: u32,
    pub time_since_fire: f32,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            damage: 10.0,
            fire_rate: 0.5,
            range: 50.0,
            ammo: 30,
            max_ammo: 30,
            time_since_fire: 0.0,
        }
    }
}

impl Weapon {
    pub fn can_fire(&self) -> bool {
        self.ammo > 0 && self.time_since_fire >= self.fire_rate
    }

    pub fn fire(&mut self) -> bool {
        if self.can_fire() {
            self.ammo -= 1;
            self.time_since_fire = 0.0;
            true
        } else {
            false
        }
    }

    pub fn reload(&mut self) {
        self.ammo = self.max_ammo;
    }
}

/// Team component for faction/side identification
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Player,
    Enemy,
    Neutral,
}

impl Default for Team {
    fn default() -> Self {
        Self::Neutral
    }
}

// ============ Item Components ============

/// 아이템 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    Weapon,
    Grimoire,
    Consumable,
}

/// 아이템 픽업 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Item {
    pub item_id: String,
    pub item_type: ItemType,
    pub is_collected: bool,
}

impl Item {
    pub fn new(item_id: String, item_type: ItemType) -> Self {
        Self {
            item_id,
            item_type,
            is_collected: false,
        }
    }
}

// ============ Trigger Components ============

/// 트리거 존 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Trigger {
    pub event_name: String,
    pub is_active: bool,
    pub triggered_count: u32,
    pub one_shot: bool,
}

impl Trigger {
    pub fn new(event_name: String) -> Self {
        Self {
            event_name,
            is_active: true,
            triggered_count: 0,
            one_shot: false,
        }
    }
}

// ============ Light Components ============

/// 라이트 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightType {
    Point,
    Spot,
    Sun,
    Area,
}

/// 라이트 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Light {
    pub light_type: LightType,
    pub intensity: f32,
    pub color: Vec3,
    pub range: f32,
    pub spot_angle: f32,
    pub cast_shadows: bool,
}

impl Light {
    pub fn point(intensity: f32, color: Vec3) -> Self {
        Self {
            light_type: LightType::Point,
            intensity,
            color,
            range: 10.0,
            spot_angle: 0.0,
            cast_shadows: true,
        }
    }

    pub fn spot(intensity: f32, color: Vec3, angle: f32) -> Self {
        Self {
            light_type: LightType::Spot,
            intensity,
            color,
            range: 15.0,
            spot_angle: angle,
            cast_shadows: true,
        }
    }

    pub fn sun(intensity: f32, color: Vec3) -> Self {
        Self {
            light_type: LightType::Sun,
            intensity,
            color,
            range: f32::INFINITY,
            spot_angle: 0.0,
            cast_shadows: true,
        }
    }
}

// ============ Scripting Components ============

/// Lua script attachment
#[derive(Component, Debug, Clone)]
pub struct ScriptComponent {
    pub script_path: String,
    pub enabled: bool,
}

impl ScriptComponent {
    pub fn new(script_path: &str) -> Self {
        Self {
            script_path: script_path.to_string(),
            enabled: true,
        }
    }
}

// ============ Utility Components ============

/// Node name for debugging
#[derive(Component, Debug, Clone)]
pub struct NodeName(pub String);

/// Parent entity reference (for hierarchy)
#[derive(Component, Debug, Clone, Copy)]
pub struct Parent(pub Entity);

/// Children entities (for hierarchy)
#[derive(Component, Debug, Clone)]
pub struct Children(pub Vec<Entity>);

impl Children {
    pub fn new(children: Vec<Entity>) -> Self {
        Self(children)
    }
}

/// 엔티티 숨김 상태 (에디터용)
#[derive(Component, Debug, Clone, Default)]
pub struct Hidden;

// ============ AI Components ============

/// AI 상태 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AiStateType {
    #[default]
    Idle,
    Patrol,
    Chase,
    Attack,
    Flee,
    Dead,
    /// 커스텀 상태 (Lua에서 정의)
    Custom(u32),
}

impl AiStateType {
    /// 상태 이름 반환
    pub fn name(&self) -> &'static str {
        match self {
            AiStateType::Idle => "Idle",
            AiStateType::Patrol => "Patrol",
            AiStateType::Chase => "Chase",
            AiStateType::Attack => "Attack",
            AiStateType::Flee => "Flee",
            AiStateType::Dead => "Dead",
            AiStateType::Custom(_) => "Custom",
        }
    }

    /// 문자열에서 상태 파싱
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "idle" => AiStateType::Idle,
            "patrol" => AiStateType::Patrol,
            "chase" => AiStateType::Chase,
            "attack" => AiStateType::Attack,
            "flee" => AiStateType::Flee,
            "dead" => AiStateType::Dead,
            _ => AiStateType::Idle,
        }
    }
}

/// AI 상태 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct AiState {
    /// 현재 상태
    pub current: AiStateType,
    /// 이전 상태
    pub previous: AiStateType,
    /// 현재 상태에 머문 시간
    pub time_in_state: f32,
    /// 상태 전환 트리거됨
    pub transition_pending: Option<AiStateType>,
}

impl Default for AiState {
    fn default() -> Self {
        Self {
            current: AiStateType::Idle,
            previous: AiStateType::Idle,
            time_in_state: 0.0,
            transition_pending: None,
        }
    }
}

impl AiState {
    /// 새 AI 상태 생성
    pub fn new(initial: AiStateType) -> Self {
        Self {
            current: initial,
            previous: initial,
            ..Default::default()
        }
    }

    /// 상태 전환 요청
    pub fn transition_to(&mut self, new_state: AiStateType) {
        if self.current != new_state {
            self.transition_pending = Some(new_state);
        }
    }

    /// 상태 전환 적용 (시스템에서 호출)
    pub fn apply_transition(&mut self) {
        if let Some(new_state) = self.transition_pending.take() {
            self.previous = self.current;
            self.current = new_state;
            self.time_in_state = 0.0;
        }
    }

    /// 상태 시간 업데이트
    pub fn update_time(&mut self, delta_time: f32) {
        self.time_in_state += delta_time;
    }

    /// 상태 변경 직후인지 확인
    pub fn just_entered(&self) -> bool {
        self.time_in_state < 0.001
    }
}

/// AI 컨트롤러 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct AiController {
    /// 감지 범위 (플레이어 발견)
    pub detection_range: f32,
    /// 공격 범위
    pub attack_range: f32,
    /// 도주 체력 임계값 (0.0 ~ 1.0)
    pub flee_health_threshold: f32,
    /// 순찰 경로 (월드 좌표)
    pub patrol_waypoints: Vec<Vec3>,
    /// 현재 순찰 인덱스
    pub current_waypoint: usize,
    /// 이동 속도
    pub move_speed: f32,
    /// 회전 속도
    pub turn_speed: f32,
    /// 타겟 엔티티
    pub target: Option<Entity>,
    /// AI 스크립트 경로 (Lua 콜백용)
    pub script_path: Option<String>,
    /// 활성화 상태
    pub enabled: bool,
}

impl Default for AiController {
    fn default() -> Self {
        Self {
            detection_range: 15.0,
            attack_range: 2.0,
            flee_health_threshold: 0.2,
            patrol_waypoints: Vec::new(),
            current_waypoint: 0,
            move_speed: 4.0,
            turn_speed: 5.0,
            target: None,
            script_path: None,
            enabled: true,
        }
    }
}

impl AiController {
    /// 기본 적 AI
    pub fn enemy() -> Self {
        Self {
            detection_range: 15.0,
            attack_range: 2.0,
            flee_health_threshold: 0.2,
            move_speed: 4.0,
            ..Default::default()
        }
    }

    /// 보스 AI (넓은 감지, 도주 안함)
    pub fn boss() -> Self {
        Self {
            detection_range: 30.0,
            attack_range: 3.0,
            flee_health_threshold: 0.0,
            move_speed: 3.0,
            ..Default::default()
        }
    }

    /// 순찰 경로 설정
    pub fn with_patrol(mut self, waypoints: Vec<Vec3>) -> Self {
        self.patrol_waypoints = waypoints;
        self
    }

    /// 다음 순찰 지점으로 이동
    pub fn next_waypoint(&mut self) {
        if !self.patrol_waypoints.is_empty() {
            self.current_waypoint = (self.current_waypoint + 1) % self.patrol_waypoints.len();
        }
    }

    /// 현재 순찰 목표 지점
    pub fn current_patrol_target(&self) -> Option<Vec3> {
        self.patrol_waypoints.get(self.current_waypoint).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_component() {
        let mut health = Health::new(100.0);
        assert_eq!(health.current, 100.0);
        assert_eq!(health.maximum, 100.0);
        assert!(!health.is_dead());
        assert_eq!(health.percentage(), 1.0);

        health.take_damage(30.0);
        assert_eq!(health.current, 70.0);
        assert_eq!(health.percentage(), 0.7);

        health.heal(15.0);
        assert_eq!(health.current, 85.0);

        health.take_damage(100.0);
        assert!(health.is_dead());
        assert_eq!(health.current, 0.0);

        health.heal(1000.0);
        assert_eq!(health.current, health.maximum);
    }

    #[test]
    fn test_weapon_component() {
        let mut weapon = Weapon::default();
        assert_eq!(weapon.ammo, 30);

        assert!(!weapon.can_fire());

        weapon.time_since_fire = weapon.fire_rate;
        assert!(weapon.can_fire());
        assert!(weapon.fire());
        assert_eq!(weapon.ammo, 29);
        assert_eq!(weapon.time_since_fire, 0.0);

        assert!(!weapon.can_fire());

        weapon.ammo = 0;
        weapon.time_since_fire = weapon.fire_rate;
        assert!(!weapon.can_fire());
        assert!(!weapon.fire());

        weapon.reload();
        assert_eq!(weapon.ammo, weapon.max_ammo);
    }

    #[test]
    fn test_velocity_component() {
        let velocity = Velocity::from_linear(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(velocity.linear, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(velocity.angular, Vec3::ZERO);

        let velocity2 = Velocity::new(Vec3::X, Vec3::Y);
        assert_eq!(velocity2.linear, Vec3::X);
        assert_eq!(velocity2.angular, Vec3::Y);
    }

    #[test]
    fn test_collider_components() {
        let box_col = BoxCollider::new(Vec3::ONE);
        assert_eq!(box_col.half_extents, Vec3::ONE);
        assert_eq!(box_col.offset, Vec3::ZERO);

        let box_col2 = BoxCollider::with_offset(Vec3::new(1.0, 2.0, 3.0), Vec3::Y);
        assert_eq!(box_col2.half_extents, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(box_col2.offset, Vec3::Y);

        let sphere_col = SphereCollider::new(2.5);
        assert_eq!(sphere_col.radius, 2.5);
        assert_eq!(sphere_col.offset, Vec3::ZERO);
    }

    #[test]
    fn test_script_component() {
        let script = ScriptComponent::new("assets/scripts/player.lua");
        assert_eq!(script.script_path, "assets/scripts/player.lua");
        assert!(script.enabled);
    }

    #[test]
    fn test_team_component() {
        assert_eq!(Team::default(), Team::Neutral);
        assert_ne!(Team::Player, Team::Enemy);
    }

    #[test]
    fn test_player_component() {
        let player = Player::new(1);
        assert_eq!(player.player_id, 1);
    }
}
