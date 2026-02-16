//! Item and Trigger Components for SKOPE Engine

use skope_ecs::prelude::*;
use serde::{Serialize, Deserialize};

/// 아이템 타입
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub item_id: String,
    pub item_type: ItemType,
    #[serde(skip, default)]
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

/// 트리거 존 컴포넌트
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub event_name: String,
    #[serde(default = "crate::default_true")]
    pub is_active: bool,
    #[serde(skip, default)]
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
