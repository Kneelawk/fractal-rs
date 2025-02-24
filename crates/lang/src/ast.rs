//! Fractal program AST constructs.

use crate::{ExpressionType, VariableDeclaration};
use anymap::{CloneAny, Map};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use rug::{Complex, Float};

/// Dynamic traits implemented by all ast attachments
pub type AstAttachment = dyn CloneAny + Send + Sync;

/// A full program
#[derive(Default, Debug, Clone)]
pub struct AstProgram {
    pub functions: HashMap<String, AstFunction>,
    pub globals: HashMap<String, VariableDeclaration>,
    pub attachments: Map<AstAttachment>,
}

#[derive(Debug, Clone)]
pub struct AstFunction {
    pub name: String,
    pub args: Vec<VariableDeclaration>,
    pub return_type: ExpressionType,
    pub expr: AstExpression,
    pub attachments: Map<AstAttachment>,
}

impl AstFunction {
    pub fn new(name: String, args: Vec<VariableDeclaration>, return_type: ExpressionType) -> Self {
        Self {
            name,
            args,
            return_type,
            expr: Default::default(),
            attachments: Map::new(),
        }
    }

    pub fn with_attachment<A: CloneAny + Send + Sync>(mut self, attachment: A) -> Self {
        self.attachments.insert(attachment);
        self
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

#[derive(Debug, Clone)]
pub enum AstExpressionImpl {
    Block {
        block: AstBlock,
    },
    Constant {
        value: AstConstant,
    },
    BinaryOp {
        ty: BinaryOpType,
        left: Box<AstExpression>,
        right: Box<AstExpression>,
    },
    UnaryOp {
        ty: UnaryOpType,
        expr: Box<AstExpression>,
    },
    FnCall {
        name: String,
        args: Vec<AstExpression>,
    },
    VarDeclare {
        name: String,
    },
    VarAssign {
        name: String,
        assign: Box<AstExpression>,
    },
    VarDeclareAssign {
        name: String,
        assign: Box<AstExpression>,
    },
    Return {
        expr: Box<AstExpression>,
    },
    Break {
        block: Option<String>,
    },
    Continue {
        block: Option<String>,
    },
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
}

impl Default for AstExpressionImpl {
    fn default() -> Self {
        Self::Block {
            block: Default::default(),
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct AstIfBlock {
    pub condition: Box<AstExpression>,
    pub block: AstBlock,
    pub attachments: Map<AstAttachment>,
}

#[derive(Default, Debug, Clone)]
pub struct AstBlock {
    pub name: Option<String>,
    pub exprs: Vec<AstExpression>,
    pub attachments: Map<AstAttachment>,
}

#[derive(Default, Debug, Clone)]
pub enum AstConstant {
    Boolean(bool),
    /// RGBA color
    Color([f32; 4]),
    Complex(Complex),
    Integer(i32),
    Number(Float),
    #[default]
    Unit,
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
