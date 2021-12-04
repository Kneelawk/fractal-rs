//! This module handles keyboard shortcuts.

#![allow(dead_code)]

use crate::gui::keyboard::{macros::shortcut, ShortcutType::*};
use heck::TitleCase;
use itertools::Itertools;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Write},
};
use winit::event::{ModifiersState, VirtualKeyCode};

pub mod macros;
pub mod tracker;

/// The list of shortcuts in the app.
const SHORTCUT_LIST: &[(ShortcutType, Shortcut)] = &[
    (App_Quit, shortcut!(Cmd - Q)),
    (App_New, shortcut!(Cmd - N)),
    (App_CloseTab, shortcut!(Cmd - W)),
    (App_Fullscreen, shortcut!(F11)),
    (App_AlternateExitFullscreen, shortcut!(Escape)),
    (Instance_Generate, shortcut!(MacAlt - G)),
    (Instance_SpawnJulia, shortcut!(MacAlt - J)),
    (Instance_SwitchToJulia, shortcut!(Shift - MacAlt - J)),
    (Instance_SwitchToMandelbrot, shortcut!(MacAlt - M)),
];

/// This enum contains an entry for each keyboard shortcut the application uses.
#[allow(non_camel_case_types)]
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, EnumIter)]
pub enum ShortcutType {
    // App shortcuts
    App_Quit,
    App_New,
    App_CloseTab,
    App_Fullscreen,
    App_AlternateExitFullscreen,

    // Instance shortcuts
    Instance_Generate,
    Instance_SpawnJulia,
    Instance_SwitchToJulia,
    Instance_SwitchToMandelbrot,
}

/// Tracks keyboard modifier presses.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
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
}

impl Modifiers {
    fn update(&mut self, state: &ModifiersState) {
        self.shift = state.shift();
        self.ctrl = state.ctrl();
        self.alt = state.alt();
        self.logo = state.logo();
    }

    pub fn command(&self) -> bool {
        #[cfg(not(target_arch = "macos"))]
        return self.ctrl;

        #[cfg(target_arch = "macos")]
        return self.logo;
    }
}

/// Describes a particular keyboard shortcut.
#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Shortcut {
    pub modifiers: Modifiers,
    pub key: VirtualKeyCode,
}

/// This struct holds a mapping from keyboard shortcuts to shortcut enum values.
pub struct ShortcutMap {
    map: HashMap<Shortcut, ShortcutType>,
}

impl ShortcutMap {
    pub fn from(shortcut_list: &[(ShortcutType, Shortcut)]) -> (ShortcutMap, ShortcutMapConflicts) {
        let mut map = HashMap::new();
        let mut names = HashMap::new();
        let mut binding_conflicts = HashMap::new();
        let mut name_conflicts = HashMap::new();

        debug!(
            "Loading shortcuts: [\n\t{}\n]",
            shortcut_list
                .iter()
                .map(|(ty, sc)| format!("{} => {}", ty, sc))
                .format(",\n\t")
        );

        for (name, binding) in shortcut_list {
            if map.contains_key(binding) {
                let conflicts = binding_conflicts.entry(*binding).or_insert(HashSet::new());
                conflicts.insert(*map.get(binding).unwrap());
                conflicts.insert(*name);
            }
            if names.contains_key(name) {
                let conflicts = name_conflicts.entry(*name).or_insert(HashSet::new());
                conflicts.insert(*names.get(name).unwrap());
                conflicts.insert(*binding);
            }

            map.insert(*binding, *name);
            names.insert(*name, *binding);
        }

        let mut conflicts = vec![];
        for (binding, names) in binding_conflicts {
            conflicts.push(ShortcutConflict::Binding {
                binding,
                names: names.into_iter().sorted().collect(),
            });
        }
        for (name, bindings) in name_conflicts {
            conflicts.push(ShortcutConflict::Name {
                name,
                bindings: bindings.into_iter().sorted().collect(),
            });
        }

        (ShortcutMap { map }, ShortcutMapConflicts { conflicts })
    }

    pub fn new() -> (ShortcutMap, ShortcutMapConflicts) {
        Self::from(SHORTCUT_LIST)
    }

    pub fn lookup(&self, keys: Option<Shortcut>) -> Option<ShortcutType> {
        keys.and_then(|shortcut| self.map.get(&shortcut).map(|ty| *ty))
    }
}

pub trait ShortcutTypeExt {
    fn is(&self, ty: ShortcutType) -> bool;
}

impl ShortcutTypeExt for Option<ShortcutType> {
    fn is(&self, ty: ShortcutType) -> bool {
        if let Some(self_ty) = self {
            *self_ty == ty
        } else {
            false
        }
    }
}

/// Represents an error while creating a shortcut map.
#[derive(Debug, Error)]
pub struct ShortcutMapConflicts {
    pub conflicts: Vec<ShortcutConflict>,
}

/// Represents a keyboard shortcut conflict.
#[derive(Debug)]
pub enum ShortcutConflict {
    /// Indicates that two shortcut name enums have the same keyboard shortcut
    /// binding.
    Binding {
        binding: Shortcut,
        names: Vec<ShortcutType>,
    },
    /// Indicates that two shortcut bindings refer to the same shortcut name.
    Name {
        name: ShortcutType,
        bindings: Vec<Shortcut>,
    },
}

impl Display for ShortcutMapConflicts {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Shortcut conflicts encountered: [\n\t{}\n]",
            self.conflicts.iter().format(",\n\t")
        )
    }
}

impl Display for ShortcutConflict {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ShortcutConflict::Binding { binding, names } => {
                write!(
                    f,
                    "Binding {} is bound to multiple shortcuts: {}",
                    binding,
                    names.iter().format(", ")
                )
            },
            ShortcutConflict::Name { name, bindings } => {
                write!(
                    f,
                    "Shortcut {} is has multiple bindings: {}",
                    name,
                    bindings.iter().format(", ")
                )
            },
        }
    }
}

impl Display for ShortcutType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let old_name = format!("{:?}", self);

        write!(
            f,
            "{}",
            old_name.split("_").map(|s| s.to_title_case()).format("/")
        )
    }
}

impl Display for Shortcut {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut s = String::new();

        if self.modifiers.ctrl {
            write!(s, "Ctrl + ")?;
        }

        if self.modifiers.logo {
            #[cfg(target_os = "macos")]
            write!(s, "Cmd + ")?;

            #[cfg(not(target_os = "macos"))]
            write!(s, "Win + ")?;
        }

        if self.modifiers.shift {
            write!(s, "Shift + ")?;
        }

        if self.modifiers.alt {
            write!(s, "Alt + ")?;
        }

        write!(s, "{:?}", self.key)?;

        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod test {
    use crate::gui::keyboard::ShortcutMap;

    #[test]
    fn test_no_conflicts() {
        let (_map, conflicts) = ShortcutMap::new();
        if !conflicts.conflicts.is_empty() {
            panic!("Conflicts: {}", conflicts);
        }
    }
}
