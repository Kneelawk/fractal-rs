//! Fractal program parser constructs.

mod lexer;

use crate::ast::{AstConstant, AstExpression, AstExpressionImpl, AstProgram, BinaryOpType};
use crate::parser::lexer::LexerToken;
use chumsky::error;
use chumsky::input::MapExtra;
use chumsky::prelude::*;
use rug::Complex;
use std::ops::Range;
use std::sync::Arc;

/// This type is usually an attachment to [`crate::ast::AstProgram`]s
#[derive(Debug, Clone)]
pub struct ProgramSource {
    source: Arc<ProgramSourceImpl>,
}

impl ProgramSource {
    /// Construct a new program source holder.
    ///
    /// [`code`] is the actual source code of the program that is being parsed.
    /// [`source`] is a string describing to the user where the code came from.
    pub fn new(code: String, source: String) -> Self {
        Self {
            source: Arc::new(ProgramSourceImpl { code, source }),
        }
    }

    pub fn code(&self) -> &str {
        &self.source.code
    }

    pub fn source(&self) -> &str {
        &self.source.source
    }
}

#[derive(Debug, Clone)]
struct ProgramSourceImpl {
    /// The actual code
    code: String,
    /// A string describing where the code came from to the user
    source: String,
}

/// This type is usually an attachment to [`AstExpression`]s
#[derive(Debug, Clone)]
pub struct ProgramSpan {
    pub source: ProgramSource,
    pub range: Range<usize>,
}

type ProgramExtra<'src> = extra::Full<error::Rich<'src, char>, (), ProgramSource>;

fn mk_span<'a, 'b, 'src>(
    map_extra: &'a mut MapExtra<'src, 'b, &'src str, ProgramExtra<'src>>,
) -> ProgramSpan {
    ProgramSpan {
        source: map_extra.ctx().clone(),
        range: map_extra.span().into_range(),
    }
}

fn expr_parser<'src>(
    prec: u32,
) -> impl Parser<'src, LexerToken<'src>, AstExpression, ProgramExtra<'src>> {
    recursive(move |expr| {
        let constant = select! {
            LexerToken::Boolean(b) => AstExpressionImpl::Constant(AstConstant::Boolean(b)),
            LexerToken::RealInteger(i) => AstExpressionImpl::Constant(AstConstant::Integer(i)),
            LexerToken::RealNumber(n) => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (n, 0)))),
            LexerToken::ImaginaryInteger(i) => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (0, i)))),
            LexerToken::ImaginaryNumber(n) => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (0, n)))),
            LexerToken::I => AstExpressionImpl::Constant(AstConstant::Complex(Complex::with_val(prec, (0, 1))))
        }.labelled("value");

        let ident = select! {LexerToken::Ident(s) => s}.labelled("identifier");

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

        let pow = expr
            .clone()
            .then_ignore(just(LexerToken::Op("^")))
            .then(expr.clone())
            .map(|(left, right)| AstExpressionImpl::BinaryOp {
                ty: BinaryOpType::Power,
                left: Box::new(left),
                right: Box::new(right),
            });

        let mul = expr
            .clone()
            .then(
                just(LexerToken::Op("*"))
                    .to(BinaryOpType::Times)
                    .or(just(LexerToken::Op("/")).to(BinaryOpType::Divide)),
            )
            .then(expr.clone())
            .map(|((a, op), b)| {
                AstExpression::new(AstExpressionImpl::BinaryOp {
                    left: Box::new(a),
                    ty: op,
                    right: Box::new(b),
                })
            });

        let add = expr
            .clone()
            .then(
                just(LexerToken::Op("+"))
                    .to(BinaryOpType::Plus)
                    .or(just(LexerToken::Op("-")).to(BinaryOpType::Minus)),
            )
            .then(expr.clone())
            .map(|((a, op), b)| {
                AstExpression::new(AstExpressionImpl::BinaryOp {
                    left: Box::new(a),
                    ty: op,
                    right: Box::new(b),
                })
            });

        todo!()
    });

    todo!()
}
//
// fn parser<'src>(prec: u32) -> impl Parser<'src, LexerToken<'src>, AstProgram, ProgramExtra<'src>> {}
//
// fn test() {}
