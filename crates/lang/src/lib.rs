//! This is the language module of the fractal-rs project. This module houses the AST and the parser
//! for the custom language used to define fractal types and fractal colors.

pub mod ast;

/// All the types available in the fractal language
pub enum Type {
    Boolean,
    Color,
    Complex,
    Function {
        arguments: Vec<Type>,
        result: Box<Type>,
    },
    Number,
    Unit,
}
