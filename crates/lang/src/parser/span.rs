use crate::parser::ProgramExtra;
use crate::parser::lexer::LexerToken;
use chumsky::input::{Input, MapExtra};
use chumsky::prelude::SimpleSpan;
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

pub fn mk_span<'a, 'b, 'src, I>(
    map_extra: &'a mut MapExtra<'src, 'b, I, ProgramExtra<'src>>,
) -> ProgramSpan
where
    I: Input<'src, Token = LexerToken<'src>, Span = SimpleSpan>,
{
    ProgramSpan {
        source: map_extra.ctx().clone(),
        range: map_extra.span().into_range(),
    }
}
