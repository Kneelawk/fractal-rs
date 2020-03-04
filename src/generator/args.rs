use num_complex::Complex;
use regex::{Regex, RegexBuilder};
use std::{num::ParseFloatError, str::FromStr};

lazy_static::lazy_static! {
static ref SMOOTHING_REGEX: Regex = RegexBuilder::new(r"^logarithmic(distance)? *\( *(?P<radius>\d+(\.\d+)?|\.\d+) *, *(?P<max_power>\d+(\.\d+)?|\.\d+) *\)$").case_insensitive(true).build().unwrap();
}

const DEFAULT_RADIUS: f64 = 4f64;
const DEFAULT_RADIUS_SQUARED: f64 = DEFAULT_RADIUS * DEFAULT_RADIUS;

#[derive(Debug, Copy, Clone)]
pub enum Smoothing {
    None,
    LogarithmicDistance {
        radius_squared: f64,
        divisor: f64,
        addend: f64,
    },
    LinearIntersection,
}

impl Smoothing {
    pub fn from_logarithmic_distance(radius: f64, max_power: f64) -> Smoothing {
        let divisor = max_power.ln();
        Smoothing::LogarithmicDistance {
            radius_squared: radius * radius,
            divisor,
            addend: (2f64.ln() + radius.ln().ln()) / divisor,
        }
    }

    pub fn radius_squared(&self) -> f64 {
        match self {
            Smoothing::None => DEFAULT_RADIUS_SQUARED,
            Smoothing::LogarithmicDistance { radius_squared, .. } => *radius_squared,
            Smoothing::LinearIntersection => DEFAULT_RADIUS_SQUARED,
        }
    }

    pub fn smooth(
        &self,
        iterations: u32,
        z_current: Complex<f64>,
        z_previous: Complex<f64>,
    ) -> f64 {
        match self {
            Smoothing::None => iterations as f64,
            Smoothing::LogarithmicDistance {
                divisor, addend, ..
            } => iterations as f64 - z_current.norm_sqr().ln().ln() / *divisor + *addend,
            Smoothing::LinearIntersection => {
                if z_current == z_previous {
                    return iterations as f64;
                }

                if z_previous.norm_sqr() > DEFAULT_RADIUS_SQUARED {
                    return iterations as f64;
                }

                if z_current.norm_sqr() < DEFAULT_RADIUS_SQUARED {
                    return iterations as f64;
                }

                let ax = z_previous.re;
                let ay = z_previous.im;
                let bx = z_current.re;
                let by = z_current.im;
                let dx = bx - ax;
                let dy = by - ay;

                iterations as f64
                    - if dx.abs() > dy.abs() {
                        let m = dy / dx;
                        let m_squared = m * m;
                        let p = m * ax - ay;

                        (bx - if bx > ax {
                            (m * p
                                + (DEFAULT_RADIUS_SQUARED * m_squared + DEFAULT_RADIUS_SQUARED
                                    - p * p)
                                    .sqrt())
                                / (m_squared + 1f64)
                        } else {
                            (m * p
                                - (DEFAULT_RADIUS_SQUARED * m_squared + DEFAULT_RADIUS_SQUARED
                                    - p * p)
                                    .sqrt())
                                / (m_squared + 1f64)
                        }) / dx
                    } else {
                        let m = dx / dy;
                        let m_squared = m * m;
                        let p = m * ay - ax;

                        (by - if by > ay {
                            (m * p
                                + (DEFAULT_RADIUS_SQUARED * m_squared + DEFAULT_RADIUS_SQUARED
                                    - p * p)
                                    .sqrt())
                                / (m_squared + 1f64)
                        } else {
                            (m * p
                                - (DEFAULT_RADIUS_SQUARED * m_squared + DEFAULT_RADIUS_SQUARED
                                    - p * p)
                                    .sqrt())
                                / (m_squared + 1f64)
                        }) / dy
                    }
            },
        }
    }
}

impl FromStr for Smoothing {
    type Err = ParseSmoothingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lowercase = s.to_ascii_lowercase();
        if s_lowercase == "none" {
            Ok(Smoothing::None)
        } else if s_lowercase == "linear" || s_lowercase == "linearintersection" {
            Ok(Smoothing::LinearIntersection)
        } else if let Some(captures) = SMOOTHING_REGEX.captures(&s_lowercase) {
            Ok(Smoothing::from_logarithmic_distance(
                captures["radius"].parse::<f64>()?,
                captures["max_power"].parse::<f64>()?,
            ))
        } else {
            Err(ParseSmoothingError::NotSmoothing)
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParseSmoothingError {
    NotSmoothing,
    ParseFloatError(ParseFloatError),
}

impl From<ParseFloatError> for ParseSmoothingError {
    fn from(e: ParseFloatError) -> Self {
        ParseSmoothingError::ParseFloatError(e)
    }
}
