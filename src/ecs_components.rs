// ECS Components for SKOPE Engine
#![allow(dead_code)]

use bevy_ecs::prelude::*;
use glam::{Mat4, Quat, Vec3};
use wgpu;

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
    pub skeleton_entity: Entity,  // 스켈레톤 엔티티 참조
}

/// 스켈레톤 컴포넌트 - 본 트리의 루트
#[derive(Component, Debug, Clone)]
pub struct Skeleton {
    pub model_name: String,       // SkinnedModelRegistry의 모델 이름
    pub skin_index: usize,        // gltf_loader::Skin 인덱스
    pub joint_entities: Vec<Entity>,  // 본 엔티티들
}

/// 본(조인트) 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Joint {
    pub joint_index: usize,        // 스킨 내 조인트 인덱스
    pub inverse_bind_matrix: Mat4,  // 역 바인드 행렬
}

/// 본 매트릭스 버퍼 (GPU 업로드용)
/// 스켈레톤마다 하나씩 존재
#[derive(Component, Debug)]
pub struct JointMatrices {
    pub matrices: Vec<Mat4>,  // joint_count개의 최종 변환 행렬
}

impl Default for JointMatrices {
    fn default() -> Self {
        Self {
            matrices: Vec::new(),
        }
    }
}

/// 스켈레탈 애니메이션 컨트롤러
/// SkinnedModelRegistry의 모델과 연동되어 애니메이션 재생 제어
#[derive(Component, Debug, Clone)]
pub struct AnimationController {
    /// 모델 이름 (SkinnedModelRegistry 키)
    pub model_name: String,
    /// 현재 애니메이션 클립 인덱스
    pub current_animation: usize,
    /// 현재 재생 시간 (초)
    pub current_time: f32,
    /// 재생 속도 배율
    pub speed: f32,
    /// 루프 재생 여부
    pub looping: bool,
    /// 재생 중 여부
    pub playing: bool,
}

impl Default for AnimationController {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            current_animation: 0,
            current_time: 0.0,
            speed: 1.0,
            looping: true,
            playing: true,
        }
    }
}

impl AnimationController {
    /// 새 애니메이션 컨트롤러 생성
    pub fn new(model_name: &str) -> Self {
        Self {
            model_name: model_name.to_string(),
            ..Default::default()
        }
    }

    /// 재생 시작
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// 일시 정지
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// 정지 및 시간 초기화
    pub fn stop(&mut self) {
        self.playing = false;
        self.current_time = 0.0;
    }

    /// 애니메이션 클립 변경
    pub fn set_animation(&mut self, index: usize) {
        if self.current_animation != index {
            self.current_animation = index;
            self.current_time = 0.0;
        }
    }

    /// 시간 업데이트 (delta_time 적용)
    pub fn update(&mut self, delta_time: f32, animation_duration: f32) {
        if !self.playing || animation_duration <= 0.0 {
            return;
        }

        self.current_time += delta_time * self.speed;

        if self.current_time >= animation_duration {
            if self.looping {
                self.current_time %= animation_duration;
            } else {
                self.current_time = animation_duration;
                self.playing = false;
            }
        }
    }

    /// 재생 진행률 (0.0 ~ 1.0)
    pub fn progress(&self, animation_duration: f32) -> f32 {
        if animation_duration <= 0.0 {
            0.0
        } else {
            (self.current_time / animation_duration).clamp(0.0, 1.0)
        }
    }
}

/// 스킨드 메시 렌더러 컴포넌트
/// 개별 엔티티의 스킨드 메시 렌더링 정보 (인스턴스별 조인트 버퍼)
#[derive(Component)]
pub struct SkinnedMeshRenderer {
    /// 모델 이름 (SkinnedModelRegistry 키)
    pub model_name: String,
    /// 메시 인덱스
    pub mesh_index: usize,
    /// 조인트 매트릭스 버퍼 (GPU)
    pub joint_buffer: wgpu::Buffer,
    /// 조인트 바인드 그룹
    pub joint_bind_group: wgpu::BindGroup,
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
    Static,     // 움직이지 않음
    Dynamic,    // 물리 시뮬레이션 적용
    Kinematic,  // 코드로만 이동 (충돌은 감지)
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
    pub enemy_prefab: String,      // 스폰할 적 prefab 이름
    pub spawn_interval: f32,       // 스폰 간격 (초)
    pub spawn_radius: f32,         // 스폰 반경
    pub max_enemies: u32,          // 최대 동시 존재 적 수
    pub current_count: u32,        // 현재 스폰된 적 수
    pub time_since_spawn: f32,     // 마지막 스폰 후 경과 시간
    pub respawn_enabled: bool,     // 적 리스폰 여부
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
    pub fire_rate: f32,         // 발사 간격 (초)
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
    #[serde(with = "vec3_serde")]
    pub color: Vec3,
    pub range: f32,           // Point/Spot 전용
    pub spot_angle: f32,      // Spot 전용
    pub cast_shadows: bool,
}

// Vec3 직렬화 헬퍼
mod vec3_serde {
    use glam::Vec3;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(v: &Vec3, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        [v.x, v.y, v.z].serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec3, D::Error>
    where D: Deserializer<'de> {
        let arr: [f32; 3] = Deserialize::deserialize(deserializer)?;
        Ok(Vec3::new(arr[0], arr[1], arr[2]))
    }
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
            flee_health_threshold: 0.0, // 도주 안함
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

// ============ Inventory Components ============

/// 아이템 효과
#[derive(Debug, Clone)]
pub enum ItemEffect {
    /// 체력 회복
    Heal { amount: f32 },
    /// 마나 회복
    ManaRestore { amount: f32 },
    /// 스탯 버프
    Buff { stat: String, value: f32, duration: f32 },
    /// 데미지
    Damage { amount: f32 },
    /// 커스텀 효과 (Lua에서 처리)
    Custom { effect_id: String },
}

/// 아이템 정의
#[derive(Debug, Clone)]
pub struct ItemDef {
    /// 아이템 ID (고유 식별자)
    pub id: String,
    /// 아이템 이름
    pub name: String,
    /// 아이템 설명
    pub description: String,
    /// 아이템 타입
    pub item_type: ItemType,
    /// 최대 스택 수량
    pub max_stack: u32,
    /// 아이템 효과
    pub effect: Option<ItemEffect>,
    /// 판매 가격
    pub sell_price: u32,
    /// 구매 가격
    pub buy_price: u32,
}

impl ItemDef {
    /// 새 아이템 정의 생성
    pub fn new(id: &str, name: &str, item_type: ItemType) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            item_type,
            max_stack: 99,
            effect: None,
            sell_price: 0,
            buy_price: 0,
        }
    }

    /// 설명 추가
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// 최대 스택 설정
    pub fn with_max_stack(mut self, max: u32) -> Self {
        self.max_stack = max;
        self
    }

    /// 효과 추가
    pub fn with_effect(mut self, effect: ItemEffect) -> Self {
        self.effect = Some(effect);
        self
    }

    /// 가격 설정
    pub fn with_prices(mut self, buy: u32, sell: u32) -> Self {
        self.buy_price = buy;
        self.sell_price = sell;
        self
    }

    /// 힐링 포션 생성
    pub fn health_potion(heal_amount: f32) -> Self {
        Self::new("health_potion", "Health Potion", ItemType::Consumable)
            .with_description("Restores health")
            .with_max_stack(10)
            .with_effect(ItemEffect::Heal { amount: heal_amount })
            .with_prices(50, 25)
    }
}

/// 인벤토리 슬롯
#[derive(Debug, Clone)]
pub struct InventorySlot {
    /// 아이템 ID
    pub item_id: String,
    /// 수량
    pub count: u32,
}

impl InventorySlot {
    pub fn new(item_id: &str, count: u32) -> Self {
        Self {
            item_id: item_id.to_string(),
            count,
        }
    }
}

/// 인벤토리 에러
#[derive(Debug, Clone)]
pub enum InventoryError {
    /// 인벤토리 가득 참
    Full,
    /// 아이템 없음
    NotFound,
    /// 수량 부족
    InsufficientCount,
    /// 잘못된 슬롯
    InvalidSlot,
}

/// 인벤토리 컴포넌트
#[derive(Component, Debug, Clone)]
pub struct Inventory {
    /// 인벤토리 슬롯들
    pub slots: Vec<Option<InventorySlot>>,
    /// 인벤토리 용량
    pub capacity: usize,
    /// 소지금
    pub gold: u32,
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(20)
    }
}

impl Inventory {
    /// 새 인벤토리 생성
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            capacity,
            gold: 0,
        }
    }

    /// 아이템 추가 (스택 가능)
    pub fn add_item(&mut self, item_id: &str, count: u32, max_stack: u32) -> Result<(), InventoryError> {
        let mut remaining = count;

        // 기존 스택에 추가 시도
        for slot in &mut self.slots {
            if remaining == 0 {
                break;
            }
            if let Some(ref mut s) = slot {
                if s.item_id == item_id && s.count < max_stack {
                    let can_add = (max_stack - s.count).min(remaining);
                    s.count += can_add;
                    remaining -= can_add;
                }
            }
        }

        // 빈 슬롯에 새로 추가
        while remaining > 0 {
            if let Some(empty_slot) = self.slots.iter_mut().find(|s| s.is_none()) {
                let to_add = remaining.min(max_stack);
                *empty_slot = Some(InventorySlot::new(item_id, to_add));
                remaining -= to_add;
            } else {
                return Err(InventoryError::Full);
            }
        }

        Ok(())
    }

    /// 아이템 제거
    pub fn remove_item(&mut self, item_id: &str, count: u32) -> Result<u32, InventoryError> {
        let mut remaining = count;
        let mut removed = 0;

        for slot in &mut self.slots {
            if remaining == 0 {
                break;
            }
            if let Some(ref mut s) = slot {
                if s.item_id == item_id {
                    let to_remove = s.count.min(remaining);
                    s.count -= to_remove;
                    remaining -= to_remove;
                    removed += to_remove;

                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
        }

        if removed == 0 {
            Err(InventoryError::NotFound)
        } else if remaining > 0 {
            Err(InventoryError::InsufficientCount)
        } else {
            Ok(removed)
        }
    }

    /// 아이템 보유 여부 확인
    pub fn has_item(&self, item_id: &str, count: u32) -> bool {
        let total: u32 = self.slots
            .iter()
            .filter_map(|s| s.as_ref())
            .filter(|s| s.item_id == item_id)
            .map(|s| s.count)
            .sum();
        total >= count
    }

    /// 아이템 총 수량
    pub fn count_item(&self, item_id: &str) -> u32 {
        self.slots
            .iter()
            .filter_map(|s| s.as_ref())
            .filter(|s| s.item_id == item_id)
            .map(|s| s.count)
            .sum()
    }

    /// 슬롯 아이템 가져오기
    pub fn get_slot(&self, index: usize) -> Option<&InventorySlot> {
        self.slots.get(index).and_then(|s| s.as_ref())
    }

    /// 슬롯 아이템 사용 (수량 1 감소)
    pub fn use_slot(&mut self, index: usize) -> Result<String, InventoryError> {
        let slot = self.slots.get_mut(index).ok_or(InventoryError::InvalidSlot)?;

        if let Some(ref mut s) = slot {
            let item_id = s.item_id.clone();
            s.count -= 1;
            if s.count == 0 {
                *slot = None;
            }
            Ok(item_id)
        } else {
            Err(InventoryError::NotFound)
        }
    }

    /// 빈 슬롯 수
    pub fn empty_slots(&self) -> usize {
        self.slots.iter().filter(|s| s.is_none()).count()
    }

    /// 인벤토리가 가득 찼는지
    pub fn is_full(&self) -> bool {
        self.empty_slots() == 0
    }
}

/// 픽업 가능한 아이템 (월드에 있는 아이템)
#[derive(Component, Debug, Clone)]
pub struct Pickupable {
    /// 아이템 ID
    pub item_id: String,
    /// 수량
    pub count: u32,
    /// 픽업 범위
    pub pickup_range: f32,
    /// 자동 픽업 여부
    pub auto_pickup: bool,
}

impl Pickupable {
    pub fn new(item_id: &str, count: u32) -> Self {
        Self {
            item_id: item_id.to_string(),
            count,
            pickup_range: 1.5,
            auto_pickup: true,
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
/// H키로 추가, Alt+H로 모두 제거
#[derive(Component, Debug, Clone, Default)]
pub struct Hidden;

// ============ Animation Components ============

/// Animator 컴포넌트 - 상태 머신 기반 애니메이션
/// Inspector에서 파라미터 조절 가능
#[derive(Component, Debug, Clone)]
pub struct Animator {
    /// 현재 상태 인덱스
    pub current_state: usize,
    /// 파라미터들 (이름 → 값)
    pub parameters: std::collections::HashMap<String, AnimatorParameter>,
    /// 재생 속도 배율
    pub speed: f32,
    /// 현재 재생 시간 (루프 시 0으로 리셋)
    pub current_time: f32,
    /// 활성화 여부
    pub enabled: bool,
    /// 사용할 애니메이션 클립 인덱스들
    pub animation_indices: Vec<usize>,
}

impl Default for Animator {
    fn default() -> Self {
        Self {
            current_state: 0,
            parameters: std::collections::HashMap::new(),
            speed: 1.0,
            current_time: 0.0,
            enabled: true,
            animation_indices: Vec::new(),
        }
    }
}

impl Animator {
    pub fn new() -> Self {
        Self::default()
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

    /// Trigger 소비 (상태 전이 후 호출)
    pub fn consume_trigger(&mut self, name: &str) {
        if let Some(AnimatorParameter::Trigger(v)) = self.parameters.get_mut(name) {
            *v = false;
        }
    }

    /// 파라미터 값 가져오기
    pub fn get_bool(&self, name: &str) -> Option<bool> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_float(&self, name: &str) -> Option<f32> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Float(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_int(&self, name: &str) -> Option<i32> {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Int(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn is_trigger_set(&self, name: &str) -> bool {
        match self.parameters.get(name) {
            Some(AnimatorParameter::Trigger(v)) => *v,
            _ => false,
        }
    }
}

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

/// Animation Player 컴포넌트 - 단순 애니메이션 재생
/// Animator보다 가벼운 단일 클립 재생용
#[derive(Component, Debug, Clone)]
pub struct AnimationPlayer {
    /// 현재 재생 중인 애니메이션 인덱스
    pub animation_index: usize,
    /// 현재 재생 시간
    pub current_time: f32,
    /// 재생 속도
    pub speed: f32,
    /// 루프 여부
    pub looping: bool,
    /// 재생 중 여부
    pub playing: bool,
}

impl Default for AnimationPlayer {
    fn default() -> Self {
        Self {
            animation_index: 0,
            current_time: 0.0,
            speed: 1.0,
            looping: true,
            playing: true,
        }
    }
}

impl AnimationPlayer {
    pub fn new(animation_index: usize) -> Self {
        Self {
            animation_index,
            ..Default::default()
        }
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn pause(&mut self) {
        self.playing = false;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.current_time = 0.0;
    }

    pub fn set_animation(&mut self, index: usize) {
        self.animation_index = index;
        self.current_time = 0.0;
    }
}

// ============ Sprite Animation Components ============

/// 스프라이트 렌더러 컴포넌트
///
/// 스프라이트 시트에서 특정 프레임을 렌더링하기 위한 정보.
#[derive(Component, Debug, Clone)]
pub struct SpriteRenderer {
    /// 스프라이트 시트 인덱스 (에셋 관리용)
    pub sprite_sheet_index: usize,
    /// 현재 프레임 인덱스
    pub current_frame: u32,
    /// 색상 틴트 (RGBA)
    pub color: [f32; 4],
    /// 좌우 반전
    pub flip_x: bool,
    /// 상하 반전
    pub flip_y: bool,
    /// 렌더링 활성화
    pub visible: bool,
    /// 렌더 순서 (Z-order)
    pub order: i32,
}

impl Default for SpriteRenderer {
    fn default() -> Self {
        Self {
            sprite_sheet_index: 0,
            current_frame: 0,
            color: [1.0, 1.0, 1.0, 1.0],
            flip_x: false,
            flip_y: false,
            visible: true,
            order: 0,
        }
    }
}

impl SpriteRenderer {
    pub fn new(sprite_sheet_index: usize) -> Self {
        Self {
            sprite_sheet_index,
            ..Default::default()
        }
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_flip(mut self, flip_x: bool, flip_y: bool) -> Self {
        self.flip_x = flip_x;
        self.flip_y = flip_y;
        self
    }

    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }
}

/// 스프라이트 애니메이터 컴포넌트
///
/// 스프라이트 프레임 애니메이션을 제어.
#[derive(Component, Debug, Clone)]
pub struct SpriteAnimator {
    /// 현재 재생 중인 클립 이름
    pub current_clip: String,
    /// 현재 프레임 인덱스 (클립 내)
    pub frame_index: usize,
    /// 경과 시간
    pub elapsed: f32,
    /// 재생 속도 배율
    pub speed: f32,
    /// 재생 중 여부
    pub playing: bool,
    /// 재생 완료 시 이벤트 이름 (옵션)
    pub on_complete: Option<String>,
}

impl Default for SpriteAnimator {
    fn default() -> Self {
        Self {
            current_clip: "idle".to_string(),
            frame_index: 0,
            elapsed: 0.0,
            speed: 1.0,
            playing: true,
            on_complete: None,
        }
    }
}

impl SpriteAnimator {
    pub fn new(clip_name: impl Into<String>) -> Self {
        Self {
            current_clip: clip_name.into(),
            ..Default::default()
        }
    }

    /// 클립 재생
    pub fn play(&mut self, clip_name: &str) {
        if self.current_clip != clip_name {
            self.current_clip = clip_name.to_string();
            self.frame_index = 0;
            self.elapsed = 0.0;
        }
        self.playing = true;
    }

    /// 일시 정지
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// 재생 재개
    pub fn resume(&mut self) {
        self.playing = true;
    }

    /// 정지 및 초기화
    pub fn stop(&mut self) {
        self.playing = false;
        self.frame_index = 0;
        self.elapsed = 0.0;
    }

    /// 프레임 리셋
    pub fn reset(&mut self) {
        self.frame_index = 0;
        self.elapsed = 0.0;
    }

    /// 재생 속도 설정
    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    /// 완료 콜백 설정
    pub fn with_on_complete(mut self, event_name: impl Into<String>) -> Self {
        self.on_complete = Some(event_name.into());
        self
    }
}

// ============ Post Processing Components ============

/// 톤매핑 모드
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Tonemapping {
    #[default]
    Aces,
    Reinhard,
    Filmic,
    None,
}

/// 블룸 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BloomSettings {
    pub intensity: f32,
    pub threshold: f32,
    pub knee: f32,
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self {
            intensity: 0.5,
            threshold: 1.0,
            knee: 0.5,
        }
    }
}

/// 아웃라인 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutlineSettings {
    pub color: [f32; 3],
    pub strength: f32,
}

impl Default for OutlineSettings {
    fn default() -> Self {
        Self {
            color: [0.02, 0.01, 0.01],
            strength: 0.7,
        }
    }
}

/// 포스트 프로세스 설정 컴포넌트 (카메라에 붙임)
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PostProcess {
    pub exposure: f32,
    pub gamma: f32,
    pub tonemapping: Tonemapping,
    pub bloom: Option<BloomSettings>,
    pub outline: Option<OutlineSettings>,
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for PostProcess {
    fn default() -> Self {
        Self {
            exposure: 1.5,
            gamma: 2.2,
            tonemapping: Tonemapping::Aces,
            bloom: None,
            outline: Some(OutlineSettings::default()),
            saturation: 1.0,
            contrast: 1.0,
        }
    }
}

impl PostProcess {
    /// 기본 설정 (블룸 + 아웃라인)
    pub fn with_bloom(mut self) -> Self {
        self.bloom = Some(BloomSettings::default());
        self
    }

    /// 노출값 설정
    pub fn with_exposure(mut self, exposure: f32) -> Self {
        self.exposure = exposure;
        self
    }

    /// 아웃라인 비활성화
    pub fn without_outline(mut self) -> Self {
        self.outline = None;
        self
    }
}

// ============ Integrated Animation System ============

/// 통합 애니메이터 컴포넌트 (엔티티별 독립)
/// AnimatorController + AnimationMixer + StateMachine 통합
/// 다중 엔티티 애니메이션 지원 (기존 Resource 싱글톤 문제 해결)
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
    pub parameters: std::collections::HashMap<String, AnimatorParameter>,
    /// 상태 정의 (이름 → 애니메이션 인덱스)
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
    pub ai_state_mappings: std::collections::HashMap<AiStateType, AiAnimationMapping>,
    /// AI 연동 활성화
    pub ai_sync_enabled: bool,
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
    pub fn evaluate(&self, params: &std::collections::HashMap<String, AnimatorParameter>) -> bool {
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

impl Default for AnimatorController {
    fn default() -> Self {
        Self {
            model_name: String::new(),
            enabled: true,
            speed: 1.0,
            current_state: 0,
            previous_state: 0,
            parameters: std::collections::HashMap::new(),
            states: Vec::new(),
            transitions: Vec::new(),
            current_time: 0.0,
            in_transition: false,
            transition_progress: 0.0,
            transition_duration: 0.25,
            ai_state_mappings: std::collections::HashMap::new(),
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
        // 현재 상태에서 나가는 전이들 검사
        let mut matched_transition: Option<(usize, f32, Vec<String>)> = None;

        for transition in &self.transitions {
            if transition.from_state != self.current_state {
                continue;
            }

            // 모든 조건 충족 확인
            let all_conditions_met = transition.conditions.iter()
                .all(|c| c.evaluate(&self.parameters));

            if all_conditions_met {
                // Trigger 파라미터 이름 수집
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

        // 전이 적용 (borrow 문제 회피)
        if let Some((to_state, duration, triggers)) = matched_transition {
            self.previous_state = self.current_state;
            self.current_state = to_state;
            self.in_transition = true;
            self.transition_progress = 0.0;
            self.transition_duration = duration;
            self.current_time = 0.0;

            // Trigger 소비
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

        // 전이 조건 검사
        if !self.in_transition {
            self.check_transitions();
        }

        // 전이 진행
        if self.in_transition {
            self.transition_progress += dt / self.transition_duration;
            if self.transition_progress >= 1.0 {
                self.transition_progress = 1.0;
                self.in_transition = false;
            }
        }

        // 현재 상태 시간 진행
        let state_speed = self.states.get(self.current_state)
            .map(|s| s.speed)
            .unwrap_or(1.0);

        self.current_time += dt * self.speed * state_speed;

        // 루프 처리
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
    ///
    /// # Arguments
    /// * `animation_names` - 애니메이션 클립 이름 목록 (GLTF에서 로드됨)
    ///
    /// # Example
    /// ```
    /// let animator = AnimatorController::new("Fox")
    ///     .with_animations(&["Walk".to_string(), "Run".to_string(), "Idle".to_string()])
    ///     .with_default_ai_mappings();
    /// ```
    pub fn with_animations(mut self, animation_names: &[String]) -> Self {
        for (i, name) in animation_names.iter().enumerate() {
            self.states.push(AnimatorState {
                name: name.clone(),
                animation_index: i,
                looping: true,  // 기본적으로 루프
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
            // 파라미터 설정
            for (name, value) in &mapping.parameters {
                match value {
                    AnimatorParameter::Bool(v) => self.set_bool(name, *v),
                    AnimatorParameter::Float(v) => self.set_float(name, *v),
                    AnimatorParameter::Int(v) => self.set_int(name, *v),
                    AnimatorParameter::Trigger(_) => self.set_trigger(name),
                }
            }

            // 상태 전이
            if !mapping.target_state.is_empty() {
                let duration = mapping.transition_duration.unwrap_or(0.25);
                self.transition_to(&mapping.target_state, duration);
            }
        }
    }

    /// 현재 블렌딩 가중치 계산 (크로스페이드용)
    /// 반환: (이전 상태 가중치, 현재 상태 가중치)
    pub fn blend_weights(&self) -> (f32, f32) {
        if self.in_transition {
            let t = self.transition_progress;
            // 스무스 블렌딩 (smoothstep)
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

        // 과치유 방지
        health.heal(1000.0);
        assert_eq!(health.current, health.maximum);
    }

    #[test]
    fn test_weapon_component() {
        let mut weapon = Weapon::default();
        assert_eq!(weapon.ammo, 30);

        // 초기 상태에서는 쿨다운 시간이 0이라 발사 불가
        assert!(!weapon.can_fire());

        // 쿨다운 후 발사 가능
        weapon.time_since_fire = weapon.fire_rate;
        assert!(weapon.can_fire());
        assert!(weapon.fire());
        assert_eq!(weapon.ammo, 29);
        assert_eq!(weapon.time_since_fire, 0.0);  // 발사 후 리셋

        // 쿨다운 전에는 발사 불가
        assert!(!weapon.can_fire());

        // 탄약 소진
        weapon.ammo = 0;
        weapon.time_since_fire = weapon.fire_rate;
        assert!(!weapon.can_fire());
        assert!(!weapon.fire());

        // 재장전
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
