use crate::liquid::{
    macros::{MacroArgument, MacroObject, MacrosRegister},
    util::{parse_arguments, ParsedMacroArgument},
};
use itertools::Itertools;
use kstring::KString;
use liquid_core::{
    error::ResultLiquidExt, BlockReflection, Language, ParseBlock, Renderable, Runtime, TagBlock,
    TagTokenIter, Template,
};
use std::{io::Write, sync::Arc};

/// Block for the definition of a complex "Partial" macro.
///
/// # Syntax
/// ```liquid
/// {% begindef 'name' [<'argument-1', 'argument-2' = 'default, ...>] %}
///   <macro contents>
/// {% enddef %}
/// ```
///
/// # Examples
/// ```liquid
/// {% begindef foo %}
///   Hello Liquid!
/// {% enddef %}
///
/// {% call foo %}
/// ```
///
/// An example of a macro with arguments:
/// ```liquid
/// {% begindef foo<input, other_input = "world!"> %}
///   {{ input }} {{ other_input }}
/// {% enddef %}
///
/// {% call foo<input = "Hello"> %}
/// ```
#[derive(Debug, Default, Copy, Clone)]
pub struct DefineBlock;

impl BlockReflection for DefineBlock {
    fn start_tag(&self) -> &str {
        "begindef"
    }

    fn end_tag(&self) -> &str {
        "enddef"
    }

    fn description(&self) -> &str {
        "Block for the definition of a complex \"Partial\" macro."
    }
}

impl ParseBlock for DefineBlock {
    fn parse(
        &self,
        mut arguments: TagTokenIter,
        mut block: TagBlock,
        options: &Language,
    ) -> liquid_core::Result<Box<dyn Renderable>> {
        let name = arguments
            .expect_next("Identifier expected.")?
            .expect_identifier()
            .into_result()?
            .to_string()
            .into();

        let mut argument_list = vec![];

        if let Some(next) = arguments.next() {
            match next.as_str() {
                "<" => {
                    parse_arguments(&mut arguments, &mut argument_list)?;
                },
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
                .trace(trace(&name, &argument_list))
                .into_err();
        }

        let content = Arc::new(Template::new(
            block
                .parse_all(options)
                .trace_with(|| trace(&name, &argument_list).into())?,
        ));

        Ok(Box::new(Macro {
            name,
            arguments: argument_list,
            content,
        }))
    }

    fn reflection(&self) -> &dyn BlockReflection {
        self
    }
}

#[derive(Debug)]
struct Macro {
    name: KString,
    arguments: Vec<ParsedMacroArgument>,
    content: Arc<Template>,
}

impl Macro {
    fn trace(&self) -> String {
        trace(&self.name, &self.arguments)
    }
}

impl Renderable for Macro {
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

        let macro_object = MacroObject::Partial {
            arguments,
            template: self.content.clone(),
        };

        runtime
            .registers()
            .get_mut::<MacrosRegister>()
            .define(self.name.clone(), macro_object);

        Ok(())
    }
}

fn trace(name: &KString, arguments: &[ParsedMacroArgument]) -> String {
    if arguments.is_empty() {
        format!("{{% macro {} %}}", name)
    } else {
        format!(
            "{{% macro {}<{}> %}}",
            name,
            arguments.iter().format_with(", ", |a, f| match a {
                ParsedMacroArgument::Required { name } => f(&format_args!("{}", name)),
                ParsedMacroArgument::Defaulted { name, default } =>
                    f(&format_args!("{} = {}", name, default)),
            })
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        liquid::{blocks::DefineBlock, tags::CallTag},
        util::tests::test_liquid,
    };

    #[test]
    fn simple_macro() {
        test_liquid!(
            tags(CallTag)
            blocks(DefineBlock)
            input(
                concat!(
                    "{% begindef foo %}",
                    "Hello Liquid!",
                    "{% enddef %}",
                    "{% call foo %}"
                )
            )
            assert_output(
                "Hello Liquid!",
                "The rendered macro should say \"Hello\"."
            )
        );
    }

    #[test]
    fn complex_macro() {
        test_liquid!(
            tags(CallTag)
            blocks(DefineBlock)
            input(
                concat!(
                    "{% begindef foo<input> %}",
                    "Hello {{ input }}",
                    "{% enddef %}",
                    "{% call foo<input = \"Liquid!\"> %}"
                )
            )
            assert_output(
                "Hello Liquid!",
                "The rendered macro should say \"Hello\"."
            )
        );
    }
}
