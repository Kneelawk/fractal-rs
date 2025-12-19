//! Fractal program parser constructs.

mod lexer;
mod span;

use crate::ast::{
    AstAnnotation, AstAnnotationArg, AstBlock, AstExpression, AstExpressionImpl, AstFunction,
    AstIfBlock, AstProgram, AstVariable, BinaryOpType, UnaryOpType,
};
use crate::parser::lexer::{LexerToken, lexer};
use crate::parser::span::mk_span;
use crate::{ExpressionType, ExpressionValue, ast_expr};
use chumsky::input::ValueInput;
use chumsky::pratt::{infix, left, postfix, prefix, right};
use chumsky::prelude::*;
use fractal_rs_3_utils::any_map;
use fractal_rs_3_utils::anymap::AnyMap;
use rug::Complex;
// export these
pub use span::ProgramSource;
pub use span::ProgramSourceSet;
pub use span::ProgramSpan;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct Spanned<T>(T, SimpleSpan);

type ProgramExtra<'src> = extra::Full<Rich<'src, LexerToken<'src>>, (), ProgramSource>;

enum ProgramComponent {
    Global(AstVariable),
    Function(AstFunction),
}

pub fn parse(source: ProgramSource, prec: u32) -> AstProgram {
    let tokens = lexer(prec).parse(source.code()).unwrap();

    let parser = parser(prec).with_ctx(source.clone());

    Parser::<_, _, ProgramExtra>::parse(
        &parser,
        tokens
            .as_slice()
            .map((tokens.len()..tokens.len()).into(), |spanned| {
                (&spanned.0, &spanned.1)
            }),
    )
    .unwrap()
}

fn ident<'src, I>() -> impl Parser<'src, I, &'src str, ProgramExtra<'src>> + Copy
where
    I: ValueInput<'src, Token = LexerToken<'src>, Span = SimpleSpan>,
{
    select! { LexerToken::Ident(s) => s }.labelled("identifier")
}

fn constant<'src, I>(prec: u32) -> impl Parser<'src, I, ExpressionValue, ProgramExtra<'src>> + Copy
where
    I: ValueInput<'src, Token = LexerToken<'src>, Span = SimpleSpan>,
{
    select! {
        LexerToken::Boolean(b) => ExpressionValue::Boolean(b),
        LexerToken::Color(c) => ExpressionValue::Color(c),
        LexerToken::RealInteger(i) => ExpressionValue::Integer(i),
        LexerToken::RealNumber(n) => ExpressionValue::Complex(Complex::with_val(prec, (n, 0))),
        LexerToken::ImaginaryInteger(i) => ExpressionValue::Complex(Complex::with_val(prec, (0, i))),
        LexerToken::ImaginaryNumber(n) => ExpressionValue::Complex(Complex::with_val(prec, (0, n))),
    }
        .labelled("value")
}

fn expr<'src, I>(
    ident: impl Parser<'src, I, &'src str, ProgramExtra<'src>> + Copy + 'src,
    constant: impl Parser<'src, I, ExpressionValue, ProgramExtra<'src>> + Copy + 'src,
) -> impl Parser<'src, I, AstExpression, ProgramExtra<'src>>
where
    I: ValueInput<'src, Token = LexerToken<'src>, Span = SimpleSpan>,
{
    let lifetime = select! { LexerToken::Lifetime(name) => name }.labelled("lifetime");

    let assign_type = select! {
        LexerToken::Op("=") => None,
        LexerToken::Op("+=") => Some(BinaryOpType::Plus),
        LexerToken::Op("-=") => Some(BinaryOpType::Minus),
        LexerToken::Op("*=") => Some(BinaryOpType::Times),
        LexerToken::Op("/=") => Some(BinaryOpType::Divide),
        LexerToken::Op("%=") => Some(BinaryOpType::Modulo),
        LexerToken::Op("^=") => Some(BinaryOpType::Power),
        LexerToken::Op("<<=") => Some(BinaryOpType::LeftShift),
        LexerToken::Op(">>=") => Some(BinaryOpType::RightShift),
        LexerToken::Op("&=") => Some(BinaryOpType::And),
        LexerToken::Op("~=") => Some(BinaryOpType::Xor),
        LexerToken::Op("|=") => Some(BinaryOpType::Or),
    };

    recursive(move |expr| {
        let stmt = expr
            .clone()
            .then(just(LexerToken::Terminator).or_not())
            .map_with(|(expr, term), m| {
                if term == Some(LexerToken::Terminator) {
                    ast_expr!(Terminated(Box::new(expr))).with_attachment(mk_span(m))
                } else {
                    expr
                }
            });

        let block = lifetime
            .then_ignore(just(LexerToken::Delim(':')))
            .or_not()
            .then_ignore(just(LexerToken::Delim('{')))
            .then(stmt.clone().repeated().collect::<Vec<_>>())
            .then_ignore(just(LexerToken::Delim('}')))
            .map_with(|(name, exprs), m| {
                AstExpression::new(AstExpressionImpl::Block(AstBlock {
                    exprs,
                    name: name.map(str::to_string),
                    attachments: any_map![mk_span(m)],
                }))
                .with_attachment(mk_span(m))
            });

        let items = expr
            .clone()
            .separated_by(just(LexerToken::Delim(',')))
            .allow_trailing()
            .collect::<Vec<_>>();

        let let_ = just(LexerToken::Let)
            .ignore_then(just(LexerToken::Mut).or_not())
            .then(ident)
            .then_ignore(just(LexerToken::Op("=")))
            .then(expr.clone())
            .map_with(|((mut_, name), value), m| {
                AstExpression::new(AstExpressionImpl::VarDeclareAssign {
                    name: name.to_string(),
                    assign: Box::new(value),
                    mutable: mut_ == Some(LexerToken::Mut),
                })
                .with_attachment(mk_span(m))
            });

        let assign =
            ident
                .then(assign_type)
                .then(expr.clone())
                .map_with(|((name, ty), expr), m| {
                    ast_expr!(VarAssign {
                        name: name.to_string(),
                        ty,
                        assign: Box::new(expr),
                    })
                    .with_attachment(mk_span(m))
                });

        let if_ = just(LexerToken::If)
            .ignore_then(
                expr.clone()
                    .delimited_by(just(LexerToken::Delim('(')), just(LexerToken::Delim(')'))),
            )
            .then(stmt.clone())
            .map_with(|(condition, true_expr), m| AstIfBlock {
                condition: Box::new(condition),
                block: Box::new(true_expr),
                attachments: any_map![mk_span(m)],
            })
            .then(just(LexerToken::Else).ignore_then(stmt.clone()).or_not())
            .map_with(|(if_block, false_expr), m| {
                ast_expr!(IfElse {
                    start: if_block,
                    chain: Default::default(),
                    end: false_expr.map(Box::new),
                })
                .with_attachment(mk_span(m))
            });

        let while_ = just(LexerToken::While)
            .ignore_then(
                expr.clone()
                    .delimited_by(just(LexerToken::Delim('(')), just(LexerToken::Delim(')'))),
            )
            .then(expr.clone())
            .map_with(|(condition, loop_expr), m| {
                ast_expr!(While {
                    condition: Box::new(condition),
                    block: Box::new(loop_expr),
                })
                .with_attachment(mk_span(m))
            });

        let for_ = just(LexerToken::For)
            .ignore_then(
                expr.clone()
                    .then(
                        just(LexerToken::Terminator)
                            .ignore_then(expr.clone())
                            .then_ignore(just(LexerToken::Terminator))
                            .then(expr.clone()),
                    )
                    .delimited_by(just(LexerToken::Delim('(')), just(LexerToken::Delim(')'))),
            )
            .then(expr.clone())
            .map_with(|((declares, (condition, after)), loop_expr), m| {
                ast_expr!(For {
                    declares: Box::new(declares),
                    condition: Box::new(condition),
                    after: Box::new(after),
                    block: Box::new(loop_expr),
                })
                .with_attachment(mk_span(m))
            });

        let parens = just(LexerToken::Delim('('))
            .ignore_then(stmt.clone())
            .then_ignore(just(LexerToken::Delim(')')))
            .map_with(|expr, m| expr.with_attachment(mk_span(m)));

        let call = ident
            .then(items.delimited_by(just(LexerToken::Delim('(')), just(LexerToken::Delim(')'))))
            .map_with(|(name, args), m| {
                AstExpression::new(AstExpressionImpl::FnCall {
                    name: name.to_string(),
                    args,
                })
                .with_attachment(mk_span(m))
            });

        let local = ident
            .map_with(|s, m| {
                AstExpression::new(AstExpressionImpl::VarUse(s.to_string()))
                    .with_attachment(mk_span(m))
            })
            .labelled("local");

        let atom = constant
            .map(|c| AstExpression::new(AstExpressionImpl::Constant(c)))
            .or(let_)
            .or(if_)
            .or(while_)
            .or(for_)
            .or(call)
            .or(assign)
            .or(local)
            .or(parens)
            .or(block)
            .recover_with(via_parser(nested_delimiters(
                LexerToken::Delim('('),
                LexerToken::Delim(')'),
                [
                    (LexerToken::Delim('['), LexerToken::Delim(']')),
                    (LexerToken::Delim('{'), LexerToken::Delim('}')),
                ],
                |span| AstExpression::new(AstExpressionImpl::Error),
            )))
            .recover_with(via_parser(nested_delimiters(
                LexerToken::Delim('{'),
                LexerToken::Delim('}'),
                [
                    (LexerToken::Delim('['), LexerToken::Delim(']')),
                    (LexerToken::Delim('('), LexerToken::Delim(')')),
                ],
                |span| AstExpression::new(AstExpressionImpl::Error),
            )));

        let op = |s| just(LexerToken::Op(s));

        atom.pratt((
            infix(right(13), op("^"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Power,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            postfix(12, op("++"), |e, _, m| {
                ast_expr!(UnaryOp {
                    ty: UnaryOpType::PostIncrement,
                    expr: Box::new(e),
                })
                .with_attachment(mk_span(m))
            }),
            postfix(12, op("--"), |e, _, m| {
                ast_expr!(UnaryOp {
                    ty: UnaryOpType::PostDecrement,
                    expr: Box::new(e),
                })
                .with_attachment(mk_span(m))
            }),
            prefix(11, op("++"), |_, e, m| {
                ast_expr!(UnaryOp {
                    ty: UnaryOpType::PreIncrement,
                    expr: Box::new(e)
                })
                .with_attachment(mk_span(m))
            }),
            prefix(11, op("--"), |_, e, m| {
                ast_expr!(UnaryOp {
                    ty: UnaryOpType::PreDecrement,
                    expr: Box::new(e)
                })
                .with_attachment(mk_span(m))
            }),
            prefix(11, op("-"), |_, e, m| {
                ast_expr!(UnaryOp {
                    ty: UnaryOpType::Minus,
                    expr: Box::new(e),
                })
                .with_attachment(mk_span(m))
            }),
            prefix(11, op("!"), |_, e, m| {
                ast_expr!(UnaryOp {
                    ty: UnaryOpType::Not,
                    expr: Box::new(e),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(10), op("*"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Times,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(10), op("/"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Divide,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(10), op("%"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Modulo,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(9), op("+"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Plus,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(9), op("-"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Minus,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(8), op("<<"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::LeftShift,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(8), op(">>"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::RightShift,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(7), op("<="), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::LessEqual,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(7), op(">="), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::GreaterEqual,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(7), op("<"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::LessThan,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(7), op(">"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::GreaterThan,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(6), op("=="), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Equals,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(6), op("!="), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::NotEquals,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(5), op("&"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::And,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(4), op("~"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Xor,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(3), op("|"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::Or,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(2), op("&&"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::AndLazy,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(1), op("||"), |a, _, b, m| {
                ast_expr!(BinaryOp {
                    ty: BinaryOpType::OrLazy,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
        ))
    })
}

fn parser<'src, I>(prec: u32) -> impl Parser<'src, I, AstProgram, ProgramExtra<'src>>
where
    I: ValueInput<'src, Token = LexerToken<'src>, Span = SimpleSpan>,
{
    let ident = ident();

    let ty = select! {
        LexerToken::Ident("Boolean") => ExpressionType::Boolean,
        LexerToken::Ident("Color") => ExpressionType::Color,
        LexerToken::Ident("Complex") => ExpressionType::Complex,
        LexerToken::Ident("Integer") => ExpressionType::Integer,
        LexerToken::Ident("Unit") => ExpressionType::Unit,
    };

    let constant = constant(prec);

    let annotation_arg = select! {
        LexerToken::Ident(s) => AstAnnotationArg::Ident(s.to_string()),
        LexerToken::RealInteger(i) => AstAnnotationArg::Integer(i),
    }
    .labelled("annotation argument");

    let annotation = just(LexerToken::Delim('#'))
        .ignore_then(
            ident
                .then(
                    annotation_arg
                        .separated_by(just(LexerToken::Delim(',')))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .delimited_by(just(LexerToken::Delim('(')), just(LexerToken::Delim(')')))
                        .or_not(),
                )
                .delimited_by(just(LexerToken::Delim('[')), just(LexerToken::Delim(']'))),
        )
        .map_with(|(name, args), m| AstAnnotation {
            name: name.to_string(),
            args: args.unwrap_or_else(Vec::new),
            attachments: any_map![mk_span(m)],
        });

    let annotation_vec = annotation.repeated().collect::<Vec<_>>();

    let expr = expr(ident, constant);

    let arg_decl = annotation_vec
        .clone()
        .then(ident)
        .then_ignore(just(LexerToken::Delim(':')))
        .then(ty)
        .map_with(|((annotations, name), ty), m| AstVariable {
            name: name.to_string(),
            ty,
            init: None,
            annotations,
            attachments: any_map![mk_span(m)],
        });

    let arg_list = arg_decl
        .separated_by(just(LexerToken::Delim(',')))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(LexerToken::Delim('(')), just(LexerToken::Delim(')')));

    let function = annotation_vec
        .clone()
        .then_ignore(just(LexerToken::Fn))
        .then(ident)
        .then(arg_list)
        .then(just(LexerToken::Delim(':')).ignore_then(ty).or_not())
        .then(expr)
        .map_with(|((((annotations, name), args), ty), expr), m| AstFunction {
            name: name.to_string(),
            args,
            explicit_ret: ty,
            expr,
            annotations,
            attachments: any_map![mk_span(m)],
        });

    let global = annotation_vec
        .clone()
        .then(ident)
        .then_ignore(just(LexerToken::Op("=")))
        .then(constant)
        .map_with(|((annotations, name), value), m| AstVariable {
            name: name.to_string(),
            ty: value.ty(),
            init: Some(value),
            annotations,
            attachments: any_map![mk_span(m)],
        });

    global
        .map(ProgramComponent::Global)
        .or(function.map(ProgramComponent::Function))
        .repeated()
        .collect::<Vec<_>>()
        .map_with(|components, m| {
            let mut program = AstProgram::default();

            for component in components {
                match component {
                    ProgramComponent::Global(global) => {
                        program.globals.insert(global.name.clone(), global);
                    }
                    ProgramComponent::Function(function) => {
                        program.functions.insert(function.name.clone(), function);
                    }
                }
            }

            program.attachments.insert(mk_span(m));
            program.attachments.insert(m.ctx().clone());

            program
        })
}

#[cfg(test)]
mod tests {
    use crate::ast::{
        AstAnnotation, AstAnnotationArg, AstBlock, AstExpression, AstExpressionImpl, AstFunction,
        AstIfBlock, AstProgram, AstVariable, BinaryOpType, UnaryOpType,
    };
    use crate::parser::parse;
    use crate::parser::span::ProgramSource;
    use crate::{ExpressionType, ExpressionValue, ast_expr};
    use fractal_rs_3_utils::hash_map;
    use pretty_assertions::assert_eq;
    use rug::Complex;
    use std::collections::HashMap;

    #[test]
    fn test_simple_ast() {
        let code = "fn main() 'my_block: {x + 2}";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: {
                let mut map = HashMap::new();
                map.insert(
                    "main".to_string(),
                    AstFunction {
                        name: "main".to_string(),
                        args: vec![],
                        explicit_ret: None,
                        expr: AstExpression::new(AstExpressionImpl::Block(AstBlock {
                            name: Some("my_block".to_string()),
                            exprs: vec![AstExpression::new(AstExpressionImpl::BinaryOp {
                                ty: BinaryOpType::Plus,
                                left: Box::new(AstExpression::new(AstExpressionImpl::VarUse(
                                    "x".to_string(),
                                ))),
                                right: Box::new(AstExpression::new(AstExpressionImpl::Constant(
                                    ExpressionValue::Integer(2),
                                ))),
                            })],
                            attachments: Default::default(),
                        })),
                        annotations: vec![],
                        attachments: Default::default(),
                    },
                );
                map
            },
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_multiple_expressions() {
        let code = "fn main() 'my_block: {let y = x + 2 x + y}";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: {
                let mut map = HashMap::new();
                map.insert(
                    "main".to_string(),
                    AstFunction {
                        name: "main".to_string(),
                        args: vec![],
                        explicit_ret: None,
                        expr: AstExpression::new(AstExpressionImpl::Block(AstBlock {
                            name: Some("my_block".to_string()),
                            exprs: vec![
                                AstExpression::new(AstExpressionImpl::VarDeclareAssign {
                                    name: "y".to_string(),
                                    assign: Box::new(AstExpression::new(
                                        AstExpressionImpl::BinaryOp {
                                            ty: BinaryOpType::Plus,
                                            left: Box::new(AstExpression::new(
                                                AstExpressionImpl::VarUse("x".to_string()),
                                            )),
                                            right: Box::new(AstExpression::new(
                                                AstExpressionImpl::Constant(
                                                    ExpressionValue::Integer(2),
                                                ),
                                            )),
                                        },
                                    )),
                                    mutable: false,
                                }),
                                AstExpression::new(AstExpressionImpl::BinaryOp {
                                    ty: BinaryOpType::Plus,
                                    left: Box::new(AstExpression::new(AstExpressionImpl::VarUse(
                                        "x".to_string(),
                                    ))),
                                    right: Box::new(AstExpression::new(AstExpressionImpl::VarUse(
                                        "y".to_string(),
                                    ))),
                                }),
                            ],
                            attachments: Default::default(),
                        })),
                        annotations: vec![],
                        attachments: Default::default(),
                    },
                );
                map
            },
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_multiple_and_unary_expressions() {
        let code = "fn main() 'my_block: {let y = x + 2; - x + y}";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: {
                let mut map = HashMap::new();
                map.insert(
                    "main".to_string(),
                    AstFunction {
                        name: "main".to_string(),
                        args: vec![],
                        explicit_ret: None,
                        expr: AstExpression::new(AstExpressionImpl::Block(AstBlock {
                            name: Some("my_block".to_string()),
                            exprs: vec![
                                AstExpression::new(AstExpressionImpl::Terminated(Box::new(
                                    AstExpression::new(AstExpressionImpl::VarDeclareAssign {
                                        name: "y".to_string(),
                                        assign: Box::new(AstExpression::new(
                                            AstExpressionImpl::BinaryOp {
                                                ty: BinaryOpType::Plus,
                                                left: Box::new(AstExpression::new(
                                                    AstExpressionImpl::VarUse("x".to_string()),
                                                )),
                                                right: Box::new(AstExpression::new(
                                                    AstExpressionImpl::Constant(
                                                        ExpressionValue::Integer(2),
                                                    ),
                                                )),
                                            },
                                        )),
                                        mutable: false,
                                    }),
                                ))),
                                AstExpression::new(AstExpressionImpl::BinaryOp {
                                    ty: BinaryOpType::Plus,
                                    left: Box::new(AstExpression::new(
                                        AstExpressionImpl::UnaryOp {
                                            ty: UnaryOpType::Minus,
                                            expr: Box::new(AstExpression::new(
                                                AstExpressionImpl::VarUse("x".to_string()),
                                            )),
                                        },
                                    )),
                                    right: Box::new(AstExpression::new(AstExpressionImpl::VarUse(
                                        "y".to_string(),
                                    ))),
                                }),
                            ],
                            attachments: Default::default(),
                        })),
                        annotations: vec![],
                        attachments: Default::default(),
                    },
                );
                map
            },
            ..Default::default()
        };

        eprintln!("code:`{}`,\nast:\n{:#?}", code, ast);

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_binary_expressions() {
        let code = "fn main() 'my_block: {let y = x + 2 -x + z}";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: {
                let mut map = HashMap::new();
                map.insert(
                    "main".to_string(),
                    AstFunction {
                        name: "main".to_string(),
                        args: vec![],
                        explicit_ret: None,
                        expr: ast_expr!(Block(AstBlock {
                            name: Some("my_block".to_string()),
                            exprs: vec![ast_expr!(VarDeclareAssign {
                                name: "y".to_string(),
                                assign: Box::new(ast_expr!(BinaryOp {
                                    ty: BinaryOpType::Plus,
                                    left: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Minus,
                                        left: Box::new(ast_expr!(BinaryOp {
                                            ty: BinaryOpType::Plus,
                                            left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                            right: Box::new(ast_expr!(Constant(
                                                ExpressionValue::Integer(2)
                                            ))),
                                        })),
                                        right: Box::new(ast_expr!(VarUse("x".to_string())))
                                    })),
                                    right: Box::new(ast_expr!(VarUse("z".to_string())))
                                })),
                                mutable: false,
                            })],
                            attachments: Default::default(),
                        })),
                        annotations: vec![],
                        attachments: Default::default(),
                    },
                );
                map
            },
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_multiple_and_parentheses_expressions() {
        let code = "fn main() 'my_block: {let y = x + 2; - (x + y)}";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: {
                let mut map = HashMap::new();
                map.insert(
                    "main".to_string(),
                    AstFunction {
                        name: "main".to_string(),
                        args: vec![],
                        explicit_ret: None,
                        expr: ast_expr!(Block(AstBlock {
                            name: Some("my_block".to_string()),
                            exprs: vec![
                                ast_expr!(Terminated(Box::new(ast_expr!(VarDeclareAssign {
                                    name: "y".to_string(),
                                    assign: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Plus,
                                        left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                        right: Box::new(ast_expr!(Constant(
                                            ExpressionValue::Integer(2,)
                                        ))),
                                    })),
                                    mutable: false,
                                })))),
                                ast_expr!(UnaryOp {
                                    ty: UnaryOpType::Minus,
                                    expr: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Plus,
                                        left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                        right: Box::new(ast_expr!(VarUse("y".to_string()))),
                                    })),
                                }),
                            ],
                            attachments: Default::default(),
                        })),
                        annotations: vec![],
                        attachments: Default::default(),
                    },
                );
                map
            },
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_annotation_ast() {
        let code = "#[main] fn main(#[constant(default, 2)] c: Complex) 'my_block: {c + 2}";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: {
                let mut map = HashMap::new();
                map.insert(
                    "main".to_string(),
                    AstFunction {
                        name: "main".to_string(),
                        args: vec![AstVariable {
                            name: "c".to_string(),
                            ty: ExpressionType::Complex,
                            init: None,
                            annotations: vec![AstAnnotation {
                                name: "c".to_string(),
                                args: vec![
                                    AstAnnotationArg::Ident("default".to_string()),
                                    AstAnnotationArg::Integer(2),
                                ],
                                attachments: Default::default(),
                            }],
                            attachments: Default::default(),
                        }],
                        explicit_ret: None,
                        expr: AstExpression::new(AstExpressionImpl::Block(AstBlock {
                            name: Some("my_block".to_string()),
                            exprs: vec![AstExpression::new(AstExpressionImpl::BinaryOp {
                                ty: BinaryOpType::Plus,
                                left: Box::new(AstExpression::new(AstExpressionImpl::VarUse(
                                    "c".to_string(),
                                ))),
                                right: Box::new(AstExpression::new(AstExpressionImpl::Constant(
                                    ExpressionValue::Integer(2),
                                ))),
                            })],
                            attachments: Default::default(),
                        })),
                        annotations: vec![AstAnnotation {
                            name: "main".to_string(),
                            args: vec![],
                            attachments: Default::default(),
                        }],
                        attachments: Default::default(),
                    },
                );
                map
            },
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_if_block() {
        let code = "fn main() { if (x < 2) x - 1 else -x }";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: hash_map![
                "main".to_string() => AstFunction {
                    name: "main".to_string(),
                    args: vec![],
                    explicit_ret: None,
                    expr: ast_expr!(Block(AstBlock {
                        exprs: vec![
                            ast_expr!(IfElse {
                                start: AstIfBlock {
                                    condition: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::LessThan,
                                        left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                        right: Box::new(ast_expr!(Constant(ExpressionValue::Integer(2))))
                                    })),
                                    block: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Minus,
                                        left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                        right: Box::new(ast_expr!(Constant(ExpressionValue::Integer(1))))
                                    })),
                                    ..Default::default()
                                },
                                chain: Default::default(),
                                end: Some(Box::new(ast_expr!(UnaryOp {
                                    ty: UnaryOpType::Minus,
                                    expr: Box::new(ast_expr!(VarUse("x".to_string())))
                                })))
                            })
                        ],
                        ..Default::default()
                    })),
                    annotations: vec![],
                    attachments: Default::default(),
                }
            ],
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_else_if_block() {
        let code = "fn main() { if (x <= 2) x - 1 else if (x >= 10) -x else 10 * -x }";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: hash_map![
                "main".to_string() => AstFunction {
                    name: "main".to_string(),
                    args: vec![],
                    explicit_ret: None,
                    expr: ast_expr!(Block(AstBlock {
                        exprs: vec![
                            ast_expr!(IfElse {
                                start: AstIfBlock {
                                    condition: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::LessEqual,
                                        left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                        right: Box::new(ast_expr!(Constant(ExpressionValue::Integer(2))))
                                    })),
                                    block: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Minus,
                                        left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                        right: Box::new(ast_expr!(Constant(ExpressionValue::Integer(1))))
                                    })),
                                    ..Default::default()
                                },
                                chain: Default::default(),
                                end: Some(Box::new(ast_expr!(IfElse {
                                    start: AstIfBlock {
                                        condition: Box::new(ast_expr!(BinaryOp {
                                            ty: BinaryOpType::GreaterEqual,
                                            left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                            right: Box::new(ast_expr!(Constant(ExpressionValue::Integer(10))))
                                        })),
                                        block: Box::new(ast_expr!(UnaryOp {
                                            ty: UnaryOpType::Minus,
                                            expr: Box::new(ast_expr!(VarUse("x".to_string())))
                                        })),
                                        ..Default::default()
                                    },
                                    chain: Default::default(),
                                    end: Some(Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Times,
                                        left: Box::new(ast_expr!(Constant(ExpressionValue::Integer(10)))),
                                        right: Box::new(ast_expr!(UnaryOp {
                                            ty: UnaryOpType::Minus,
                                            expr: Box::new(ast_expr!(VarUse("x".to_string())))
                                        }))
                                    })))
                                })))
                            })
                        ],
                        ..Default::default()
                    })),
                    annotations: vec![],
                    attachments: Default::default(),
                }
            ],
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code: `{}`", code);
    }

    #[test]
    fn test_while_block() {
        let code = "fn main(x: Integer) { while (x > 1) x-- }";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: hash_map![
                "main".to_string() => AstFunction {
                    name: "main".to_string(),
                    args: vec![AstVariable {
                        name: "x".to_string(),
                        ty: ExpressionType::Integer,
                        init: None,
                        annotations: Default::default(),
                        attachments: Default::default(),
                    }],
                    explicit_ret: None,
                    expr: ast_expr!(Block(AstBlock {
                        name: None,
                        exprs: vec![
                            ast_expr!(While {
                                condition: Box::new(ast_expr!(BinaryOp {
                                    ty: BinaryOpType::GreaterThan,
                                    left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                    right: Box::new(ast_expr!(Constant(ExpressionValue::Integer(1)))),
                                })),
                                block: Box::new(ast_expr!(UnaryOp {
                                    ty: UnaryOpType::PostDecrement,
                                    expr: Box::new(ast_expr!(VarUse("x".to_string())))
                                }))
                            })
                        ],
                        attachments: Default::default()
                    })),
                    annotations: vec![],
                    attachments: Default::default()
                }
            ],
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code `{}`", code);
    }

    #[test]
    fn test_for_block() {
        let code = "fn main(x: Integer, c: Complex) { let mut z = 0.0; for (let mut n = 0; n < x; n++) z = z ^ 2.0 + c; z }";
        let source = ProgramSource::new(code, "test-impl");

        let ast = parse(source, 24);

        let expected = AstProgram {
            functions: hash_map![
                "main".to_string() => AstFunction {
                    name: "main".to_string(),
                    args: vec![
                        AstVariable {
                            name: "x".to_string(),
                            ty: ExpressionType::Integer,
                            init: None,
                            annotations: Default::default(),
                            attachments: Default::default(),
                        },
                        AstVariable {
                            name: "c".to_string(),
                            ty: ExpressionType::Complex,
                            init: None,
                            annotations: Default::default(),
                            attachments: Default::default(),
                        }
                    ],
                    explicit_ret: None,
                    expr: ast_expr!(Block(AstBlock {
                        name: None,
                        exprs: vec![
                            ast_expr!(Terminated(Box::new(ast_expr!(VarDeclareAssign {
                                name: "z".to_string(),
                                mutable: true,
                                assign: Box::new(ast_expr!(Constant(ExpressionValue::Complex(Complex::with_val(24, (0.0, 0.0)))))),
                            })))),
                            ast_expr!(Terminated(Box::new(ast_expr!(For {
                                declares: Box::new(ast_expr!(VarDeclareAssign {
                                    name: "n".to_string(),
                                    mutable: true,
                                    assign: Box::new(ast_expr!(Constant(ExpressionValue::Integer(0)))),
                                })),
                                condition: Box::new(ast_expr!(BinaryOp {
                                    ty: BinaryOpType::LessThan,
                                    left: Box::new(ast_expr!(VarUse("n".to_string()))),
                                    right: Box::new(ast_expr!(VarUse("x".to_string()))),
                                })),
                                after: Box::new(ast_expr!(UnaryOp {
                                    ty: UnaryOpType::PostIncrement,
                                    expr: Box::new(ast_expr!(VarUse("n".to_string())))
                                })),
                                block: Box::new(ast_expr!(VarAssign {
                                    name: "z".to_string(),
                                    ty: None,
                                    assign: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Plus,
                                        left: Box::new(ast_expr!(BinaryOp {
                                            ty: BinaryOpType::Power,
                                            left: Box::new(ast_expr!(VarUse("z".to_string()))),
                                            right: Box::new(ast_expr!(Constant(ExpressionValue::Complex(Complex::with_val(24, (2.0, 0.0)))))),
                                        })),
                                        right: Box::new(ast_expr!(VarUse("c".to_string())))
                                    }))
                                }))
                            })))),
                            ast_expr!(VarUse("z".to_string()))
                        ],
                        attachments: Default::default(),
                    })),
                    annotations: Default::default(),
                    attachments: Default::default(),
                }
            ],
            ..Default::default()
        };

        assert_eq!(expected, ast, "Code `{}`", code);
    }
}
