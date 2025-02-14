//! view.rs - This file contains the general purpose `View` struct as well as
//! its dependencies. This file is designed to be copied from fractal generator
//! to fractal generator.

// This is a general purpose file copied from fractal generator to fractal
// generator. Many of the functions and constructs here are only used by some of
// the fractal generators.
#![allow(dead_code)]

use rug::ops::{CompleteRound, SubFrom};
use rug::{Assign, Complex, Float};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::ops::{AddAssign, DivAssign, MulAssign};
use rug::float::Round;
use streaming_iterator::StreamingIterator;

/// A view represents an image's width, height, and mapping onto the complex
/// plane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct View {
    /// The precision of all float values in this view
    pub prec: u32,
    /// The width of this view's piece of the larger image in pixels
    pub image_width: usize,
    /// The height of this view's piece of the larger image in pixels
    pub image_height: usize,
    /// The x position of the pixel at the top-left corner of this view
    pub image_x: usize,
    /// The y position of the pixel at the top-left corner of this view
    pub image_y: usize,
    /// The amount of space within one pixel along the x axis
    pub image_scale_x: Float,
    /// The amount of space within one pixel along the y axis
    pub image_scale_y: Float,
    /// The starting real position of this view in the complex plane
    pub plane_start_x: Float,
    /// The starting imaginary position of this view in the complex plane
    pub plane_start_y: Float,
}

impl View {
    /// Creates a view centered at (0 + 0i) on the complex plane with the same
    /// scaling for both x and y axis.
    pub fn new_centered_uniform(
        prec: u32,
        image_width: usize,
        image_height: usize,
        plane_width: &Float,
    ) -> View {
        let image_scale = (plane_width / image_width).complete(prec);
        let plane_height = (image_height * &image_scale).complete(prec);

        let mut plane_width = plane_width.clone();
        plane_width.set_prec(prec);

        View {
            prec,
            image_width,
            image_height,
            image_x: 0,
            image_y: 0,
            image_scale_x: image_scale.clone(),
            image_scale_y: image_scale,
            plane_start_x: -plane_width / 2f32,
            plane_start_y: -plane_height / 2f32,
        }
    }

    /// Creates a view centered at (`center_x` + `center_y`i) on the complex
    /// plane with the same scaling for both x and y axis.
    pub fn new_uniform(
        prec: u32,
        image_width: usize,
        image_height: usize,
        plane_width: &Float,
        center_x: &Float,
        center_y: &Float,
    ) -> View {
        let image_scale = (plane_width / image_width).complete(prec);
        let plane_height = (image_height * &image_scale).complete(prec);

        let mut plane_start_x = plane_width.clone();
        plane_start_x.set_prec(prec);
        plane_start_x.div_assign(2u32);
        plane_start_x.sub_from(center_x);

        let mut plane_start_y = plane_height;
        plane_start_y.div_assign(2u32);
        plane_start_y.sub_from(center_y);

        View {
            prec,
            image_width,
            image_height,
            image_x: 0,
            image_y: 0,
            image_scale_x: image_scale.clone(),
            image_scale_y: image_scale,
            plane_start_x,
            plane_start_y,
        }
    }

    pub fn new_empty(prec: u32) -> View {
        View {
            prec,
            image_width: 0,
            image_height: 0,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::new(prec),
            image_scale_y: Float::new(prec),
            plane_start_x: Float::new(prec),
            plane_start_y: Float::new(prec),
        }
    }

    /// Divides this view into a set of consecutive sub-views each of which
    /// containing no more pixels than `pixel_count`.
    pub fn subdivide_to_pixel_count(&self, pixel_count: usize) -> SubViewIter {
        SubViewIter::new_per_pixel(self, pixel_count)
    }

    /// Divides this view into a set of `pieces` consecutive sub-views.
    pub fn subdivide_height(&self, pieces: usize) -> SubViewIter {
        SubViewIter::new_split_height(self, pieces)
    }

    /// Divides this view into a set of consecutive rectangle sub-views.
    pub fn subdivide_rectangles(&self, max_width: usize, max_height: usize) -> SubViewIter {
        SubViewIter::new_rectangles(self, max_width, max_height)
    }

    /// Gets the coordinates on the complex plane for a given local pixel
    /// coordinate, assigning them into the given complex number.
    ///
    /// This method places the complex coordinate directly in the middle of the
    /// pixel instead of at the corner.
    pub fn get_local_plane_coordinates(&self, (x, y): (usize, usize), c: &mut Complex) {
        // Note the `+ 0.5`. This means that a pixel's value is at its center instead of
        // its corner.
        c.mut_real().assign(&self.image_scale_x);
        c.mut_real().mul_assign(x as f32 + 0.5);
        c.mut_real().add_assign(&self.plane_start_x);
        c.mut_imag().assign(&self.image_scale_x);
        c.mut_imag().mul_assign(y as f32 + 0.5);
        c.mut_imag().add_assign(&self.plane_start_y);
    }

    /// Gets the coordinates on the complex plane for a given local pixel
    /// coordinate.
    ///
    /// This method places the complex coordinate directly in the middle of the
    /// pixel instead of at the corner.
    pub fn get_local_plane_coordinates_new(&self, pos: (usize, usize)) -> Complex {
        let mut c = Complex::new(self.prec);
        self.get_local_plane_coordinates(pos, &mut c);
        c
    }

    /// Gets the coordinates on the complex plane for a given local subpixel
    /// pixel coordinate.
    ///
    /// This method assumes that subpixel coordinates range from 0.0 to 1.0 with
    /// 0.5 being in the middle of a pixel.
    pub fn get_local_subpixel_plane_coordinates(&self, (x, y): (f32, f32), c: &mut Complex) {
        // Note that there is no `+ 0.5` here because that is handled by what ever is
        // supplying the sub-pixel coordinates.
        c.mut_real().assign(&self.image_scale_x);
        c.mut_real().mul_assign(x);
        c.mut_real().add_assign(&self.plane_start_x);
        c.mut_imag().assign(&self.image_scale_x);
        c.mut_imag().mul_assign(y);
        c.mut_imag().add_assign(&self.plane_start_y);
    }

    /// Gets the coordinates on the complex plane for a given local subpixel
    /// pixel coordinate.
    ///
    /// This method assumes that subpixel coordinates range from 0.0 to 1.0 with
    /// 0.5 being in the middle of a pixel.
    pub fn get_local_subpixel_plane_coordinates_new(&self, pos: (f32, f32)) -> Complex {
        let mut c = Complex::new(self.prec);
        self.get_local_subpixel_plane_coordinates(pos, &mut c);
        c
    }

    /// Gets the local pixel coordinates for a given coordinate on the complex
    /// plane.
    pub fn get_local_pixel_coordinates(
        &self,
        plane_coordinates: &Complex,
    ) -> (ConstrainedValue<usize>, ConstrainedValue<usize>) {
        let mut buf = Float::new(self.prec);
        (
            if plane_coordinates.real() >= &self.plane_start_x {
                buf.assign(plane_coordinates.real() - &self.plane_start_x);
                buf.div_assign(&self.image_scale_x);
                let x = buf.to_integer_round(Round::Down).and_then(|(i, _o)| i.to_usize());

                if let Some(x) = x {
                    if x < self.image_width {
                        ConstrainedValue::WithinConstraint(x)
                    } else {
                        ConstrainedValue::GreaterThanConstraint
                    }
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
            if plane_coordinates.imag() >= &self.plane_start_y {
                buf.assign(plane_coordinates.imag() - &self.plane_start_y);
                buf.div_assign(&self.image_scale_y);
                let y = buf.to_integer_round(Round::Down).and_then(|(i, _o)| i.to_usize());

                if let Some(y) = y {
                    if y < self.image_height {
                        ConstrainedValue::WithinConstraint(y)
                    } else {
                        ConstrainedValue::GreaterThanConstraint
                    }
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
        )
    }

    /// Gets the local pixel coordinates for a given coordinate on the complex
    /// plane. This value is not constrained by this view's size and can be
    /// negative.
    pub fn get_local_unconstrained_pixel_coordinates(
        &self,
        plane_coordinates: &Complex,
    ) -> (isize, isize) {
        let mut buf = Float::new(self.prec);
        buf.assign(plane_coordinates.real() - &self.plane_start_x);
        buf.div_assign(&self.image_scale_x);
        let x = buf.to_integer_round(Round::Down).and_then(|(i, _o)| i.to_isize());
        let nan_x = buf.is_nan();
        let pos_x = buf.is_sign_positive();

        buf.assign(plane_coordinates.imag() - &self.plane_start_y);
        buf.div_assign(&self.image_scale_y);
        let y =  buf.to_integer_round(Round::Down).and_then(|(i, _o)| i.to_isize());
        let nan_y = buf.is_nan();
        let pos_y = buf.is_sign_positive();

        (
            if let Some(x) = x {
                x
            } else if nan_x {
                0
            } else if pos_x {
                isize::MAX
            } else {
                isize::MIN
            },
            if let Some(y) = y {
                y
            } else if nan_y {
                0
            } else if pos_y {
                isize::MAX
            } else {
                isize::MIN
            },
        )
    }

    /// Checks if this view is directly after the other view as a child of the
    /// parent view.
    ///
    /// Note: This method only reliably works if both views share the same
    /// direct parent. It would not make sense to check if a view from one
    /// parent is directly after a view from a different parent, even if they
    /// share a common ancestor, because their shapes and parents' orderings
    /// could be different. This means that once views are completed, they
    /// should be stitched back together unless their parent is the root view.
    pub fn is_directly_after(&self, other: &View, parent: &View) -> bool {
        if self.image_x == parent.image_x {
            // This view is at the beginning x of the parent, so the previous view must
            // extend to the end x of the parent.
            other.image_y + other.image_height == self.image_y
                && other.image_x + other.image_width == parent.image_x + parent.image_width
        } else {
            // Otherwise, this view must have the same y-value as the previous one and must
            // have an x value right at the end of the previous one.
            other.image_y == self.image_y && other.image_x + other.image_width == self.image_x
        }
    }
}

/// Special ordering for Views that ignores view size and only considers initial
/// view position.
impl PartialOrd for View {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        (&self.plane_start_y, &self.plane_start_x)
            .partial_cmp(&(&other.plane_start_y, &other.plane_start_x))
    }
}

/// Represents a value that may be out of bounds.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConstrainedValue<T> {
    LessThanConstraint,
    WithinConstraint(T),
    GreaterThanConstraint,
}

impl<T> ConstrainedValue<T> {
    /// Converts this constrained value into its inner type, the `min_value` if
    /// this is `LessThanConstraint`, or the `max_value` if this is
    /// `GreaterThanConstraint`.
    pub fn into_value(self, min_value: T, max_value: T) -> T {
        match self {
            ConstrainedValue::LessThanConstraint => min_value,
            ConstrainedValue::WithinConstraint(value) => value,
            ConstrainedValue::GreaterThanConstraint => max_value,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SubViewIter<'a> {
    SplitHeight {
        temp: Option<View>,
        view: &'a View,
        pieces: usize,
        remainder: usize,

        // index stuff
        index: usize,
        image_y: usize,
    },
    SplitRow {
        temp: Option<View>,
        view: &'a View,
        width_pieces: usize,
        remainder: usize,

        // index stuff
        image_y: usize,
        image_x: usize,
        index: usize,
    },
    Rectangles {
        temp: Option<View>,
        view: &'a View,
        width_pieces: usize,
        height_pieces: usize,
        remainder_x: usize,
        remainder_y: usize,

        // index stuff
        image_x: usize,
        image_y: usize,
        index_x: usize,
        index_y: usize,
    },
    Single {
        init: bool,
        view: Option<&'a View>,
    },
}

impl<'a> SubViewIter<'a> {
    fn new_split_height(view: &'a View, pieces: usize) -> SubViewIter<'a> {
        let remainder = view.image_height % pieces;
        SubViewIter::SplitHeight {
            temp: Some(View::new_empty(view.prec)),
            view,
            pieces,
            remainder,
            index: 0,
            image_y: 0,
        }
    }

    fn new_per_pixel(view: &'a View, pixel_count: usize) -> SubViewIter<'a> {
        if view.image_width * view.image_height < pixel_count {
            SubViewIter::Single {
                init: false,
                view: Some(view),
            }
        } else if view.image_width <= pixel_count {
            let chunk_height = pixel_count / view.image_width;
            let pieces = view.image_height.div_ceil(chunk_height);
            SubViewIter::new_split_height(view, pieces)
        } else {
            let width_pieces = view.image_width.div_ceil(pixel_count);
            let remainder = view.image_height % width_pieces;
            SubViewIter::SplitRow {
                temp: Some(View::new_empty(view.prec)),
                view,
                width_pieces,
                remainder,
                image_y: 0,
                image_x: 0,
                index: 0,
            }
        }
    }

    fn new_rectangles(view: &'a View, max_width: usize, max_height: usize) -> SubViewIter<'a> {
        if view.image_width <= max_width {
            if view.image_height <= max_height {
                SubViewIter::Single {
                    init: false,
                    view: Some(view),
                }
            } else {
                let pieces = view.image_height.div_ceil(max_height);
                SubViewIter::new_split_height(view, pieces)
            }
        } else {
            let width_pieces = view.image_width.div_ceil(max_width);
            let height_pieces = view.image_height.div_ceil(max_height);
            let remainder_x = view.image_width % width_pieces;
            let remainder_y = view.image_height % height_pieces;
            SubViewIter::Rectangles {
                temp: Some(View::new_empty(view.prec)),
                view,
                width_pieces,
                height_pieces,
                remainder_x,
                remainder_y,
                image_x: 0,
                image_y: 0,
                index_x: 0,
                index_y: 0,
            }
        }
    }
}

impl<'a> StreamingIterator for SubViewIter<'a> {
    type Item = View;

    fn advance(&mut self) {
        match self {
            SubViewIter::SplitHeight {
                temp,
                view,
                pieces,
                remainder,
                index,
                image_y,
            } => {
                if index < pieces && temp.is_some() {
                    let image_height =
                        view.image_height / *pieces + if index < remainder { 1 } else { 0 };

                    let temp = temp.as_mut().unwrap();
                    temp.image_width = view.image_width;
                    temp.image_height = image_height;
                    temp.image_x = view.image_x;
                    temp.image_y = *image_y;
                    temp.image_scale_x.assign(&view.image_scale_x);
                    temp.image_scale_y.assign(&view.image_scale_y);
                    temp.plane_start_x.assign(&view.plane_start_x);

                    temp.plane_start_y.assign(&view.image_scale_y);
                    temp.plane_start_y.mul_assign(*image_y);
                    temp.plane_start_y.add_assign(&view.plane_start_y);

                    *image_y += image_height;
                    *index += 1;
                } else {
                    *temp = None;
                }
            }
            SubViewIter::SplitRow {
                temp,
                view,
                width_pieces,
                remainder,
                image_y,
                image_x,
                index,
            } => {
                if index >= width_pieces {
                    *index = 0;
                    *image_x = 0;
                    *image_y += 1;
                }

                if *image_y < view.image_height && temp.is_some() {
                    let image_width =
                        view.image_width / *width_pieces + if index < remainder { 1 } else { 0 };

                    let temp = temp.as_mut().unwrap();
                    temp.image_width = image_width;
                    temp.image_height = 1;
                    temp.image_x = view.image_x + *image_x;
                    temp.image_y = view.image_y + *image_y;
                    temp.image_scale_x.assign(&view.image_scale_x);
                    temp.image_scale_y.assign(&view.image_scale_y);

                    temp.plane_start_x.assign(&view.image_scale_x);
                    temp.plane_start_x.mul_assign(*image_x);
                    temp.plane_start_x.add_assign(&view.plane_start_x);

                    temp.plane_start_y.assign(&view.image_scale_y);
                    temp.plane_start_y.mul_assign(*image_y);
                    temp.plane_start_y.add_assign(&view.plane_start_y);

                    *image_x += image_width;
                    *index += 1;
                } else {
                    *temp = None;
                }
            }
            SubViewIter::Rectangles {
                temp,
                view,
                width_pieces,
                height_pieces,
                remainder_x,
                remainder_y,
                image_x,
                image_y,
                index_x,
                index_y,
            } => {
                if index_x >= width_pieces {
                    let prev_image_height = view.image_height / *height_pieces
                        + if index_y < remainder_y { 1 } else { 0 };

                    *index_x = 0;
                    *index_y += 1;
                    *image_x = 0;
                    *image_y += prev_image_height;
                }

                let image_height =
                    view.image_height / *height_pieces + if index_y < remainder_y { 1 } else { 0 };

                if *image_y < view.image_height && temp.is_some() {
                    let image_width = view.image_width / *width_pieces
                        + if index_x < remainder_x { 1 } else { 0 };

                    let temp = temp.as_mut().unwrap();
                    temp.image_width = image_width;
                    temp.image_height = image_height;
                    temp.image_x = view.image_x + *image_x;
                    temp.image_y = view.image_y + *image_y;
                    temp.image_scale_x.assign(&view.image_scale_x);
                    temp.image_scale_y.assign(&view.image_scale_y);

                    temp.plane_start_x.assign(&view.image_scale_x);
                    temp.plane_start_x.mul_assign(*image_x);
                    temp.plane_start_x.add_assign(&view.plane_start_x);

                    temp.plane_start_y.assign(&view.image_scale_y);
                    temp.plane_start_y.mul_assign(*image_y);
                    temp.plane_start_y.add_assign(&view.plane_start_y);

                    *image_x += image_width;
                    *index_x += 1;
                } else {
                    *temp = None;
                }
            }
            SubViewIter::Single { init, view } => {
                if *init {
                    *view = None;
                } else {
                    *init = true;
                }
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        match self {
            SubViewIter::SplitHeight { temp, .. } => temp.as_ref(),
            SubViewIter::SplitRow { temp, .. } => temp.as_ref(),
            SubViewIter::Rectangles { temp, .. } => temp.as_ref(),
            SubViewIter::Single { view, .. } => view.clone(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            SubViewIter::SplitHeight { pieces, index, .. } => {
                let remaining = *pieces - *index;
                (remaining, Some(remaining))
            }
            SubViewIter::SplitRow {
                view,
                width_pieces,
                index,
                ..
            } => {
                let pieces = *width_pieces * view.image_height;
                let remaining = pieces - *index;
                (remaining, Some(remaining))
            }
            SubViewIter::Rectangles {
                width_pieces,
                height_pieces,
                index_x,
                index_y,
                ..
            } => {
                let pieces = *width_pieces * *height_pieces;
                let index = *index_x + *index_y * pieces;
                let remaining = pieces - index;
                (remaining, Some(remaining))
            }
            SubViewIter::Single { .. } => (1, Some(1)),
        }
    }
}

// Unit Tests.

#[cfg(test)]
mod tests {
    use crate::view::{ConstrainedValue, View};
    use rug::{Complex, Float};
    use streaming_iterator::StreamingIterator;

    #[test]
    fn is_directly_after_divided_height() {
        let parent = View {
            prec: 24,
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };
        let child1 = View {
            prec: 24,
            image_width: 10,
            image_height: 5,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };
        let child2 = View {
            prec: 0,
            image_width: 10,
            image_height: 5,
            image_x: 0,
            image_y: 5,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };

        assert!(child2.is_directly_after(&child1, &parent));
    }

    #[test]
    fn is_directly_after_divided_width() {
        let parent = View {
            prec: 24,
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };
        let child1 = View {
            prec: 24,
            image_width: 5,
            image_height: 5,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };
        let child2 = View {
            prec: 24,
            image_width: 5,
            image_height: 5,
            image_x: 5,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };

        assert!(child2.is_directly_after(&child1, &parent));
    }

    #[test]
    fn is_directly_after_wrapped_width() {
        let parent = View {
            prec: 24,
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };
        let child1 = View {
            prec: 24,
            image_width: 5,
            image_height: 5,
            image_x: 5,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };
        let child2 = View {
            prec: 24,
            image_width: 5,
            image_height: 5,
            image_x: 0,
            image_y: 5,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };

        assert!(child2.is_directly_after(&child1, &parent));
    }

    #[test]
    fn subdivide_height() {
        let view = View {
            prec: 24,
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };

        let mut iter = view.subdivide_height(3);

        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 10,
                image_height: 4,
                image_x: 0,
                image_y: 0,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 0.0),
                plane_start_y: Float::with_val(24, 0.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 10,
                image_height: 3,
                image_x: 0,
                image_y: 4,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 0.0),
                plane_start_y: Float::with_val(24, 4.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 10,
                image_height: 3,
                image_x: 0,
                image_y: 7,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 0.0),
                plane_start_y: Float::with_val(24, 7.0),
            })
        );
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn subdivide_to_pixel_count() {
        let view = View {
            prec: 24,
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };

        let mut iter = view.subdivide_to_pixel_count(4);

        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 4,
                image_height: 1,
                image_x: 0,
                image_y: 0,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 0.0),
                plane_start_y: Float::with_val(24, 0.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 3,
                image_height: 1,
                image_x: 4,
                image_y: 0,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 4.0),
                plane_start_y: Float::with_val(24, 0.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 3,
                image_height: 1,
                image_x: 7,
                image_y: 0,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 7.0),
                plane_start_y: Float::with_val(24, 0.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 4,
                image_height: 1,
                image_x: 0,
                image_y: 1,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 0.0),
                plane_start_y: Float::with_val(24, 1.0),
            })
        );
    }

    #[test]
    fn subdivide_rectangles() {
        let view = View {
            prec: 24,
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: Float::with_val(24, 1.0),
            image_scale_y: Float::with_val(24, 1.0),
            plane_start_x: Float::with_val(24, 0.0),
            plane_start_y: Float::with_val(24, 0.0),
        };

        let mut iter = view.subdivide_rectangles(4, 4);

        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 4,
                image_height: 4,
                image_x: 0,
                image_y: 0,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 0.0),
                plane_start_y: Float::with_val(24, 0.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 3,
                image_height: 4,
                image_x: 4,
                image_y: 0,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 4.0),
                plane_start_y: Float::with_val(24, 0.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 3,
                image_height: 4,
                image_x: 7,
                image_y: 0,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 7.0),
                plane_start_y: Float::with_val(24, 0.0),
            })
        );
        assert_eq!(
            iter.next(),
            Some(&View {
                prec: 24,
                image_width: 4,
                image_height: 3,
                image_x: 0,
                image_y: 4,
                image_scale_x: Float::with_val(24, 1.0),
                image_scale_y: Float::with_val(24, 1.0),
                plane_start_x: Float::with_val(24, 0.0),
                plane_start_y: Float::with_val(24, 4.0),
            })
        );
    }

    /// This tests converting pixels to complex coordinates and back.
    #[test]
    fn coordinate_conversion_pixel() {
        let view = View::new_centered_uniform(24, 256, 256, &Float::with_val(24, 3.0));

        let coord = (23, 52);

        let complex = view.get_local_plane_coordinates_new(coord);

        let new_coord = view.get_local_pixel_coordinates(&complex);

        match new_coord {
            (ConstrainedValue::WithinConstraint(x), ConstrainedValue::WithinConstraint(y)) => {
                assert_eq!((x, y), coord);
            }
            (x, y) => {
                panic!("X or Y is outside bounds! X: {:?}, Y: {:?}", x, y);
            }
        }
    }

    /// This tests converting sub-pixels to complex coordinates and back into
    /// normal pixels to make sure they fall within the same region.
    #[test]
    fn coordinate_conversion_subpixel() {
        use crate::util::build_linear_offsets;

        let view = View::new_centered_uniform(24, 256, 256, &Float::with_val(24, 3.0));

        let mut complex = Complex::new(24);

        for pixel_y in 0usize..256 {
            for pixel_x in 0usize..256 {
                let offsets = build_linear_offsets(16);

                for offset in offsets {
                    let subpixel_x = offset.0 + pixel_x as f32;
                    let subpixel_y = offset.1 + pixel_y as f32;

                    view.get_local_subpixel_plane_coordinates(
                        (subpixel_x, subpixel_y),
                        &mut complex,
                    );

                    let new_coord = view.get_local_pixel_coordinates(&complex);

                    match new_coord {
                        (
                            ConstrainedValue::WithinConstraint(x),
                            ConstrainedValue::WithinConstraint(y),
                        ) => {
                            assert_eq!(
                                (x, y),
                                (pixel_x, pixel_y),
                                "Input X: {}, Output X: {}, Input Y: {}, Output Y: {}",
                                subpixel_x,
                                x,
                                subpixel_y,
                                y
                            );
                        }
                        (x, y) => {
                            panic!("X or Y is outside bounds! Input X: {}, Output X: {:?}, Input Y: {}, Output Y: {:?}", subpixel_x, x, subpixel_y, y);
                        }
                    }
                }
            }
        }
    }

    /// This test makes sure sub-pixels that are less than bounds are
    /// represented that way.
    #[test]
    fn coordinate_conversion_below_bounds() {
        use crate::util::build_linear_offsets;

        let view = View::new_centered_uniform(24, 256, 256, &Float::with_val(24, 3.0));

        let offsets = build_linear_offsets(16);

        let mut complex = Complex::new(24);

        for offset in offsets {
            view.get_local_subpixel_plane_coordinates(
                (offset.0 - 1.0, offset.1 - 1.0),
                &mut complex,
            );

            let new_coord = view.get_local_pixel_coordinates(&complex);

            assert_eq!(
                new_coord,
                (
                    ConstrainedValue::LessThanConstraint,
                    ConstrainedValue::LessThanConstraint
                ),
                "Input X: {}, Output X: {:?}, Input Y: {}, Output Y: {:?}",
                offset.0 - 1.0,
                new_coord.0,
                offset.1 - 1.0,
                new_coord.1
            );
        }
    }

    /// This test makes sure sub-pixels that are greater than bounds are
    /// represented that way.
    #[test]
    fn coordinate_conversion_above_bounds() {
        use crate::util::build_linear_offsets;

        let view = View::new_centered_uniform(24, 256, 256, &Float::with_val(24, 3.0));

        let offsets = build_linear_offsets(16);

        let mut complex = Complex::new(24);

        for offset in offsets {
            view.get_local_subpixel_plane_coordinates(
                (offset.0 + 256.0, offset.0 + 256.0),
                &mut complex,
            );

            let new_coord = view.get_local_pixel_coordinates(&complex);

            assert_eq!(
                new_coord,
                (
                    ConstrainedValue::GreaterThanConstraint,
                    ConstrainedValue::GreaterThanConstraint
                ),
                "Input X: {}, Output X: {:?}, Input Y: {}, Output Y: {:?}",
                offset.0 + 256.0,
                new_coord.0,
                offset.0 + 256.0,
                new_coord.1
            );
        }
    }
}
