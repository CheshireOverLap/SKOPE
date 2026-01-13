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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ItemType {
    Weapon,
    Grimoire,
    Consumable,
    Equipment,
    Material,
    Quest,
    Key,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LightType {
    Point,
    Spot,
    Sun,
    Area,
}

/// 라이트 컴포넌트
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Light {
    pub light_type: LightType,
    pub intensity: f32,
    #[serde(with = "crate::vec3_serde")]
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

// ============ Animation Components ============

use std::collections::HashMap;

/// Animator 파라미터 타입
#[derive(Debug, Clone, PartialEq)]
pub enum AnimatorParameter {
    Bool(bool),
    Float(f32),
    Int(i32),
    Trigger(bool),
}

impl AnimatorParameter {
    /// 타입 이름 반환
    pub fn type_name(&self) -> &'static str {
        match self {
            AnimatorParameter::Bool(_) => "Bool",
            AnimatorParameter::Float(_) => "Float",
            AnimatorParameter::Int(_) => "Int",
            AnimatorParameter::Trigger(_) => "Trigger",
        }
    }
}

/// 애니메이터 상태 정의
#[derive(Debug, Clone)]
pub struct AnimatorState {
    /// 상태 이름
    pub name: String,
    /// 대응하는 애니메이션 클립 인덱스
    pub animation_index: usize,
    /// 루프 여부
    pub looping: bool,
    /// 속도 배율
    pub speed: f32,
}

impl AnimatorState {
    pub fn new(name: &str, animation_index: usize) -> Self {
        Self {
            name: name.to_string(),
            animation_index,
            looping: true,
            speed: 1.0,
        }
    }

    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }
}

/// 전이 조건
#[derive(Debug, Clone)]
pub enum TransitionCondition {
    /// Bool 파라미터가 특정 값일 때
    BoolEquals { param: String, value: bool },
    /// Float 파라미터가 임계값보다 클 때
    FloatGreater { param: String, threshold: f32 },
    /// Float 파라미터가 임계값보다 작을 때
    FloatLess { param: String, threshold: f32 },
    /// Int 파라미터가 특정 값일 때
    IntEquals { param: String, value: i32 },
    /// Trigger 파라미터가 발동되었을 때
    TriggerSet { param: String },
}

impl TransitionCondition {
    pub fn evaluate(&self, params: &HashMap<String, AnimatorParameter>) -> bool {
        match self {
            TransitionCondition::BoolEquals { param, value } => {
                matches!(params.get(param), Some(AnimatorParameter::Bool(v)) if v == value)
            }
            TransitionCondition::FloatGreater { param, threshold } => {
                matches!(params.get(param), Some(AnimatorParameter::Float(v)) if v > threshold)
            }
            TransitionCondition::FloatLess { param, threshold } => {
                matches!(params.get(param), Some(AnimatorParameter::Float(v)) if v < threshold)
            }
            TransitionCondition::IntEquals { param, value } => {
                matches!(params.get(param), Some(AnimatorParameter::Int(v)) if v == value)
            }
            TransitionCondition::TriggerSet { param } => {
                matches!(params.get(param), Some(AnimatorParameter::Trigger(true)))
            }
        }
    }
}

/// 애니메이터 전이 정의
#[derive(Debug, Clone)]
pub struct AnimatorTransition {
    /// 출발 상태 인덱스
    pub from_state: usize,
    /// 도착 상태 인덱스
    pub to_state: usize,
    /// 전이 조건
    pub conditions: Vec<TransitionCondition>,
    /// 전이 지속 시간 (크로스페이드)
    pub duration: f32,
    /// 어느 위치에서든 전이 가능 (Exit Time 무시)
    pub has_exit_time: bool,
    /// 종료 시간 비율 (0.0 ~ 1.0)
    pub exit_time: f32,
}

impl AnimatorTransition {
    pub fn new(from: usize, to: usize, duration: f32) -> Self {
        Self {
            from_state: from,
            to_state: to,
            conditions: Vec::new(),
            duration,
            has_exit_time: false,
            exit_time: 1.0,
        }
    }

    pub fn with_condition(mut self, condition: TransitionCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn with_exit_time(mut self, exit_time: f32) -> Self {
        self.has_exit_time = true;
        self.exit_time = exit_time;
        self
    }
}

/// AI 상태 → 애니메이션 매핑
#[derive(Debug, Clone)]
pub struct AiAnimationMapping {
    /// 대상 상태 이름
    pub target_state: String,
    /// 설정할 파라미터들
    pub parameters: Vec<(String, AnimatorParameter)>,
    /// 전이 지속 시간 오버라이드
    pub transition_duration: Option<f32>,
}

/// 통합 애니메이터 컨트롤러 컴포넌트 (엔티티별 독립)
#[derive(Component, Debug, Clone)]
pub struct AnimatorController {
    // ---- 기본 설정 ----
    /// 모델 이름 (SkinnedModelRegistry 키)
    pub model_name: String,
    /// 활성화 여부
    pub enabled: bool,
    /// 재생 속도 배율
    pub speed: f32,

    // ---- 상태 머신 ----
    /// 현재 상태 인덱스
    pub current_state: usize,
    /// 이전 상태 인덱스
    pub previous_state: usize,
    /// 파라미터들 (이름 → 값)
    pub parameters: HashMap<String, AnimatorParameter>,
    /// 상태 정의
    pub states: Vec<AnimatorState>,
    /// 전이 정의
    pub transitions: Vec<AnimatorTransition>,

    // ---- 런타임 상태 ----
    /// 현재 재생 시간
    pub current_time: f32,
    /// 전이 중 여부
    pub in_transition: bool,
    /// 전이 진행률 (0.0 ~ 1.0)
    pub transition_progress: f32,
    /// 현재 전이 지속 시간
    pub transition_duration: f32,

    // ---- AI 연동 ----
    /// AI 상태 → 애니메이션 매핑
    pub ai_state_mappings: HashMap<AiStateType, AiAnimationMapping>,
    /// AI 연동 활성화
    pub ai_sync_enabled: bool,
}

impl Default for AnimatorController {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            enabled: true,
            speed: 1.0,
            current_state: 0,
            previous_state: 0,
            parameters: HashMap::new(),
            states: Vec::new(),
            transitions: Vec::new(),
            current_time: 0.0,
            in_transition: false,
            transition_progress: 0.0,
            transition_duration: 0.25,
            ai_state_mappings: HashMap::new(),
            ai_sync_enabled: false,
        }
    }
}

impl AnimatorController {
    /// 새 애니메이터 컨트롤러 생성
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            ..Default::default()
        }
    }

    /// 상태 추가
    pub fn add_state(&mut self, state: AnimatorState) -> usize {
        let idx = self.states.len();
        self.states.push(state);
        idx
    }

    /// 전이 추가
    pub fn add_transition(&mut self, transition: AnimatorTransition) {
        self.transitions.push(transition);
    }

    /// Bool 파라미터 추가
    pub fn add_bool(&mut self, name: &str, value: bool) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Bool(value));
        self
    }

    /// Float 파라미터 추가
    pub fn add_float(&mut self, name: &str, value: f32) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Float(value));
        self
    }

    /// Int 파라미터 추가
    pub fn add_int(&mut self, name: &str, value: i32) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Int(value));
        self
    }

    /// Trigger 파라미터 추가
    pub fn add_trigger(&mut self, name: &str) -> &mut Self {
        self.parameters.insert(name.to_string(), AnimatorParameter::Trigger(false));
        self
    }

    /// Bool 파라미터 설정
    pub fn set_bool(&mut self, name: &str, value: bool) {
        if let Some(AnimatorParameter::Bool(v)) = self.parameters.get_mut(name) {
            *v = value;
        }
    }

    /// Float 파라미터 설정
    pub fn set_float(&mut self, name: &str, value: f32) {
        if let Some(AnimatorParameter::Float(v)) = self.parameters.get_mut(name) {
            *v = value;
        }
    }

    /// Int 파라미터 설정
    pub fn set_int(&mut self, name: &str, value: i32) {
        if let Some(AnimatorParameter::Int(v)) = self.parameters.get_mut(name) {
            *v = value;
        }
    }

    /// Trigger 발동
    pub fn set_trigger(&mut self, name: &str) {
        if let Some(AnimatorParameter::Trigger(v)) = self.parameters.get_mut(name) {
            *v = true;
        }
    }

    /// Trigger 소비
    pub fn consume_trigger(&mut self, name: &str) {
        if let Some(AnimatorParameter::Trigger(v)) = self.parameters.get_mut(name) {
            *v = false;
        }
    }

    /// 상태 이름으로 인덱스 찾기
    pub fn find_state(&self, name: &str) -> Option<usize> {
        self.states.iter().position(|s| s.name == name)
    }

    /// 상태 직접 전환
    pub fn transition_to_state(&mut self, state_index: usize, duration: f32) {
        if state_index < self.states.len() && state_index != self.current_state {
            self.previous_state = self.current_state;
            self.current_state = state_index;
            self.in_transition = true;
            self.transition_progress = 0.0;
            self.transition_duration = duration;
            self.current_time = 0.0;
        }
    }

    /// 상태 이름으로 전환
    pub fn transition_to(&mut self, state_name: &str, duration: f32) {
        if let Some(idx) = self.find_state(state_name) {
            self.transition_to_state(idx, duration);
        }
    }

    /// 현재 상태의 애니메이션 인덱스
    pub fn current_animation_index(&self) -> Option<usize> {
        self.states.get(self.current_state).map(|s| s.animation_index)
    }

    /// 이전 상태의 애니메이션 인덱스
    pub fn previous_animation_index(&self) -> Option<usize> {
        self.states.get(self.previous_state).map(|s| s.animation_index)
    }

    /// 전이 조건 검사 및 자동 전이
    pub fn check_transitions(&mut self) {
        let mut matched_transition: Option<(usize, f32, Vec<String>)> = None;

        for transition in &self.transitions {
            if transition.from_state != self.current_state {
                continue;
            }

            let all_conditions_met = transition.conditions.iter()
                .all(|c| c.evaluate(&self.parameters));

            if all_conditions_met {
                let triggers_to_consume: Vec<String> = transition.conditions.iter()
                    .filter_map(|c| {
                        if let TransitionCondition::TriggerSet { param } = c {
                            Some(param.clone())
                        } else {
                            None
                        }
                    })
                    .collect();

                matched_transition = Some((transition.to_state, transition.duration, triggers_to_consume));
                break;
            }
        }

        if let Some((to_state, duration, triggers)) = matched_transition {
            self.previous_state = self.current_state;
            self.current_state = to_state;
            self.in_transition = true;
            self.transition_progress = 0.0;
            self.transition_duration = duration;
            self.current_time = 0.0;

            for param in triggers {
                self.consume_trigger(&param);
            }
        }
    }

    /// 시간 업데이트
    pub fn update(&mut self, dt: f32, animation_duration: f32) {
        if !self.enabled {
            return;
        }

        if !self.in_transition {
            self.check_transitions();
        }

        if self.in_transition {
            self.transition_progress += dt / self.transition_duration;
            if self.transition_progress >= 1.0 {
                self.transition_progress = 1.0;
                self.in_transition = false;
            }
        }

        let state_speed = self.states.get(self.current_state)
            .map(|s| s.speed)
            .unwrap_or(1.0);

        self.current_time += dt * self.speed * state_speed;

        if animation_duration > 0.0 && self.current_time >= animation_duration {
            let is_looping = self.states.get(self.current_state)
                .map(|s| s.looping)
                .unwrap_or(true);

            if is_looping {
                self.current_time %= animation_duration;
            } else {
                self.current_time = animation_duration;
            }
        }
    }

    /// 기본 AI-애니메이션 매핑 설정
    pub fn with_default_ai_mappings(mut self) -> Self {
        use AiStateType::*;

        self.ai_state_mappings.insert(Idle, AiAnimationMapping {
            target_state: "Idle".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(0.0)),
                ("IsMoving".to_string(), AnimatorParameter::Bool(false)),
            ],
            transition_duration: Some(0.2),
        });

        self.ai_state_mappings.insert(Patrol, AiAnimationMapping {
            target_state: "Walk".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(0.3)),
                ("IsMoving".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.25),
        });

        self.ai_state_mappings.insert(Chase, AiAnimationMapping {
            target_state: "Run".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(1.0)),
                ("IsMoving".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.15),
        });

        self.ai_state_mappings.insert(Attack, AiAnimationMapping {
            target_state: "Attack".to_string(),
            parameters: vec![
                ("IsAttacking".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.1),
        });

        self.ai_state_mappings.insert(Flee, AiAnimationMapping {
            target_state: "Run".to_string(),
            parameters: vec![
                ("Speed".to_string(), AnimatorParameter::Float(1.2)),
                ("IsFleeing".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.15),
        });

        self.ai_state_mappings.insert(Dead, AiAnimationMapping {
            target_state: "Death".to_string(),
            parameters: vec![
                ("IsDead".to_string(), AnimatorParameter::Bool(true)),
            ],
            transition_duration: Some(0.3),
        });

        self.ai_sync_enabled = true;
        self
    }

    /// AI 매핑 추가
    pub fn add_ai_mapping(&mut self, ai_state: AiStateType, mapping: AiAnimationMapping) -> &mut Self {
        self.ai_state_mappings.insert(ai_state, mapping);
        self
    }

    /// AI 동기화 활성화
    pub fn with_ai_sync(mut self, enabled: bool) -> Self {
        self.ai_sync_enabled = enabled;
        self
    }

    /// GLTF 애니메이션 목록으로 상태 자동 생성
    pub fn with_animations(mut self, animation_names: &[String]) -> Self {
        for (i, name) in animation_names.iter().enumerate() {
            self.states.push(AnimatorState {
                name: name.clone(),
                animation_index: i,
                looping: true,
                speed: 1.0,
            });
        }
        self
    }

    /// AI 상태에 따른 애니메이션 적용
    pub fn apply_ai_state(&mut self, ai_state: &AiStateType) {
        if !self.ai_sync_enabled {
            return;
        }

        if let Some(mapping) = self.ai_state_mappings.get(ai_state).cloned() {
            for (name, value) in &mapping.parameters {
                match value {
                    AnimatorParameter::Bool(v) => self.set_bool(name, *v),
                    AnimatorParameter::Float(v) => self.set_float(name, *v),
                    AnimatorParameter::Int(v) => self.set_int(name, *v),
                    AnimatorParameter::Trigger(_) => self.set_trigger(name),
                }
            }

            if !mapping.target_state.is_empty() {
                let duration = mapping.transition_duration.unwrap_or(0.25);
                self.transition_to(&mapping.target_state, duration);
            }
        }
    }

    /// 현재 블렌딩 가중치 계산 (크로스페이드용)
    pub fn blend_weights(&self) -> (f32, f32) {
        if self.in_transition {
            let t = self.transition_progress;
            let smooth_t = t * t * (3.0 - 2.0 * t);
            (1.0 - smooth_t, smooth_t)
        } else {
            (0.0, 1.0)
        }
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
