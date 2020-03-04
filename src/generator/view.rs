use num_complex::Complex;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct View {
    pub image_width: u32,
    pub image_height: u32,
    pub image_scale_x: f64,
    pub image_scale_y: f64,
    pub plane_start_x: f64,
    pub plane_start_y: f64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ConstrainedValue<T> {
    LessThanConstraint,
    WithinConstraint(T),
    GreaterThanConstraint,
}

impl View {
    pub fn new_centered_uniform(image_width: u32, image_height: u32, plane_width: f64) -> View {
        let image_scale = plane_width / image_width as f64;
        let plane_height = image_height as f64 * image_scale;

        View {
            image_width,
            image_height,
            image_scale_x: image_scale,
            image_scale_y: image_scale,
            plane_start_x: -plane_width / 2f64,
            plane_start_y: -plane_height / 2f64,
        }
    }

    pub fn new_uniform(
        image_width: u32,
        image_height: u32,
        plane_width: f64,
        center_x: f64,
        center_y: f64,
    ) -> View {
        let image_scale = plane_width / image_width as f64;
        let plane_height = image_height as f64 * image_scale;

        View {
            image_width,
            image_height,
            image_scale_x: image_scale,
            image_scale_y: image_scale,
            plane_start_x: center_x - plane_width / 2f64,
            plane_start_y: center_y - plane_height / 2f64,
        }
    }

    pub fn subdivide_to_pixel_count(&self, pixel_count: u32) -> Vec<View> {
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
                        plane_start_x: self.plane_start_x + image_x as f64 * self.image_scale_x,
                        plane_start_y: self.plane_start_y + image_y as f64 * self.image_scale_y,
                    });

                    image_x += image_width;
                }
            }

            views
        }
    }

    pub fn subdivide_height(&self, pieces: u32) -> Vec<View> {
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
                plane_start_y: self.plane_start_y + image_y as f64 * self.image_scale_y,
            });

            image_y += image_height;
        }

        views
    }

    pub fn get_plane_coordinates(&self, (x, y): (u32, u32)) -> Complex<f64> {
        Complex::<f64>::new(
            x as f64 * self.image_scale_x + self.plane_start_x,
            y as f64 * self.image_scale_y + self.plane_start_y,
        )
    }

    pub fn get_pixel_coordinates(
        &self,
        plane_coordinates: Complex<f64>,
    ) -> (ConstrainedValue<u32>, ConstrainedValue<u32>) {
        (
            if plane_coordinates.re > self.plane_start_x {
                let x = ((plane_coordinates.re - self.plane_start_x) / self.image_scale_x) as u32;

                if x < self.image_width {
                    ConstrainedValue::WithinConstraint(x)
                } else {
                    ConstrainedValue::GreaterThanConstraint
                }
            } else {
                ConstrainedValue::LessThanConstraint
            },
            if plane_coordinates.im > self.plane_start_y {
                let y = ((plane_coordinates.im - self.plane_start_y) / self.image_scale_y) as u32;

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
