//! Contiguous first-level world: Town · Forest · Swamp.

use glam::Vec2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZoneId {
    Town,
    Forest,
    Swamp,
}

impl ZoneId {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Town => "Lion's Haven (Town)",
            Self::Forest => "Valley Forest",
            Self::Swamp => "Blackmarsh Edge",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Town => "Safe haven · vendors · Bible lectern · campaign board.",
            Self::Forest => "Pines and paths · lesser foes · first loot.",
            Self::Swamp => "Fog and corruption · harder foes · better drops.",
        }
    }

    pub fn ground_color(self) -> [u8; 3] {
        match self {
            Self::Town => [72, 68, 58],
            Self::Forest => [34, 72, 42],
            Self::Swamp => [42, 58, 48],
        }
    }

    pub fn neighbors(self) -> &'static [ZoneId] {
        match self {
            Self::Town => &[ZoneId::Forest],
            Self::Forest => &[ZoneId::Town, ZoneId::Swamp],
            Self::Swamp => &[ZoneId::Forest],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Portal {
    pub to: ZoneId,
    pub pos: Vec2,
    pub label: &'static str,
}

#[derive(Debug, Clone)]
pub struct Enemy {
    pub id: u64,
    pub name: &'static str,
    pub pos: Vec2,
    pub health: f32,
    pub max_health: f32,
    pub damage: f32,
    pub xp: u32,
    pub evil: bool,
}

#[derive(Debug, Clone)]
pub struct Npc {
    pub name: &'static str,
    pub pos: Vec2,
    pub line: &'static str,
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub id: ZoneId,
    pub size: Vec2,
    pub portals: Vec<Portal>,
    pub enemies: Vec<Enemy>,
    pub npcs: Vec<Npc>,
    pub spawn: Vec2,
}

impl Zone {
    pub fn build(id: ZoneId) -> Self {
        match id {
            ZoneId::Town => Self {
                id,
                size: Vec2::new(900.0, 700.0),
                spawn: Vec2::new(450.0, 520.0),
                portals: vec![Portal {
                    to: ZoneId::Forest,
                    pos: Vec2::new(450.0, 80.0),
                    label: "→ Valley Forest",
                }],
                enemies: Vec::new(),
                npcs: vec![
                    Npc {
                        name: "Elder Miriam",
                        pos: Vec2::new(320.0, 400.0),
                        line: "Be strong in the Lord (Eph 6:10). The forest stirs — walk in light.",
                    },
                    Npc {
                        name: "Armorer Caleb",
                        pos: Vec2::new(580.0, 420.0),
                        line: "Bring back what the wild drops. Faith sharpens steel.",
                    },
                    Npc {
                        name: "Lectern",
                        pos: Vec2::new(450.0, 300.0),
                        line: "Open the Bible from the menu or press B.",
                    },
                ],
            },
            ZoneId::Forest => Self {
                id,
                size: Vec2::new(1100.0, 850.0),
                spawn: Vec2::new(200.0, 700.0),
                portals: vec![
                    Portal {
                        to: ZoneId::Town,
                        pos: Vec2::new(120.0, 760.0),
                        label: "← Lion's Haven",
                    },
                    Portal {
                        to: ZoneId::Swamp,
                        pos: Vec2::new(980.0, 200.0),
                        label: "→ Blackmarsh Edge",
                    },
                ],
                enemies: vec![
                    Enemy {
                        id: 1,
                        name: "Shadow Wolf",
                        pos: Vec2::new(420.0, 480.0),
                        health: 40.0,
                        max_health: 40.0,
                        damage: 8.0,
                        xp: 18,
                        evil: true,
                    },
                    Enemy {
                        id: 2,
                        name: "Briar Bandit",
                        pos: Vec2::new(700.0, 360.0),
                        health: 55.0,
                        max_health: 55.0,
                        damage: 10.0,
                        xp: 24,
                        evil: true,
                    },
                    Enemy {
                        id: 3,
                        name: "Fallen Scout",
                        pos: Vec2::new(860.0, 560.0),
                        health: 48.0,
                        max_health: 48.0,
                        damage: 9.0,
                        xp: 20,
                        evil: true,
                    },
                ],
                npcs: vec![Npc {
                    name: "Waystone",
                    pos: Vec2::new(250.0, 650.0),
                    line: "Town behind you. Marsh beyond the pines. Stay the path.",
                }],
            },
            ZoneId::Swamp => Self {
                id,
                size: Vec2::new(1000.0, 800.0),
                spawn: Vec2::new(120.0, 400.0),
                portals: vec![Portal {
                    to: ZoneId::Forest,
                    pos: Vec2::new(80.0, 400.0),
                    label: "← Valley Forest",
                }],
                enemies: vec![
                    Enemy {
                        id: 10,
                        name: "Marsh Wraith",
                        pos: Vec2::new(480.0, 360.0),
                        health: 70.0,
                        max_health: 70.0,
                        damage: 14.0,
                        xp: 36,
                        evil: true,
                    },
                    Enemy {
                        id: 11,
                        name: "Bog Brute",
                        pos: Vec2::new(700.0, 520.0),
                        health: 95.0,
                        max_health: 95.0,
                        damage: 16.0,
                        xp: 42,
                        evil: true,
                    },
                    Enemy {
                        id: 12,
                        name: "Corrupt Watcher",
                        pos: Vec2::new(620.0, 220.0),
                        health: 80.0,
                        max_health: 80.0,
                        damage: 15.0,
                        xp: 40,
                        evil: true,
                    },
                ],
                npcs: vec![Npc {
                    name: "Ruined Cross",
                    pos: Vec2::new(300.0, 300.0),
                    line: "Even here, the Word stands. Clear the marsh of evil.",
                }],
            },
        }
    }
}

/// Shared campaign map metadata (all zones = one bigger world).
#[derive(Debug, Clone, Default)]
pub struct WorldMap {
    pub discovered: Vec<ZoneId>,
}

impl WorldMap {
    pub fn discover(&mut self, z: ZoneId) {
        if !self.discovered.contains(&z) {
            self.discovered.push(z);
        }
    }
}
