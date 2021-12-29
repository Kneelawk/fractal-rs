use crate::liquid::macros::{MacroArgument, MacroObject, MacrosRegister};
use kstring::KStringCow;
use liquid_core::{
    runtime::StackFrame, Expression, Filter, FilterParameters, Renderable, Runtime, Value,
    ValueView,
};
use std::collections::HashMap;

#[derive(Debug, FilterParameters)]
struct CallArgs {
    #[parameter(description = "The name of the macro to call.", arg_type = "str")]
    name: Expression,

    #[parameter(
        description = "The name of the macro parameter to send the input to.",
        arg_type = "str",
        mode = "keyword"
    )]
    param: Option<Expression>,
}

/// Filter that calls a macro and supplies the input to that macro.
///
/// # Parameters
/// * `name` - The name of the macro to call.
///   - Type: `str`
///   - Positional or keyword: Positional
/// * `param` - The name of the macro parameter to send the input to.
///   - Type: `str`
///   - Positional or keyword: Keyword
#[derive(Debug, Copy, Clone, ParseFilter, FilterReflection)]
#[filter(
    name = "call",
    description = "Calls a macro and supplies the input to that macro.",
    parameters(CallArgs),
    parsed(CallFilterImpl)
)]
pub struct CallFilter;

#[derive(Debug, FromFilterParameters, Display_filter)]
#[name = "call"]
struct CallFilterImpl {
    #[parameters]
    args: CallArgs,
}

impl Filter for CallFilterImpl {
    fn evaluate(&self, input: &dyn ValueView, runtime: &dyn Runtime) -> liquid_core::Result<Value> {
        let args = self.args.evaluate(runtime)?;

        let using = {
            let register = runtime.registers().get_mut::<MacrosRegister>();
            register
                .get(args.name.as_str())
                .ok_or_else(|| {
                    let available = itertools::join(register.available(), ", ");
                    liquid_core::Error::with_msg("Unknown macro")
                        .context("requested macro", args.name.clone())
                        .context("available macros", available)
                })?
                .clone()
        };

        match &using {
            MacroObject::Flag => {
                if args.param.is_some() {
                    return liquid_core::Error::with_msg("Provided arguments to a flag macro")
                        .context(
                            "provided argument",
                            args.param.map(|s| s.into_owned()).unwrap(),
                        )
                        .into_err();
                }
            },
            MacroObject::FilterChain { arguments, .. } => check_arguments(arguments, &args.param)?,
            MacroObject::Partial { arguments, .. } => check_arguments(arguments, &args.param)?,
        }

        if using.is_flag() {
            Ok(Value::Nil)
        } else {
            let mut pass_through = HashMap::new();
            if let Some(param) = args.param {
                pass_through.insert(param, input);
            }

            let scope = StackFrame::new(runtime, &pass_through);

            match using {
                MacroObject::Flag => unreachable!(),
                MacroObject::FilterChain { chain, .. } => {
                    chain.evaluate(&scope).map(|v| v.into_owned())
                },
                MacroObject::Partial { template, .. } => {
                    template.render(&scope).map(|s| Value::scalar(s))
                },
            }
        }
    }
}

fn check_arguments(
    macro_arguments: &Vec<MacroArgument>,
    param: &Option<KStringCow>,
) -> liquid_core::Result<()> {
    let mut missing = vec![];
    let mut param_invalid = param.is_some();
    let mut required_names = vec![];
    let mut argument_names = vec![];

    for argument in macro_arguments.iter() {
        match argument {
            MacroArgument::Required { name } => {
                let as_str = name.as_str();
                required_names.push(as_str);
                argument_names.push(as_str);
                if let Some(param) = param {
                    if param.as_str() == as_str {
                        param_invalid = false;
                    } else {
                        missing.push(as_str)
                    }
                } else {
                    missing.push(as_str)
                }
            },
            MacroArgument::Defaulted { name, .. } => {
                let as_str = name.as_str();
                argument_names.push(as_str);
                if let Some(param) = param {
                    if param.as_str() == as_str {
                        param_invalid = false;
                    }
                }
            },
        }
    }

    if param_invalid {
        return liquid_core::Error::with_msg("Provided unknown argument to macro")
            .context(
                "unknown argument",
                param.clone().map(|s| s.into_owned()).unwrap(),
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

#[cfg(test)]
mod tests {
    use crate::{
        liquid::{blocks::DefineBlock, filters::CallFilter, tags::DefineTag},
        util::tests::test_liquid,
    };
    use liquid_lib::stdlib::{Replace, Upcase};

    #[test]
    fn simple_test() {
        test_liquid!(
            tags(DefineTag)
            filters(CallFilter)
            input(
                concat!(
                    "{% define foo = \"Hello\" %}",
                    "{{ \"\" | call: \"foo\" }}"
                )
            )
            assert_output(
                "Hello",
                "The rendered string should be \"Hello\""
            )
        );
    }

    #[test]
    fn param_test() {
        test_liquid!(
            tags(DefineTag)
            filters(CallFilter, Upcase, Replace)
            input(
                concat!(
                    "{% define foo<input> = input | upcase | replace: \"WORLD\", \"CAT\" %}",
                    "{{ \"Hello World!\" | call: \"foo\", param: \"input\" }}"
                )
            )
            assert_output(
                "HELLO CAT!",
                "The rendered string should be \"HELLO CAT!\""
            )
        );
    }

    #[test]
    fn complex_test() {
        test_liquid!(
            blocks(DefineBlock)
            filters(CallFilter)
            input(
                concat!(
                    "{% begindef foo<input> %}",
                    "Hello {{ input }}",
                    "{% enddef %}",
                    "{{ \"Liquid!\" | call: \"foo\", param: \"input\" }}"
                )
            )
            assert_output(
                "Hello Liquid!",
                "The rendered string should be \"Hello World!\""
            )
        );
    }
}
