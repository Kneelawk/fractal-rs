use crate::{
    config::ErrorKind::{DecodeFile, EncodeFile, FileWrite, OpenFile, ReadFile, WriteOpen},
    generator::{
        args::{ParseSmoothingError, ParseSmoothingErrorKind, Smoothing},
        view::View,
    },
};
use num_complex::Complex32;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
struct ConfigRaw {
    #[serde(default)]
    image: ConfigImage,
    #[serde(default)]
    fractal: ConfigFractal,
    #[serde(default)]
    misc: ConfigMisc,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigImage {
    #[serde(rename = "file-prefix", default = "default_file_prefix")]
    file_prefix: String,
    #[serde(rename = "image-width", default = "default_image_width")]
    image_width: usize,
    #[serde(rename = "image-height", default = "default_image_height")]
    image_height: usize,
}

impl Default for ConfigImage {
    fn default() -> Self {
        ConfigImage {
            file_prefix: default_file_prefix(),
            image_width: default_image_width(),
            image_height: default_image_height(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigFractal {
    #[serde(rename = "plane-width", default = "default_plane_width")]
    plane_width: f32,
    #[serde(rename = "plane-height", default = "default_plane_height")]
    plane_height: Option<f32>,
    #[serde(rename = "center-x", default = "default_center_x")]
    center_x: f32,
    #[serde(rename = "center-y", default = "default_center_y")]
    center_y: f32,
    #[serde(default = "default_mandelbrot")]
    mandelbrot: bool,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(rename = "c-real", default = "default_c_real")]
    c_real: f32,
    #[serde(rename = "c-imag", default = "default_c_imag")]
    c_imag: f32,
    #[serde(default = "default_smoothing")]
    smoothing: String,
}

impl Default for ConfigFractal {
    fn default() -> Self {
        ConfigFractal {
            plane_width: default_plane_width(),
            plane_height: default_plane_height(),
            center_x: default_center_x(),
            center_y: default_center_y(),
            mandelbrot: default_mandelbrot(),
            iterations: default_iterations(),
            c_real: default_c_real(),
            c_imag: default_c_imag(),
            smoothing: default_smoothing(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigMisc {
    #[serde(rename = "chunk-size", default = "default_chunk_size")]
    chunk_size: usize,
    #[serde(rename = "thread-count", default = "default_thread_count")]
    thread_count: usize,
}

impl Default for ConfigMisc {
    fn default() -> Self {
        ConfigMisc {
            chunk_size: default_chunk_size(),
            thread_count: default_thread_count(),
        }
    }
}

pub struct Config {
    pub view: View,
    pub center_x: f32,
    pub center_y: f32,
    pub smoothing: Smoothing,
    pub mandelbrot: bool,
    pub iterations: u32,
    pub c: Complex32,
    pub file_prefix: String,
    pub chunk_size: usize,
    pub thread_count: usize,
}

impl Config {
    pub fn load(cfg_path: &Path) -> Result<Config> {
        let mut cfg_file = File::open(cfg_path).chain_err(|| OpenFile(cfg_path.to_owned()))?;
        let mut cfg_string = String::new();
        cfg_file
            .read_to_string(&mut cfg_string)
            .chain_err(|| ReadFile(cfg_path.to_owned()))?;
        let cfg_raw: ConfigRaw =
            toml::from_str(&cfg_string).chain_err(|| DecodeFile(cfg_path.to_owned()))?;

        let view = if cfg_raw.fractal.plane_height.is_some() {
            View::new(
                cfg_raw.image.image_width,
                cfg_raw.image.image_height,
                cfg_raw.fractal.plane_width,
                cfg_raw.fractal.plane_height.unwrap(),
                cfg_raw.fractal.center_x,
                cfg_raw.fractal.center_y,
            )
        } else {
            View::new_uniform(
                cfg_raw.image.image_width,
                cfg_raw.image.image_height,
                cfg_raw.fractal.plane_width,
                cfg_raw.fractal.center_x,
                cfg_raw.fractal.center_y,
            )
        };

        let smoothing = cfg_raw.fractal.smoothing.parse::<Smoothing>()?;

        Ok(Config {
            view,
            center_x: cfg_raw.fractal.center_x,
            center_y: cfg_raw.fractal.center_y,
            smoothing,
            mandelbrot: cfg_raw.fractal.mandelbrot,
            iterations: cfg_raw.fractal.iterations,
            c: Complex32 {
                re: cfg_raw.fractal.c_real,
                im: cfg_raw.fractal.c_imag,
            },
            file_prefix: cfg_raw.image.file_prefix,
            chunk_size: cfg_raw.misc.chunk_size,
            thread_count: cfg_raw.misc.thread_count,
        })
    }

    pub fn generate(cfg_path: &Path) -> Result<()> {
        let cfg_raw: ConfigRaw = if cfg_path.exists() {
            let mut cfg_file = File::open(cfg_path).chain_err(|| OpenFile(cfg_path.to_owned()))?;
            let mut cfg_string = String::new();
            cfg_file
                .read_to_string(&mut cfg_string)
                .chain_err(|| ReadFile(cfg_path.to_owned()))?;
            toml::from_str(&cfg_string).chain_err(|| DecodeFile(cfg_path.to_owned()))?
        } else {
            toml::from_str("").unwrap()
        };

        let cfg_string = toml::to_string(&cfg_raw).chain_err(|| EncodeFile)?;
        let mut cfg_file = OpenOptions::new()
            .create(true)
            .write(true)
            .open(cfg_path)
            .chain_err(|| WriteOpen(cfg_path.to_owned()))?;
        cfg_file
            .write_all(cfg_string.as_bytes())
            .chain_err(|| FileWrite(cfg_path.to_owned()))?;

        Ok(())
    }
}

error_chain! {
    links {
        ParseSmoothing(ParseSmoothingError, ParseSmoothingErrorKind);
    }
    errors {
        OpenFile(p: PathBuf) {
            description("Error opening the config file")
            display("Error opening config file {:?}", p)
        }
        ReadFile(p: PathBuf) {
            description("Error reading the config file")
            display("Error reading config file {:?}", p)
        }
        DecodeFile(p: PathBuf) {
            description("Error decoding the config file")
            display("Error decoding config file {:?}", p)
        }
        EncodeFile {
            description("Error encoding the config file")
            display("Error encoding the config file")
        }
        WriteOpen(p: PathBuf) {
            description("Error opening the config file for writing")
            display("Error opening the config file {:?} for writing", p)
        }
        FileWrite(p: PathBuf) {
            description("Error writing to the config file")
            display("Error writing to the config file {:?}", p)
        }
    }
}

fn default_file_prefix() -> String {
    "fractal".to_string()
}

fn default_image_width() -> usize {
    1024
}

fn default_image_height() -> usize {
    1024
}

fn default_plane_width() -> f32 {
    3f32
}

fn default_plane_height() -> Option<f32> {
    None
}

fn default_center_x() -> f32 {
    0f32
}

fn default_center_y() -> f32 {
    0f32
}

fn default_mandelbrot() -> bool {
    true
}

fn default_iterations() -> u32 {
    100
}

fn default_c_real() -> f32 {
    -0.059182f32
}

fn default_c_imag() -> f32 {
    0.669273f32
}

fn default_smoothing() -> String {
    "LogarithmicDistance(4, 2)".to_string()
}

fn default_chunk_size() -> usize {
    1048576
}

fn default_thread_count() -> usize {
    10
}
