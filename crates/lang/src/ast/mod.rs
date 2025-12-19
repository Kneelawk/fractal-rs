//! Fractal program AST constructs.

mod visitor;

use crate::{ExpressionValue, ExpressionType};
use fractal_rs_3_utils::anymap::{AnyMap, DebugCloneAnySync};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[macro_export]
macro_rules! ast_expr {
    ($name:ident) => {
        AstExpression::new(AstExpressionImpl::$name)
    };
    ($name:ident $($insides:tt)*) => {
        AstExpression::new(AstExpressionImpl::$name $($insides)* )
    };
}

type AstAttachment = dyn DebugCloneAnySync;

/// A full program
#[derive(Default, Debug, Clone)]
pub struct AstProgram {
    pub functions: HashMap<String, AstFunction>,
    pub globals: HashMap<String, AstVariable>,
    pub attachments: AnyMap<AstAttachment>,
}

impl PartialEq for AstProgram {
    fn eq(&self, other: &Self) -> bool {
        self.functions == other.functions && self.globals == other.globals
    }
}

#[derive(Debug, Clone)]
pub struct AstFunction {
    pub name: String,
    pub args: Vec<AstVariable>,
    pub explicit_ret: Option<ExpressionType>,
    pub expr: AstExpression,
    pub annotations: Vec<AstAnnotation>,
    pub attachments: AnyMap<AstAttachment>,
}

impl AstFunction {
    pub fn new(
        name: impl ToString,
        args: Vec<AstVariable>,
        return_type: Option<ExpressionType>,
    ) -> Self {
        Self {
            name: name.to_string(),
            args,
            explicit_ret: return_type,
            expr: Default::default(),
            annotations: vec![],
            attachments: AnyMap::new(),
        }
    }

    pub fn with_attachment<A: DebugCloneAnySync>(mut self, attachment: A) -> Self {
        self.attachments.insert(attachment);
        self
    }
}

impl PartialEq for AstFunction {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.args == other.args
            && self.explicit_ret == other.explicit_ret
            && self.expr == other.expr
    }
}

#[derive(Default, Debug, Clone)]
pub struct AstExpression {
    pub expr: AstExpressionImpl,
    pub attachments: AnyMap<AstAttachment>,
}

impl AstExpression {
    pub fn new(expr: AstExpressionImpl) -> Self {
        Self {
            expr,
            attachments: Default::default(),
        }
    }

    pub fn with_attachment<A: DebugCloneAnySync>(mut self, attachment: A) -> Self {
        self.attachments.insert(attachment);
        self
    }
}

impl PartialEq for AstExpression {
    fn eq(&self, other: &Self) -> bool {
        self.expr == other.expr
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum AstExpressionImpl {
    /// A block of expressions
    ///
    /// Result is the result of the last expression in the block
    Block(AstBlock),
    Constant(ExpressionValue),
    BinaryOp {
        ty: BinaryOpType,
        left: Box<AstExpression>,
        right: Box<AstExpression>,
    },
    UnaryOp {
        ty: UnaryOpType,
        expr: Box<AstExpression>,
    },
    /// Only declares a variable
    ///
    /// Result is a unit
    VarDeclare {
        name: String,
        mutable: bool,
    },
    /// Assigns to a variable
    ///
    /// Result is the same value being assigned
    VarAssign {
        name: String,
        ty: Option<BinaryOpType>,
        assign: Box<AstExpression>,
    },
    /// Declares and assigns to a variable
    ///
    /// Result is the same value being assigned
    VarDeclareAssign {
        name: String,
        assign: Box<AstExpression>,
        mutable: bool,
    },
    VarUse(String),
    FnCall {
        name: String,
        args: Vec<AstExpression>,
    },
    /// Makes an expression result into a unit
    Terminated(Box<AstExpression>),
    Return(Box<AstExpression>),
    Break(Option<String>),
    Continue(Option<String>),
    /// If/Else chain
    ///
    /// Result type of all blocks must be the same
    ///
    /// Result is the result of the block that is run
    IfElse {
        start: AstIfBlock,
        chain: Vec<AstIfBlock>,
        end: Option<Box<AstExpression>>,
    },
    /// While loop
    ///
    /// Result is Unit
    While {
        condition: Box<AstExpression>,
        block: Box<AstExpression>,
    },
    /// For loop
    ///
    /// Result is Unit
    For {
        declares: Box<AstExpression>,
        condition: Box<AstExpression>,
        after: Box<AstExpression>,
        block: Box<AstExpression>,
    },
    /// Error: Cannot be run
    Error,
}

impl Default for AstExpressionImpl {
    fn default() -> Self {
        Self::Block(Default::default())
    }
}

#[derive(Default, Debug, Clone)]
pub struct AstIfBlock {
    pub condition: Box<AstExpression>,
    pub block: Box<AstExpression>,
    pub attachments: AnyMap<AstAttachment>,
}

impl PartialEq for AstIfBlock {
    fn eq(&self, other: &Self) -> bool {
        self.condition == other.condition && self.block == other.block
    }
}

#[derive(Default, Debug, Clone)]
pub struct AstBlock {
    pub name: Option<String>,
    pub exprs: Vec<AstExpression>,
    pub attachments: AnyMap<AstAttachment>,
}

impl PartialEq for AstBlock {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.exprs == other.exprs
    }
}

/// A variable name and type
#[derive(Debug, Clone)]
pub struct AstVariable {
    pub name: String,
    pub ty: ExpressionType,
    pub init: Option<ExpressionValue>,
    pub annotations: Vec<AstAnnotation>,
    pub attachments: AnyMap<AstAttachment>,
}

impl PartialEq for AstVariable {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.ty == other.ty
    }
}

#[derive(Debug, Clone)]
pub struct AstAnnotation {
    pub name: String,
    pub args: Vec<AstAnnotationArg>,
    pub attachments: AnyMap<AstAttachment>,
}

impl PartialEq for AstAnnotation {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.args == other.args
    }
}

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum AstAnnotationArg {
    Ident(String),
    Integer(i32),
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum BinaryOpType {
    Plus,
    Minus,
    Times,
    Divide,
    Modulo,
    Power,
    LeftShift,
    RightShift,
    Equals,
    NotEquals,
    And,
    AndLazy,
    Or,
    OrLazy,
    Xor,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
}

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum UnaryOpType {
    Not,
    Minus,
    PreIncrement,
    PostIncrement,
    PreDecrement,
    PostDecrement,
}
