//! This is the language module of the fractal-rs project. This module houses the AST and the parser
//! for the custom language used to define fractal types and fractal colors.

use serde::{Deserialize, Serialize};

pub mod ast;
pub mod parser;

/// The types that an expression can be
#[derive(
    Default, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub enum ExpressionType {
    Boolean,
    Color,
    Complex,
    Integer,
    Number,
    #[default]
    Unit,
}

/// A function signature
#[derive(Default, Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionSignature {
    params: Vec<ExpressionType>,
}

/// The function signature plus return type
#[derive(Default, Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionType {
    signature: FunctionSignature,
    ret: ExpressionType,
}

/// The function signature plus name
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionName {
    name: String,
    signature: FunctionSignature,
}

/// The function name plus return type
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    name: FunctionName,
    ret: ExpressionType,
}
