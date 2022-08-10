use kstring::KString;
use liquid_core::{parser::TagToken, Expression, TagTokenIter};

/// A parsed argument.
///
/// This can either be a required argument with no default, or an optional
/// argument with a default.
#[derive(Debug, Clone)]
pub enum ParsedMacroArgument {
    Required { name: KString },
    Defaulted { name: KString, default: Expression },
}

/// Parses a string of arguments, inserting them into `argument_list`.
///
/// The string of arguments is in the form `key-1 [= default-1], key-2 [=
/// default-2]>`. The initial `<` is left off as it is normally used to
/// determine whether a string of argument is expected.
pub fn parse_arguments(
    arguments: &mut TagTokenIter,
    argument_list: &mut Vec<ParsedMacroArgument>,
) -> liquid_core::Result<()> {
    {
        let (first_arg, next) = parse_argument(arguments)?;
        argument_list.push(first_arg);

        if let Some(next) = next {
            match next.as_str() {
                "," => {},
                ">" => return Ok(()),
                _ => {
                    return Err(next.raise_custom_error(
                        "Expected comma separating arguments \",\" or end of argument list \">\"",
                    ))
                },
            }
        }
    }

    loop {
        let (next_arg, next) = parse_argument(arguments)?;
        argument_list.push(next_arg);

        if let Some(next) = next {
            match next.as_str() {
                "," => {},
                ">" => return Ok(()),
                _ => {
                    return Err(next.raise_custom_error(
                        "Expected comma separating arguments \",\" or end of argument list \">\"",
                    ))
                },
            }
        } else {
            return Err(arguments.raise_error("Expected end of argument list \">\""));
        }
    }
}

/// Parses a single argument.
///
/// Arguments are in the form: `key [= default]`.
pub fn parse_argument<'a>(
    arguments: &'a mut TagTokenIter,
) -> liquid_core::Result<(ParsedMacroArgument, Option<TagToken<'a>>)> {
    let name = arguments
        .expect_next("Expected argument name identifier.")?
        .expect_identifier()
        .into_result_custom_msg("Expected argument name identifier.")?
        .to_string()
        .into();

    let next = arguments.next();

    if let Some(next) = next {
        match next.as_str() {
            "=" => {
                let default = arguments.expect_next("Expected argument default expression.")?
                    .expect_value()
                    .into_result_custom_msg("Expected argument default expression.")?;

                Ok((ParsedMacroArgument::Defaulted { name, default }, arguments.next()))
            },
            "," => Ok((ParsedMacroArgument::Required { name }, Some(next))),
            ">" => Ok((ParsedMacroArgument::Required { name }, Some(next))),
            _ => Err(next.raise_custom_error("Expected argument defaulting operator \"=\", comma separating arguments \",\", or end of argument list \">\"")),
        }
    } else {
        Err(arguments.raise_error("Expected end of argument list \">\""))
    }
}
