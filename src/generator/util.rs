use crate::generator::BYTES_PER_PIXEL;
use cgmath::Vector2;
use core::mem;
use num_traits::One;
use std::ops::{Add, Div, Mul, Sub};

/// Finds the smallest multiple of base that contains value.
pub fn smallest_multiple_containing<T>(value: T, base: T) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + One + Div<Output = T> + Mul<Output = T>,
{
    (value + base - T::one()) / base * base
}

/// Copies a rectangle of pixels from one buffer to another.
pub fn copy_region(
    src: &[u8],
    src_width: usize,
    src_x: usize,
    src_y: usize,
    dest: &mut [u8],
    dest_width: usize,
    dest_x: usize,
    dest_y: usize,
    width: usize,
    height: usize,
) {
    if width > src_width {
        panic!("Source width is smaller than the region being copied");
    }
    if width > dest_width {
        panic!("Dest width is smaller than the region being copied");
    }
    if src.len() < (src_width * src_y + src_x * height + width * height) * BYTES_PER_PIXEL {
        panic!("Source buffer is too small to contain the source region");
    }
    if dest.len() < (dest_width * dest_y + dest_x * height + width * height) * BYTES_PER_PIXEL {
        panic!("Dest buffer is too small to contain the dest region")
    }

    let strip_size = width * BYTES_PER_PIXEL;

    for y in 0..height {
        let sy = y + src_y;
        let dy = y + dest_y;
        let si = (sy * src_width + src_x) * BYTES_PER_PIXEL;
        let di = (dy * dest_width + dest_x) * BYTES_PER_PIXEL;
        dest[di..di + strip_size].copy_from_slice(&src[si..si + strip_size]);
    }
}

/// Builds an array of offsets for the four-point multisampling scheme.
pub fn build_four_points_offsets(offset: f32) -> Vec<Vector2<f32>> {
    vec![
        Vector2 {
            x: 0.5 - offset,
            y: 0.5 - offset,
        },
        Vector2 {
            x: 0.5 + offset,
            y: 0.5 - offset,
        },
        Vector2 {
            x: 0.5 - offset,
            y: 0.5 + offset,
        },
        Vector2 {
            x: 0.5 + offset,
            y: 0.5 + offset,
        },
    ]
}

/// Builds an array of offsets for the linear multisampling scheme.
pub fn build_linear_offsets(axial_points: u32) -> Vec<Vector2<f32>> {
    let mut vec = vec![];

    let offset = 1.0 / axial_points as f32;
    let initial_offset = offset / 2.0;

    for y in 0..axial_points {
        for x in 0..axial_points {
            vec.push(Vector2 {
                x: x as f32 * offset + initial_offset,
                y: y as f32 * offset + initial_offset,
            })
        }
    }

    vec
}

/// Designed to allow the use of `f32` and `f64` as map keys.
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Ord, Eq, Hash)]
#[allow(unused)]
pub struct FloatKey {
    mantissa: u64,
    exponent: i16,
    sign: i8,
}

#[allow(unused)]
impl FloatKey {
    /// Constructs a FloatKey from a f32.
    pub fn from_f32(value: f32) -> FloatKey {
        let bits: u32 = unsafe { mem::transmute(value) };
        let sign: i8 = if bits >> 31 == 0 { 1 } else { -1 };
        let mut exponent: i16 = ((bits >> 23) & 0xff) as i16;
        let mantissa = if exponent == 0 {
            (bits & 0x7fffff) << 1
        } else {
            (bits & 0x7fffff) | 0x800000
        };
        // Exponent bias + mantissa shift
        exponent -= 127 + 23;

        FloatKey {
            mantissa: mantissa as u64,
            exponent,
            sign,
        }
    }
}

// Unit Tests.

#[cfg(test)]
mod tests {
    use crate::generator::util::smallest_multiple_containing;

    #[test]
    fn smallest_multiple_containing_below() {
        assert_eq!(smallest_multiple_containing(63, 64), 64);
    }

    #[test]
    fn smallest_multiple_containing_equal() {
        assert_eq!(smallest_multiple_containing(64, 64), 64);
    }

    #[test]
    fn smallest_multiple_containing_above() {
        assert_eq!(smallest_multiple_containing(65, 64), 128);
    }
}
