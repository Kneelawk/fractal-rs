use regex::{Regex, RegexBuilder};
use std::{num::ParseFloatError, str::FromStr};

lazy_static::lazy_static! {
static ref SMOOTHING_REGEX: Regex = RegexBuilder::new(r"^logarithmic(distance)? *\( *(?P<radius>\d+(\.\d+)?|\.\d+) *, *(?P<max_power>\d+(\.\d+)?|\.\d+) *\)$").case_insensitive(true).build().unwrap();
}

/// Represents an operation for smoothing an integer iteration count into a
/// floating point value.
#[derive(Debug, Copy, Clone)]
pub enum Smoothing {
    None,
    LogarithmicDistance {
        radius_squared: f32,
        divisor: f32,
        addend: f32,
    },
    LinearIntersection,
}

impl Smoothing {
    /// Creates a logarithmic distance smoothing from the given radius and max
    /// power.
    pub fn from_logarithmic_distance(radius: f32, max_power: f32) -> Smoothing {
        let divisor = max_power.ln();
        Smoothing::LogarithmicDistance {
            radius_squared: radius * radius,
            divisor,
            addend: (2f32.ln() + radius.ln().ln()) / divisor,
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
                captures["radius"].parse::<f32>()?,
                captures["max_power"].parse::<f32>()?,
            ))
        } else {
            Err(ParseSmoothingError::NotSmoothing)
        }
    }
}

/// Returned if an error occurred while parsing a smoothing operation from a
/// string.
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
