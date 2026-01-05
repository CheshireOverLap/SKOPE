// ECS Components for SKOPE Engine
#![allow(dead_code)]

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
    pub range: f32,           // Point/Spot 전용
    pub spot_angle: f32,      // Spot 전용
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

// ============ AI Components ============

/// AI 상태 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
