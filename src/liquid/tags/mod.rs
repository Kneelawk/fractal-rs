//! blocks/mod.rs - This contains the custom tags for fractal-rs-2 templates.
mod macros_call;
mod macros_define;
mod macros_undef;

pub use macros_call::CallTag;
pub use macros_define::DefineTag;
pub use macros_undef::UndefTag;
