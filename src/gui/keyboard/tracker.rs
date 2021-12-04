//! keyboard.rs - Utility for tracking Winit keyboard events and turning them
//! into useful data.

#![allow(dead_code)]

use crate::gui::keyboard::{Modifiers, Shortcut};
use std::collections::HashSet;
use winit::event::{ElementState, KeyboardInput, ModifiersState, VirtualKeyCode};

/// Tracks keyboard presses.
pub struct KeyboardTracker {
    pressed_keys: HashSet<VirtualKeyCode>,
    released_keys: HashSet<VirtualKeyCode>,
    modifiers: Modifiers,
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

    /// Makes a shortcut for the currently pressed set of keys. This shortcut is
    /// then passed to a `ShortcutMap` to convert it into a shortcut enum.
    pub fn make_shortcut(&self) -> Option<Shortcut> {
        if self.pressed_keys.len() == 1 {
            Some(Shortcut {
                modifiers: self.modifiers,
                key: *self.pressed_keys.iter().next().unwrap(),
            })
        } else {
            None
        }
    }
}
