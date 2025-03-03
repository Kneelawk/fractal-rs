//! Fractal program parser constructs.

mod lexer;
mod span;

use crate::ast::{
    AstBlock, AstConstant, AstExpression, AstExpressionImpl, BinaryOpType, UnaryOpType,
};
use crate::parser::lexer::LexerToken;
use crate::parser::span::mk_span;
use chumsky::input::ValueInput;
use chumsky::pratt::{infix, left, prefix, right};
use chumsky::prelude::*;
use rug::Complex;
use span::ProgramSource;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
struct Spanned<T>(T, SimpleSpan);

type ProgramExtra<'src> = extra::Full<Rich<'src, LexerToken<'src>>, (), ProgramSource>;

fn expr_parser<'src, I>(prec: u32) -> impl Parser<'src, I, AstExpression, ProgramExtra<'src>>
where
    I: ValueInput<'src, Token = LexerToken<'src>, Span = SimpleSpan>,
{
    recursive(move |expr| {
        let constant = select! {
            LexerToken::Boolean(b) => AstExpressionImpl::Constant(AstConstant::Boolean(b)),
            LexerToken::RealInteger(i) => AstExpressionImpl::Constant(AstConstant::Integer(i)),
            LexerToken::RealNumber(n) => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (n, 0)))),
            LexerToken::ImaginaryInteger(i) => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (0, i)))),
            LexerToken::ImaginaryNumber(n) => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (0, n)))),
            LexerToken::I => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (0, 1))))
        }.map(AstExpression::new).labelled("value");

        let ident = select! { LexerToken::Ident(s) => s }.labelled("identifier");

        let lifetime = select! { LexerToken::Lifetime(name) => name }.labelled("lifetime");

        let items = expr
            .clone()
            .separated_by(just(LexerToken::Delim(',')))
            .allow_trailing()
            .collect::<Vec<_>>();

        let let_ = just(LexerToken::Let)
            .ignore_then(ident)
            .then_ignore(just(LexerToken::Op("=")))
            .then(expr.clone())
            .map(|(name, value)| {
                AstExpression::new(AstExpressionImpl::VarDeclareAssign {
                    name: name.to_string(),
                    assign: Box::new(value),
                    mutable: false,
                })
            });

        let parens = just(LexerToken::Delim('('))
            .ignore_then(expr.clone())
            .then_ignore(just(LexerToken::Delim(')')));

        // should i move this out and make it recursive?
        let block = lifetime
            .then_ignore(just(LexerToken::Delim(':')))
            .or_not()
            .then_ignore(just(LexerToken::Delim('{')))
            .then(expr.clone().repeated().collect::<Vec<_>>())
            .then_ignore(just(LexerToken::Delim('}')))
            .map(|(name, exprs)| {
                AstExpression::new(AstExpressionImpl::Block(AstBlock {
                    exprs,
                    name: name.map(str::to_string),
                    ..Default::default()
                }))
            });

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
    })
}
//
// fn parser<'src>(prec: u32) -> impl Parser<'src, LexerToken<'src>, AstProgram, ProgramExtra<'src>> {}
//
// fn test() {}
