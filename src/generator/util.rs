use num_traits::One;
use std::ops::{Add, Div, Mul, Sub};

/// Finds the smallest multiple of base that contains value.
pub fn smallest_multiple_containing<T>(value: T, base: T) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + One + Div<Output = T> + Mul<Output = T>,
{
    (value + base - T::one()) / base * base
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
