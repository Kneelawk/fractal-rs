//! This module handles keyboard shortcuts.

#![allow(dead_code)]

use crate::gui::keyboard::{macros::shortcut, ShortcutName::*};
use heck::TitleCase;
use itertools::Itertools;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter},
};
use winit::event::{ModifiersState, VirtualKeyCode};

pub mod macros;
pub mod tracker;

/// The list of shortcuts in the app.
const SHORTCUT_LIST: &[(ShortcutName, Shortcut)] = &[
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
pub enum ShortcutName {
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
    bindings: HashMap<Shortcut, Vec<ShortcutName>>,
    names: HashMap<ShortcutName, Vec<Shortcut>>,
    current_shortcuts: HashSet<ShortcutName>,
}

impl ShortcutMap {
    pub fn from(shortcut_list: &[(ShortcutName, Shortcut)]) -> (ShortcutMap, ShortcutMapConflicts) {
        let mut bindings: HashMap<_, Vec<ShortcutName>> = HashMap::new();
        let mut names: HashMap<_, Vec<Shortcut>> = HashMap::new();
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
            if bindings.contains_key(binding) {
                let conflicts = binding_conflicts.entry(*binding).or_insert(HashSet::new());

                let names = bindings.get(binding).unwrap();
                if names.len() == 1 {
                    conflicts.insert(names[0]);
                }

                conflicts.insert(*name);
            }
            if names.contains_key(name) {
                let conflicts = name_conflicts.entry(*name).or_insert(HashSet::new());

                let bindings = names.get(name).unwrap();
                if bindings.len() == 1 {
                    conflicts.insert(bindings[0]);
                }

                conflicts.insert(*binding);
            }

            bindings.entry(*binding).or_insert(vec![]).push(*name);
            names.entry(*name).or_insert(vec![]).push(*binding);
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

        (
            ShortcutMap {
                bindings,
                names,
                current_shortcuts: Default::default(),
            },
            ShortcutMapConflicts { conflicts },
        )
    }

    pub fn new() -> (ShortcutMap, ShortcutMapConflicts) {
        Self::from(SHORTCUT_LIST)
    }

    pub fn lookup(&mut self, keys: &Vec<Shortcut>) -> ShortcutLookup {
        self.current_shortcuts.clear();

        for shortcut in keys {
            if let Some(shortcuts) = self.bindings.get(&shortcut) {
                for shortcut in shortcuts {
                    self.current_shortcuts.insert(*shortcut);
                }
            }
        }

        ShortcutLookup {
            shortcuts: &self.current_shortcuts,
            names: &self.names,
        }
    }
}

/// Represents the result of a shortcut lookup operation.
///
/// This contains a set of all the currently pressed shortcuts, as well as a map
/// from shortcut names the current keybindings for those names.
#[derive(Copy, Clone)]
pub struct ShortcutLookup<'a> {
    shortcuts: &'a HashSet<ShortcutName>,
    names: &'a HashMap<ShortcutName, Vec<Shortcut>>,
}

impl<'a> ShortcutLookup<'a> {
    /// Is this shortcut name currently pressed?
    pub fn is(&self, name: ShortcutName) -> bool {
        self.shortcuts.contains(&name)
    }

    /// Gets a list of the current bindings for a given shortcut name, if any.
    pub fn keys_for(&self, name: ShortcutName) -> KeysFor {
        KeysFor(self.names.get(&name))
    }
}

/// Represents a list of the current bindings for a given shortcut name, if any.
pub struct KeysFor<'a>(Option<&'a Vec<Shortcut>>);

/// Represents an error while creating a shortcut map.
#[derive(Debug, Error, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct ShortcutMapConflicts {
    pub conflicts: Vec<ShortcutConflict>,
}

/// Represents a keyboard shortcut conflict.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum ShortcutConflict {
    /// Indicates that two shortcut name enums have the same keyboard shortcut
    /// binding.
    Binding {
        binding: Shortcut,
        names: Vec<ShortcutName>,
    },
    /// Indicates that two shortcut bindings refer to the same shortcut name.
    Name {
        name: ShortcutName,
        bindings: Vec<Shortcut>,
    },
}

impl<'a> Display for KeysFor<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(bindings) = self.0 {
            write!(f, "{}", bindings.iter().format(", "))
        } else {
            write!(f, "")
        }
    }
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

impl Display for ShortcutName {
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
        #[cfg(target_os = "macos")]
        {
            if self.modifiers.ctrl {
                write!(f, "^-")?;
            }
            if self.modifiers.logo {
                write!(f, "\u{2318}-")?;
            }
            if self.modifiers.shift {
                write!(f, "\u{21E7}-")?;
            }
            if self.modifiers.alt {
                write!(f, "\u{2325}-")?;
            }
            return write!(f, "{:?}", self.key);
        }
        #[cfg(not(target_os = "macos"))]
        {
            if self.modifiers.ctrl {
                write!(f, "Ctrl+")?;
            }
            if self.modifiers.logo {
                write!(f, "Logo+")?;
            }
            if self.modifiers.shift {
                write!(f, "Shift+")?;
            }
            if self.modifiers.alt {
                write!(f, "Alt+")?;
            }
            return write!(f, "{:?}", self.key);
        }
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
