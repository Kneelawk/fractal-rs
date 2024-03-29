use crate::generator::{args::Smoothing, color::FromHSBA, view::View, FractalOpts};
use cgmath::Vector4;
use num_complex::Complex;

/// Structs implementing this trait can be used to generate pixel colors on a
/// CPU.
pub trait CpuFractalOpts {
    /// Generates a value between 0 and iterations corresponding to the smoothed
    /// iteration count for that location on the complex plane.
    fn gen_value(&self, loc: Complex<f32>) -> f32;

    /// Generates a color from a iteration count value.
    fn gen_color(&self, value: f32) -> Vector4<f32>;

    /// Generates an iteration count value for a given pixel location and view.
    fn gen_pixel_value(&self, view: View, x: f32, y: f32) -> f32 {
        self.gen_value(view.get_local_subpixel_plane_coordinates((x, y)))
    }

    /// Generates a pixel color for a given pixel location and view.
    fn gen_pixel(&self, view: View, x: f32, y: f32) -> Vector4<f32> {
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

        let mut n = 0;
        while n < self.iterations {
            if z.norm_sqr() > self.radius_squared {
                break;
            }

            z_prev = z;

            z = z * z + c;

            n += 1;
        }

        if n < self.iterations {
            self.smoothing.smooth(n, z, z_prev, self.radius_squared)
        } else {
            n as f32
        }
    }

    fn gen_color(&self, value: f32) -> Vector4<f32> {
        if value < self.iterations as f32 {
            Vector4::<f32>::from_hsba(
                value * 3.3f32 / 256f32 % 1f32,
                1f32,
                value * 16f32 / 256f32 % 1f32,
                1f32,
            )
        } else {
            Vector4::<f32> {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            }
        }
    }
}

/// Structs implementing this trait can be used to smooth an integer iteration
/// count into a floating-point value.
pub trait CpuSmoothing {
    /// Smooths an integer iteration count based on the current and previous
    /// values of the complex number.
    fn smooth(
        &self,
        iterations: u32,
        z_current: Complex<f32>,
        z_previous: Complex<f32>,
        radius_squared: f32,
    ) -> f32;
}

impl CpuSmoothing for Smoothing {
    fn smooth(
        &self,
        iterations: u32,
        z_current: Complex<f32>,
        z_previous: Complex<f32>,
        radius_squared: f32,
    ) -> f32 {
        match self {
            Smoothing::None => iterations as f32,
            Smoothing::LogarithmicDistance {
                divisor, addend, ..
            } => iterations as f32 - z_current.norm_sqr().ln().ln() / *divisor + *addend,
            Smoothing::LinearIntersection => {
                if z_current == z_previous {
                    return iterations as f32;
                }

                if z_previous.norm_sqr() > radius_squared {
                    return iterations as f32;
                }

                if z_current.norm_sqr() < radius_squared {
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
                            (m * p + (radius_squared * m_squared_1 - p * p).sqrt()) / m_squared_1
                        } else {
                            (m * p - (radius_squared * m_squared_1 - p * p).sqrt()) / m_squared_1
                        }) / dx
                    } else {
                        let m = dx / dy;
                        let m_squared_1 = m * m + 1.0;
                        let p = m * ay - ax;

                        (by - if by > ay {
                            (m * p + (radius_squared * m_squared_1 - p * p).sqrt()) / m_squared_1
                        } else {
                            (m * p - (radius_squared * m_squared_1 - p * p).sqrt()) / m_squared_1
                        }) / dy
                    }
            },
        }
    }
}
