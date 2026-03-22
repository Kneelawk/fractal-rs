use fractal_rs_3_lang::ExpressionType;
use fractal_rs_3_lang::ast::BinaryOpType;
use fractal_rs_3_lang::parser::ProgramSpan;
use std::borrow::Cow;
use std::fmt::{Display, Formatter};

pub struct RuntimeError {
    kind: RuntimeErrorKind,
    span: Option<ProgramSpan>,
}

impl RuntimeError {
    pub fn function_not_found(name: String, span: Option<ProgramSpan>) -> Self {
        Self {
            kind: RuntimeErrorKind::FunctionNotFound(name),
            span,
        }
    }

    pub fn function_incorrect_number_of_args(
        name: String,
        expected: usize,
        provided: usize,
        span: Option<ProgramSpan>,
    ) -> Self {
        Self {
            kind: RuntimeErrorKind::FunctionIncorrectNumberOfArgs(name, expected, provided),
            span,
        }
    }

    pub fn wrong_expression_type(
        expected: ExpressionType,
        actual: ExpressionType,
        span: Option<ProgramSpan>,
    ) -> Self {
        Self {
            kind: RuntimeErrorKind::WrongExpressionType(expected, actual),
            span,
        }
    }

    pub fn incompatible_operator(
        op: BinaryOpType,
        left: ExpressionType,
        right: ExpressionType,
        span: Option<ProgramSpan>,
    ) -> Self {
        Self {
            kind: RuntimeErrorKind::IncompatibleOperator(op, left, right),
            span,
        }
    }

    pub fn error(span: Option<ProgramSpan>) -> Self {
        Self {
            kind: RuntimeErrorKind::AstError,
            span,
        }
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let suffix = if let Some(span) = &self.span {
            Cow::Owned(format!(", at: {:?}", span))
        } else {
            Cow::Borrowed("")
        };

        match &self.kind {
            RuntimeErrorKind::FunctionNotFound(name) => {
                write!(f, "Function not found: {name}{suffix}")
            }
            RuntimeErrorKind::FunctionIncorrectNumberOfArgs(name, expected, provided) => write!(
                f,
                "Function {name} incorrect number of args provided: {provided}, expected: {expected}{suffix}"
            ),
            RuntimeErrorKind::WrongExpressionType(expected, actual) => write!(
                f,
                "Encountered expression of the wrong type. Expected {expected}, actual: {actual}{suffix}"
            ),
            RuntimeErrorKind::IncompatibleOperator(op, a, b) => write!(
                f,
                "Attempted fo perform an incompatible binary operation. Binary operation: {op}, left expression: {a}, right expression: {b}{suffix}"
            ),
            RuntimeErrorKind::AstError => write!(f, "AST error{suffix}"),
        }
    }
}

#[derive(Debug)]
pub enum RuntimeErrorKind {
    FunctionNotFound(String),
    FunctionIncorrectNumberOfArgs(String, usize, usize),
    WrongExpressionType(ExpressionType, ExpressionType),
    IncompatibleOperator(BinaryOpType, ExpressionType, ExpressionType),
    AstError,
}
