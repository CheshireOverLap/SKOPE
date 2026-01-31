//! Inventory Components
//!
//! 아이템 및 인벤토리 관련 컴포넌트

use bevy_ecs::prelude::*;
use super::gameplay::ItemType;

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
