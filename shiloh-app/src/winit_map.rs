//! Maps winit keys/buttons → `shiloh-input` (shared host path).

use shiloh_input::{KeyCode, MouseButton};
use winit::event::MouseButton as WinitMouse;
use winit::keyboard::KeyCode as WinitKey;

pub fn map_key(code: WinitKey) -> KeyCode {
    match code {
        WinitKey::KeyA => KeyCode::A,
        WinitKey::KeyB => KeyCode::B,
        WinitKey::KeyC => KeyCode::C,
        WinitKey::KeyD => KeyCode::D,
        WinitKey::KeyE => KeyCode::E,
        WinitKey::KeyF => KeyCode::F,
        WinitKey::KeyG => KeyCode::G,
        WinitKey::KeyH => KeyCode::H,
        WinitKey::KeyI => KeyCode::I,
        WinitKey::KeyJ => KeyCode::J,
        WinitKey::KeyK => KeyCode::K,
        WinitKey::KeyL => KeyCode::L,
        WinitKey::KeyM => KeyCode::M,
        WinitKey::KeyN => KeyCode::N,
        WinitKey::KeyO => KeyCode::O,
        WinitKey::KeyP => KeyCode::P,
        WinitKey::KeyQ => KeyCode::Q,
        WinitKey::KeyR => KeyCode::R,
        WinitKey::KeyS => KeyCode::S,
        WinitKey::KeyT => KeyCode::T,
        WinitKey::KeyU => KeyCode::U,
        WinitKey::KeyV => KeyCode::V,
        WinitKey::KeyW => KeyCode::W,
        WinitKey::KeyX => KeyCode::X,
        WinitKey::KeyY => KeyCode::Y,
        WinitKey::KeyZ => KeyCode::Z,
        WinitKey::Space => KeyCode::Space,
        WinitKey::Escape => KeyCode::Escape,
        WinitKey::Enter => KeyCode::Enter,
        WinitKey::ShiftLeft => KeyCode::LeftShift,
        WinitKey::ControlLeft => KeyCode::LeftCtrl,
        WinitKey::ArrowUp => KeyCode::ArrowUp,
        WinitKey::ArrowDown => KeyCode::ArrowDown,
        WinitKey::ArrowLeft => KeyCode::ArrowLeft,
        WinitKey::ArrowRight => KeyCode::ArrowRight,
        WinitKey::Digit1 => KeyCode::Digit1,
        WinitKey::Digit2 => KeyCode::Digit2,
        WinitKey::Digit3 => KeyCode::Digit3,
        _ => KeyCode::Unknown,
    }
}

pub fn map_mouse(button: WinitMouse) -> MouseButton {
    match button {
        WinitMouse::Left => MouseButton::Left,
        WinitMouse::Right => MouseButton::Right,
        WinitMouse::Middle => MouseButton::Middle,
        WinitMouse::Back | WinitMouse::Forward | WinitMouse::Other(_) => MouseButton::Other(0),
    }
}
