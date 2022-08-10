use crate::liquid::macros::MacrosRegister;
use kstring::KString;
use liquid_core::{
    error::ResultLiquidExt, parser::BlockElement, BlockReflection, Language, ParseBlock,
    Renderable, Runtime, TagBlock, TagTokenIter, Template,
};
use std::io::Write;

/// If-block specifically checking if a macro has been defined.
///
/// # Syntax
/// ```liquid
/// {% ifdef 'name' %}
///   <if-defined contents>
/// {% else %}
///   <if-not-defined contents>
/// {% endifdef %}
/// ```
///
/// # Examples
/// ```liquid
/// {% define foo %}
///
/// {% ifdef foo %}
///   Foo is defined!
/// {% endifdef %}
/// ```
///
/// An example with an else block:
/// ```liquid
/// {% define foo %}
///
/// {% ifdef foo %}
///   Foo is defined!
/// {% else %}
///   Foo is not defined!
/// {% endifdef %}
#[derive(Default, Debug, Copy, Clone)]
pub struct IfDefBlock;

impl BlockReflection for IfDefBlock {
    fn start_tag(&self) -> &str {
        "ifdef"
    }

    fn end_tag(&self) -> &str {
        "endifdef"
    }

    fn description(&self) -> &str {
        "If-block specifically checking if a macro has been defined."
    }
}

impl ParseBlock for IfDefBlock {
    fn parse(
        &self,
        arguments: TagTokenIter,
        block: TagBlock,
        options: &Language,
    ) -> liquid_core::Result<Box<dyn Renderable>> {
        IfDef::parse(false, arguments, block, options)
            .map(|if_def| Box::new(if_def) as Box<dyn Renderable>)
    }

    fn reflection(&self) -> &dyn BlockReflection {
        self
    }
}

/// If-block specifically checking if a macro has *not* been defined.
///
/// # Syntax
/// ```liquid
/// {% ifndef 'name' %}
///   <if-not-defined contents>
/// {% else %}
///   <if-defined contents>
/// {% endifndef %}
/// ```
///
/// # Examples
/// ```liquid
/// {% define foo %}
///
/// {% ifndef foo %}
///   Foo is not defined!
/// {% endifndef %}
/// ```
///
/// An example with an else block:
/// ```liquid
/// {% define foo %}
///
/// {% ifndef foo %}
///   Foo is defined!
/// {% else %}
///   Foo is not defined!
/// {% endifndef %}
/// ```
///
/// A common use-case for these bocks is making sure a template is only included
/// once using the familiar C-style `ifndef`-`define` system:
/// ```liquid
/// {% ifndef INCLUDE_NAME %}
/// {% define INCLUDE_NAME %}
///
/// <include-name file contents>
///
/// {% endifndef %}
/// ```
#[derive(Default, Debug, Copy, Clone)]
pub struct IfNDefBlock;

impl BlockReflection for IfNDefBlock {
    fn start_tag(&self) -> &str {
        "ifndef"
    }

    fn end_tag(&self) -> &str {
        "endifndef"
    }

    fn description(&self) -> &str {
        "If-block specifically checking if a macro has *not* been defined."
    }
}

impl ParseBlock for IfNDefBlock {
    fn parse(
        &self,
        arguments: TagTokenIter,
        block: TagBlock,
        options: &Language,
    ) -> liquid_core::Result<Box<dyn Renderable>> {
        IfDef::parse(true, arguments, block, options)
            .map(|if_def| Box::new(if_def) as Box<dyn Renderable>)
    }

    fn reflection(&self) -> &dyn BlockReflection {
        self
    }
}

#[derive(Debug)]
struct IfDef {
    not: bool,
    macro_name: KString,
    if_def: Option<Template>,
    if_ndef: Option<Template>,
}

impl IfDef {
    fn parse(
        not: bool,
        mut arguments: TagTokenIter<'_>,
        mut tokens: TagBlock<'_, '_>,
        options: &Language,
    ) -> liquid_core::Result<Self> {
        let macro_name = arguments
            .expect_next("Expected macro name identifier.")?
            .expect_identifier()
            .into_result()?
            .to_string()
            .into();

        let mut if_block = vec![];
        let mut else_block = None;

        while let Some(element) = tokens.next()? {
            match element {
                BlockElement::Tag(tag) => match tag.name() {
                    "else" => else_block = Some(tokens.parse_all(options)?),
                    _ => if_block.push(tag.parse(&mut tokens, options)?),
                },
                _ => if_block.push(element.parse(&mut tokens, options)?),
            }
        }

        let (if_def, if_ndef) = if not {
            (
                else_block.map(Template::new),
                Some(if_block).map(Template::new),
            )
        } else {
            (
                Some(if_block).map(Template::new),
                else_block.map(Template::new),
            )
        };

        Ok(Self {
            not,
            macro_name,
            if_def,
            if_ndef,
        })
    }

    fn trace(&self) -> String {
        let block_name = if self.not { "ifndef" } else { "ifdef" };

        format!("{{% {} {} %}}", block_name, &self.macro_name)
    }
}

impl Renderable for IfDef {
    fn render_to(&self, writer: &mut dyn Write, runtime: &dyn Runtime) -> liquid_core::Result<()> {
        let defined = runtime
            .registers()
            .get_mut::<MacrosRegister>()
            .is_defined(&self.macro_name);

        if defined {
            if let Some(if_def) = &self.if_def {
                if_def
                    .render_to(writer, runtime)
                    .trace_with(|| self.trace().into())?;
            }
        } else {
            if let Some(if_ndef) = &self.if_ndef {
                if_ndef
                    .render_to(writer, runtime)
                    .trace_with(|| self.trace().into())?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        liquid::{
            blocks::{IfDefBlock, IfNDefBlock},
            tags::DefineTag,
        },
        util::tests::test_liquid,
    };
    use liquid_core::object;
    use liquid_lib::stdlib::IfBlock;

    #[test]
    fn simple_ifdef_test() {
        test_liquid!(
            tags(DefineTag)
            blocks(IfDefBlock)
            input(
                concat!(
                    "{% define foo %}",
                    "{% ifdef foo %}",
                    "Foo is defined.",
                    "{% else %}",
                    "Foo is not defined.",
                    "{% endifdef %}"
                )
            )
            assert_output(
                "Foo is defined.",
                "The rendered string should be \"Foo is defined.\""
            )
        );
    }

    #[test]
    fn simple_ifdef_test_2() {
        test_liquid!(
            tags(DefineTag)
            blocks(IfDefBlock)
            input(
                concat!(
                    "{% ifdef foo %}",
                    "Foo is defined.",
                    "{% else %}",
                    "Foo is not defined.",
                    "{% endifdef %}"
                )
            )
            assert_output(
                "Foo is not defined.",
                "The rendered string should be \"Foo is defined.\""
            )
        );
    }

    #[test]
    fn simple_ifndef_test() {
        test_liquid!(
            tags(DefineTag)
            blocks(IfNDefBlock)
            input(
                concat!(
                    "{% define foo %}",
                    "{% ifndef foo %}",
                    "Foo is not defined.",
                    "{% else %}",
                    "Foo is defined.",
                    "{% endifndef %}"
                )
            )
            assert_output(
                "Foo is defined.",
                "The rendered string should be \"Foo is defined.\""
            )
        );
    }

    #[test]
    fn simple_ifndef_test_2() {
        test_liquid!(
            tags(DefineTag)
            blocks(IfNDefBlock)
            input(
                concat!(
                    "{% ifndef foo %}",
                    "Foo is not defined.",
                    "{% else %}",
                    "Foo is defined.",
                    "{% endifndef %}"
                )
            )
            assert_output(
                "Foo is not defined.",
                "The rendered string should be \"Foo is defined.\""
            )
        );
    }

    #[test]
    fn nested_ifdef_test() {
        test_liquid!(
            tags(DefineTag)
            blocks(IfDefBlock)
            input(
                concat!(
                    "{% define foo %}",
                    "{% ifdef foo %}",
                    "Foo is defined",
                    "{% ifdef bar %}",
                    " and bar is defined",
                    "{% else %}",
                    " and bar is not defined",
                    "{% endifdef %}",
                    "{% else %}",
                    "Foo is not defined",
                    "{% ifdef bar %}",
                    " and bar is defined",
                    "{% else %}",
                    " and bar is not defined",
                    "{% endifdef %}",
                    "{% endifdef %}"
                )
            )
            assert_output(
                "Foo is defined and bar is not defined",
                "The rendered string should be \"Foo is defined and bar is not defined\""
            )
        );
    }

    #[test]
    fn nested_ifdef_ifndef_test() {
        test_liquid!(
            tags(DefineTag)
            blocks(IfDefBlock, IfNDefBlock)
            input(
                concat!(
                    "{% define foo %}",
                    "{% ifdef foo %}",
                    "Foo is defined",
                    "{% ifndef bar %}",
                    " and bar is not defined",
                    "{% else %}",
                    " and bar is defined",
                    "{% endifndef %}",
                    "{% else %}",
                    "Foo is not defined",
                    "{% ifndef bar %}",
                    " and bar is not defined",
                    "{% else %}",
                    " and bar is defined",
                    "{% endifndef %}",
                    "{% endifdef %}"
                )
            )
            assert_output(
                "Foo is defined and bar is not defined",
                "The rendered string should be \"Foo is defined and bar is not defined\""
            )
        );
    }

    #[test]
    fn nested_ifdef_if_test() {
        test_liquid!(
            tags(DefineTag)
            blocks(IfDefBlock, IfBlock)
            globals(
                object!({
                    "bar": "baz"
                })
            )
            input(
                concat!(
                    "{% define foo %}",
                    "{% ifdef foo %}",
                    "Foo is defined",
                    "{% if bar == \"baz\" %}",
                    " and bar is \"baz\"",
                    "{% else %}",
                    " and bar is not \"baz\"",
                    "{% endif %}",
                    "{% else %}",
                    "Foo is not defined",
                    "{% if bar == \"baz\" %}",
                    " and bar is \"baz\"",
                    "{% else %}",
                    " and bar is not \"baz\"",
                    "{% endif %}",
                    "{% endifdef %}"
                )
            )
            assert_output(
                "Foo is defined and bar is \"baz\"",
                "The rendered string should be 'Foo is defined and bar is \"baz\"'"
            )
        );
    }
}
