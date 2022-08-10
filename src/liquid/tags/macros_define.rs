use crate::liquid::{
    macros::{MacroArgument, MacroObject, MacrosRegister},
    util::{parse_arguments, ParsedMacroArgument},
};
use itertools::Itertools;
use kstring::KString;
use liquid_core::{
    error::ResultLiquidExt, parser::FilterChain, Language, ParseTag, Renderable, Runtime,
    TagReflection, TagTokenIter,
};
use std::{io::Write, sync::Arc};

/// Tag for the definition of a "Flag" or "FilterChain" macro.
///
/// # Syntax
/// ```liquid
/// {% define 'name' [[<'argument-1', 'argument-2' = 'default', ...>] = 'definition'] %}
/// ```
///
/// # Examples
/// ```liquid
/// {% define foo %}
/// {% ifdef foo %}
///   Foo is defined.
/// {% endifdef %}
/// ```
///
/// An example of a macro with a definition:
/// ```liquid
/// {% define foo = "bar" %}
///
/// {% call foo %}
/// ```
///
/// An example of a macro with arguments:
/// ```liquid
/// {% define add_one<input> = input | plus: 1 %}
///
/// {% call add_one<input = 2> %}
/// ```
///
/// # Questions
/// * Why are arguments surrounded in `<...>`?
///   - This is because the `liquid` parser only supports angle-brackets as
///     arbitrary characters within tag arguments. `(...)` is reserved by ranges
///     and can only be parsed as such. `[...]` is reserved for array lookups
///     and can only be parsed as such. `{...}` is unsupported.
#[derive(Copy, Clone, Debug, Default)]
pub struct DefineTag;

impl TagReflection for DefineTag {
    fn tag(&self) -> &str {
        "define"
    }

    fn description(&self) -> &str {
        "The definition of a \"Flag\" or \"FilterChain\" macro."
    }
}

impl ParseTag for DefineTag {
    fn parse(
        &self,
        mut arguments: TagTokenIter,
        options: &Language,
    ) -> liquid_core::Result<Box<dyn Renderable>> {
        let name = arguments
            .expect_next("Identifier expected.")?
            .expect_identifier()
            .into_result()?
            .to_string()
            .into();

        let mut argument_list = vec![];
        let mut content = None;

        if let Some(next) = arguments.next() {
            match next.as_str() {
                "<" => {
                    parse_arguments(&mut arguments, &mut argument_list)?;

                    arguments
                        .expect_next("Expected definition operator \"=\"")?
                        .expect_str("=")
                        .into_result_custom_msg("Expected definition operator \"=\"")?;

                    content = Some(Arc::new(parse_definition(&mut arguments, options)?));
                },
                "=" => content = Some(Arc::new(parse_definition(&mut arguments, options)?)),
                _ => {
                    return Err(next.raise_custom_error(
                        "Expected definition operator \"=\" or start of argument list \"<\"",
                    ))
                },
            }
        }

        arguments.expect_nothing()?;

        // Detect duplicates and raise an error.
        let duplicates: Vec<_> = argument_list
            .iter()
            .map(|arg| match arg {
                ParsedMacroArgument::Required { name } => name.as_str(),
                ParsedMacroArgument::Defaulted { name, .. } => name.as_str(),
            })
            .duplicates()
            .collect();
        if !duplicates.is_empty() {
            return liquid_core::Error::with_msg("Duplicate arguments encountered")
                .context("duplicate arguments", itertools::join(duplicates, ", "))
                .trace(trace(&name, &argument_list, &content))
                .into_err();
        }

        Ok(Box::new(ShortMacro {
            name,
            arguments: argument_list,
            content,
        }))
    }

    fn reflection(&self) -> &dyn TagReflection {
        self
    }
}

fn parse_definition(
    arguments: &mut TagTokenIter,
    options: &Language,
) -> liquid_core::Result<FilterChain> {
    arguments
        .expect_next("FilterChain expected.")?
        .expect_filter_chain(options)
        .into_result()
}

#[derive(Debug)]
struct ShortMacro {
    name: KString,
    arguments: Vec<ParsedMacroArgument>,
    content: Option<Arc<FilterChain>>,
}

impl ShortMacro {
    fn trace(&self) -> String {
        trace(&self.name, &self.arguments, &self.content)
    }
}

impl Renderable for ShortMacro {
    fn render_to(&self, _writer: &mut dyn Write, runtime: &dyn Runtime) -> liquid_core::Result<()> {
        let arguments = self
            .arguments
            .iter()
            .map(|arg| match arg {
                ParsedMacroArgument::Required { name } => {
                    Ok(MacroArgument::Required { name: name.clone() })
                },
                ParsedMacroArgument::Defaulted { name, default } => Ok(MacroArgument::Defaulted {
                    name: name.clone(),
                    default: default
                        .evaluate(runtime)
                        .trace_with(|| self.trace().into())?
                        .into_owned(),
                }),
            })
            .collect::<liquid_core::Result<Vec<_>>>()?;

        let macro_object = if let Some(content) = &self.content {
            MacroObject::FilterChain {
                arguments,
                chain: content.clone(),
            }
        } else {
            MacroObject::Flag
        };

        runtime
            .registers()
            .get_mut::<MacrosRegister>()
            .define(self.name.clone(), macro_object);

        Ok(())
    }
}

fn trace(
    name: &KString,
    arguments: &[ParsedMacroArgument],
    content: &Option<Arc<FilterChain>>,
) -> String {
    if let Some(content) = content {
        if arguments.is_empty() {
            format!("{{% define {} = {} %}}", name, content)
        } else {
            format!(
                "{{% define {}<{}> = {} %}}",
                name,
                arguments.iter().format_with(", ", |a, f| match a {
                    ParsedMacroArgument::Required { name } => f(&format_args!("{}", name)),
                    ParsedMacroArgument::Defaulted { name, default } =>
                        f(&format_args!("{} = {}", name, default)),
                }),
                content
            )
        }
    } else {
        format!("{{% define {} %}}", name)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        liquid::{
            macros::MacrosRegister,
            tags::{CallTag, DefineTag},
        },
        util::tests::test_liquid,
    };
    use liquid_core::Runtime;
    use liquid_lib::stdlib::Plus;

    #[test]
    fn macro_is_defined_1() {
        test_liquid!(
            tags(DefineTag)
            input("{% define foo %}")
            block(|runtime, _rendered| {
                assert!(
                    runtime
                        .registers()
                        .get_mut::<MacrosRegister>()
                        .is_defined("foo"),
                    "Macro foo should be defined."
                );
            })
        );
    }

    #[test]
    fn flag_macro() {
        test_liquid!(
            tags(DefineTag, CallTag)
            input(concat!("{% define foo %}", "{% call foo %}"))
            assert_output("", "The rendered macro should be empty.")
        );
    }

    #[test]
    fn simple_macro_1() {
        test_liquid!(
            tags(DefineTag, CallTag)
            input(concat!("{% define foo = \"Hello\" %}", "{% call foo %}"))
            assert_output("Hello", "The rendered macro should say \"Hello\".")
        );
    }

    #[test]
    fn simple_macro_2() {
        test_liquid!(
            tags(DefineTag, CallTag)
            filters(Plus)
            input(
                concat!(
                    "{% define foo<input> = input | plus: 1 %}",
                    "{% call foo<input = 2> %}"
                )
            )
            assert_output("3", "The rendered macro should be \"3\".")
        );
    }
}
