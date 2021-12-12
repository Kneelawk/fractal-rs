//! This module handles keyboard shortcuts.

#![allow(dead_code)]

use crate::{
    gui::keyboard::{macros::shortcut, storage::CfgKeybinds, ShortcutName::*},
    storage::CfgSingleton,
};
use heck::TitleCase;
use itertools::Itertools;
use serde::{de::Visitor, Deserialize, Deserializer, Serialize, Serializer};
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Write},
};
use winit::event::{ModifiersState, VirtualKeyCode};

pub mod macros;
pub mod storage;
pub mod tracker;
pub mod tree;

/// The list of shortcuts in the app.
pub const DEFAULT_SHORTCUT_LIST: &[(ShortcutName, Shortcut)] = &[
    // App shortcuts
    (App_AlternateExitFullscreen, shortcut!(Escape)),
    (App_CloseTab, shortcut!(Cmd - W)),
    (App_Fullscreen, shortcut!(F11)),
    (App_Quit, shortcut!(Cmd - Q)),
    (App_New, shortcut!(Cmd - N)),
    // Tab shortcuts
    (Tab_DeselectPosition, shortcut!(MacAlt - D)),
    (Tab_Generate, shortcut!(MacAlt - G)),
    (Tab_SpawnJulia, shortcut!(Shift - MacAlt - J)),
    (Tab_SwitchToJulia, shortcut!(MacAlt - J)),
    (Tab_SwitchToMandelbrot, shortcut!(MacAlt - M)),
];

/// This enum contains an entry for each keyboard shortcut the application uses.
#[allow(non_camel_case_types)]
#[derive(
    Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, EnumIter, Serialize, Deserialize,
)]
pub enum ShortcutName {
    // App shortcuts
    App_AlternateExitFullscreen,
    App_Fullscreen,
    App_CloseTab,
    App_Quit,
    App_New,

    // Tab shortcuts
    Tab_DeselectPosition,
    Tab_Generate,
    Tab_SpawnJulia,
    Tab_SwitchToJulia,
    Tab_SwitchToMandelbrot,
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
    conflicts: ShortcutMapConflicts,
    defaults: Vec<(ShortcutName, Shortcut)>,
    modifications: HashSet<ShortcutName>,
    enabled: bool,
}

impl ShortcutMap {
    /// Loads this shortcut map from a list of shortcut associations.
    pub fn from_list_and_defaults(
        shortcut_list: &[(ShortcutName, Shortcut)],
        defaults: &[(ShortcutName, Shortcut)],
    ) -> ShortcutMap {
        let mut bindings: HashMap<_, Vec<ShortcutName>> = HashMap::new();
        let mut names: HashMap<_, Vec<Shortcut>> = HashMap::new();
        let mut binding_conflicts = HashMap::new();
        let mut name_conflicts = HashMap::new();

        info!(
            "Loading shortcuts: [\n\t{}\n]",
            shortcut_list
                .iter()
                .map(|(ty, sc)| format!("{} => {}", ty, sc))
                .format(",\n\t")
                .to_string()
        );

        for (name, binding) in shortcut_list {
            Self::impl_add_association(
                &mut bindings,
                &mut names,
                &mut binding_conflicts,
                &mut name_conflicts,
                *name,
                *binding,
            );
        }

        let mut conflicts = ShortcutMapConflicts {
            binding_conflicts,
            name_conflicts,
            binding_conflicts_by_name: Default::default(),
        };
        conflicts.update_reverse_maps();

        let modifications = Self::calculate_modifications(&names, defaults);

        ShortcutMap {
            bindings,
            names,
            current_shortcuts: Default::default(),
            conflicts,
            defaults: defaults.to_vec(),
            modifications,
            enabled: true,
        }
    }

    pub fn from_list(shortcut_list: &[(ShortcutName, Shortcut)]) -> ShortcutMap {
        Self::from_list_and_defaults(shortcut_list, DEFAULT_SHORTCUT_LIST)
    }

    /// Loads this shortcut map from the default list of shortcut associations.
    pub fn new() -> ShortcutMap {
        Self::from_list(DEFAULT_SHORTCUT_LIST)
    }

    /// Loads this shortcut map from the keyboard shortcuts config singleton.
    /// Note that the config singleton must have been loaded already.
    ///
    /// # Panics
    /// This method panics if the keybinds config singleton has not yet been
    /// loaded.
    pub fn load() -> ShortcutMap {
        let cfg = CfgKeybinds::read();
        Self::from_list(&cfg.bindings)
    }

    /// Gets this map's list of shortcut conflicts.
    pub fn get_conflicts(&self) -> &ShortcutMapConflicts {
        &self.conflicts
    }

    /// Looks up a set of pressed keys to see what shortcut are associated with
    /// those keys.
    pub fn update(&mut self, keys: &[Shortcut]) {
        self.current_shortcuts.clear();

        for shortcut in keys {
            if let Some(shortcuts) = self.bindings.get(&shortcut) {
                for shortcut in shortcuts {
                    self.current_shortcuts.insert(*shortcut);
                }
            }
        }
    }

    /// Is this shortcut name currently pressed?
    pub fn is_pressed(&self, name: ShortcutName) -> bool {
        self.current_shortcuts.contains(&name) && self.enabled
    }

    /// Sets whether this shortcut map will show key-presses. If this is set to
    /// false, then [`is_pressed`] will always return `false`.
    ///
    /// [`is_pressed`]: Self::is_pressed()
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Gets a list of the current bindings for a given shortcut name, if any.
    pub fn keys_for(&self, name: &ShortcutName) -> KeysFor {
        KeysFor(self.names.get(name))
    }

    /// Replaces all bindings for a shortcut name with the binding given.
    pub fn replace_associations(&mut self, name: ShortcutName, binding: Option<Shortcut>) {
        // replace the association
        Self::impl_remove_name(
            &mut self.bindings,
            &mut self.names,
            &mut self.conflicts.binding_conflicts,
            &mut self.conflicts.name_conflicts,
            &name,
        );

        if let Some(binding) = binding {
            Self::impl_add_association(
                &mut self.bindings,
                &mut self.names,
                &mut self.conflicts.binding_conflicts,
                &mut self.conflicts.name_conflicts,
                name,
                binding,
            );
        }

        // recalculate the modifications
        self.modifications = Self::calculate_modifications(&self.names, &self.defaults);
        self.conflicts.update_reverse_maps();
    }

    /// Resets a the bindings for a given shortcut name to their default values.
    pub fn reset_associations(&mut self, name: ShortcutName) {
        // remove the association
        Self::impl_remove_name(
            &mut self.bindings,
            &mut self.names,
            &mut self.conflicts.binding_conflicts,
            &mut self.conflicts.name_conflicts,
            &name,
        );

        // apply default associations
        let defaults: Vec<_> = self.defaults.iter().copied().collect();
        for (default_name, default_binding) in defaults {
            if default_name == name {
                Self::impl_add_association(
                    &mut self.bindings,
                    &mut self.names,
                    &mut self.conflicts.binding_conflicts,
                    &mut self.conflicts.name_conflicts,
                    default_name,
                    default_binding,
                );
            }
        }

        self.modifications = Self::calculate_modifications(&self.names, &self.defaults);
        self.conflicts.update_reverse_maps();
    }

    /// Returns whether a shortcut's bindings have been modified from their
    /// defaults.
    pub fn is_shortcut_modified(&self, name: &ShortcutName) -> bool {
        self.modifications.contains(name)
    }

    /// Returns whether any shortcuts' bindings have been modified from their
    /// defaults.
    pub fn is_modified(&self) -> bool {
        !self.modifications.is_empty()
    }

    /// Creates a list of shortcut associations for this map.
    pub fn to_shortcut_list(&self) -> Vec<(ShortcutName, Shortcut)> {
        let names: Vec<_> = self.names.keys().copied().sorted().collect();
        let mut list = vec![];

        for name in names {
            let bindings = self.names.get(&name).unwrap().clone();
            for binding in bindings {
                list.push((name, binding));
            }
        }

        info!(
            "Storing shortcuts: [\n\t{}\n]",
            list.iter()
                .map(|(ty, sc)| format!("{} => {}", ty, sc))
                .format(",\n\t")
                .to_string()
        );

        list
    }

    /// Stores the current set of keyboard shortcuts into the keybinds
    /// configuration singleton.
    ///
    /// # Panics
    /// This method panics if the keybinds config singleton has not yet been
    /// loaded.
    pub fn store(&self) {
        let list = self.to_shortcut_list();
        CfgKeybinds::write().bindings = list;
    }

    /// Removes all bindings for a given shortcut name. This version does not do
    /// modification recalculation.
    fn impl_remove_name(
        bindings: &mut HashMap<Shortcut, Vec<ShortcutName>>,
        names: &mut HashMap<ShortcutName, Vec<Shortcut>>,
        binding_conflicts: &mut HashMap<Shortcut, HashSet<ShortcutName>>,
        name_conflicts: &mut HashMap<ShortcutName, HashSet<Shortcut>>,
        name: &ShortcutName,
    ) {
        // First, remove all existing bindings
        if let Some(existing_bindings) = names.get_mut(name) {
            for existing_binding in existing_bindings.iter_mut() {
                let mut remove = false;
                if let Some(existing_names) = bindings.get_mut(existing_binding) {
                    existing_names.retain(|existing_name| existing_name != name);
                    remove = existing_names.is_empty();
                }
                if remove {
                    bindings.remove(existing_binding);
                }
            }
            existing_bindings.clear();
        }
        names.remove(name);

        // Next, remove all current conflicts involving the shortcut_name
        binding_conflicts.retain(|_conflict_binding, conflict_names| {
            conflict_names.remove(name);
            conflict_names.len() > 1
        });
        name_conflicts.remove(name);
    }

    /// Adds a binding for a given shortcut name. This version does not do
    /// modification recalculation.
    fn impl_add_association(
        bindings: &mut HashMap<Shortcut, Vec<ShortcutName>>,
        names: &mut HashMap<ShortcutName, Vec<Shortcut>>,
        binding_conflicts: &mut HashMap<Shortcut, HashSet<ShortcutName>>,
        name_conflicts: &mut HashMap<ShortcutName, HashSet<Shortcut>>,
        name: ShortcutName,
        binding: Shortcut,
    ) {
        // First, we check for conflicts
        if bindings.contains_key(&binding) {
            let conflicts = binding_conflicts.entry(binding).or_insert(HashSet::new());

            let names = bindings.get(&binding).unwrap();
            if names.len() == 1 {
                conflicts.insert(names[0]);
            }

            conflicts.insert(name);
        }
        if names.contains_key(&name) {
            let conflicts = name_conflicts.entry(name).or_insert(HashSet::new());

            let bindings = names.get(&name).unwrap();
            if bindings.len() == 1 {
                conflicts.insert(bindings[0]);
            }

            conflicts.insert(binding);
        }

        // Then we add the association to the bindings maps
        bindings.entry(binding).or_insert(vec![]).push(name);
        names.entry(name).or_insert(vec![]).push(binding);
    }

    //
    // Modifications stuff
    //

    /// Calculates all the shortcut names with modified bindings.
    fn calculate_modifications(
        names: &HashMap<ShortcutName, Vec<Shortcut>>,
        defaults: &[(ShortcutName, Shortcut)],
    ) -> HashSet<ShortcutName> {
        let mut modifications = HashSet::new();
        let mut names: HashMap<_, _> = names
            .iter()
            .map(|(name, bindings)| (*name, bindings.iter().copied().collect::<HashSet<_>>()))
            .collect();

        for (default_name, default_binding) in defaults {
            let mut remove = false;

            if let Some(bindings) = names.get_mut(default_name) {
                debug_assert!(!bindings.is_empty(), "Encountered name with empty bindings set (name should have been removed) (this is a bug)");

                // mark names that have a binding, just not the correct one
                if !bindings.contains(default_binding) {
                    modifications.insert(*default_name);
                }

                // remove the correct binding
                bindings.remove(default_binding);
                remove = bindings.is_empty();
            } else {
                // mark any names that don't have a binding
                modifications.insert(*default_name);
            }

            // remove names with no bindings left
            if remove {
                names.remove(default_name);
            }
        }

        // mark names with bindings left over as modified
        for name in names.keys() {
            modifications.insert(*name);
        }

        modifications
    }
}

/// Represents a list of the current bindings for a given shortcut name, if any.
pub struct KeysFor<'a>(Option<&'a Vec<Shortcut>>);

//
// Conflicts stuff
//

/// Represents an error while creating a shortcut map.
#[derive(Debug, Error, Clone)]
pub struct ShortcutMapConflicts {
    binding_conflicts: HashMap<Shortcut, HashSet<ShortcutName>>,
    name_conflicts: HashMap<ShortcutName, HashSet<Shortcut>>,
    binding_conflicts_by_name: HashMap<ShortcutName, Vec<ShortcutName>>,
}

impl ShortcutMapConflicts {
    pub fn is_empty(&self) -> bool {
        self.binding_conflicts.is_empty() && self.name_conflicts.is_empty()
    }

    fn update_reverse_maps(&mut self) {
        self.binding_conflicts_by_name.clear();

        for (_, conflicts) in self.binding_conflicts.iter() {
            let conflicts_sorted: Vec<_> = conflicts.iter().sorted().collect();
            for conflict in conflicts.iter() {
                self.binding_conflicts_by_name.insert(
                    *conflict,
                    conflicts_sorted
                        .iter()
                        .copied()
                        .copied()
                        .filter(|name| name != conflict)
                        .collect(),
                );
            }
        }
    }

    pub fn binding_conflicts_for_name(&self, name: &ShortcutName) -> Option<&[ShortcutName]> {
        self.binding_conflicts_by_name
            .get(name)
            .map(|v| v.as_slice())
    }
}

//
// Display stuff
//

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
            self.binding_conflicts
                .iter()
                .map(|(binding, names)| format!(
                    "Binding {} is bound to multiple shortcuts: {}",
                    binding,
                    names.iter().format(", ")
                ))
                .chain(self.name_conflicts.iter().map(|(name, bindings)| format!(
                    "Shortcut {} is has multiple bindings: {}",
                    name,
                    bindings.iter().format(", ")
                )))
                .format(",\n\t")
        )
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

//
// Serialization stuff
//

impl Shortcut {
    /// Converts this shortcut into a string using the same formatting on all
    /// OSes.
    pub fn to_os_independent_string(&self) -> String {
        let mut s = String::new();
        if self.modifiers.ctrl {
            write!(s, "Ctrl+").expect("Error formatting string (this is a bug)");
        }
        if self.modifiers.logo {
            write!(s, "Logo+").expect("Error formatting string (this is a bug)");
        }
        if self.modifiers.shift {
            write!(s, "Shift+").expect("Error formatting string (this is a bug)");
        }
        if self.modifiers.alt {
            write!(s, "Alt+").expect("Error formatting string (this is a bug)");
        }
        write!(s, "{:?}", self.key).expect("Error formatting string (this is a bug)");
        s
    }

    /// Parses a shortcut string and converts it into a shortcut.
    ///
    /// This parses a string of the format `Ctrl+Alt+Delete` or `Shift+Logo+A`.
    /// A string is composed of a set of modifier keys and one non-modifier key.
    /// Any keys after the non-modifier key will be ignored.
    pub fn from_os_independent_string(s: impl AsRef<str>) -> Result<Self, ShortcutParseError> {
        let s = s.as_ref();
        let mut modifiers = Modifiers {
            shift: false,
            ctrl: false,
            alt: false,
            logo: false,
        };

        for segment in s.split('+') {
            let segment = segment.trim();
            match segment {
                "Ctrl" => modifiers.ctrl = true,
                "Logo" => modifiers.logo = true,
                "Shift" => modifiers.shift = true,
                "Alt" => modifiers.alt = true,
                non_modifier => {
                    return if let Some(key) = parse_virtual_keycode(non_modifier) {
                        Ok(Shortcut { modifiers, key })
                    } else {
                        Err(ShortcutParseError::InvalidKey(non_modifier.to_string()))
                    }
                },
            }
        }

        Err(ShortcutParseError::NoNonModifier)
    }
}

/// If an error is produced while parsing a string.
#[derive(Debug, Error, Ord, PartialOrd, Eq, PartialEq)]
pub enum ShortcutParseError {
    #[error("Encountered unknown key: '{}'", .0)]
    InvalidKey(String),
    #[error("No non-modifier key detected")]
    NoNonModifier,
}

impl Serialize for Shortcut {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_os_independent_string())
    }
}

impl<'de> Deserialize<'de> for Shortcut {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ShortcutVisitor)
    }
}

/// Visitor for deserializing [`Shortcut`]s.
struct ShortcutVisitor;

impl<'de> Visitor<'de> for ShortcutVisitor {
    type Value = Shortcut;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(
            formatter,
            "Expecting shortcut string in the form `Ctrl+Alt+A`"
        )
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match Shortcut::from_os_independent_string(v) {
            Ok(shortcut) => Ok(shortcut),
            Err(e) => Err(E::custom(format!("error parsing shortcut string: {}", e))),
        }
    }
}

/// Converts a string into a [`winit::event::VirtualKeyCode`].
fn parse_virtual_keycode(s: &str) -> Option<VirtualKeyCode> {
    match s {
        // The '1' key over the letters.
        "Key1" => Some(VirtualKeyCode::Key1),
        // The '2' key over the letters.
        "Key2" => Some(VirtualKeyCode::Key2),
        // The '3' key over the letters.
        "Key3" => Some(VirtualKeyCode::Key3),
        // The '4' key over the letters.
        "Key4" => Some(VirtualKeyCode::Key4),
        // The '5' key over the letters.
        "Key5" => Some(VirtualKeyCode::Key5),
        // The '6' key over the letters.
        "Key6" => Some(VirtualKeyCode::Key6),
        // The '7' key over the letters.
        "Key7" => Some(VirtualKeyCode::Key7),
        // The '8' key over the letters.
        "Key8" => Some(VirtualKeyCode::Key8),
        // The '9' key over the letters.
        "Key9" => Some(VirtualKeyCode::Key9),
        // The '0' key over the 'O' and 'P' keys.
        "Key0" => Some(VirtualKeyCode::Key0),

        "A" => Some(VirtualKeyCode::A),
        "B" => Some(VirtualKeyCode::B),
        "C" => Some(VirtualKeyCode::C),
        "D" => Some(VirtualKeyCode::D),
        "E" => Some(VirtualKeyCode::E),
        "F" => Some(VirtualKeyCode::F),
        "G" => Some(VirtualKeyCode::G),
        "H" => Some(VirtualKeyCode::H),
        "I" => Some(VirtualKeyCode::I),
        "J" => Some(VirtualKeyCode::J),
        "K" => Some(VirtualKeyCode::K),
        "L" => Some(VirtualKeyCode::L),
        "M" => Some(VirtualKeyCode::M),
        "N" => Some(VirtualKeyCode::N),
        "O" => Some(VirtualKeyCode::O),
        "P" => Some(VirtualKeyCode::P),
        "Q" => Some(VirtualKeyCode::Q),
        "R" => Some(VirtualKeyCode::R),
        "S" => Some(VirtualKeyCode::S),
        "T" => Some(VirtualKeyCode::T),
        "U" => Some(VirtualKeyCode::U),
        "V" => Some(VirtualKeyCode::V),
        "W" => Some(VirtualKeyCode::W),
        "X" => Some(VirtualKeyCode::X),
        "Y" => Some(VirtualKeyCode::Y),
        "Z" => Some(VirtualKeyCode::Z),

        // The Escape key, next to F1.
        "Escape" => Some(VirtualKeyCode::Escape),

        "F1" => Some(VirtualKeyCode::F1),
        "F2" => Some(VirtualKeyCode::F2),
        "F3" => Some(VirtualKeyCode::F3),
        "F4" => Some(VirtualKeyCode::F4),
        "F5" => Some(VirtualKeyCode::F5),
        "F6" => Some(VirtualKeyCode::F6),
        "F7" => Some(VirtualKeyCode::F7),
        "F8" => Some(VirtualKeyCode::F8),
        "F9" => Some(VirtualKeyCode::F9),
        "F10" => Some(VirtualKeyCode::F10),
        "F11" => Some(VirtualKeyCode::F11),
        "F12" => Some(VirtualKeyCode::F12),
        "F13" => Some(VirtualKeyCode::F13),
        "F14" => Some(VirtualKeyCode::F14),
        "F15" => Some(VirtualKeyCode::F15),
        "F16" => Some(VirtualKeyCode::F16),
        "F17" => Some(VirtualKeyCode::F17),
        "F18" => Some(VirtualKeyCode::F18),
        "F19" => Some(VirtualKeyCode::F19),
        "F20" => Some(VirtualKeyCode::F20),
        "F21" => Some(VirtualKeyCode::F21),
        "F22" => Some(VirtualKeyCode::F22),
        "F23" => Some(VirtualKeyCode::F23),
        "F24" => Some(VirtualKeyCode::F24),

        // Print Screen/SysRq.
        "Snapshot" => Some(VirtualKeyCode::Snapshot),
        // Scroll Lock.
        "Scroll" => Some(VirtualKeyCode::Scroll),
        // Pause/Break key, next to Scroll lock.
        "Pause" => Some(VirtualKeyCode::Pause),

        // `Insert`, next to Backspace.
        "Insert" => Some(VirtualKeyCode::Insert),
        "Home" => Some(VirtualKeyCode::Home),
        "Delete" => Some(VirtualKeyCode::Delete),
        "End" => Some(VirtualKeyCode::End),
        "PageDown" => Some(VirtualKeyCode::PageDown),
        "PageUp" => Some(VirtualKeyCode::PageUp),

        "Left" => Some(VirtualKeyCode::Left),
        "Up" => Some(VirtualKeyCode::Up),
        "Right" => Some(VirtualKeyCode::Right),
        "Down" => Some(VirtualKeyCode::Down),

        // The Backspace key, right over Enter.
        // TODO: rename
        "Back" => Some(VirtualKeyCode::Back),
        // The Enter key.
        "Return" => Some(VirtualKeyCode::Return),
        // The space bar.
        "Space" => Some(VirtualKeyCode::Space),

        // The "Compose" key on Linux.
        "Compose" => Some(VirtualKeyCode::Compose),

        "Caret" => Some(VirtualKeyCode::Caret),

        "Numlock" => Some(VirtualKeyCode::Numlock),
        "Numpad0" => Some(VirtualKeyCode::Numpad0),
        "Numpad1" => Some(VirtualKeyCode::Numpad1),
        "Numpad2" => Some(VirtualKeyCode::Numpad2),
        "Numpad3" => Some(VirtualKeyCode::Numpad3),
        "Numpad4" => Some(VirtualKeyCode::Numpad4),
        "Numpad5" => Some(VirtualKeyCode::Numpad5),
        "Numpad6" => Some(VirtualKeyCode::Numpad6),
        "Numpad7" => Some(VirtualKeyCode::Numpad7),
        "Numpad8" => Some(VirtualKeyCode::Numpad8),
        "Numpad9" => Some(VirtualKeyCode::Numpad9),
        "NumpadAdd" => Some(VirtualKeyCode::NumpadAdd),
        "NumpadDivide" => Some(VirtualKeyCode::NumpadDivide),
        "NumpadDecimal" => Some(VirtualKeyCode::NumpadDecimal),
        "NumpadComma" => Some(VirtualKeyCode::NumpadComma),
        "NumpadEnter" => Some(VirtualKeyCode::NumpadEnter),
        "NumpadEquals" => Some(VirtualKeyCode::NumpadEquals),
        "NumpadMultiply" => Some(VirtualKeyCode::NumpadMultiply),
        "NumpadSubtract" => Some(VirtualKeyCode::NumpadSubtract),

        "AbntC1" => Some(VirtualKeyCode::AbntC1),
        "AbntC2" => Some(VirtualKeyCode::AbntC2),
        "Apostrophe" => Some(VirtualKeyCode::Apostrophe),
        "Apps" => Some(VirtualKeyCode::Apps),
        "Asterisk" => Some(VirtualKeyCode::Asterisk),
        "At" => Some(VirtualKeyCode::At),
        "Ax" => Some(VirtualKeyCode::Ax),
        "Backslash" => Some(VirtualKeyCode::Backslash),
        "Calculator" => Some(VirtualKeyCode::Calculator),
        "Capital" => Some(VirtualKeyCode::Capital),
        "Colon" => Some(VirtualKeyCode::Colon),
        "Comma" => Some(VirtualKeyCode::Comma),
        "Convert" => Some(VirtualKeyCode::Convert),
        "Equals" => Some(VirtualKeyCode::Equals),
        "Grave" => Some(VirtualKeyCode::Grave),
        "Kana" => Some(VirtualKeyCode::Kana),
        "Kanji" => Some(VirtualKeyCode::Kanji),
        "LAlt" => Some(VirtualKeyCode::LAlt),
        "LBracket" => Some(VirtualKeyCode::LBracket),
        "LControl" => Some(VirtualKeyCode::LControl),
        "LShift" => Some(VirtualKeyCode::LShift),
        "LWin" => Some(VirtualKeyCode::LWin),
        "Mail" => Some(VirtualKeyCode::Mail),
        "MediaSelect" => Some(VirtualKeyCode::MediaSelect),
        "MediaStop" => Some(VirtualKeyCode::MediaStop),
        "Minus" => Some(VirtualKeyCode::Minus),
        "Mute" => Some(VirtualKeyCode::Mute),
        "MyComputer" => Some(VirtualKeyCode::MyComputer),
        // also called "Next"
        "NavigateForward" => Some(VirtualKeyCode::NavigateForward),
        // also called "Prior"
        "NavigateBackward" => Some(VirtualKeyCode::NavigateBackward),
        "NextTrack" => Some(VirtualKeyCode::NextTrack),
        "NoConvert" => Some(VirtualKeyCode::NoConvert),
        "OEM102" => Some(VirtualKeyCode::OEM102),
        "Period" => Some(VirtualKeyCode::Period),
        "PlayPause" => Some(VirtualKeyCode::PlayPause),
        "Plus" => Some(VirtualKeyCode::Plus),
        "Power" => Some(VirtualKeyCode::Power),
        "PrevTrack" => Some(VirtualKeyCode::PrevTrack),
        "RAlt" => Some(VirtualKeyCode::RAlt),
        "RBracket" => Some(VirtualKeyCode::RBracket),
        "RControl" => Some(VirtualKeyCode::RControl),
        "RShift" => Some(VirtualKeyCode::RShift),
        "RWin" => Some(VirtualKeyCode::RWin),
        "Semicolon" => Some(VirtualKeyCode::Semicolon),
        "Slash" => Some(VirtualKeyCode::Slash),
        "Sleep" => Some(VirtualKeyCode::Sleep),
        "Stop" => Some(VirtualKeyCode::Stop),
        "Sysrq" => Some(VirtualKeyCode::Sysrq),
        "Tab" => Some(VirtualKeyCode::Tab),
        "Underline" => Some(VirtualKeyCode::Underline),
        "Unlabeled" => Some(VirtualKeyCode::Unlabeled),
        "VolumeDown" => Some(VirtualKeyCode::VolumeDown),
        "VolumeUp" => Some(VirtualKeyCode::VolumeUp),
        "Wake" => Some(VirtualKeyCode::Wake),
        "WebBack" => Some(VirtualKeyCode::WebBack),
        "WebFavorites" => Some(VirtualKeyCode::WebFavorites),
        "WebForward" => Some(VirtualKeyCode::WebForward),
        "WebHome" => Some(VirtualKeyCode::WebHome),
        "WebRefresh" => Some(VirtualKeyCode::WebRefresh),
        "WebSearch" => Some(VirtualKeyCode::WebSearch),
        "WebStop" => Some(VirtualKeyCode::WebStop),
        "Yen" => Some(VirtualKeyCode::Yen),
        "Copy" => Some(VirtualKeyCode::Copy),
        "Paste" => Some(VirtualKeyCode::Paste),
        "Cut" => Some(VirtualKeyCode::Cut),

        // fallback case
        &_ => None,
    }
}

//
// Tests
//

#[cfg(test)]
mod test {
    use crate::{
        gui::keyboard::{
            macros::shortcut, Shortcut, ShortcutMap, ShortcutName::*, DEFAULT_SHORTCUT_LIST,
        },
        util::{hash_map, hash_set},
    };

    #[test]
    fn test_no_conflicts() {
        let map = ShortcutMap::new();
        let conflicts = map.get_conflicts();
        if !conflicts.is_empty() {
            panic!("Conflicts: {}", conflicts);
        }
    }

    #[test]
    fn test_shortcut_to_string() {
        assert_eq!(
            "Ctrl+Alt+Delete".to_string(),
            shortcut!(Ctrl - Alt - Delete).to_os_independent_string(),
            "Left: correct, right: testing",
        );
        assert_eq!(
            "Logo+Shift+A".to_string(),
            shortcut!(Shift - Logo - A).to_os_independent_string(),
            "Left: correct, right: testing",
        );
    }

    #[test]
    fn test_shortcut_from_string() {
        assert_eq!(
            Ok(shortcut!(Ctrl - Alt - Delete)),
            Shortcut::from_os_independent_string("Ctrl+Alt+Delete"),
            "Left: correct, right: testing"
        );
        assert_eq!(
            Ok(shortcut!(Shift - Logo - A)),
            Shortcut::from_os_independent_string("Shift+Logo+A"),
            "Left: correct, right: testing"
        );
    }

    #[test]
    fn test_shortcut_map_normal_load() {
        let mut map = ShortcutMap::from_list(&[
            (App_New, shortcut!(Ctrl - N)),
            (App_Quit, shortcut!(Ctrl - Q)),
        ]);

        assert!(
            map.get_conflicts().is_empty(),
            "Map conflicts should be empty. Conflicts: {}",
            map.get_conflicts()
        );

        map.update(&[shortcut!(Ctrl - N)]);
        assert!(map.is_pressed(App_New), "App_New should be pressed");
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");

        map.update(&[shortcut!(Ctrl - Q)]);
        assert!(map.is_pressed(App_Quit), "App_Quit should be pressed");
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");

        map.update(&[]);
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");
    }

    #[test]
    fn test_shortcut_map_conflicting_load() {
        let mut map = ShortcutMap::from_list(&[
            (App_New, shortcut!(Ctrl - N)),
            (App_Quit, shortcut!(Ctrl - N)),
        ]);

        assert!(
            !map.get_conflicts().is_empty(),
            "Map conflicts should not be empty."
        );
        assert_eq!(
            map.get_conflicts().binding_conflicts,
            hash_map!(shortcut!(Ctrl-N) => hash_set!(App_New, App_Quit)),
            "Conflicts should be binding conflicts for 'Ctrl+N' between App_New and App_Quit"
        );
        assert_eq!(
            map.get_conflicts().name_conflicts,
            Default::default(),
            "There should not be any name conflicts."
        );

        map.update(&[shortcut!(Ctrl - N)]);
        assert!(map.is_pressed(App_New), "App_New should be pressed");
        assert!(map.is_pressed(App_Quit), "App_Quit should be pressed");

        map.update(&[shortcut!(Ctrl - Q)]);
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");

        map.update(&[]);
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");
    }

    #[test]
    fn test_shortcut_map_replace_associations() {
        let mut map = ShortcutMap::from_list(&[(Tab_Generate, shortcut!(Alt - G))]);

        map.update(&[shortcut!(Alt - G)]);
        assert!(
            map.is_pressed(Tab_Generate),
            "Tab_Generate should be pressed"
        );
        map.update(&[shortcut!(Alt - F)]);
        assert!(
            !map.is_pressed(Tab_Generate),
            "Tab_Generate should not be pressed"
        );

        assert_eq!(
            map.names.get(&Tab_Generate),
            Some(&vec![shortcut!(Alt - G)]),
            "Tab_Generate name should have only a Alt+G binding"
        );
        assert_eq!(
            map.bindings.get(&shortcut!(Alt - G)),
            Some(&vec![Tab_Generate]),
            "Alt+G should only be bound to Tab_Generate"
        );

        map.replace_associations(Tab_Generate, shortcut!(Alt - F));

        map.update(&[shortcut!(Alt - F)]);
        assert!(
            map.is_pressed(Tab_Generate),
            "Tab_Generate should be pressed"
        );
        map.update(&[shortcut!(Alt - G)]);
        assert!(
            !map.is_pressed(Tab_Generate),
            "Tab_Generate should not be pressed"
        );

        assert_eq!(
            map.names.get(&Tab_Generate),
            Some(&vec![shortcut!(Alt - F)]),
            "Tab_Generate name should have only a Alt+F binding"
        );
        assert_eq!(
            map.bindings.get(&shortcut!(Alt - F)),
            Some(&vec![Tab_Generate]),
            "Alt+F should only be bound to Tab_Generate"
        );
    }

    #[test]
    fn test_shortcut_map_resolve_conflicts() {
        let mut map = ShortcutMap::from_list(&[
            (App_New, shortcut!(Ctrl - N)),
            (App_Quit, shortcut!(Ctrl - N)),
        ]);

        assert!(
            !map.get_conflicts().is_empty(),
            "Map conflicts should not be empty."
        );
        assert_eq!(
            map.get_conflicts().binding_conflicts,
            hash_map!(shortcut!(Ctrl-N) => hash_set!(App_New, App_Quit)),
            "Conflicts should be binding conflicts for 'Ctrl+N' between App_New and App_Quit"
        );
        assert_eq!(
            map.get_conflicts().name_conflicts,
            Default::default(),
            "There should not be any name conflicts."
        );

        map.replace_associations(App_Quit, shortcut!(Ctrl - Q));

        assert!(
            map.get_conflicts().is_empty(),
            "Map conflicts should be empty. Conflicts: {}",
            map.get_conflicts()
        );

        map.update(&[shortcut!(Ctrl - N)]);
        assert!(map.is_pressed(App_New), "App_New should be pressed");
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");

        map.update(&[shortcut!(Ctrl - Q)]);
        assert!(map.is_pressed(App_Quit), "App_Quit should be pressed");
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");

        map.update(&[]);
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");
    }

    #[test]
    fn test_shortcut_map_not_resolve_conflicts() {
        let mut map = ShortcutMap::from_list(&[
            (App_New, shortcut!(Ctrl - N)),
            (App_Quit, shortcut!(Ctrl - N)),
            (App_CloseTab, shortcut!(Ctrl - N)),
        ]);

        assert!(
            !map.get_conflicts().is_empty(),
            "Map conflicts should not be empty."
        );
        assert_eq!(
            map.get_conflicts().binding_conflicts,
            hash_map!(shortcut!(Ctrl-N) => hash_set!(App_New, App_Quit, App_CloseTab)),
            "Conflicts should be binding conflicts for 'Ctrl+N' between App_New, App_Quit, and App_CloseTab"
        );
        assert_eq!(
            map.get_conflicts().name_conflicts,
            Default::default(),
            "There should not be any name conflicts."
        );

        map.replace_associations(App_Quit, shortcut!(Ctrl - Q));

        assert!(
            !map.get_conflicts().is_empty(),
            "Map conflicts should not be empty."
        );
        assert_eq!(
            map.get_conflicts().binding_conflicts,
            hash_map!(shortcut!(Ctrl-N) => hash_set!(App_New, App_CloseTab)),
            "Conflicts should be binding conflicts for 'Ctrl+N' between App_New and App_CloseTab"
        );
        assert_eq!(
            map.get_conflicts().name_conflicts,
            Default::default(),
            "There should not be any name conflicts."
        );

        map.update(&[shortcut!(Ctrl - N)]);
        assert!(map.is_pressed(App_New), "App_New should be pressed");
        assert!(
            map.is_pressed(App_CloseTab),
            "App_CloseTab should be pressed"
        );
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");

        map.update(&[shortcut!(Ctrl - Q)]);
        assert!(map.is_pressed(App_Quit), "App_Quit should be pressed");
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");
        assert!(
            !map.is_pressed(App_CloseTab),
            "App_CloseTab should not be pressed"
        );

        map.update(&[]);
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");
        assert!(
            !map.is_pressed(App_CloseTab),
            "App_CloseTab should not be pressed"
        );
    }

    #[test]
    fn test_shortcut_map_create_conflicts() {
        let mut map = ShortcutMap::from_list(&[
            (App_New, shortcut!(Ctrl - N)),
            (App_Quit, shortcut!(Ctrl - Q)),
        ]);

        assert!(
            map.get_conflicts().is_empty(),
            "Map conflicts should be empty. Conflicts: {}",
            map.get_conflicts()
        );

        map.replace_associations(App_Quit, shortcut!(Ctrl - N));

        assert!(
            !map.get_conflicts().is_empty(),
            "Map conflicts should not be empty."
        );
        assert_eq!(
            map.get_conflicts().binding_conflicts,
            hash_map!(shortcut!(Ctrl-N) => hash_set!(App_New, App_Quit)),
            "Conflicts should be binding conflicts for 'Ctrl+N' between App_New and App_Quit"
        );
        assert_eq!(
            map.get_conflicts().name_conflicts,
            Default::default(),
            "There should not be any name conflicts."
        );

        map.update(&[shortcut!(Ctrl - N)]);
        assert!(map.is_pressed(App_New), "App_New should be pressed");
        assert!(map.is_pressed(App_Quit), "App_Quit should be pressed");

        map.update(&[shortcut!(Ctrl - Q)]);
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");

        map.update(&[]);
        assert!(!map.is_pressed(App_New), "App_New should not be pressed");
        assert!(!map.is_pressed(App_Quit), "App_Quit should not be pressed");
    }

    #[test]
    fn test_shortcut_map_not_modified() {
        let map = ShortcutMap::from_list_and_defaults(DEFAULT_SHORTCUT_LIST, DEFAULT_SHORTCUT_LIST);

        assert!(
            !map.is_modified(),
            "A default map should not have any modifications"
        );
    }

    #[test]
    fn test_shortcut_map_starting_modified() {
        let map = ShortcutMap::from_list_and_defaults(
            &[
                (App_New, shortcut!(Ctrl - N)),
                (App_Quit, shortcut!(Ctrl - W)),
            ],
            &[
                (App_New, shortcut!(Ctrl - N)),
                (App_Quit, shortcut!(Ctrl - Q)),
            ],
        );

        assert!(
            map.is_modified(),
            "The shortcut map should have been modified"
        );

        assert!(
            map.is_shortcut_modified(&App_Quit),
            "App_Quit should have been modified"
        );
        assert!(
            !map.is_shortcut_modified(&App_New),
            "App-New should not have been modified"
        );
    }

    #[test]
    fn test_shortcut_map_becoming_modified() {
        let mut map = ShortcutMap::from_list_and_defaults(
            &[
                (App_New, shortcut!(Ctrl - N)),
                (App_Quit, shortcut!(Ctrl - Q)),
            ],
            &[
                (App_New, shortcut!(Ctrl - N)),
                (App_Quit, shortcut!(Ctrl - Q)),
            ],
        );

        map.replace_associations(App_Quit, shortcut!(Ctrl - W));

        assert!(
            map.is_modified(),
            "The shortcut map should have been modified"
        );

        assert!(
            map.is_shortcut_modified(&App_Quit),
            "App_Quit should have been modified"
        );
        assert!(
            !map.is_shortcut_modified(&App_New),
            "App-New should not have been modified"
        );
    }

    #[test]
    fn test_shortcut_map_modified_reset() {
        let mut map = ShortcutMap::from_list_and_defaults(
            &[
                (App_New, shortcut!(Ctrl - N)),
                (App_Quit, shortcut!(Ctrl - W)),
            ],
            &[
                (App_New, shortcut!(Ctrl - N)),
                (App_Quit, shortcut!(Ctrl - Q)),
            ],
        );

        map.reset_associations(App_Quit);

        assert!(
            !map.is_modified(),
            "A default map should not have any modifications"
        );
    }
}
