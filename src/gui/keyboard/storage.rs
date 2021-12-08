use crate::{
    gui::keyboard::{Shortcut, ShortcutName, DEFAULT_SHORTCUT_LIST},
    CfgSingleton,
};
use parking_lot::RwLock;

const FILE_NAME: &str = "keybinds.ron";

lazy_static! {
    static ref SINGLETON: RwLock<Option<CfgKeybinds>> = RwLock::new(None);
}

/// Configuration for keyboard shortcuts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgKeybinds {
    #[serde(default = "default_bindings")]
    pub bindings: Vec<(ShortcutName, Shortcut)>,
}

impl CfgSingleton for CfgKeybinds {
    fn singleton() -> &'static RwLock<Option<Self>> {
        &SINGLETON
    }

    fn file_name() -> &'static str {
        FILE_NAME
    }

    fn type_name() -> &'static str {
        "CfgKeybinds"
    }
}

impl Default for CfgKeybinds {
    fn default() -> Self {
        Self {
            bindings: default_bindings(),
        }
    }
}

fn default_bindings() -> Vec<(ShortcutName, Shortcut)> {
    DEFAULT_SHORTCUT_LIST.to_vec()
}
