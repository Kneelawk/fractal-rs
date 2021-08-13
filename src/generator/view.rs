use std::cmp::Ordering;

use num_complex::Complex;

/// A view represents an image's width, height, and mapping onto the complex
/// plane.
// TODO: Fix PartialOrd stuff. (This should be ordered solely based on this view's position within
//  an image)
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct View {
    pub image_width: usize,
    pub image_height: usize,
    pub image_x: usize,
    pub image_y: usize,
    pub image_scale_x: f32,
    pub image_scale_y: f32,
    pub plane_start_x: f32,
    pub plane_start_y: f32,
}

/// Represents a value that may be out of bounds.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConstrainedValue<T> {
    LessThanConstraint,
    WithinConstraint(T),
    GreaterThanConstraint,
}

impl View {
    /// Creates a view centered at (0 + 0i) on the complex plane with the same
    /// scaling for both x and y axis.
    pub fn new_centered_uniform(image_width: usize, image_height: usize, plane_width: f32) -> View {
        let image_scale = plane_width / image_width as f32;
        let plane_height = image_height as f32 * image_scale;

        View {
            image_width,
            image_height,
            image_x: 0,
            image_y: 0,
            image_scale_x: image_scale,
            image_scale_y: image_scale,
            plane_start_x: -plane_width / 2f32,
            plane_start_y: -plane_height / 2f32,
        }
    }

    /// Creates a view centered at (`center_x` + `center_y`i) on the complex
    /// plane with the same scaling for both x and y axis.
    pub fn new_uniform(
        image_width: usize,
        image_height: usize,
        plane_width: f32,
        center_x: f32,
        center_y: f32,
    ) -> View {
        let image_scale = plane_width / image_width as f32;
        let plane_height = image_height as f32 * image_scale;

        View {
            image_width,
            image_height,
            image_x: 0,
            image_y: 0,
            image_scale_x: image_scale,
            image_scale_y: image_scale,
            plane_start_x: center_x - plane_width / 2f32,
            plane_start_y: center_y - plane_height / 2f32,
        }
    }

    /// Divides this view into a set of consecutive sub-views each of which
    /// containing no more pixels than `pixel_count`.
    pub fn subdivide_to_pixel_count(&self, pixel_count: usize) -> SubViewIter {
        SubViewIter::new_per_pixel(*self, pixel_count)
    }

    /// Divides this view into a set of `pieces` consecutive sub-views.
    pub fn subdivide_height(&self, pieces: usize) -> SubViewIter {
        SubViewIter::new_split_height(*self, pieces)
    }

    /// Divides this view into a set of consecutive rectangle sub-views.
    pub fn subdivide_rectangles(&self, max_width: usize, max_height: usize) -> SubViewIter {
        SubViewIter::new_rectangles(*self, max_width, max_height)
    }

    /// Gets the coordinates on the complex plane for a given local pixel
    /// coordinate.
    pub fn get_local_plane_coordinates(&self, (x, y): (usize, usize)) -> Complex<f32> {
        Complex::<f32>::new(
            x as f32 * self.image_scale_x + self.plane_start_x,
            y as f32 * self.image_scale_y + self.plane_start_y,
        )
    }

    /// Gets the local pixel coordinates for a given coordinate on the complex
    /// plane.
    pub fn get_local_pixel_coordinates(
        &self,
        plane_coordinates: Complex<f32>,
    ) -> (ConstrainedValue<usize>, ConstrainedValue<usize>) {
        (
            if plane_coordinates.re >= self.plane_start_x {
                let x = ((plane_coordinates.re - self.plane_start_x) / self.image_scale_x) as usize;

                if x < self.image_width {
                    ConstrainedValue::WithinConstraint(x)
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
            if plane_coordinates.im >= self.plane_start_y {
                let y = ((plane_coordinates.im - self.plane_start_y) / self.image_scale_y) as usize;

                if y < self.image_height {
                    ConstrainedValue::WithinConstraint(y)
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
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
        (self.plane_start_y, self.plane_start_x)
            .partial_cmp(&(other.plane_start_y, other.plane_start_x))
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SubViewIter {
    SplitHeight {
        view: View,
        pieces: usize,
        remainder: usize,

        // index stuff
        index: usize,
        image_y: usize,
    },
    SplitRow {
        view: View,
        width_pieces: usize,
        remainder: usize,

        // index stuff
        image_y: usize,
        image_x: usize,
        index: usize,
    },
    Rectangles {
        view: View,
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
    Single(Option<View>),
}

impl SubViewIter {
    fn new_split_height(view: View, pieces: usize) -> SubViewIter {
        SubViewIter::SplitHeight {
            view,
            pieces,
            remainder: view.image_height % pieces,
            index: 0,
            image_y: 0,
        }
    }

    fn new_per_pixel(view: View, pixel_count: usize) -> SubViewIter {
        if view.image_width * view.image_height < pixel_count {
            SubViewIter::Single(Some(view))
        } else if view.image_width <= pixel_count {
            let chunk_height = pixel_count / view.image_width;
            SubViewIter::new_split_height(
                view,
                (view.image_height + chunk_height - 1) / chunk_height,
            )
        } else {
            let width_pieces = (view.image_width + pixel_count - 1) / pixel_count;
            SubViewIter::SplitRow {
                view,
                width_pieces,
                remainder: view.image_height % width_pieces,
                image_y: 0,
                image_x: 0,
                index: 0,
            }
        }
    }

    fn new_rectangles(view: View, max_width: usize, max_height: usize) -> SubViewIter {
        if view.image_width <= max_width {
            if view.image_height <= max_height {
                SubViewIter::Single(Some(view))
            } else {
                SubViewIter::new_split_height(
                    view,
                    (view.image_height + max_height - 1) / max_height,
                )
            }
        } else {
            let width_pieces = (view.image_width + max_width - 1) / max_width;
            let height_pieces = (view.image_height + max_height - 1) / max_height;
            SubViewIter::Rectangles {
                view,
                width_pieces,
                height_pieces,
                remainder_x: view.image_width % width_pieces,
                remainder_y: view.image_height % height_pieces,
                image_x: 0,
                image_y: 0,
                index_x: 0,
                index_y: 0,
            }
        }
    }
}

impl Iterator for SubViewIter {
    type Item = View;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            SubViewIter::SplitHeight {
                view,
                pieces,
                remainder,
                index,
                image_y,
            } => {
                if index >= pieces {
                    None
                } else {
                    let image_height =
                        view.image_height / *pieces + if index < remainder { 1 } else { 0 };

                    let res = Some(View {
                        image_width: view.image_width,
                        image_height,
                        image_x: view.image_x,
                        image_y: view.image_y + *image_y,
                        image_scale_x: view.image_scale_x,
                        image_scale_y: view.image_scale_y,
                        plane_start_x: view.plane_start_x,
                        plane_start_y: view.plane_start_y + *image_y as f32 * view.image_scale_y,
                    });

                    *image_y += image_height;
                    *index += 1;

                    res
                }
            },
            SubViewIter::SplitRow {
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

                if *image_y >= view.image_height {
                    None
                } else {
                    let image_width =
                        view.image_width / *width_pieces + if index < remainder { 1 } else { 0 };

                    let res = Some(View {
                        image_width,
                        image_height: 1,
                        image_x: view.image_x + *image_x,
                        image_y: view.image_y + *image_y,
                        image_scale_x: view.image_scale_x,
                        image_scale_y: view.image_scale_y,
                        plane_start_x: view.plane_start_x + *image_x as f32 * view.image_scale_x,
                        plane_start_y: view.plane_start_y + *image_y as f32 * view.image_scale_y,
                    });

                    *image_x += image_width;
                    *index += 1;

                    res
                }
            },
            SubViewIter::Rectangles {
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

                if *image_y >= view.image_height {
                    None
                } else {
                    let image_width = view.image_width / *width_pieces
                        + if index_x < remainder_x { 1 } else { 0 };

                    let res = Some(View {
                        image_width,
                        image_height,
                        image_x: view.image_x + *image_x,
                        image_y: view.image_y + *image_y,
                        image_scale_x: view.image_scale_x,
                        image_scale_y: view.image_scale_y,
                        plane_start_x: view.plane_start_x + *image_x as f32 * view.image_scale_x,
                        plane_start_y: view.plane_start_y + *image_y as f32 * view.image_scale_y,
                    });

                    *image_x += image_width;
                    *index_x += 1;

                    res
                }
            },
            SubViewIter::Single(single) => single.take(),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            SubViewIter::SplitHeight { pieces, .. } => (*pieces, Some(*pieces)),
            SubViewIter::SplitRow {
                view, width_pieces, ..
            } => {
                let pieces = *width_pieces * view.image_height;
                (pieces, Some(pieces))
            },
            SubViewIter::Rectangles {
                width_pieces,
                height_pieces,
                ..
            } => {
                let pieces = *width_pieces * *height_pieces;
                (pieces, Some(pieces))
            },
            SubViewIter::Single(_) => (1, Some(1)),
        }
    }
}

impl ExactSizeIterator for SubViewIter {}

// Unit Tests.

#[cfg(test)]
mod tests {
    use crate::generator::view::View;

    #[test]
    fn is_directly_after_divided_height() {
        let parent = View {
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };
        let child1 = View {
            image_width: 10,
            image_height: 5,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };
        let child2 = View {
            image_width: 10,
            image_height: 5,
            image_x: 0,
            image_y: 5,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };

        assert!(child2.is_directly_after(&child1, &parent));
    }

    #[test]
    fn is_directly_after_divided_width() {
        let parent = View {
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };
        let child1 = View {
            image_width: 5,
            image_height: 5,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };
        let child2 = View {
            image_width: 5,
            image_height: 5,
            image_x: 5,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };

        assert!(child2.is_directly_after(&child1, &parent));
    }

    #[test]
    fn is_directly_after_wrapped_width() {
        let parent = View {
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };
        let child1 = View {
            image_width: 5,
            image_height: 5,
            image_x: 5,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };
        let child2 = View {
            image_width: 5,
            image_height: 5,
            image_x: 0,
            image_y: 5,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };

        assert!(child2.is_directly_after(&child1, &parent));
    }

    #[test]
    fn subdivide_height() {
        let view = View {
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };

        let mut iter = view.subdivide_height(3);

        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 10,
                image_height: 4,
                image_x: 0,
                image_y: 0,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 0.0,
                plane_start_y: 0.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 10,
                image_height: 3,
                image_x: 0,
                image_y: 4,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 0.0,
                plane_start_y: 4.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 10,
                image_height: 3,
                image_x: 0,
                image_y: 7,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 0.0,
                plane_start_y: 7.0,
            })
        );
    }

    #[test]
    fn subdivide_to_pixel_count() {
        let view = View {
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };

        let mut iter = view.subdivide_to_pixel_count(4);

        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 4,
                image_height: 1,
                image_x: 0,
                image_y: 0,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 0.0,
                plane_start_y: 0.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 3,
                image_height: 1,
                image_x: 4,
                image_y: 0,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 4.0,
                plane_start_y: 0.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 3,
                image_height: 1,
                image_x: 7,
                image_y: 0,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 7.0,
                plane_start_y: 0.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 4,
                image_height: 1,
                image_x: 0,
                image_y: 1,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 0.0,
                plane_start_y: 1.0,
            })
        );
    }

    #[test]
    fn subdivide_rectangles() {
        let view = View {
            image_width: 10,
            image_height: 10,
            image_x: 0,
            image_y: 0,
            image_scale_x: 1.0,
            image_scale_y: 1.0,
            plane_start_x: 0.0,
            plane_start_y: 0.0,
        };

        let mut iter = view.subdivide_rectangles(4, 4);

        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 4,
                image_height: 4,
                image_x: 0,
                image_y: 0,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 0.0,
                plane_start_y: 0.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 3,
                image_height: 4,
                image_x: 4,
                image_y: 0,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 4.0,
                plane_start_y: 0.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 3,
                image_height: 4,
                image_x: 7,
                image_y: 0,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 7.0,
                plane_start_y: 0.0,
            })
        );
        assert_eq!(
            iter.next(),
            Some(View {
                image_width: 4,
                image_height: 3,
                image_x: 0,
                image_y: 4,
                image_scale_x: 1.0,
                image_scale_y: 1.0,
                plane_start_x: 0.0,
                plane_start_y: 4.0,
            })
        );
    }
}
