//! Scene/entity script attachment metadata (Phase 5).

use serde::{Deserialize, Serialize};

/// Which scripting backend a [`ScriptComponent`] refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptKind {
    Rhai,
    Visual,
}

/// Component linking an entity to a script asset path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub path: String,
    pub kind: ScriptKind,
}

impl ScriptComponent {
    pub fn rhai(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: ScriptKind::Rhai,
        }
    }

    pub fn visual(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            kind: ScriptKind::Visual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde_roundtrip() {
        let c = ScriptComponent::rhai("scripts/player.rhai");
        let json = serde_json::to_string(&c).unwrap();
        let back: ScriptComponent = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.kind, ScriptKind::Rhai);
    }
}
