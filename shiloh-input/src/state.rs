//! Double-buffered input snapshot (current vs previous frame).

use ahash::AHashSet;
use glam::Vec2;

use crate::device::{KeyCode, MouseButton};

#[derive(Debug, Default, Clone)]
struct FrameButtons {
    keys: AHashSet<KeyCode>,
    mouse: AHashSet<MouseButton>,
}

#[derive(Debug, Default)]
pub struct InputState {
    current: FrameButtons,
    previous: FrameButtons,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub scroll_delta: Vec2,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call once at the start of each frame before pumping events.
    pub fn begin_frame(&mut self) {
        self.previous = self.current.clone();
        self.mouse_delta = Vec2::ZERO;
        self.scroll_delta = Vec2::ZERO;
    }

    pub fn key_down(&mut self, key: KeyCode) {
        self.current.keys.insert(key);
    }

    pub fn key_up(&mut self, key: KeyCode) {
        self.current.keys.remove(&key);
    }

    pub fn mouse_down(&mut self, button: MouseButton) {
        self.current.mouse.insert(button);
    }

    pub fn mouse_up(&mut self, button: MouseButton) {
        self.current.mouse.remove(&button);
    }

    pub fn set_mouse_position(&mut self, pos: Vec2) {
        self.mouse_delta += pos - self.mouse_position;
        self.mouse_position = pos;
    }

    #[inline]
    pub fn is_key_down(&self, key: KeyCode) -> bool {
        self.current.keys.contains(&key)
    }

    #[inline]
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.current.keys.contains(&key) && !self.previous.keys.contains(&key)
    }

    #[inline]
    pub fn is_key_released(&self, key: KeyCode) -> bool {
        !self.current.keys.contains(&key) && self.previous.keys.contains(&key)
    }

    #[inline]
    pub fn is_mouse_down(&self, button: MouseButton) -> bool {
        self.current.mouse.contains(&button)
    }
}
