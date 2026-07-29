//! Logical actions mapped from physical inputs.

use ahash::AHashMap;

use crate::device::KeyCode;
use crate::state::InputState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Action(pub &'static str);

#[derive(Debug, Default)]
pub struct ActionMap {
    keys: AHashMap<Action, KeyCode>,
}

impl ActionMap {
    pub fn bind_key(&mut self, action: Action, key: KeyCode) {
        self.keys.insert(action, key);
    }

    pub fn pressed(&self, input: &InputState, action: Action) -> bool {
        self.keys
            .get(&action)
            .is_some_and(|k| input.is_key_pressed(*k))
    }

    pub fn down(&self, input: &InputState, action: Action) -> bool {
        self.keys
            .get(&action)
            .is_some_and(|k| input.is_key_down(*k))
    }
}
