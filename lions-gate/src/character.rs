//! Hero classes — Christian ARPG, **no mana**.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeroClass {
    /// Front-line armored fighter.
    Knight,
    /// Fast scout / ranger of the paths.
    Pathfinder,
    /// Balanced guardian of the flock — prayer cooldown, not a mana bar.
    Shepherd,
}

impl HeroClass {
    pub fn all() -> &'static [HeroClass] {
        &[Self::Knight, Self::Pathfinder, Self::Shepherd]
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Knight => "Knight",
            Self::Pathfinder => "Pathfinder",
            Self::Shepherd => "Shepherd",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::Knight => "Heavy armor · strong melee · holds the gate.",
            Self::Pathfinder => "Swift · high critical strike · maps the wild.",
            Self::Shepherd => "Steady faith · prayer cooldown heals allies · no mana pool.",
        }
    }

    pub fn max_health(self) -> f32 {
        match self {
            Self::Knight => 140.0,
            Self::Pathfinder => 100.0,
            Self::Shepherd => 120.0,
        }
    }

    pub fn damage(self) -> f32 {
        match self {
            Self::Knight => 18.0,
            Self::Pathfinder => 14.0,
            Self::Shepherd => 12.0,
        }
    }

    pub fn move_speed(self) -> f32 {
        match self {
            Self::Knight => 165.0,
            Self::Pathfinder => 220.0,
            Self::Shepherd => 180.0,
        }
    }

    /// Seconds between prayer / special (not a mana resource).
    pub fn prayer_cooldown(self) -> f32 {
        match self {
            Self::Knight => 6.0,
            Self::Pathfinder => 5.0,
            Self::Shepherd => 4.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hero {
    pub name: String,
    pub class: HeroClass,
    pub health: f32,
    pub max_health: f32,
    pub level: u32,
    pub xp: u32,
    pub xp_to_level: u32,
    pub gold: u32,
    /// Prayer / special remaining cooldown (seconds). Not mana.
    pub prayer_cd: f32,
}

impl Hero {
    pub fn new(name: impl Into<String>, class: HeroClass) -> Self {
        let max_health = class.max_health();
        Self {
            name: name.into(),
            class,
            health: max_health,
            max_health,
            level: 1,
            xp: 0,
            xp_to_level: 100,
            gold: 25,
            prayer_cd: 0.0,
        }
    }

    pub fn alive(&self) -> bool {
        self.health > 0.0
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.xp += amount;
        while self.xp >= self.xp_to_level {
            self.xp -= self.xp_to_level;
            self.level += 1;
            self.xp_to_level = 100 + self.level * 40;
            self.max_health += 12.0;
            self.health = self.max_health;
        }
    }
}
