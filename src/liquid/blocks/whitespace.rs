use itertools::Itertools;
use kstring::KString;
use liquid_core::{
    error::ResultLiquidExt, parser, BlockReflection, Expression, Language, ParseBlock, Renderable,
    Runtime, TagBlock, TagTokenIter, Template, ValueView,
};
use regex::Regex;
use std::{
    collections::HashMap,
    fmt::{Debug, Formatter},
    io::Write,
};

lazy_static! {
    static ref CHECK_REGEX: Regex = Regex::new("(\\{%|%\\}|\\s)").unwrap();
}

/// Block for controlling whitespace produced.
///
/// Configurable existing whitespace characters are removed and special `{% sp
/// %}`, `{% nl %}`, `{% cr %}`, and `{% tab %}` tags are used instead.
#[derive(Debug, Default, Copy, Clone)]
pub struct WhitespaceBlock;

impl BlockReflection for WhitespaceBlock {
    fn start_tag(&self) -> &str {
        "whitespace"
    }

    fn end_tag(&self) -> &str {
        "endwhitespace"
    }

    fn description(&self) -> &str {
        "Block for controlling whitespace produced."
    }
}

impl ParseBlock for WhitespaceBlock {
    fn parse(
        &self,
        mut arguments: TagTokenIter,
        mut block: TagBlock,
        options: &Language,
    ) -> liquid_core::Result<Box<dyn Renderable>> {
        let mut args = HashMap::new();
        parse_arguments(&mut arguments, &mut args)?;

        let content = block.escape_liquid(true)?.to_string().into();

        Ok(Box::new(Whitespace {
            options: options.clone(),
            args,
            content,
        }))
    }

    fn reflection(&self) -> &dyn BlockReflection {
        self
    }
}

struct Whitespace {
    options: Language,
    args: HashMap<WhitespaceArgs, Expression>,
    content: KString,
}

impl Whitespace {
    fn trace(&self) -> String {
        trace(&self.args)
    }
}

impl Debug for Whitespace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Whitespace")
            .field("options", &"Language".to_string())
            .field("args", &self.args)
            .field("content", &self.content)
            .finish()
    }
}

impl Renderable for Whitespace {
    fn render_to(&self, writer: &mut dyn Write, runtime: &dyn Runtime) -> liquid_core::Result<()> {
        let mut evaluated_args = HashMap::new();
        for (&k, v) in self.args.iter() {
            evaluated_args.insert(
                k,
                v.evaluate(runtime)
                    .trace(self.trace())?
                    .to_kstr()
                    .to_string(),
            );
        }

        let escaped = escape(&self.content, &evaluated_args).trace(self.trace())?;

        let parsed = Template::new(parser::parse(&escaped, &self.options).trace(self.trace())?);

        parsed.render_to(writer, runtime).trace(self.trace())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum WhitespaceArgs {
    Spaces,
    Tabs,
    Newlines,
    CarriageReturns,
}

impl WhitespaceArgs {
    fn default_value(&self) -> KString {
        match self {
            WhitespaceArgs::Spaces => "\\{% ?sp ?%\\}".to_string().into(),
            WhitespaceArgs::Newlines => "\\{% ?nl ?%\\}".to_string().into(),
            WhitespaceArgs::Tabs => "\\{% ?tab ?%\\}".to_string().into(),
            WhitespaceArgs::CarriageReturns => "\\{% ?cr ?%\\}".to_string().into(),
        }
    }

    fn expanded_value(&self) -> &'static str {
        match self {
            WhitespaceArgs::Spaces => " ",
            WhitespaceArgs::Tabs => "\t",
            WhitespaceArgs::Newlines => "\n",
            WhitespaceArgs::CarriageReturns => "\r",
        }
    }
}

fn parse_arguments(
    arguments: &mut TagTokenIter,
    parsed: &mut HashMap<WhitespaceArgs, Expression>,
) -> liquid_core::Result<()> {
    loop {
        let ws_name = match arguments.next() {
            None => return Ok(()),
            Some(tok) => match tok.as_str() {
                "sp" => WhitespaceArgs::Spaces,
                "nl" => WhitespaceArgs::Newlines,
                "tab" => WhitespaceArgs::Tabs,
                "cr" => WhitespaceArgs::CarriageReturns,
                _ => {
                    return Err(tok.raise_custom_error(
                        "Expected either \"sp\", \"nl\", \"cr\", or \"tab\" whitespace identifier names.",
                    ))
                },
            },
        };

        match arguments.next() {
            None => {
                parsed.insert(
                    ws_name,
                    Expression::Literal(ws_name.default_value().to_value()),
                );
                return Ok(());
            },
            Some(tok) => {
                match tok.as_str() {
                    "," => {
                        parsed.insert(
                            ws_name,
                            Expression::Literal(ws_name.default_value().to_value()),
                        );
                        // continue loop
                    },
                    "=" => {
                        let val = arguments
                            .expect_next("Expected whitespace identifier string.")?
                            .expect_value()
                            .into_result_custom_msg("Expected whitespace identifier string.")?;
                        parsed.insert(ws_name, val);

                        match arguments.next() {
                            None => return Ok(()),
                            Some(tok) => match tok.as_str() {
                                "," => {} // continue loop
                                _ => return Err(tok.raise_custom_error("Expected comma separating arguments \",\" or no more arguments."))
                            }
                        }
                    },
                    _ => return Err(tok.raise_custom_error(
                        "Expected comma separating arguments\",\", assignment operator \"=\", or no more arguments.",
                    )),
                }
            },
        }
    }
}

fn trace(args: &HashMap<WhitespaceArgs, Expression>) -> String {
    let args_str = args
        .iter()
        .map(|(&k, v)| match k {
            WhitespaceArgs::Spaces => format!("sp={}", v.to_string()),
            WhitespaceArgs::Newlines => format!("nl={}", v.to_string()),
            WhitespaceArgs::Tabs => format!("tab={}", v.to_string()),
            WhitespaceArgs::CarriageReturns => format!("cr={}", v.to_string()),
        })
        .join(" ");

    format!("{{% whitespace {} %}}", args_str)
}

fn escape(
    content: &KString,
    args: &HashMap<WhitespaceArgs, String>,
) -> liquid_core::Result<String> {
    let remove_spaces = args.contains_key(&WhitespaceArgs::Spaces);
    let remove_newlines = args.contains_key(&WhitespaceArgs::Newlines);
    let remove_carriage_returns = args.contains_key(&WhitespaceArgs::CarriageReturns);
    let remove_tabs = args.contains_key(&WhitespaceArgs::Tabs);

    let mut in_tag = false;
    let mut last_end = 0;

    let mut res = String::new();

    for mat in CHECK_REGEX.find_iter(content) {
        res.push_str(content.get(last_end..mat.start()).unwrap());

        let str = mat.as_str();
        match str {
            "{%" => {
                in_tag = true;
                res.push_str("{%")
            },
            "%}" => {
                in_tag = false;
                res.push_str("%}")
            },
            " " => {
                if !remove_spaces || in_tag {
                    res.push_str(" ");
                }
            },
            "\n" => {
                if !remove_newlines || in_tag {
                    res.push_str("\n");
                }
            },
            "\r" => {
                if !remove_carriage_returns || in_tag {
                    res.push_str("\r");
                }
            },
            "\t" => {
                if !remove_tabs || in_tag {
                    res.push_str("\t");
                }
            },
            _ => res.push_str(str),
        }

        last_end = mat.end();
    }

    res.push_str(content.get(last_end..content.len()).unwrap());

    for (arg, key) in args.iter() {
        res = Regex::new(key)
            .map_err(|err| liquid_core::Error::with_msg("Search string regex error").cause(err))?
            .replace_all(&res, arg.expanded_value())
            .to_string();
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    use crate::{liquid::blocks::WhitespaceBlock, util::tests::test_liquid};
    use liquid_lib::stdlib::ForBlock;

    #[test]
    fn simple_whitespace() {
        test_liquid!(
            blocks(WhitespaceBlock)
            input("{% whitespace sp %} {% endwhitespace %}")
            assert_output(
                "",
                "The rendered macro should remove all whitespace."
            )
        )
    }

    #[test]
    fn simple_whitespace_2() {
        test_liquid!(
            blocks(WhitespaceBlock)
            input(r#"{% whitespace sp, nl %}
            Hello World!
            {% endwhitespace %}"#)
            assert_output(
                "HelloWorld!",
                "The rendered macro should remove all whitespace and newlines."
            )
        )
    }

    #[test]
    fn simple_whitespace_3() {
        test_liquid!(
            blocks(WhitespaceBlock)
            input(r#"{% whitespace sp, nl %}
            Hello{% sp %}World!
            {% endwhitespace %}"#)
            assert_output(
                "Hello World!",
                "The rendered macro should remove all whitespace and add the escaped whitespace."
            )
        )
    }

    #[test]
    fn newlines_only() {
        test_liquid!(
            blocks(WhitespaceBlock)
            input("{% whitespace nl %}Hello {% nl %}World!{% endwhitespace %}")
            assert_output(
                "Hello \nWorld!",
                "The rendered macro should only remove newlines."
            )
        )
    }

    #[test]
    fn no_remove_inside_tags() {
        test_liquid!(
            blocks(WhitespaceBlock, ForBlock)
            globals(liquid::object!({
                "list": vec!["one", "two", "three"]
            }))
            input(r#"{% whitespace sp, nl %}
            {% for item in list %}
                {{ item }}{% nl %}
            {% endfor %}
            {% endwhitespace %}"#)
            assert_output(
                "one\ntwo\nthree\n",
                "The rendered macro should only remove whitespaces outside tags."
            )
        )
    }
}
