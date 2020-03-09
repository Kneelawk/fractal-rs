use num_complex::Complex;

/// A view represents an image's width, height, and mapping onto the complex
/// plane.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct View {
    pub image_width: usize,
    pub image_height: usize,
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
            image_scale_x: image_scale,
            image_scale_y: image_scale,
            plane_start_x: center_x - plane_width / 2f32,
            plane_start_y: center_y - plane_height / 2f32,
        }
    }

    /// Divides this view into a set of consecutive sub-views each of which
    /// containing no more pixels than `pixel_count`.
    pub fn subdivide_to_pixel_count(&self, pixel_count: usize) -> Vec<View> {
        if pixel_count >= self.image_width * self.image_height {
            vec![*self]
        } else if pixel_count >= self.image_width {
            let chunk_height = pixel_count / self.image_width;

            self.subdivide_height((self.image_height + chunk_height - 1) / chunk_height)
        } else {
            let mut views = vec![];
            let width_pieces = (self.image_width + pixel_count - 1) / pixel_count;
            let remainder = self.image_height % width_pieces;

            for image_y in 0..self.image_height {
                let mut image_x = 0;

                for i in 0..width_pieces {
                    let image_width =
                        self.image_width / width_pieces + if i < remainder { 1 } else { 0 };

                    views.push(View {
                        image_width,
                        image_height: 1,
                        image_scale_x: self.image_scale_x,
                        image_scale_y: self.image_scale_y,
                        plane_start_x: self.plane_start_x + image_x as f32 * self.image_scale_x,
                        plane_start_y: self.plane_start_y + image_y as f32 * self.image_scale_y,
                    });

                    image_x += image_width;
                }
            }

            views
        }
    }

    /// Divides this view into a set of `pieces` consecutive sub-views.
    pub fn subdivide_height(&self, pieces: usize) -> Vec<View> {
        let mut views = vec![];

        let remainder = self.image_height % pieces;
        let mut image_y = 0;

        for i in 0..pieces {
            let image_height = self.image_height / pieces + if i < remainder { 1 } else { 0 };

            views.push(View {
                image_width: self.image_width,
                image_height,
                image_scale_x: self.image_scale_x,
                image_scale_y: self.image_scale_y,
                plane_start_x: self.plane_start_x,
                plane_start_y: self.plane_start_y + image_y as f32 * self.image_scale_y,
            });

            image_y += image_height;
        }

        views
    }

    /// Gets the coordinates on the complex plane for a given pixel coordinate.
    pub fn get_plane_coordinates(&self, (x, y): (usize, usize)) -> Complex<f32> {
        Complex::<f32>::new(
            x as f32 * self.image_scale_x + self.plane_start_x,
            y as f32 * self.image_scale_y + self.plane_start_y,
        )
    }

    /// Gets the pixel coordinates for a given coordinate on the complex plane.
    pub fn get_pixel_coordinates(
        &self,
        plane_coordinates: Complex<f32>,
    ) -> (ConstrainedValue<usize>, ConstrainedValue<usize>) {
        (
            if plane_coordinates.re > self.plane_start_x {
                let x = ((plane_coordinates.re - self.plane_start_x) / self.image_scale_x) as usize;

                if x < self.image_width {
                    ConstrainedValue::WithinConstraint(x)
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
            if plane_coordinates.im > self.plane_start_y {
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
}
