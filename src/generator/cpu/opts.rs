use crate::generator::{args::Smoothing, color::RGBAColor, view::View, FractalOpts};
use num_complex::Complex;
use crate::generator::args::DEFAULT_RADIUS_SQUARED;

/// Structs implementing this trait can be used to generate pixel colors on a
/// CPU.
pub trait CpuFractalOpts {
    /// Generates a value between 0 and iterations corresponding to the smoothed
    /// iteration count for that location on the complex plane.
    fn gen_value(&self, loc: Complex<f32>) -> f32;

    /// Generates a color from a iteration count value.
    fn gen_color(&self, value: f32) -> RGBAColor;

    /// Generates an iteration count value for a given pixel location and view.
    fn gen_pixel_value(&self, view: View, x: usize, y: usize) -> f32 {
        self.gen_value(view.get_local_plane_coordinates((x, y)))
    }

    /// Generates a pixel color for a given pixel location and view.
    fn gen_pixel(&self, view: View, x: usize, y: usize) -> RGBAColor {
        self.gen_color(self.gen_pixel_value(view, x, y))
    }
}

impl CpuFractalOpts for FractalOpts {
    fn gen_value(&self, loc: Complex<f32>) -> f32 {
        let (mut z, c): (Complex<f32>, Complex<f32>) = if self.mandelbrot {
            (Complex::<f32>::new(0f32, 0f32), loc)
        } else {
            (loc, self.c)
        };

        let mut z_prev = z;

        let radius_squared = self.smoothing.radius_squared();

        let mut n = 0;
        while n < self.iterations {
            if z.norm_sqr() > radius_squared {
                break;
            }

            z_prev = z;

            z = z * z + c;

            n += 1;
        }

        if n < self.iterations {
            self.smoothing.smooth(n, z, z_prev)
        } else {
            n as f32
        }
    }

    fn gen_color(&self, value: f32) -> RGBAColor {
        if value < self.iterations as f32 {
            RGBAColor::from_hsb(
                value * 3.3f32 / 256f32 % 1f32,
                1f32,
                value * 16f32 / 256f32 % 1f32,
                1f32,
            )
        } else {
            RGBAColor::new(0, 0, 0, 255)
        }
    }
}

/// Structs implementing this trait can be used to smooth an integer iteration
/// count into a floating-point value.
pub trait CpuSmoothing {
    /// Gets the radius squared used to calculate if a complex number has
    /// escaped the circle around the origin.
    fn radius_squared(&self) -> f32;

    /// Smooths an integer iteration count based on the current and previous
    /// values of the complex number.
    fn smooth(&self, iterations: u32, z_current: Complex<f32>, z_previous: Complex<f32>) -> f32;
}

impl CpuSmoothing for Smoothing {
    fn radius_squared(&self) -> f32 {
        match self {
            Smoothing::None => DEFAULT_RADIUS_SQUARED,
            Smoothing::LogarithmicDistance { radius_squared, .. } => *radius_squared,
            Smoothing::LinearIntersection => DEFAULT_RADIUS_SQUARED,
        }
    }

    fn smooth(&self, iterations: u32, z_current: Complex<f32>, z_previous: Complex<f32>) -> f32 {
        match self {
            Smoothing::None => iterations as f32,
            Smoothing::LogarithmicDistance {
                divisor, addend, ..
            } => iterations as f32 - z_current.norm_sqr().ln().ln() / *divisor + *addend,
            Smoothing::LinearIntersection => {
                if z_current == z_previous {
                    return iterations as f32;
                }

                if z_previous.norm_sqr() > DEFAULT_RADIUS_SQUARED {
                    return iterations as f32;
                }

                if z_current.norm_sqr() < DEFAULT_RADIUS_SQUARED {
                    return iterations as f32;
                }

                let ax = z_previous.re;
                let ay = z_previous.im;
                let bx = z_current.re;
                let by = z_current.im;
                let dx = bx - ax;
                let dy = by - ay;

                iterations as f32
                    - if dx.abs() > dy.abs() {
                        let m = dy / dx;
                        let m_squared_1 = m * m + 1.0;
                        let p = m * ax - ay;

                        (bx - if bx > ax {
                            (m * p + (DEFAULT_RADIUS_SQUARED * m_squared_1 - p * p).sqrt())
                                / m_squared_1
                        } else {
                            (m * p - (DEFAULT_RADIUS_SQUARED * m_squared_1 - p * p).sqrt())
                                / m_squared_1
                        }) / dx
                    } else {
                        let m = dx / dy;
                        let m_squared_1 = m * m + 1.0;
                        let p = m * ay - ax;

                        (by - if by > ay {
                            (m * p + (DEFAULT_RADIUS_SQUARED * m_squared_1 - p * p).sqrt())
                                / m_squared_1
                        } else {
                            (m * p - (DEFAULT_RADIUS_SQUARED * m_squared_1 - p * p).sqrt())
                                / m_squared_1
                        }) / dy
                    }
            },
        }
    }
}
