//! keyboard.rs - Utility for tracking Winit keyboard events and turning them
//! into useful data.

#![allow(dead_code)]

use std::collections::HashSet;
use winit::event::{ElementState, KeyboardInput, ModifiersState, VirtualKeyCode};

/// Tracks keyboard presses.
pub struct KeyboardTracker {
    pressed_keys: HashSet<VirtualKeyCode>,
    released_keys: HashSet<VirtualKeyCode>,
    modifiers: Modifiers,
}

/// Tracks keyboard modifier presses.
pub struct Modifiers {
    /// Whether the 'Shift' key is pressed.
    pub shift: bool,
    /// Whether the 'Ctrl' key is pressed.
    pub ctrl: bool,
    /// Whether the 'Alt' key (Windows & Linux) or 'Option' key (Mac) is
    /// pressed.
    pub alt: bool,
    /// Whether the Logo key (Windows key on Windows, Command key on Mac) is
    /// pressed.
    pub logo: bool,
    /// Whether the OS-appropriate command/ctrl key is pressed.
    pub command: bool,
}

impl KeyboardTracker {
    pub fn new() -> KeyboardTracker {
        KeyboardTracker {
            pressed_keys: Default::default(),
            released_keys: Default::default(),
            modifiers: Modifiers {
                shift: false,
                ctrl: false,
                alt: false,
                logo: false,
                command: false,
            },
        }
    }

    pub fn keyboard_input(&mut self, input: &KeyboardInput) {
        if let Some(keycode) = input.virtual_keycode {
            match input.state {
                ElementState::Pressed => self.pressed_keys.insert(keycode),
                ElementState::Released => self.released_keys.insert(keycode),
            };
        }
    }

    pub fn reset_keyboard_input(&mut self) {
        self.pressed_keys.clear();
        self.released_keys.clear();
    }

    pub fn modifiers_changed(&mut self, modifiers_state: &ModifiersState) {
        self.modifiers.update(modifiers_state);
    }

    pub fn modifiers(&self) -> &Modifiers {
        &self.modifiers
    }

    pub fn was_pressed(&self, key: VirtualKeyCode) -> bool {
        self.pressed_keys.contains(&key)
    }

    pub fn was_released(&self, key: VirtualKeyCode) -> bool {
        self.released_keys.contains(&key)
    }
}

impl Modifiers {
    fn update(&mut self, state: &ModifiersState) {
        self.shift = state.shift();
        self.ctrl = state.ctrl();
        self.alt = state.alt();
        self.logo = state.logo();

        #[cfg(not(target_arch = "macos"))]
        {
            self.command = self.ctrl;
        }

        #[cfg(target_arch = "macos")]
        {
            self.command = self.logo;
        }
    }
}
