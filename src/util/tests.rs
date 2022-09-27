macro_rules! test_liquid {
    (
        $(tags($($tags:expr),+))?
        $(blocks($($blocks:expr),+))?
        $(filters($($filters:expr),+))?
        $(globals($globals:expr))?
        input($input:expr)
        assert_output($output:expr$(, $($args:tt)+)?)
    ) => {{
        use crate::liquid::language::LanguageBuilder;

        let options = LanguageBuilder::default()$($(.tag($tags))+)?$($(.block($blocks))+)?$($(.filter($filters))+)?.build();

        test_liquid!(options(options) $(globals($globals))? input($input) block(|_runtime, rendered| {assert_eq!(rendered, $output $(, $($args)+)?);}));
    }};
    (
        $(tags($($tags:expr),+))?
        $(blocks($($blocks:expr),+))?
        $(filters($($filters:expr),+))?
        $(globals($globals:expr))?
        input($input:expr)
        block(|$runtime:ident, $rendered:ident| $test:block)
    ) => {{
        use crate::liquid::language::LanguageBuilder;

        let options = LanguageBuilder::default()$($(.tag($tags))+)?$($(.block($blocks))+)?$($(.filter($filters))+)?.build();

        test_liquid!(options(options) $(globals($globals))? input($input) block(|$runtime, $rendered| $test));
    }};
    (
        options($options:expr)
        input($input:expr)
        block(|$runtime:ident, $rendered:ident| $test:block)
    ) => {{
        use crate::{
            util::result::ResultExt,
        };
        use liquid_core::{parser, runtime::RuntimeBuilder, Renderable, Template};

        let options = $options;
        let parsed = Template::new(
            parser::parse($input, &options)
                .on_err(|e| panic!("Error parsing:\n{}", e))
                .unwrap(),
        );

        let runtime = RuntimeBuilder::new().build();
        let rendered = parsed
            .render(&runtime)
            .on_err(|e| panic!("Error rendering:\n{}", e))
            .unwrap();

        {
            let $runtime = runtime;
            let $rendered = rendered;

            $test
        }
    }};
    (
        options($options:expr)
        globals($globals:expr)
        input($input:expr)
        block(|$runtime:ident, $rendered:ident| $test:block)
    ) => {{
        use crate::{
            util::result::ResultExt,
        };
        use liquid_core::{parser, runtime::RuntimeBuilder, Renderable, Template};

        let options = $options;
        let parsed = Template::new(
            parser::parse($input, &options)
                .on_err(|e| panic!("Error parsing:\n{}", e))
                .unwrap(),
        );

        let globals = $globals;
        let runtime = RuntimeBuilder::new().set_globals(&globals).build();
        let rendered = parsed
            .render(&runtime)
            .on_err(|e| panic!("Error rendering:\n{}", e))
            .unwrap();

        {
            let $runtime = runtime;
            let $rendered = rendered;

            $test
        }
    }};
}

pub(crate) use test_liquid;
