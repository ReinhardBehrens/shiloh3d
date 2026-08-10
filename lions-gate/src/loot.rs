//! Diablo-style loot loop — drops, rarity, inventory (no mana potions as primary).

use glam::Vec2;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Blessed,
}

impl Rarity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Common => "Common",
            Self::Uncommon => "Uncommon",
            Self::Rare => "Rare",
            Self::Blessed => "Blessed",
        }
    }

    pub fn color(self) -> egui::Color32 {
        match self {
            Self::Common => egui::Color32::from_rgb(200, 200, 200),
            Self::Uncommon => egui::Color32::from_rgb(80, 200, 120),
            Self::Rare => egui::Color32::from_rgb(80, 140, 255),
            Self::Blessed => egui::Color32::from_rgb(240, 200, 80),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemKind {
    Weapon,
    Armor,
    Relic,
    Provision,
    GoldPile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub name: String,
    pub kind: ItemKind,
    pub rarity: Rarity,
    pub power: i32,
    pub gold_value: u32,
}

#[derive(Debug, Clone)]
pub struct GroundLoot {
    pub item: Item,
    pub pos: Vec2,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<Option<Item>>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            capacity,
        }
    }

    pub fn try_add(&mut self, item: Item) -> Result<(), Item> {
        if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
            *slot = Some(item);
            Ok(())
        } else {
            Err(item)
        }
    }

    pub fn occupied(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}

pub fn roll_drop<R: Rng>(rng: &mut R, swamp: bool) -> Item {
    let roll: f32 = rng.r#gen();
    let rarity = if swamp {
        if roll < 0.45 {
            Rarity::Common
        } else if roll < 0.75 {
            Rarity::Uncommon
        } else if roll < 0.93 {
            Rarity::Rare
        } else {
            Rarity::Blessed
        }
    } else if roll < 0.60 {
        Rarity::Common
    } else if roll < 0.88 {
        Rarity::Uncommon
    } else if roll < 0.98 {
        Rarity::Rare
    } else {
        Rarity::Blessed
    };

    let kind_roll: u8 = rng.gen_range(0..5);
    let (kind, name, power, gold) = match kind_roll {
        0 => (
            ItemKind::Weapon,
            match rarity {
                Rarity::Blessed => "Sword of the Word",
                Rarity::Rare => "Tempered Blade",
                Rarity::Uncommon => "Iron Longsword",
                Rarity::Common => "Worn Shortsword",
            },
            match rarity {
                Rarity::Blessed => 28,
                Rarity::Rare => 18,
                Rarity::Uncommon => 12,
                Rarity::Common => 6,
            },
            match rarity {
                Rarity::Blessed => 120,
                Rarity::Rare => 60,
                Rarity::Uncommon => 25,
                Rarity::Common => 8,
            },
        ),
        1 => (
            ItemKind::Armor,
            match rarity {
                Rarity::Blessed => "Mail of Ephod",
                Rarity::Rare => "Reinforced Hauberk",
                Rarity::Uncommon => "Leather Jack",
                Rarity::Common => "Padded Vest",
            },
            match rarity {
                Rarity::Blessed => 22,
                Rarity::Rare => 14,
                Rarity::Uncommon => 8,
                Rarity::Common => 4,
            },
            match rarity {
                Rarity::Blessed => 100,
                Rarity::Rare => 50,
                Rarity::Uncommon => 20,
                Rarity::Common => 6,
            },
        ),
        2 => (
            ItemKind::Relic,
            match rarity {
                Rarity::Blessed => "Relic: Lion Seal",
                Rarity::Rare => "Prayer Cord",
                Rarity::Uncommon => "Wooden Cross",
                Rarity::Common => "Cloth Bookmark",
            },
            match rarity {
                Rarity::Blessed => 16,
                Rarity::Rare => 10,
                Rarity::Uncommon => 5,
                Rarity::Common => 2,
            },
            match rarity {
                Rarity::Blessed => 90,
                Rarity::Rare => 40,
                Rarity::Uncommon => 15,
                Rarity::Common => 5,
            },
        ),
        3 => (
            ItemKind::Provision,
            "Travel Bread",
            0,
            4,
        ),
        _ => (ItemKind::GoldPile, "Coin Purse", 0, rng.gen_range(5..40)),
    };

    Item {
        name: name.into(),
        kind,
        rarity,
        power,
        gold_value: gold,
    }
}
