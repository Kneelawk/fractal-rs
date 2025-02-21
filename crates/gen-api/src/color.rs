//! This module supports color operations.

use serde::{Deserialize, Serialize};

/// Trait for any color that can be created by converting HSBA values into RGBA
/// values.
pub trait FromHsba: Sized {
    type Element: Clone;

    /// Converts a f32 into this color's internal element type.
    fn f32_to_element(value: f32) -> Self::Element;

    /// Creates an instance of this color from RGBA values.
    fn compose(r: Self::Element, g: Self::Element, b: Self::Element, a: Self::Element) -> Self;

    /// Creates an instance of this color from HSBA values. All HSBA values must
    /// be in the range 0..1.
    fn from_hsba(hsba: Hsba) -> Self {
        let brightness = hsba.brightness();
        let saturation = hsba.saturation();
        let alpha = Self::f32_to_element(hsba.alpha());
        if hsba.saturation() == 0f32 {
            let brightness = Self::f32_to_element(brightness);
            Self::compose(brightness.clone(), brightness.clone(), brightness, alpha)
        } else {
            let sector = (hsba.hue() % 1f32) * 6f32;
            let offset_in_sector = sector - sector.floor();
            let off = brightness * (1f32 - saturation);
            let fade_out = brightness * (1f32 - saturation * offset_in_sector);
            let fade_in = brightness * (1f32 - saturation * (1f32 - offset_in_sector));
            let brightness = Self::f32_to_element(brightness);
            let off = Self::f32_to_element(off);
            match sector as u32 {
                0 => Self::compose(brightness, Self::f32_to_element(fade_in), off, alpha),
                1 => Self::compose(Self::f32_to_element(fade_out), brightness, off, alpha),
                2 => Self::compose(off, brightness, Self::f32_to_element(fade_in), alpha),
                3 => Self::compose(off, Self::f32_to_element(fade_out), brightness, alpha),
                4 => Self::compose(Self::f32_to_element(fade_in), off, brightness, alpha),
                5 => Self::compose(brightness, off, Self::f32_to_element(fade_out), alpha),
                _ => unreachable!("Invalid color wheel sector {}", sector),
            }
        }
    }
}

/// Simple representation of an RGBA color
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct Rgba8(pub [u8; 4]);

/// Simple representation of an HSBA color
#[derive(Copy, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct Hsba(pub [f32; 4]);

/// Simple representation of an RGBA color using 32-bit floats
#[derive(Copy, Clone, PartialOrd, PartialEq, Serialize, Deserialize)]
pub struct RgbaF32(pub [f32; 4]);

impl Rgba8 {
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

    pub fn hue(&self) -> f32 {
        self.0[0]
    }

    pub fn saturation(&self) -> f32 {
        self.0[1]
    }

    pub fn brightness(&self) -> f32 {
        self.0[2]
    }

    pub fn alpha(&self) -> f32 {
        self.0[3]
    }
}

impl RgbaF32 {
    /// Creates a new RGBA color from parts
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r, g, b, a])
    }

    pub fn r(&self) -> f32 {
        self.0[0]
    }

    pub fn g(&self) -> f32 {
        self.0[1]
    }

    pub fn b(&self) -> f32 {
        self.0[2]
    }

    pub fn a(&self) -> f32 {
        self.0[3]
    }
}

impl FromHsba for Rgba8 {
    type Element = u8;

    fn f32_to_element(value: f32) -> Self::Element {
        (value * 255f32 + 0.5f32) as u8
    }

    fn compose(r: Self::Element, g: Self::Element, b: Self::Element, a: Self::Element) -> Self {
        Self::new(r, g, b, a)
    }
}

impl FromHsba for RgbaF32 {
    type Element = f32;

    fn f32_to_element(value: f32) -> Self::Element {
        value
    }

    fn compose(r: Self::Element, g: Self::Element, b: Self::Element, a: Self::Element) -> Self {
        Self::new(r, g, b, a)
    }
}

impl From<(u8, u8, u8, u8)> for Rgba8 {
    fn from(value: (u8, u8, u8, u8)) -> Self {
        Self([value.0, value.1, value.2, value.3])
    }
}

impl From<[u8; 4]> for Rgba8 {
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

impl From<(f32, f32, f32, f32)> for RgbaF32 {
    fn from(value: (f32, f32, f32, f32)) -> Self {
        Self([value.0, value.1, value.2, value.3])
    }
}

impl From<[f32; 4]> for RgbaF32 {
    fn from(value: [f32; 4]) -> Self {
        Self(value)
    }
}

impl From<Hsba> for Rgba8 {
    fn from(value: Hsba) -> Self {
        Self::from_hsba(value)
    }
}

impl From<Hsba> for RgbaF32 {
    fn from(value: Hsba) -> Self {
        Self::from_hsba(value)
    }
}
