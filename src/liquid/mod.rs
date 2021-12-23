//! template/mod.rs - This module has the components for the application's
//! template engine(s).
use liquid_core::{parser::PluginRegistry, Language, ParseBlock, ParseFilter, ParseTag};
use liquid_lib::stdlib;

pub mod partials;

pub fn default_language() -> LanguageBuilder {
    LanguageBuilder::default().stdlib()
}

#[derive(Default, Clone)]
pub struct LanguageBuilder {
    blocks: PluginRegistry<Box<dyn ParseBlock>>,
    tags: PluginRegistry<Box<dyn ParseTag>>,
    filters: PluginRegistry<Box<dyn ParseFilter>>,
}

impl LanguageBuilder {
    /// Adds a block.
    pub fn block<B: Into<Box<dyn ParseBlock>>>(mut self, block: B) -> Self {
        let block = block.into();
        self.blocks
            .register(block.reflection().start_tag().to_owned(), block);
        self
    }

    /// Adds a tag.
    pub fn tag<T: Into<Box<dyn ParseTag>>>(mut self, tag: T) -> Self {
        let tag = tag.into();
        self.tags.register(tag.reflection().tag().to_owned(), tag);
        self
    }

    /// Adds a filter.
    pub fn filter<F: Into<Box<dyn ParseFilter>>>(mut self, filter: F) -> Self {
        let filter = filter.into();
        self.filters
            .register(filter.reflection().name().to_owned(), filter);
        self
    }

    /// Adds all stdlib blocks, tags, and filters.
    pub fn stdlib(self) -> Self {
        self.tag(stdlib::AssignTag)
            .tag(stdlib::BreakTag)
            .tag(stdlib::ContinueTag)
            .tag(stdlib::CycleTag)
            .tag(stdlib::IncludeTag)
            .tag(stdlib::IncrementTag)
            .tag(stdlib::DecrementTag)
            .block(stdlib::RawBlock)
            .block(stdlib::IfBlock)
            .block(stdlib::UnlessBlock)
            .block(stdlib::IfChangedBlock)
            .block(stdlib::ForBlock)
            .block(stdlib::TableRowBlock)
            .block(stdlib::CommentBlock)
            .block(stdlib::CaptureBlock)
            .block(stdlib::CaseBlock)
            .filter(stdlib::Abs)
            .filter(stdlib::Append)
            .filter(stdlib::AtLeast)
            .filter(stdlib::AtMost)
            .filter(stdlib::Capitalize)
            .filter(stdlib::Ceil)
            .filter(stdlib::Compact)
            .filter(stdlib::Concat)
            .filter(stdlib::Date)
            .filter(stdlib::Default)
            .filter(stdlib::DividedBy)
            .filter(stdlib::Downcase)
            .filter(stdlib::Escape)
            .filter(stdlib::EscapeOnce)
            .filter(stdlib::First)
            .filter(stdlib::Floor)
            .filter(stdlib::Join)
            .filter(stdlib::Last)
            .filter(stdlib::Lstrip)
            .filter(stdlib::Map)
            .filter(stdlib::Minus)
            .filter(stdlib::Modulo)
            .filter(stdlib::NewlineToBr)
            .filter(stdlib::Plus)
            .filter(stdlib::Prepend)
            .filter(stdlib::Remove)
            .filter(stdlib::RemoveFirst)
            .filter(stdlib::Replace)
            .filter(stdlib::ReplaceFirst)
            .filter(stdlib::Reverse)
            .filter(stdlib::Round)
            .filter(stdlib::Rstrip)
            .filter(stdlib::Size)
            .filter(stdlib::Slice)
            .filter(stdlib::Sort)
            .filter(stdlib::SortNatural)
            .filter(stdlib::Split)
            .filter(stdlib::Strip)
            .filter(stdlib::StripHtml)
            .filter(stdlib::StripNewlines)
            .filter(stdlib::Times)
            .filter(stdlib::Truncate)
            .filter(stdlib::TruncateWords)
            .filter(stdlib::Uniq)
            .filter(stdlib::Upcase)
            .filter(stdlib::UrlDecode)
            .filter(stdlib::UrlEncode)
            .filter(stdlib::Where)
    }

    /// Builds a liquid language from this builder.
    pub fn build(self) -> Language {
        let mut language = Language::empty();

        language.blocks = self.blocks;
        language.tags = self.tags;
        language.filters = self.filters;

        language
    }
}
