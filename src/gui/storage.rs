use crate::CfgSingleton;
use parking_lot::RwLock;

const FILE_NAME: &str = "ui.ron";

lazy_static! {
    static ref SINGLETON: RwLock<Option<CfgUiSettings>> = RwLock::new(None);
}

/// This holds GUI-specific settings: things like initial window
/// size/fullscreen.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgUiSettings {
    /// Whether the app should start in fullscreen mode.
    #[serde(default = "default_start_fullscreen")]
    pub start_fullscreen: bool,

    /// Width of the app window when the app first starts up.
    #[serde(default = "default_initial_window_width")]
    pub initial_window_width: u32,

    /// Height of the app window when the app first starts up.
    #[serde(default = "default_initial_window_height")]
    pub initial_window_height: u32,
}

impl CfgSingleton for CfgUiSettings {
    fn singleton() -> &'static RwLock<Option<Self>> {
        &SINGLETON
    }

    fn file_name() -> &'static str {
        FILE_NAME
    }

    fn type_name() -> &'static str {
        "CfgUiSettings"
    }
}

impl Default for CfgUiSettings {
    fn default() -> Self {
        Self {
            start_fullscreen: default_start_fullscreen(),
            initial_window_width: default_initial_window_width(),
            initial_window_height: default_initial_window_height(),
        }
    }
}

fn default_start_fullscreen() -> bool {
    false
}

fn default_initial_window_width() -> u32 {
    1600
}

fn default_initial_window_height() -> u32 {
    900
}
