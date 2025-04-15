//! This is the language module of the fractal-rs project. This module houses the AST and the parser
//! for the custom language used to define fractal types and fractal colors.

use crate::ast::AstProgram;
use crate::parser::{ProgramSourceSet, ProgramSpan};
use serde::{Deserialize, Serialize};
use std::fmt::{Debug, Display, Formatter};

pub mod ast;
pub mod parser;

#[derive(Default, Debug, Clone)]
pub struct LangProgram {
    pub source_set: ProgramSourceSet,
    pub program: AstProgram,
}

impl LangProgram {
    pub fn parse(code: impl ToString, source_location: impl ToString) {}
}

/// The types that an expression can be
#[derive(
    Default, Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub enum ExpressionType {
    Boolean,
    Color,
    Complex,
    Integer,
    #[default]
    Unit,
}

/// A function signature
#[derive(Default, Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub params: Vec<ExpressionType>,
}

/// The function signature plus return type
#[derive(Default, Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionType {
    pub signature: FunctionSignature,
    pub ret: ExpressionType,
}

/// The function signature plus name
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionName {
    pub name: String,
    pub signature: FunctionSignature,
}

/// The function name plus return type
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: FunctionName,
    pub ret: ExpressionType,
}

/// Holds all errors collected while loading a program
#[derive(Debug)]
pub struct ProgramLoadError {
    pub syntax_errors: Vec<ariadne::Report<'static, ProgramSpan>>,
    pub semantic_errors: Vec<ariadne::Report<'static, ProgramSpan>>,
}

impl Display for ProgramLoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Syntax errors:\n")?;
        for se in self.syntax_errors.iter() {}
        todo!()
    }
}

impl std::error::Error for ProgramLoadError {}
