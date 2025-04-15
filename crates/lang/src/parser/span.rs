use crate::parser::ProgramExtra;
use crate::parser::lexer::LexerToken;
use chumsky::input::{Checkpoint, Cursor, Input, MapExtra};
use chumsky::inspector::Inspector;
use chumsky::prelude::SimpleSpan;
use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, btree_map};
use std::ops::Range;
use std::sync::Arc;

/// Holds all source files in a full program
#[derive(Debug, Default, Clone)]
pub struct ProgramSourceSet {
    files: BTreeMap<String, ProgramSource>,
}

impl ProgramSourceSet {
    pub fn new(map: BTreeMap<String, ProgramSource>) -> Self {
        Self { files: map }
    }

    pub fn put(&mut self, source: ProgramSource) {
        self.files.insert(source.source().to_string(), source);
    }

    pub fn get(&self, source: &str) -> Option<ProgramSource> {
        self.files.get(source).cloned()
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn iter(&self) -> btree_map::Iter<String, ProgramSource> {
        self.files.iter()
    }
}

impl Serialize for ProgramSourceSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.files.len()))?;
        for (k, v) in self.files.iter() {
            map.serialize_entry(k, v.code())?;
        }
        map.end()
    }
}

/// This type is usually an attachment to [`crate::ast::AstProgram`]s
#[derive(Debug, Clone)]
pub struct ProgramSource {
    source: Arc<ProgramSourceImpl>,
}

impl Default for ProgramSource {
    fn default() -> Self {
        Self::new("", "<no source>")
    }
}

impl ProgramSource {
    /// Construct a new program source holder.
    ///
    /// [`code`] is the actual source code of the program that is being parsed.
    /// [`source`] is a string describing to the user where the code came from.
    pub fn new(code: impl ToString, source: impl ToString) -> Self {
        Self {
            source: Arc::new(ProgramSourceImpl {
                code: code.to_string(),
                source: source.to_string(),
            }),
        }
    }

    pub fn code(&self) -> &str {
        &self.source.code
    }

    pub fn source(&self) -> &str {
        &self.source.source
    }
}

impl<'a, I: Input<'a>> Inspector<'a, I> for ProgramSource {
    type Checkpoint = ();

    fn on_token(&mut self, token: &I::Token) {}

    fn on_save<'parse>(&self, cursor: &Cursor<'a, 'parse, I>) -> Self::Checkpoint {}

    fn on_rewind<'parse>(&mut self, marker: &Checkpoint<'a, 'parse, I, Self::Checkpoint>) {}
}

impl AsRef<str> for ProgramSource {
    fn as_ref(&self) -> &str {
        self.source()
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

impl ariadne::Span for ProgramSpan {
    type SourceId = str;

    fn source(&self) -> &Self::SourceId {
        self.source.source()
    }

    fn start(&self) -> usize {
        self.range.start
    }

    fn end(&self) -> usize {
        self.range.end
    }
}

impl AsRef<str> for ProgramSpan {
    fn as_ref(&self) -> &str {
        &self.source.code()[self.range.clone()]
    }
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
