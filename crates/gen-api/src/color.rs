//! This module supports color operations.

use serde::{Deserialize, Serialize};

/// Simple representation of an RGBA color
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Rgba(pub [u8; 4]);

/// Simple representation of an HSBA color
#[derive(Copy, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct Hsba(pub [f32; 4]);

impl Rgba {
    /// Creates a new RGBA color from parts
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    pub fn r(&self) -> u8 {
        self.0[0]
    }

    pub fn g(&self) -> u8 {
        self.0[1]
    }

    pub fn b(&self) -> u8 {
        self.0[2]
    }

    pub fn a(&self) -> u8 {
        self.0[3]
    }
}

impl Hsba {
    /// Creates a new HSBA color from parts
    pub fn new(hue: f32, saturation: f32, brightness: f32, alpha: f32) -> Self {
        Self([hue, saturation, brightness, alpha])
    }
}

impl From<(u8, u8, u8, u8)> for Rgba {
    fn from(value: (u8, u8, u8, u8)) -> Self {
        Self([value.0, value.1, value.2, value.3])
    }
}

impl From<[u8; 4]> for Rgba {
    fn from(value: [u8; 4]) -> Self {
        Self(value)
    }
}

impl From<(f32, f32, f32, f32)> for Hsba {
    fn from(value: (f32, f32, f32, f32)) -> Self {
        Self([value.0, value.1, value.2, value.3])
    }
}

impl From<[f32; 4]> for Hsba {
    fn from(value: [f32; 4]) -> Self {
        Self(value)
    }
}
