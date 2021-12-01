use crate::util::result::ResultExt;
use std::{fs::create_dir_all, path::PathBuf};

const STORAGE_DIR_NAME: &str = ".fractal-rs-2";
const CONFIG_DIR_NAME: &str = "config";
const DEBUG_DIR_NAME: &str = "debug";
const LOGS_DIR_NAME: &str = "logs";

pub fn fractal_rs_2_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|p| p.join(STORAGE_DIR_NAME))
}

pub fn config_dir() -> PathBuf {
    let dir = fractal_rs_2_dir()
        .map(|p| p.join(CONFIG_DIR_NAME))
        .unwrap_or(PathBuf::from(CONFIG_DIR_NAME));
    create_dir_all(&dir).on_err(|e| error!("Error creating config dir: {:?}", e));
    dir
}

pub fn debug_dir() -> PathBuf {
    let dir = fractal_rs_2_dir()
        .map(|p| p.join(DEBUG_DIR_NAME))
        .unwrap_or(PathBuf::from(DEBUG_DIR_NAME));
    create_dir_all(&dir).on_err(|e| error!("Error debug config dir: {:?}", e));
    dir
}

pub fn logs_dir() -> PathBuf {
    let dir = fractal_rs_2_dir()
        .map(|p| p.join(LOGS_DIR_NAME))
        .unwrap_or(PathBuf::from(LOGS_DIR_NAME));
    create_dir_all(&dir).on_err(|e| error!("Error logs config dir: {:?}", e));
    dir
}
