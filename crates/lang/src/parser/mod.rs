//! Fractal program parser constructs.

mod lexer;

use crate::ast::{AstConstant, AstExpression, AstExpressionImpl, AstProgram};
use crate::parser::lexer::LexerToken;
use chumsky::input::MapExtra;
use chumsky::prelude::recursive;
use chumsky::span::Span;
use chumsky::{Parser, error, extra, select, text};
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

// fn expr_parser<'src>(
//     prec: u32,
// ) -> impl Parser<'src, LexerToken<'src>, AstExpression, ProgramExtra<'src>> {
//     recursive(|expr| {
//         let constant = select! {
//             LexerToken::Boolean(b) => AstExpressionImpl::Constant(AstConstant::Boolean(b)),
//             LexerToken::Integer(i) => AstExpressionImpl::Constant(AstConstant::Integer(i)),
//             // LexerToken::
//         };
//     })
// }
//
// fn parser<'src>(prec: u32) -> impl Parser<'src, LexerToken<'src>, AstProgram, ProgramExtra<'src>> {}
//
// fn test() {}
