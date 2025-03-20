//! Fractal program AST constructs.

use crate::ExpressionType;
use anymap::{CloneAny, Map};
use rug::Complex;
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

/// Dynamic traits implemented by all ast attachments
pub type AstAttachment = dyn CloneAny + Send + Sync;

/// A full program
#[derive(Default, Debug, Clone)]
pub struct AstProgram {
    pub functions: HashMap<String, AstFunction>,
    pub globals: HashMap<String, AstVariable>,
    pub attachments: Map<AstAttachment>,
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
    pub attachments: Map<AstAttachment>,
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
            attachments: Map::new(),
        }
    }

    pub fn with_attachment<A: CloneAny + Send + Sync>(mut self, attachment: A) -> Self {
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
    pub attachments: Map<AstAttachment>,
}

impl AstExpression {
    pub fn new(expr: AstExpressionImpl) -> Self {
        Self {
            expr,
            attachments: Default::default(),
        }
    }

    pub fn with_attachment<A: CloneAny + Send + Sync>(mut self, attachment: A) -> Self {
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
    Block(AstBlock),
    Constant(AstConstant),
    BinaryOp {
        ty: BinaryOpType,
        left: Box<AstExpression>,
        right: Box<AstExpression>,
    },
    UnaryOp {
        ty: UnaryOpType,
        expr: Box<AstExpression>,
    },
    VarDeclare {
        name: String,
        mutable: bool,
    },
    VarAssign {
        name: String,
        assign: Box<AstExpression>,
    },
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
    Terminated(Box<AstExpression>),
    Return(Box<AstExpression>),
    Break(Option<String>),
    Continue(Option<String>),
    IfElse {
        start: AstIfBlock,
        chain: Vec<AstIfBlock>,
        end: Option<AstIfBlock>,
    },
    While {
        condition: Box<AstExpression>,
        block: AstBlock,
    },
    For {
        declares: Box<AstExpression>,
        condition: Box<AstExpression>,
        after: Box<AstExpression>,
        block: AstBlock,
    },
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
    pub block: AstBlock,
    pub attachments: Map<AstAttachment>,
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
    pub attachments: Map<AstAttachment>,
}

impl PartialEq for AstBlock {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.exprs == other.exprs
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum AstConstant {
    Boolean(bool),
    /// RGBA color
    Color([f32; 4]),
    Complex(Complex),
    Integer(i32),
    #[default]
    Unit,
}

impl AstConstant {
    pub fn ty(&self) -> ExpressionType {
        match self {
            AstConstant::Boolean(_) => ExpressionType::Boolean,
            AstConstant::Color(_) => ExpressionType::Color,
            AstConstant::Complex(_) => ExpressionType::Complex,
            AstConstant::Integer(_) => ExpressionType::Integer,
            AstConstant::Unit => ExpressionType::Unit,
        }
    }
}

/// A variable name and type
#[derive(Debug, Clone)]
pub struct AstVariable {
    pub name: String,
    pub ty: ExpressionType,
    pub init: Option<AstConstant>,
    pub annotations: Vec<AstAnnotation>,
    pub attachments: Map<AstAttachment>,
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
    pub attachments: Map<AstAttachment>,
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
    Power,
    Equals,
    NotEquals,
    And,
    AndLazy,
    Or,
    OrLazy,
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
