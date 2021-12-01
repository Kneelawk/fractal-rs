mod fancy_file;

use crate::{logging::fancy_file::FancyFileAppenderDeserializer, util::files::config_dir};
use log4rs::config::Deserializers;
use std::{fs::OpenOptions, io::Write};

const DEFAULT_CONFIG_FILE: &str = "fractal-rs-2.log4rs.yaml";
const DEFAULT_CONFIG: &[u8] = include_bytes!("default.log4rs.yaml");

pub fn init() {
    let config_file_path = config_dir().join(DEFAULT_CONFIG_FILE);
    if !config_file_path.exists() {
        let mut write_cfg_file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&config_file_path)
            .unwrap();
        write_cfg_file.write_all(DEFAULT_CONFIG).unwrap();
    }

    let mut deserializers = Deserializers::new();
    deserializers.insert("fancy_file", FancyFileAppenderDeserializer);

    log4rs::init_file(&config_file_path, deserializers).unwrap();
}
