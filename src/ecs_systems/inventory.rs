//! Inventory System
//!
//! 인벤토리 관리 및 아이템 픽업 시스템

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::ecs_components::{
    Health, Inventory, ItemDef, ItemEffect, ItemType, Pickupable, Transform,
};
use crate::ecs_systems::ai::PlayerTag;

/// 아이템 레지스트리 (리소스)
#[derive(Resource, Default)]
pub struct ItemRegistry {
    items: HashMap<String, ItemDef>,
}

impl ItemRegistry {
    /// 새 레지스트리 생성
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_default_items();
        registry
    }

    /// 기본 아이템 등록
    fn register_default_items(&mut self) {
        // 힐링 포션들
        self.register(ItemDef::health_potion(30.0));
        self.register(
            ItemDef::new("health_potion_large", "Large Health Potion", ItemType::Consumable)
                .with_description("Restores a large amount of health")
                .with_max_stack(10)
                .with_effect(ItemEffect::Heal { amount: 75.0 })
                .with_prices(100, 50),
        );

        // 마나 포션
        self.register(
            ItemDef::new("mana_potion", "Mana Potion", ItemType::Consumable)
                .with_description("Restores mana")
                .with_max_stack(10)
                .with_effect(ItemEffect::ManaRestore { amount: 30.0 })
                .with_prices(50, 25),
        );

        // 골드 코인
        self.register(
            ItemDef::new("gold_coin", "Gold Coin", ItemType::Material)
                .with_description("Shiny gold coin")
                .with_max_stack(999)
                .with_prices(1, 1),
        );

        // 열쇠
        self.register(
            ItemDef::new("key_bronze", "Bronze Key", ItemType::Key)
                .with_description("Opens bronze locks")
                .with_max_stack(1),
        );
        self.register(
            ItemDef::new("key_silver", "Silver Key", ItemType::Key)
                .with_description("Opens silver locks")
                .with_max_stack(1),
        );
        self.register(
            ItemDef::new("key_gold", "Gold Key", ItemType::Key)
                .with_description("Opens gold locks")
                .with_max_stack(1),
        );
    }

    /// 아이템 등록
    pub fn register(&mut self, item: ItemDef) {
        self.items.insert(item.id.clone(), item);
    }

    /// 아이템 정보 가져오기
    pub fn get(&self, item_id: &str) -> Option<&ItemDef> {
        self.items.get(item_id)
    }

}

/// 아이템 자동 픽업 시스템
pub fn item_pickup_system(
    mut commands: Commands,
    mut player_query: Query<(&Transform, &mut Inventory), With<PlayerTag>>,
    pickup_query: Query<(Entity, &Transform, &Pickupable)>,
    registry: Res<ItemRegistry>,
) {
    let Ok((player_transform, mut inventory)) = player_query.get_single_mut() else {
        return;
    };

    let player_pos = player_transform.translation;

    for (entity, transform, pickupable) in pickup_query.iter() {
        // 자동 픽업만 처리
        if !pickupable.auto_pickup {
            continue;
        }

        let distance = player_pos.distance(transform.translation);
        if distance > pickupable.pickup_range {
            continue;
        }

        // 아이템 정보 조회
        let max_stack = registry
            .get(&pickupable.item_id)
            .map(|def| def.max_stack)
            .unwrap_or(99);

        // 인벤토리에 추가 시도
        if inventory.add_item(&pickupable.item_id, pickupable.count, max_stack).is_ok() {
            log::info!(
                "[Inventory] Picked up {} x{}",
                pickupable.item_id,
                pickupable.count
            );
            commands.entity(entity).despawn();
        }
    }
}

/// 아이템 사용 시스템 (체력 회복 등)
pub fn item_use_system(
    registry: Res<ItemRegistry>,
    mut query: Query<(&mut Inventory, Option<&mut Health>)>,
    mut use_events: EventReader<ItemUseEvent>,
) {
    for event in use_events.read() {
        let Ok((mut inventory, health)) = query.get_mut(event.entity) else {
            continue;
        };

        // 슬롯 사용
        let item_id = match inventory.use_slot(event.slot_index) {
            Ok(id) => id,
            Err(e) => {
                log::warn!("[Inventory] Failed to use slot {}: {:?}", event.slot_index, e);
                continue;
            }
        };

        // 아이템 효과 적용
        if let Some(item_def) = registry.get(&item_id) {
            if let Some(ref effect) = item_def.effect {
                apply_item_effect(effect, health);
                log::info!("[Inventory] Used item: {}", item_def.name);
            }
        }
    }
}

/// 아이템 효과 적용
fn apply_item_effect(effect: &ItemEffect, health: Option<Mut<Health>>) {
    match effect {
        ItemEffect::Heal { amount } => {
            if let Some(mut h) = health {
                h.heal(*amount);
            }
        }
        ItemEffect::ManaRestore { .. } => {
            // 마나 시스템 있으면 여기서 처리
        }
        ItemEffect::Buff { stat, value, duration } => {
            log::info!("[Inventory] Buff applied: {} +{} for {}s", stat, value, duration);
            // 버프 시스템과 연동
        }
        ItemEffect::Damage { amount } => {
            if let Some(mut h) = health {
                h.take_damage(*amount);
            }
        }
        ItemEffect::Custom { effect_id } => {
            log::info!("[Inventory] Custom effect: {}", effect_id);
            // Lua 콜백으로 처리
        }
    }
}

/// 아이템 사용 이벤트
#[derive(Event)]
pub struct ItemUseEvent {
    pub entity: Entity,
    pub slot_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inventory_add_remove() {
        let mut inventory = Inventory::new(5);

        // 아이템 추가
        assert!(inventory.add_item("health_potion", 3, 10).is_ok());
        assert!(inventory.has_item("health_potion", 3));
        assert_eq!(inventory.count_item("health_potion"), 3);

        // 아이템 제거
        assert!(inventory.remove_item("health_potion", 2).is_ok());
        assert_eq!(inventory.count_item("health_potion"), 1);

        // 부족한 수량 제거 시도
        assert!(inventory.remove_item("health_potion", 5).is_err());
    }

    #[test]
    fn test_inventory_stacking() {
        let mut inventory = Inventory::new(3);

        // 스택 가능 아이템 추가
        assert!(inventory.add_item("coin", 50, 100).is_ok());
        assert!(inventory.add_item("coin", 30, 100).is_ok());
        assert_eq!(inventory.count_item("coin"), 80);
        assert_eq!(inventory.empty_slots(), 2); // 하나의 슬롯에 80개

        // 스택 오버플로우 (100 넘으면 새 슬롯)
        assert!(inventory.add_item("coin", 50, 100).is_ok());
        assert_eq!(inventory.empty_slots(), 1);
    }

    #[test]
    fn test_item_registry() {
        let registry = ItemRegistry::new();

        assert!(registry.get("health_potion").is_some());
        assert!(registry.get("mana_potion").is_some());

        let potion = registry.get("health_potion").unwrap();
        assert_eq!(potion.name, "Health Potion");
        assert_eq!(potion.item_type, ItemType::Consumable);
    }
}
