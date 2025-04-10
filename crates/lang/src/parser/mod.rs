//! Fractal program parser constructs.

mod lexer;
mod span;

use crate::ExpressionType;
use crate::ast::{
    AstAnnotation, AstAnnotationArg, AstBlock, AstConstant, AstExpression, AstExpressionImpl,
    AstFunction, AstProgram, AstVariable, BinaryOpType, UnaryOpType,
};
use crate::parser::lexer::{LexerToken, lexer};
use crate::parser::span::mk_span;
use chumsky::input::ValueInput;
use chumsky::pratt::{infix, left, prefix, right};
use chumsky::prelude::*;
use rug::Complex;
use span::ProgramSource;

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
    let ast = Parser::<_, _, ProgramExtra>::parse(
        &parser,
        tokens
            .as_slice()
            .map((tokens.len()..tokens.len()).into(), |spanned| {
                (&spanned.0, &spanned.1)
            }),
    )
    .unwrap();
    ast
}

fn parser<'src, I>(prec: u32) -> impl Parser<'src, I, AstProgram, ProgramExtra<'src>>
where
    I: ValueInput<'src, Token = LexerToken<'src>, Span = SimpleSpan>,
{
    let ident = select! { LexerToken::Ident(s) => s }.labelled("identifier");
    let lifetime = select! { LexerToken::Lifetime(name) => name }.labelled("lifetime");

    let ty = select! {
        LexerToken::Ident("Boolean") => ExpressionType::Boolean,
        LexerToken::Ident("Color") => ExpressionType::Color,
        LexerToken::Ident("Complex") => ExpressionType::Complex,
        LexerToken::Ident("Integer") => ExpressionType::Integer,
        LexerToken::Ident("Unit") => ExpressionType::Unit,
    };

    let constant = select! {
        LexerToken::Boolean(b) => AstConstant::Boolean(b),
        LexerToken::RealInteger(i) => AstConstant::Integer(i),
        LexerToken::RealNumber(n) => AstConstant::Complex(Complex::with_val(prec, (n, 0))),
        LexerToken::ImaginaryInteger(i) => AstConstant::Complex(Complex::with_val(prec, (0, i))),
        LexerToken::ImaginaryNumber(n) => AstConstant::Complex(Complex::with_val(prec, (0, n))),
        LexerToken::I => AstConstant::Complex(Complex::with_val(prec, (0, 1))),
    }
    .labelled("value");

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
            attachments: {
                let mut map = anymap::Map::new();
                map.insert(mk_span(m));
                map
            },
        });

    let annotation_vec = annotation.repeated().collect::<Vec<_>>();

    let expr = recursive(move |expr| {
        let block = lifetime
            .then_ignore(just(LexerToken::Delim(':')))
            .or_not()
            .then_ignore(just(LexerToken::Delim('{')))
            .then(
                expr.clone()
                    .then_ignore(just(LexerToken::Terminator).or_not())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(LexerToken::Delim('}')))
            .map_with(|(name, exprs), m| {
                AstExpression::new(AstExpressionImpl::Block(AstBlock {
                    exprs,
                    name: name.map(str::to_string),
                    attachments: {
                        let mut map = anymap::Map::new();
                        map.insert(mk_span(m));
                        map
                    },
                }))
                .with_attachment(mk_span(m))
            });

        let items = expr
            .clone()
            .separated_by(just(LexerToken::Delim(',')))
            .allow_trailing()
            .collect::<Vec<_>>();

        let let_ = just(LexerToken::Let)
            .ignore_then(ident)
            .then_ignore(just(LexerToken::Op("=")))
            .then(expr.clone())
            .map_with(|(name, value), m| {
                AstExpression::new(AstExpressionImpl::VarDeclareAssign {
                    name: name.to_string(),
                    assign: Box::new(value),
                    mutable: false,
                })
                .with_attachment(mk_span(m))
            });

        let parens = just(LexerToken::Delim('('))
            .ignore_then(expr.clone())
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
            .or(call)
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
            infix(right(4), op("^"), |a, _, b, m| {
                AstExpression::new(AstExpressionImpl::BinaryOp {
                    ty: BinaryOpType::Power,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            prefix(3, op("-"), |_, e, m| {
                AstExpression::new(AstExpressionImpl::UnaryOp {
                    ty: UnaryOpType::Minus,
                    expr: Box::new(e),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(2), op("*"), |a, _, b, m| {
                AstExpression::new(AstExpressionImpl::BinaryOp {
                    ty: BinaryOpType::Times,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(2), op("/"), |a, _, b, m| {
                AstExpression::new(AstExpressionImpl::BinaryOp {
                    ty: BinaryOpType::Divide,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(1), op("+"), |a, _, b, m| {
                AstExpression::new(AstExpressionImpl::BinaryOp {
                    ty: BinaryOpType::Plus,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
            infix(left(1), op("-"), |a, _, b, m| {
                AstExpression::new(AstExpressionImpl::BinaryOp {
                    ty: BinaryOpType::Minus,
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .with_attachment(mk_span(m))
            }),
        ))
    });

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
            attachments: {
                let mut map = anymap::Map::new();
                map.insert(mk_span(m));
                map
            },
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
            attachments: {
                let mut map = anymap::Map::new();
                map.insert(mk_span(m));
                map
            },
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
            attachments: {
                let mut map = anymap::Map::new();
                map.insert(mk_span(m));
                map
            },
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
        AstAnnotation, AstAnnotationArg, AstBlock, AstConstant, AstExpression, AstExpressionImpl,
        AstFunction, AstProgram, AstVariable, BinaryOpType, UnaryOpType,
    };
    use crate::parser::parse;
    use crate::parser::span::ProgramSource;
    use crate::{ExpressionType, ast_expr};
    use std::collections::HashMap;

    #[test]
    fn test_simple_ast() {
        let source = ProgramSource::new("fn main() 'my_block: {x + 2}", "test-impl");

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
                                    AstConstant::Integer(2),
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

        assert_eq!(expected, ast);
    }

    #[test]
    fn test_multiple_expressions() {
        let source = ProgramSource::new("fn main() 'my_block: {let y = x + 2 x + y}", "test-impl");

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
                                                AstExpressionImpl::Constant(AstConstant::Integer(
                                                    2,
                                                )),
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

        assert_eq!(expected, ast);
    }

    #[test]
    fn test_multiple_and_unary_expressions() {
        let source =
            ProgramSource::new("fn main() 'my_block: {let y = x + 2; - x + y}", "test-impl");

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
                                                AstExpressionImpl::Constant(AstConstant::Integer(
                                                    2,
                                                )),
                                            )),
                                        },
                                    )),
                                    mutable: false,
                                }),
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

        assert_eq!(expected, ast);
    }

    #[test]
    fn test_binary_expressions() {
        let source = ProgramSource::new("fn main() 'my_block: {let y = x + 2 -x + z}", "test-impl");

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
                                                AstConstant::Integer(2)
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

        assert_eq!(expected, ast);
    }

    #[test]
    fn test_multiple_and_parentheses_expressions() {
        let source = ProgramSource::new(
            "fn main() 'my_block: {let y = x + 2; - (x + y)}",
            "test-impl",
        );

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
                                ast_expr!(VarDeclareAssign {
                                    name: "y".to_string(),
                                    assign: Box::new(ast_expr!(BinaryOp {
                                        ty: BinaryOpType::Plus,
                                        left: Box::new(ast_expr!(VarUse("x".to_string()))),
                                        right: Box::new(ast_expr!(Constant(AstConstant::Integer(
                                            2,
                                        )))),
                                    })),
                                    mutable: false,
                                }),
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

        assert_eq!(expected, ast);
    }

    #[test]
    fn test_annotation_ast() {
        let source = ProgramSource::new(
            "#[main] fn main(#[constant(default, 2)] c: Complex) 'my_block: {c + 2}",
            "test-impl",
        );

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
                                    AstConstant::Integer(2),
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

        assert_eq!(expected, ast);
    }
}
