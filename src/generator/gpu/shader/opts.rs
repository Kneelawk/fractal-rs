use liquid_core::{object, Object};

use crate::generator::{
    args::{Multisampling, Smoothing},
    gpu::shader::ShaderError,
    FractalOpts,
};

/// Structs implementing this trait can be used when generating fractals on the
/// GPU.
pub trait GpuFractalOpts {
    fn globals(&self) -> Result<Object, ShaderError>;
}

impl GpuFractalOpts for FractalOpts {
    fn globals(&self) -> Result<Object, ShaderError> {
        let opts_obj = object!({
            "c_real": self.c.re,
            "c_imag": self.c.im,
            "iterations": self.iterations,
            "mandelbrot": self.mandelbrot,
            "radius_squared": self.radius_squared,
            "smoothing": self.smoothing.opts()?,
            "multisampling": self.multisampling.opts()?,
        });

        Ok(object!({ "opts": opts_obj }))
    }
}

/// Structs implementing this trait can be used as smoothing options for
/// generating fractals on the GPU.
pub trait GpuSmoothing {
    fn opts(&self) -> Result<Object, ShaderError>;
}

impl GpuSmoothing for Smoothing {
    fn opts(&self) -> Result<Object, ShaderError> {
        Ok(match self {
            Smoothing::None => object!({ "kind": "none" }),
            Smoothing::LogarithmicDistance { divisor, addend } => object!({
                "kind": "log",
                "divisor": divisor,
                "addend": addend,
            }),
            Smoothing::LinearIntersection => object!({ "kind": "linear" }),
        })
    }
}

/// Structs implementing this trait can be used as multisampling options for
/// generating on the GPU.
pub trait GpuMultisampling {
    fn opts(&self) -> Result<Object, ShaderError>;
}

impl GpuMultisampling for Multisampling {
    fn opts(&self) -> Result<Object, ShaderError> {
        Ok(object!({
            "sample_count": self.sample_count(),
            "sample_offsets": self.offsets(),
        }))
    }
}
