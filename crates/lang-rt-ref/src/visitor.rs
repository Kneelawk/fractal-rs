use crate::error::RuntimeError;
use fractal_rs_3_lang::ast::visitor::{AstVisitor, VisitResult};
use fractal_rs_3_lang::ast::{
    AstAnnotation, AstBlock, AstExpression, AstExpressionImpl, AstFunction, AstIfBlock, AstProgram,
    AstVariable, BinaryOpType,
};
use fractal_rs_3_lang::parser::ProgramSpan;
use fractal_rs_3_lang::{ExpressionType, ExpressionValue};
use rug::Complex;
use rug::ops::Pow;
use std::borrow::Cow;
use std::collections::HashMap;

pub struct ReferenceRuntime<'a> {
    program: Cow<'a, AstProgram>,
}

impl<'a> ReferenceRuntime<'a> {
    pub fn new(program: Cow<'a, AstProgram>) -> Self {
        Self { program }
    }

    pub fn run(
        &self,
        prec: u32,
        globals: HashMap<String, ExpressionValue>,
        fn_name: impl AsRef<str>,
        args: Vec<ExpressionValue>,
    ) -> Result<ExpressionValue, RuntimeError> {
        let mut fn_args = HashMap::new();
        if let Some(fun) = self.program.functions.get(fn_name.as_ref()) {
            if args.len() != fun.args.len() {
                return Err(RuntimeError::function_incorrect_number_of_args(
                    fn_name.as_ref().to_string(),
                    fun.args.len(),
                    args.len(),
                    None,
                ));
            }

            for (index, arg) in fun.args.iter().enumerate() {
                fn_args.insert(arg.name.clone(), args[index].clone());
            }
        } else {
            return Err(RuntimeError::function_not_found(
                fn_name.as_ref().to_string(),
                None,
            ));
        }

        let mut visitor = RuntimeVisitor {
            program: &self.program,
            scopes: vec![globals, fn_args],
            prec,
        };
        visitor.visit_function(&self.program.functions[fn_name.as_ref()])
    }
}

struct RuntimeVisitor<'a> {
    program: &'a AstProgram,
    scopes: Vec<HashMap<String, ExpressionValue>>,
    prec: u32,
}

struct ReferenceRuntimeResult;
impl VisitResult for ReferenceRuntimeResult {
    type Program = ();
    type Function = Result<ExpressionValue, RuntimeError>;
    type Expression = Result<ExpressionValue, RuntimeError>;
    type IfBlock = Result<Option<ExpressionValue>, RuntimeError>;
    type Block = Result<ExpressionValue, RuntimeError>;
    type Variable = ();
    type Annotation = ();
}

impl RuntimeVisitor<'_> {
    fn push_scope(&mut self) {
        self.scopes.push(Default::default());
    }

    fn pop_scope(&mut self) {
        self.scopes
            .pop()
            .expect("Tried to pop an empty scope - mismatched push/pop");
    }
}

impl AstVisitor<ReferenceRuntimeResult> for RuntimeVisitor<'_> {
    fn visit_program(&mut self, _program: &AstProgram) {
        unimplemented!("rt-ref RuntimeVisitor should never have visit_program called")
    }

    fn visit_function(&mut self, function: &AstFunction) -> Result<ExpressionValue, RuntimeError> {
        todo!()
    }

    fn visit_expression(
        &mut self,
        expression: &AstExpression,
    ) -> Result<ExpressionValue, RuntimeError> {
        match &expression.expr {
            AstExpressionImpl::Block(b) => self.visit_block(&b),
            AstExpressionImpl::Constant(c) => Ok(c.clone()),
            AstExpressionImpl::BinaryOp { ty, left, right } => {
                let left = self.visit_expression(left)?;
                let right = || self.visit_expression(right);
                match ty {
                    BinaryOpType::Plus => simple_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("plus op missing span"),
                    ),
                    BinaryOpType::Minus => simple_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("minus op missing span"),
                    ),
                    BinaryOpType::Times => simple_ops2(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("times op missing span"),
                        self.prec,
                    ),
                    BinaryOpType::Divide => simple_ops2(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("divide op missing span"),
                        self.prec,
                    ),
                    BinaryOpType::Modulo => simple_ops2(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("modulo op missing span"),
                        self.prec,
                    ),
                    BinaryOpType::Power => simple_ops2(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("power op missing span"),
                        self.prec,
                    ),
                    BinaryOpType::LeftShift => bitwise_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("left-shift op missing span"),
                    ),
                    BinaryOpType::RightShift => bitwise_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("right-shift op missing span"),
                    ),
                    BinaryOpType::Equals => equals_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("equals op missing span"),
                    ),
                    BinaryOpType::NotEquals => equals_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("not-equals op missing span"),
                    ),
                    BinaryOpType::And => bitwise_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("and op missing span"),
                    ),
                    BinaryOpType::AndLazy => {
                        let aty = left.ty();
                        let ExpressionValue::Boolean(a) = left else {
                            // FIXME: no way to tell unevaluated code type without expression-type assignment
                            return Err(RuntimeError::incompatible_operator(
                                *ty,
                                aty,
                                ExpressionType::Boolean,
                                Some(expression.span().expect("and-lazy op missing span").clone()),
                            ));
                        };

                        if !a {
                            Ok(ExpressionValue::Boolean(false))
                        } else {
                            let b = right()?;
                            let bty = b.ty();
                            let ExpressionValue::Boolean(b) = b else {
                                return Err(RuntimeError::incompatible_operator(
                                    *ty,
                                    aty,
                                    bty,
                                    Some(
                                        expression
                                            .span()
                                            .expect("and-lazy op missing span")
                                            .clone(),
                                    ),
                                ));
                            };
                            Ok(ExpressionValue::Boolean(b))
                        }
                    }
                    BinaryOpType::Or => bitwise_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("or op missing span"),
                    ),
                    BinaryOpType::OrLazy => {
                        let aty = left.ty();
                        let ExpressionValue::Boolean(a) = left else {
                            // FIXME: no way to tell unevaluated code type without expression-type assignment
                            return Err(RuntimeError::incompatible_operator(
                                *ty,
                                aty,
                                ExpressionType::Boolean,
                                Some(expression.span().expect("or-lazy op missing span").clone()),
                            ));
                        };

                        if a {
                            Ok(ExpressionValue::Boolean(true))
                        } else {
                            let b = right()?;
                            let bty = b.ty();
                            let ExpressionValue::Boolean(b) = b else {
                                return Err(RuntimeError::incompatible_operator(
                                    *ty,
                                    aty,
                                    bty,
                                    Some(
                                        expression.span().expect("or-lazy op missing span").clone(),
                                    ),
                                ));
                            };
                            Ok(ExpressionValue::Boolean(b))
                        }
                    }
                    BinaryOpType::Xor => bitwise_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("xor op missing span"),
                    ),
                    BinaryOpType::LessThan => compare_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("less-than op missing span"),
                    ),
                    BinaryOpType::LessEqual => compare_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("less-equals op missing span"),
                    ),
                    BinaryOpType::GreaterThan => compare_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("greater-than op missing span"),
                    ),
                    BinaryOpType::GreaterEqual => compare_ops(
                        *ty,
                        left,
                        right()?,
                        expression.span().expect("greater-equals op missing span"),
                    ),
                }
            }
            AstExpressionImpl::UnaryOp { .. } => {}
            AstExpressionImpl::VarDeclare { .. } => {}
            AstExpressionImpl::VarAssign { .. } => {}
            AstExpressionImpl::VarDeclareAssign { .. } => {}
            AstExpressionImpl::VarUse(_) => {}
            AstExpressionImpl::FnCall { .. } => {}
            AstExpressionImpl::Terminated(_) => {}
            AstExpressionImpl::Return(_) => {}
            AstExpressionImpl::Break(_) => {}
            AstExpressionImpl::Continue(_) => {}
            AstExpressionImpl::IfElse { .. } => {}
            AstExpressionImpl::While { .. } => {}
            AstExpressionImpl::For { .. } => {}
            AstExpressionImpl::Error => {}
        }
    }

    fn visit_if_block(
        &mut self,
        if_block: &AstIfBlock,
    ) -> Result<Option<ExpressionValue>, RuntimeError> {
        let condition = self.visit_expression(&if_block.condition)?;
        match condition {
            ExpressionValue::Boolean(b) => {
                if b {
                    Ok(Some(self.visit_expression(&if_block.block)?))
                } else {
                    Ok(None)
                }
            }
            _ => Err(RuntimeError::wrong_expression_type(
                ExpressionType::Boolean,
                condition.ty(),
                Some(
                    if_block
                        .attachments
                        .get::<ProgramSpan>()
                        .expect("if_block missing span")
                        .clone(),
                ),
            )),
        }
    }

    fn visit_block(&mut self, block: &AstBlock) -> Result<ExpressionValue, RuntimeError> {
        self.push_scope();

        let mut res = ExpressionValue::Unit;
        for expr in &block.exprs {
            // TODO: continue/break vertical control-flow handling
            res = self.visit_expression(expr)?;
        }

        self.pop_scope();

        Ok(res)
    }

    fn visit_variable(&mut self, _variable: &AstVariable) {
        unimplemented!("rt-ref RuntimeVisitor should never have visit_variable called")
    }

    fn visit_annotation(&mut self, _annotation: &AstAnnotation) {
        unimplemented!("rt-ref RuntimeVisitor should never have visit_annotation called")
    }
}

fn simple_ops(
    op: BinaryOpType,
    a: ExpressionValue,
    b: ExpressionValue,
    span: &ProgramSpan,
) -> Result<ExpressionValue, RuntimeError> {
    let aty = a.ty();
    let bty = b.ty();
    match a {
        ExpressionValue::Color(ca) => {
            if let ExpressionValue::Color(cb) = b {
                let do_op = match op {
                    BinaryOpType::Plus => {
                        fn f(a: f32, b: f32) -> f32 {
                            a + b
                        }
                        f
                    }
                    BinaryOpType::Minus => {
                        fn f(a: f32, b: f32) -> f32 {
                            a - b
                        }
                        f
                    }
                    _ => unreachable!("unsupported simple_ops: {op}"),
                };
                Ok(ExpressionValue::Color([
                    do_op(ca[0], cb[0]),
                    do_op(ca[1], cb[1]),
                    do_op(ca[2], cb[2]),
                    do_op(ca[3], cb[3]),
                ]))
            } else {
                Err(RuntimeError::incompatible_operator(
                    op,
                    aty,
                    bty,
                    Some(span.clone()),
                ))
            }
        }
        ExpressionValue::Complex(ca) => match b {
            ExpressionValue::Complex(cb) => match op {
                BinaryOpType::Plus => Ok(ExpressionValue::Complex(ca + cb)),
                BinaryOpType::Minus => Ok(ExpressionValue::Complex(ca - cb)),
                _ => unreachable!("unsupported simple_ops: {op}"),
            },
            ExpressionValue::Integer(ib) => match op {
                BinaryOpType::Plus => Ok(ExpressionValue::Complex(ca + ib)),
                BinaryOpType::Minus => Ok(ExpressionValue::Complex(ca - ib)),
                _ => unreachable!("unsupported simple_ops: {op}"),
            },
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        ExpressionValue::Integer(ia) => match b {
            ExpressionValue::Complex(cb) => match op {
                BinaryOpType::Plus => Ok(ExpressionValue::Complex(ia + cb)),
                BinaryOpType::Minus => Ok(ExpressionValue::Complex(ia - cb)),
                _ => unreachable!("unsupported simple_ops: {op}"),
            },
            ExpressionValue::Integer(ib) => match op {
                BinaryOpType::Plus => Ok(ExpressionValue::Integer(ia + ib)),
                BinaryOpType::Minus => Ok(ExpressionValue::Integer(ia - ib)),
                _ => unreachable!("unsupported simple_ops: {op}"),
            },
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        _ => Err(RuntimeError::incompatible_operator(
            op,
            aty,
            bty,
            Some(span.clone()),
        )),
    }
}

fn simple_ops2(
    op: BinaryOpType,
    a: ExpressionValue,
    b: ExpressionValue,
    span: &ProgramSpan,
    prec: u32,
) -> Result<ExpressionValue, RuntimeError> {
    let aty = a.ty();
    let bty = b.ty();
    let do_op = match op {
        BinaryOpType::Times => {
            fn f(a: f32, b: f32) -> f32 {
                a * b
            }
            f
        }
        BinaryOpType::Divide => {
            fn f(a: f32, b: f32) -> f32 {
                a / b
            }
            f
        }
        BinaryOpType::Modulo => {
            fn f(a: f32, b: f32) -> f32 {
                a % b
            }
            f
        }
        BinaryOpType::Power => {
            fn f(a: f32, b: f32) -> f32 {
                a.powf(b)
            }
            f
        }
        _ => unreachable!("unsupported simple_ops2: {op}"),
    };

    match a {
        ExpressionValue::Color(ca) => match b {
            ExpressionValue::Color(cb) => Ok(ExpressionValue::Color([
                do_op(ca[0], cb[0]),
                do_op(ca[1], cb[1]),
                do_op(ca[2], cb[2]),
                do_op(ca[3], cb[3]),
            ])),
            ExpressionValue::Complex(cb) => {
                let b = cb.real().to_f32();
                Ok(ExpressionValue::Color([
                    do_op(ca[0], b),
                    do_op(ca[1], b),
                    do_op(ca[2], b),
                    do_op(ca[3], b),
                ]))
            }
            ExpressionValue::Integer(ib) => {
                let b = ib as f32;
                Ok(ExpressionValue::Color([
                    do_op(ca[0], b),
                    do_op(ca[1], b),
                    do_op(ca[2], b),
                    do_op(ca[3], b),
                ]))
            }
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        ExpressionValue::Complex(ca) => match b {
            ExpressionValue::Color(cb) => {
                let a = ca.real().to_f32();
                Ok(ExpressionValue::Color([
                    do_op(a, cb[0]),
                    do_op(a, cb[1]),
                    do_op(a, cb[2]),
                    do_op(a, cb[3]),
                ]))
            }
            ExpressionValue::Complex(cb) => match op {
                BinaryOpType::Times => Ok(ExpressionValue::Complex(ca * cb)),
                BinaryOpType::Divide => Ok(ExpressionValue::Complex(ca / cb)),
                BinaryOpType::Modulo => Ok(ExpressionValue::Complex(Complex::with_val(
                    prec,
                    ca.real() % cb.real(),
                ))),
                BinaryOpType::Power => Ok(ExpressionValue::Complex(ca.pow(cb))),
                _ => unreachable!("unsupported simple_ops2: {op}"),
            },
            ExpressionValue::Integer(ib) => match op {
                BinaryOpType::Times => Ok(ExpressionValue::Complex(ca * ib)),
                BinaryOpType::Divide => Ok(ExpressionValue::Complex(ca / ib)),
                BinaryOpType::Modulo => Ok(ExpressionValue::Complex(Complex::with_val(
                    prec,
                    ca.real() % ib,
                ))),
                BinaryOpType::Power => Ok(ExpressionValue::Complex(ca.pow(ib))),
                _ => unreachable!("unsupported simple_ops2: {op}"),
            },
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        ExpressionValue::Integer(ia) => match b {
            ExpressionValue::Color(cb) => {
                let a = ia as f32;
                Ok(ExpressionValue::Color([
                    do_op(a, cb[0]),
                    do_op(a, cb[1]),
                    do_op(a, cb[2]),
                    do_op(a, cb[3]),
                ]))
            }
            ExpressionValue::Complex(cb) => match op {
                BinaryOpType::Times => Ok(ExpressionValue::Complex(ia * cb)),
                BinaryOpType::Divide => Ok(ExpressionValue::Complex(ia / cb)),
                BinaryOpType::Modulo => Ok(ExpressionValue::Complex(Complex::with_val(
                    prec,
                    ia % cb.real(),
                ))),
                BinaryOpType::Power => Ok(ExpressionValue::Complex(
                    Complex::with_val(prec, ia).pow(cb),
                )),
                _ => unreachable!("unsupported simple_ops2: {op}"),
            },
            ExpressionValue::Integer(ib) => match op {
                BinaryOpType::Times => Ok(ExpressionValue::Integer(ia * ib)),
                BinaryOpType::Divide => Ok(ExpressionValue::Integer(ia / ib)),
                BinaryOpType::Modulo => Ok(ExpressionValue::Integer(ia % ib)),
                BinaryOpType::Power => Ok(ExpressionValue::Integer(ia.pow(ib.max(0) as u32))),
                _ => unreachable!("unsupported simple_ops2: {op}"),
            },
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        _ => Err(RuntimeError::incompatible_operator(
            op,
            aty,
            bty,
            Some(span.clone()),
        )),
    }
}

fn bitwise_ops(
    op: BinaryOpType,
    a: ExpressionValue,
    b: ExpressionValue,
    span: &ProgramSpan,
) -> Result<ExpressionValue, RuntimeError> {
    let aty = a.ty();
    let bty = b.ty();

    if let ExpressionValue::Boolean(a) = a {
        let ExpressionValue::Boolean(b) = b else {
            return Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            ));
        };

        return match op {
            BinaryOpType::And => Ok(ExpressionValue::Boolean(a & b)),
            BinaryOpType::Or => Ok(ExpressionValue::Boolean(a | b)),
            BinaryOpType::Xor => Ok(ExpressionValue::Boolean(a != b)),
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        };
    }

    let ExpressionValue::Integer(a) = a else {
        return Err(RuntimeError::incompatible_operator(
            op,
            aty,
            bty,
            Some(span.clone()),
        ));
    };
    let ExpressionValue::Integer(b) = b else {
        return Err(RuntimeError::incompatible_operator(
            op,
            aty,
            bty,
            Some(span.clone()),
        ));
    };

    match op {
        BinaryOpType::LeftShift => Ok(ExpressionValue::Integer(a << b)),
        BinaryOpType::RightShift => Ok(ExpressionValue::Integer(a >> b)),
        BinaryOpType::And => Ok(ExpressionValue::Integer(a & b)),
        BinaryOpType::Or => Ok(ExpressionValue::Integer(a | b)),
        BinaryOpType::Xor => Ok(ExpressionValue::Integer(a ^ b)),
        _ => unreachable!("unsupported bitwise_op: {op}"),
    }
}

fn equals_ops(
    op: BinaryOpType,
    a: ExpressionValue,
    b: ExpressionValue,
    span: &ProgramSpan,
) -> Result<ExpressionValue, RuntimeError> {
    let aty = a.ty();
    let bty = b.ty();

    match a {
        ExpressionValue::Boolean(ba) => {
            if let ExpressionValue::Boolean(bb) = b {
                match op {
                    BinaryOpType::Equals => Ok(ExpressionValue::Boolean(ba == bb)),
                    BinaryOpType::NotEquals => Ok(ExpressionValue::Boolean(ba != bb)),
                    _ => unreachable!("unsupported equals_op: {op}"),
                }
            } else {
                Err(RuntimeError::incompatible_operator(
                    op,
                    aty,
                    bty,
                    Some(span.clone()),
                ))
            }
        }
        ExpressionValue::Color(ca) => {
            if let ExpressionValue::Color(cb) = b {
                match op {
                    BinaryOpType::Equals => Ok(ExpressionValue::Boolean(ca == cb)),
                    BinaryOpType::NotEquals => Ok(ExpressionValue::Boolean(ca != cb)),
                    _ => unreachable!("unsupported equals_op: {op}"),
                }
            } else {
                Err(RuntimeError::incompatible_operator(
                    op,
                    aty,
                    bty,
                    Some(span.clone()),
                ))
            }
        }
        ExpressionValue::Complex(ca) => match b {
            ExpressionValue::Complex(cb) => match op {
                BinaryOpType::Equals => Ok(ExpressionValue::Boolean(ca == cb)),
                BinaryOpType::NotEquals => Ok(ExpressionValue::Boolean(ca != cb)),
                _ => unreachable!("unsupported equals_op: {op}"),
            },
            ExpressionValue::Integer(ib) => match op {
                BinaryOpType::Equals => Ok(ExpressionValue::Boolean(ca == ib)),
                BinaryOpType::NotEquals => Ok(ExpressionValue::Boolean(ca != ib)),
                _ => unreachable!("unsupported equals_op: {op}"),
            },
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        ExpressionValue::Integer(ia) => match b {
            ExpressionValue::Complex(cb) => match op {
                BinaryOpType::Equals => Ok(ExpressionValue::Boolean(ia == cb)),
                BinaryOpType::NotEquals => Ok(ExpressionValue::Boolean(ia != cb)),
                _ => unreachable!("unsupported equals_op: {op}"),
            },
            ExpressionValue::Integer(ib) => match op {
                BinaryOpType::Equals => Ok(ExpressionValue::Boolean(ia == ib)),
                BinaryOpType::NotEquals => Ok(ExpressionValue::Boolean(ia != ib)),
                _ => unreachable!("unsupported equals_op: {op}"),
            },
            _ => unreachable!("unsupported equals_op: {op}"),
        },
        ExpressionValue::Unit => {
            if let ExpressionValue::Unit = b {
                Ok(ExpressionValue::Boolean(op == BinaryOpType::Equals))
            } else {
                Err(RuntimeError::incompatible_operator(
                    op,
                    aty,
                    bty,
                    Some(span.clone()),
                ))
            }
        }
    }
}

fn compare_ops(
    op: BinaryOpType,
    a: ExpressionValue,
    b: ExpressionValue,
    span: &ProgramSpan,
) -> Result<ExpressionValue, RuntimeError> {
    let aty = a.ty();
    let bty = b.ty();

    match a {
        ExpressionValue::Complex(a) => match b {
            ExpressionValue::Complex(b) => Ok(ExpressionValue::Boolean(match op {
                BinaryOpType::LessThan => a.real() < b.real(),
                BinaryOpType::LessEqual => a.real() <= b.real(),
                BinaryOpType::GreaterThan => a.real() > b.real(),
                BinaryOpType::GreaterEqual => a.real() >= b.real(),
                _ => unreachable!("unsupported compare_op: {op}"),
            })),
            ExpressionValue::Integer(b) => Ok(ExpressionValue::Boolean(match op {
                BinaryOpType::LessThan => a.real() < &b,
                BinaryOpType::LessEqual => a.real() <= &b,
                BinaryOpType::GreaterThan => a.real() > &b,
                BinaryOpType::GreaterEqual => a.real() >= &b,
                _ => unreachable!("unsupported compare_op: {op}"),
            })),
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        ExpressionValue::Integer(a) => match b {
            ExpressionValue::Complex(b) => Ok(ExpressionValue::Boolean(match op {
                BinaryOpType::LessThan => &a < b.real(),
                BinaryOpType::LessEqual => &a <= b.real(),
                BinaryOpType::GreaterThan => &a > b.real(),
                BinaryOpType::GreaterEqual => &a >= b.real(),
                _ => unreachable!("unsupported compare_op: {op}"),
            })),
            ExpressionValue::Integer(b) => Ok(ExpressionValue::Boolean(match op {
                BinaryOpType::LessThan => a < b,
                BinaryOpType::LessEqual => a <= b,
                BinaryOpType::GreaterThan => a > b,
                BinaryOpType::GreaterEqual => a >= b,
                _ => unreachable!("unsupported compare_op: {op}"),
            })),
            _ => Err(RuntimeError::incompatible_operator(
                op,
                aty,
                bty,
                Some(span.clone()),
            )),
        },
        _ => Err(RuntimeError::incompatible_operator(
            op,
            aty,
            bty,
            Some(span.clone()),
        )),
    }
}
