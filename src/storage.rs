//! This module contains systems for global settings storage.
//!
//! Global settings means: this module contains settings that should be loaded
//! by all types of fractal generator, not just GUI-based ones.

use crate::util::{files::config_dir, result::ResultExt};
use parking_lot::{
    MappedRwLockReadGuard, MappedRwLockWriteGuard, RwLock, RwLockReadGuard, RwLockWriteGuard,
};
use ron::ser::PrettyConfig;
use serde::{de::DeserializeOwned, Serialize};
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

    /// Whether to cache fractal generators between renders. You pretty much
    /// only want this off if you're doing shader development.
    #[serde(default = "default_cache_generators")]
    pub cache_generators: bool,
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

impl CfgSingleton for CfgGeneral {
    fn singleton() -> &'static RwLock<Option<Self>> {
        &SINGLETON
    }

    fn file_name() -> &'static str {
        FILE_NAME
    }

    fn type_name() -> &'static str {
        "CfgGeneral"
    }
}

impl Default for CfgGeneral {
    fn default() -> Self {
        Self {
            fractal_generator_type: Default::default(),
            fractal_chunk_size_power: default_fractal_chunk_size_power(),
            cache_generators: true,
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

fn default_cache_generators() -> bool {
    true
}

/// Implemented by any struct that is loaded as a singleton from a config file.
pub trait CfgSingleton: Serialize + DeserializeOwned + Default + Sized + 'static {
    /// This config-singleton's singleton.
    fn singleton() -> &'static RwLock<Option<Self>>;

    /// The file name that this config singleton is stored into.
    fn file_name() -> &'static str;

    /// The name of the type of this singleton. This is used in error messages.
    fn type_name() -> &'static str;

    /// Load this config from the filesystem into the singleton. This returns an
    /// error if an error occurred.
    fn load() -> Result<(), CfgError> {
        let general_path = config_dir().join(Self::file_name());

        let general_cfg: Self = if general_path.exists() {
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
        *Self::singleton().write() = Some(general_cfg);

        Ok(())
    }

    /// Gets `read` access to the singleton.
    fn read() -> MappedRwLockReadGuard<'static, Self> {
        RwLockReadGuard::map(Self::singleton().read(), |option| {
            option
                .as_ref()
                .expect(&format!("{} has not been loaded", Self::type_name()))
        })
    }

    /// Reads this singleton, returning a clone so the read-lock is not held.
    fn read_clone() -> Self
    where
        Self: Clone,
    {
        Self::singleton()
            .read()
            .clone()
            .expect(&format!("{} has not been loaded", Self::type_name()))
    }

    /// Gets `write` access to the singleton.
    fn write() -> MappedRwLockWriteGuard<'static, Self> {
        RwLockWriteGuard::map(Self::singleton().write(), |option| {
            option
                .as_mut()
                .expect(&format!("{} has not been loaded", Self::type_name()))
        })
    }

    /// Stores this config from the singleton into the filesystem. This returns
    /// an error if an error occurred.
    fn store() -> Result<(), CfgError> {
        let lock = Self::read();
        let general_cfg: &Self = &lock;

        let general_path = config_dir().join(Self::file_name());
        let mut general_file = File::create(&general_path)?;
        let str = ron::ser::to_string_pretty(&general_cfg, PrettyConfig::new())?;
        write!(general_file, "{}", str)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CfgError {
    #[error("IO Error while loading config")]
    IOError(#[from] io::Error),
    #[error("Ron Error while loading config")]
    RonError(#[from] ron::Error),
}
