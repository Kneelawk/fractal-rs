use num_traits::One;
use std::ops::{Add, Div, Mul, Sub};

/// Finds the smallest multiple of base that contains value.
pub fn smallest_multiple_containing<T>(value: T, base: T) -> T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + One + Div<Output = T> + Mul<Output = T>,
{
    (value + base - T::one()) / base * base
}
