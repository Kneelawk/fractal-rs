//! Fractal program parser constructs.

use crate::ast::{AstConstant, AstExpression, AstExpressionImpl};
use chumsky::input::MapExtra;
use chumsky::{Parser, error, extra, text};
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

type ProgramExtra<'a> = extra::Full<error::Rich<'a, char>, (), ProgramSource>;

fn mk_span<'a, 'b, 'src>(
    map_extra: &'a mut MapExtra<'src, 'b, &'src str, ProgramExtra<'src>>,
) -> ProgramSpan {
    ProgramSpan {
        source: map_extra.ctx().clone(),
        range: Default::default(),
    }
}

fn integer<'a>() -> impl Parser<'a, &'a str, AstExpression, ProgramExtra<'a>> {
    text::int(10).map_with(|s: &str, metadata| {
        AstExpression::new(AstExpressionImpl::Constant {
            value: AstConstant::Integer(s.parse().unwrap()),
        })
            .with_attachment(mk_span(metadata))
    })
}

fn test() {}
