use cgmath::Vector4;
use std::mem::transmute;

/// Trait for any color that can be created by converting HSBA values into RGBA
/// values.
pub trait FromHSBA: Sized {
    type Element: Clone;

    /// Converts a f32 into this color's internal element type.
    fn f32_to_element(value: f32) -> Self::Element;

    /// Creates an instance of this color from RGBA values.
    fn compose(r: Self::Element, g: Self::Element, b: Self::Element, a: Self::Element) -> Self;

    /// Creates an instance of this color from HSBA values. All HSBA values must
    /// be in the range 0..1.
    fn from_hsba(hue: f32, saturation: f32, brightness: f32, alpha: f32) -> Self {
        let alpha = Self::f32_to_element(alpha);
        if saturation == 0f32 {
            let brightness = Self::f32_to_element(brightness);
            Self::compose(brightness.clone(), brightness.clone(), brightness, alpha)
        } else {
            let sector = (hue % 1f32) * 6f32;
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

impl FromHSBA for RGBA8Color {
    type Element = u8;

    fn f32_to_element(value: f32) -> Self::Element {
        (value * 255f32 + 0.5f32) as u8
    }

    fn compose(r: Self::Element, g: Self::Element, b: Self::Element, a: Self::Element) -> Self {
        RGBA8Color { r, g, b, a }
    }
}

impl FromHSBA for Vector4<f32> {
    type Element = f32;

    fn f32_to_element(value: f32) -> Self::Element {
        value
    }

    fn compose(r: Self::Element, g: Self::Element, b: Self::Element, a: Self::Element) -> Self {
        Vector4 {
            x: r,
            y: g,
            z: b,
            w: a,
        }
    }
}

/// Represents an RGBA color, with 8 bits per channel.
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RGBA8Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl From<Vector4<f32>> for RGBA8Color {
    fn from(v: Vector4<f32>) -> Self {
        RGBA8Color {
            r: Self::f32_to_element(v.x),
            g: Self::f32_to_element(v.y),
            b: Self::f32_to_element(v.z),
            a: Self::f32_to_element(v.w),
        }
    }
}

impl From<RGBA8Color> for [u8; 4] {
    fn from(c: RGBA8Color) -> Self {
        unsafe { transmute(c) }
    }
}
