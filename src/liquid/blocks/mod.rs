//! blocks/mod.rs - This contains the custom blocks for fractal-rs-2 templates.
mod macros_define;
mod macros_if;
mod whitespace;

pub use macros_define::DefineBlock;
pub use macros_if::{IfDefBlock, IfNDefBlock};
pub use whitespace::WhitespaceBlock;
