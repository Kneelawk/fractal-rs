//! This module contains systems for global settings storage.
//!
//! Global settings means: this module contains settings that should be loaded
//! by all types of fractal generator, not just GUI-based ones.

use crate::util::{files::config_dir, result::ResultExt};
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use ron::ser::PrettyConfig;
use std::{
    fs::File,
    io,
    io::{Read, Write},
};

const FILE_NAME: &str = "general.ron";

lazy_static! {
    static ref SINGLETON: RwLock<Option<CfgGeneral>> = RwLock::new(None);
}

/// Persistent settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CfgGeneral {
    /// The generator backend used to generate fractals. This will be replace by
    /// the generator-node system when it's ready.
    #[serde(default)]
    pub fractal_generator_type: CfgFractalGeneratorType,

    /// The power of 2 for the side length of generator chunks. For example, if
    /// this were 8, then fractal generators would generate fractals in chunks
    /// of 256x256.
    #[serde(default = "default_fractal_chunk_size_power")]
    pub fractal_chunk_size_power: usize,
}

/// Represents a selection of which type of generator backend should be used to
/// generate fractals.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum CfgFractalGeneratorType {
    /// Generate fractals on the CPU.
    Cpu,
    /// Generate fractals on the GPU.
    Gpu,
    /// Generate fractals on the GPU, but using an adapter and device separate
    /// from the ones used for the GUI if any. This is the same as `Gpu` when
    /// not using the GUI.
    GpuDedicated,
}

impl CfgGeneral {
    pub fn load() -> Result<(), CfgError> {
        let general_path = config_dir().join(FILE_NAME);

        let general_cfg: CfgGeneral = if general_path.exists() {
            let mut general_file = File::open(&general_path)?;
            let mut str = String::new();
            general_file.read_to_string(&mut str)?;
            ron::from_str(&str)
                .on_err(|e| warn!("Error while parsing cfg file: {:?} Resetting...", e))
                .unwrap_or_default()
        } else {
            Default::default()
        };

        // Store into the singleton
        *SINGLETON.write() = Some(general_cfg);

        Ok(())
    }

    pub fn read() -> MappedRwLockReadGuard<'static, CfgGeneral> {
        RwLockReadGuard::map(SINGLETON.read(), |option| {
            option.as_ref().expect("CfgGeneral has not been loaded")
        })
    }

    pub fn write() -> MappedRwLockWriteGuard<'static, CfgGeneral> {
        RwLockWriteGuard::map(SINGLETON.write(), |option| {
            option.as_mut().expect("CfgGeneral has not been loaded")
        })
    }

    pub fn store() -> Result<(), CfgError> {
        let lock = Self::read();
        let general_cfg: &CfgGeneral = &lock;

        let general_path = config_dir().join(FILE_NAME);
        let mut general_file = File::create(&general_path)?;
        let str = ron::ser::to_string_pretty(&general_cfg, PrettyConfig::new())?;
        write!(general_file, "{}", str)?;
        Ok(())
    }
}

impl Default for CfgGeneral {
    fn default() -> Self {
        Self {
            fractal_generator_type: Default::default(),
            fractal_chunk_size_power: 8,
        }
    }
}

impl Default for CfgFractalGeneratorType {
    fn default() -> Self {
        Self::Gpu
    }
}

fn default_fractal_chunk_size_power() -> usize {
    8
}

#[derive(Debug, Error)]
pub enum CfgError {
    #[error("IO Error while loading config")]
    IOError(#[from] io::Error),
    #[error("Ron Error while loading config")]
    RonError(#[from] ron::Error),
}
