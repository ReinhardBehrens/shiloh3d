//! Prefab placeholder — serialized entity templates.

#[derive(Debug, Clone)]
pub struct Prefab {
    pub name: String,
}

impl Prefab {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}
