use std::mem::transmute;

/// Represents an RGBA color, with 8 bits per channel.
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct RGBAColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RGBAColor {
    /// Creates a new RGBAColor from the given color byte values.
    pub fn new(red: u8, green: u8, blue: u8, alpha: u8) -> RGBAColor {
        RGBAColor {
            r: red,
            g: green,
            b: blue,
            a: alpha,
        }
    }

    /// Creates a new RGBAColor from these HSBA values. All HSBA values must be
    /// in the range 0..1.
    pub fn from_hsb(hue: f32, saturation: f32, brightness: f32, alpha: f32) -> RGBAColor {
        let alpha = (alpha * 255f32 + 0.5f32) as u8;
        if saturation == 0f32 {
            let brightness = (brightness * 255f32 + 0.5f32) as u8;
            RGBAColor {
                r: brightness,
                g: brightness,
                b: brightness,
                a: alpha,
            }
        } else {
            let sector = (hue % 1f32) * 6f32;
            let offset_in_sector = sector - sector.floor();
            let off = brightness * (1f32 - saturation);
            let fade_out = brightness * (1f32 - saturation * offset_in_sector);
            let fade_in = brightness * (1f32 - saturation * (1f32 - offset_in_sector));
            match sector as u32 {
                0 => RGBAColor {
                    r: (brightness * 255f32 + 0.5f32) as u8,
                    g: (fade_in * 255f32 + 0.5f32) as u8,
                    b: (off * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                1 => RGBAColor {
                    r: (fade_out * 255f32 + 0.5f32) as u8,
                    g: (brightness * 255f32 + 0.5f32) as u8,
                    b: (off * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                2 => RGBAColor {
                    r: (off * 255f32 + 0.5f32) as u8,
                    g: (brightness * 255f32 + 0.5f32) as u8,
                    b: (fade_in * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                3 => RGBAColor {
                    r: (off * 255f32 + 0.5f32) as u8,
                    g: (fade_out * 255f32 + 0.5f32) as u8,
                    b: (brightness * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                4 => RGBAColor {
                    r: (fade_in * 255f32 + 0.5f32) as u8,
                    g: (off * 255f32 + 0.5f32) as u8,
                    b: (brightness * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                5 => RGBAColor {
                    r: (brightness * 255f32 + 0.5f32) as u8,
                    g: (off * 255f32 + 0.5f32) as u8,
                    b: (fade_out * 255f32 + 0.5f32) as u8,
                    a: alpha,
                },
                _ => unreachable!("Invalid color wheel sector {}", sector),
            }
        }
    }
}

impl From<RGBAColor> for [u8; 4] {
    fn from(c: RGBAColor) -> Self {
        unsafe { transmute(c) }
    }
}
