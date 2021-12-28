use crate::liquid::macros::{MacroArgument, MacroObject, MacrosRegister};
use itertools::Itertools;
use kstring::KString;
use liquid_core::{
    error::{ResultLiquidExt, ResultLiquidReplaceExt},
    runtime::StackFrame,
    Expression, Language, ParseTag, Renderable, Runtime, TagReflection, TagTokenIter, ValueView,
};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
};

/// Tag for the use of a macro.
///
/// # Syntax
/// ```liquid
/// {% call 'name' [<'argument-1' = 'value-1', 'argument-2' = 'value-2', ...>] %}
/// ```
///
/// # Examples
/// ```liquid
/// {% define foo = "bar" %}
///
/// {% call foo %}
/// ```
///
/// An example of using a macro with arguments:
/// ```liquid
/// {% define add_one<input> = input | plus: 1 %}
///
/// {% call add_one<input = 2> %}
/// ```
#[derive(Copy, Clone, Debug, Default)]
pub struct CallTag;

impl TagReflection for CallTag {
    fn tag(&self) -> &str {
        "call"
    }

    fn description(&self) -> &str {
        "Use an already existing macro."
    }
}

impl ParseTag for CallTag {
    fn parse(
        &self,
        mut arguments: TagTokenIter,
        _options: &Language,
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
                "<" => parse_arguments(&mut arguments, &mut argument_list)?,
                _ => {
                    return Err(next.raise_custom_error(
                        "Expected definition operator \"=\" or start of argument list \"<\"",
                    ))
                },
            }
        }

        Ok(Box::new(Call {
            name,
            arguments: argument_list,
        }))
    }

    fn reflection(&self) -> &dyn TagReflection {
        self
    }
}

fn parse_arguments(
    arguments: &mut TagTokenIter,
    argument_list: &mut Vec<CallArgument>,
) -> liquid_core::Result<()> {
    argument_list.push(parse_argument(arguments)?);

    while let Some(next) = arguments.next() {
        match next.as_str() {
            "," => {},
            ">" => return Ok(()),
            _ => {
                return Err(next.raise_custom_error(
                    "Expected comma separating arguments \",\" or end of argument list \">\"",
                ))
            },
        }

        argument_list.push(parse_argument(arguments)?);
    }

    Err(arguments.raise_error("Expected end of argument list \">\""))
}

fn parse_argument(arguments: &mut TagTokenIter) -> liquid_core::Result<CallArgument> {
    let name = arguments
        .expect_next("Expected macro argument identifier.")?
        .expect_identifier()
        .into_result_custom_msg("Expected macro argument identifier.")?
        .to_string()
        .into();

    arguments
        .expect_next("Expected argument assignemnent operator \"=\"")?
        .expect_str("=")
        .into_result_custom_msg("Expected argument assignment operator \"=\"")?;

    let value = arguments
        .expect_next("Expected macro argument value expression.")?
        .expect_value()
        .into_result_custom_msg("Expected macro argument value expression.")?;

    Ok(CallArgument { name, value })
}

#[derive(Debug)]
struct CallArgument {
    name: KString,
    value: Expression,
}

#[derive(Debug)]
struct Call {
    name: KString,
    arguments: Vec<CallArgument>,
}

impl Call {
    fn trace(&self) -> String {
        if self.arguments.is_empty() {
            format!("{{% call {} %}}", &self.name)
        } else {
            format!(
                "{{% call {}<{}> %}}",
                &self.name,
                self.arguments
                    .iter()
                    .format_with(", ", |a, f| f(&format_args!("{} = {}", &a.name, &a.value)))
            )
        }
    }
}

impl Renderable for Call {
    fn render_to(&self, writer: &mut dyn Write, runtime: &dyn Runtime) -> liquid_core::Result<()> {
        let using = {
            let register = runtime.registers().get_mut::<MacrosRegister>();
            register
                .get(self.name.as_str())
                .ok_or_else(|| {
                    let available = itertools::join(register.available(), ", ");
                    liquid_core::Error::with_msg("Unknown macro")
                        .context("requested macro", self.name.clone())
                        .context("available macros", available)
                })?
                .clone()
        };

        let argument_list: Vec<_> = self.arguments.iter().map(|arg| arg.name.as_str()).collect();
        match &using {
            MacroObject::Flag => {
                if !argument_list.is_empty() {
                    return liquid_core::Error::with_msg("Provided arguments to a flag macro")
                        .context(
                            "provided arguments",
                            itertools::join(argument_list.iter(), ", "),
                        )
                        .into_err();
                }
            },
            MacroObject::FilterChain { arguments, .. } => {
                check_arguments(arguments, &argument_list)?
            },
            MacroObject::Partial { arguments, .. } => check_arguments(arguments, &argument_list)?,
        }

        if !using.is_flag() {
            let mut pass_through = HashMap::new();
            for CallArgument { name, value } in self.arguments.iter() {
                pass_through.insert(
                    name.clone(),
                    value.evaluate(runtime).trace_with(|| self.trace().into())?,
                );
            }

            let scope = StackFrame::new(runtime, &pass_through);

            match using {
                MacroObject::Flag => {},
                MacroObject::FilterChain { chain, .. } => {
                    write!(
                        writer,
                        "{}",
                        chain
                            .evaluate(&scope)
                            .trace_with(|| self.trace().into())?
                            .render()
                    )
                    .replace("Failed to render")
                    .trace_with(|| self.trace().into())?;
                },
                MacroObject::Partial { template, .. } => {
                    template
                        .render_to(writer, &scope)
                        .trace_with(|| self.trace().into())?;
                },
            }
        }

        Ok(())
    }
}

fn check_arguments(
    macro_arguments: &Vec<MacroArgument>,
    argument_list: &Vec<&str>,
) -> liquid_core::Result<()> {
    let mut provided: HashSet<_> = argument_list.iter().copied().collect();
    let mut missing = vec![];
    let mut required_names = vec![];
    let mut argument_names = vec![];

    for arg in macro_arguments.iter() {
        match arg {
            MacroArgument::Required { name } => {
                let as_str = name.as_str();
                required_names.push(as_str);
                argument_names.push(as_str);
                if !provided.remove(as_str) {
                    missing.push(as_str);
                }
            },
            MacroArgument::Defaulted { name, .. } => {
                let as_str = name.as_str();
                argument_names.push(as_str);
                provided.remove(as_str);
            },
        }
    }

    if !provided.is_empty() {
        return liquid_core::Error::with_msg("Provided unknown argument to macro")
            .context(
                "unknown arguments",
                itertools::join(provided.iter().copied().sorted(), ", "),
            )
            .context("available arguments", itertools::join(argument_names, ", "))
            .into_err();
    }

    if !missing.is_empty() {
        return liquid_core::Error::with_msg("Missing macro arguments")
            .context("missing arguments", itertools::join(missing, ", "))
            .context("required arguments", itertools::join(required_names, ", "))
            .into_err();
    }

    Ok(())
}
