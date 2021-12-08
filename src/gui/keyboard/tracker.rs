//! keyboard.rs - Utility for tracking Winit keyboard events and turning them
//! into useful data.

#![allow(dead_code)]

use crate::gui::keyboard::{Modifiers, Shortcut};
use itertools::Itertools;
use std::collections::HashSet;
use winit::event::{ElementState, KeyboardInput, ModifiersState, VirtualKeyCode};

/// Tracks keyboard presses.
pub struct KeyboardTracker {
    pressed_keys: HashSet<VirtualKeyCode>,
    released_keys: HashSet<VirtualKeyCode>,
    modifiers: Modifiers,
    current_shortcuts: Vec<Shortcut>,
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
            current_shortcuts: vec![],
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

    /// Updates this tracker's list of the currently pressed keys.
    pub fn update_shortcuts(&mut self) {
        self.current_shortcuts.clear();
        for key in self.pressed_keys.iter().sorted() {
            self.current_shortcuts.push(Shortcut {
                modifiers: self.modifiers,
                key: *key,
            });
        }
    }

    /// Gets this tracker's list of currently pressed keys.
    pub fn get_shortcuts(&self) -> &[Shortcut] {
        &self.current_shortcuts
    }
}
