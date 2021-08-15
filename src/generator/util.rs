use crate::generator::BYTES_PER_PIXEL;
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
