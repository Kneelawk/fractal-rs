use crate::liquid::macros::MacrosRegister;
use kstring::KString;
use liquid_core::{Language, ParseTag, Renderable, Runtime, TagReflection, TagTokenIter};
use std::io::Write;

/// Deletes a macro if it is currently defined.
#[derive(Debug, Copy, Clone, Default)]
pub struct UndefTag;

impl TagReflection for UndefTag {
    fn tag(&self) -> &str {
        "undef"
    }

    fn description(&self) -> &str {
        "Deletes a macro if it is currently defined"
    }
}

impl ParseTag for UndefTag {
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

        Ok(Box::new(DelMacro { name }))
    }

    fn reflection(&self) -> &dyn TagReflection {
        self
    }
}

#[derive(Debug)]
struct DelMacro {
    name: KString,
}

impl Renderable for DelMacro {
    fn render_to(&self, _writer: &mut dyn Write, runtime: &dyn Runtime) -> liquid_core::Result<()> {
        runtime
            .registers()
            .get_mut::<MacrosRegister>()
            .undefine(self.name.as_str());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        liquid::{
            macros::MacrosRegister,
            tags::{DefineTag, UndefTag},
        },
        util::tests::test_liquid,
    };
    use liquid_core::Runtime;

    #[test]
    fn undefine_test() {
        test_liquid!(
            tags(DefineTag, UndefTag)
            input(concat!("{% define foo %}", "{% undef foo %}"))
            block(|runtime, _rendered| {
                assert!(
                    !runtime
                        .registers()
                        .get_mut::<MacrosRegister>()
                        .is_defined("foo"),
                    "Macro foo should not be defined."
                );
            })
        );
    }
}
