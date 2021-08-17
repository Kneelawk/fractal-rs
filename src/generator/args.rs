use regex::{Regex, RegexBuilder};
use std::{str::FromStr};

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

    fn from_str(s: &str) -> ParseSmoothingResult<Self> {
        let s_lowercase = s.to_ascii_lowercase();
        if s_lowercase == "none" {
            Ok(Smoothing::None)
        } else if s_lowercase == "linear" || s_lowercase == "linearintersection" {
            Ok(Smoothing::LinearIntersection)
        } else if let Some(captures) = SMOOTHING_REGEX.captures(&s_lowercase) {
            let radius_str = &captures["radius"];
            let max_power_str = &captures["max_power"];
            Ok(Smoothing::from_logarithmic_distance(
                radius_str.parse::<f32>().chain_err(|| {
                    ParseSmoothingErrorKind::ParseFloatError(radius_str.to_string())
                })?,
                max_power_str.parse::<f32>().chain_err(|| {
                    ParseSmoothingErrorKind::ParseFloatError(max_power_str.to_string())
                })?,
            ))
        } else {
            bail!(ParseSmoothingErrorKind::NotSmoothing(s.to_string()))
        }
    }
}

error_chain! {
    types {
        ParseSmoothingError, ParseSmoothingErrorKind, ParseSmoothingResultExt, ParseSmoothingResult;
    }

    errors {
        NotSmoothing(s: String) {
            description("Input string does not represent a smoothing value")
            display("Input string '{}' does not represent a smoothing value", s)
        }
        ParseFloatError(s: String) {
            description("Input string contains an invalid floating point value")
            display("Input string '{}' contains an invalid floating point value", s)
        }
    }
}
