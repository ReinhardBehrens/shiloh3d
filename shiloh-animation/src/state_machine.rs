//! Simple animation state machine.

#[derive(Debug, Clone)]
pub struct AnimState {
    pub name: String,
    pub clip_index: usize,
}

#[derive(Debug, Clone)]
pub struct AnimTransition {
    pub from: usize,
    pub to: usize,
    pub duration: f32,
}

#[derive(Debug, Default)]
pub struct AnimStateMachine {
    pub states: Vec<AnimState>,
    pub transitions: Vec<AnimTransition>,
    pub current: usize,
}

impl AnimStateMachine {
    pub fn goto(&mut self, state: usize) {
        if state < self.states.len() {
            self.current = state;
        }
    }
}
