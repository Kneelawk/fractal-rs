use crate::{
    generator::gpu::shader::source::{
        dynamic::DynamicShaderTemplateLoader, r#static::StaticShaderTemplateLoader,
    },
    liquid::default_language,
};
use anyhow::Context;
use liquid::object;
use liquid_core::{Language, Object, ObjectView};
use std::{borrow::Cow, env, env::VarError, sync::Arc};

mod dynamic;
mod r#static;

const SHADER_PATH_ENV_VAR: &str = "FRACTAL_RS_2_SHADER_PATH";

lazy_static! {
    static ref EMPTY_GLOBALS: Object = object!({});
    static ref LIQUID_LANGUAGE: Arc<Language> = make_language();
}

/// Gets or creates a ShaderTemplateLoader instance depending on circumstances.
pub fn obtain_loader() -> anyhow::Result<Box<dyn ShaderTemplateLoader + Send>> {
    match env::var(SHADER_PATH_ENV_VAR) {
        Ok(path) => {
            info!(
                "Using dynamic shader template loader with path: \"{}\"",
                &path
            );

            Ok(Box::new(DynamicShaderTemplateLoader::new(&path).context(
                "Error creating dynamic shader template loader",
            )?))
        },
        Err(VarError::NotPresent) => {
            info!("Using static shader template loader");

            Ok(Box::new(StaticShaderTemplateLoader))
        },
        Err(e) => Err(e).context("Error getting shader path environment variable"),
    }
}

/// Options for filling a shader template.
pub struct ShaderTemplateOpts<'a> {
    pub path: Cow<'a, str>,
    pub globals: &'a dyn ObjectView,
}

impl<'a> Default for ShaderTemplateOpts<'a> {
    fn default() -> Self {
        Self {
            path: Default::default(),
            globals: &*EMPTY_GLOBALS,
        }
    }
}

/// Instance of an object used for loading and filling templates.
///
/// What caching happens between instances is implementation-specific.
pub trait ShaderTemplateLoader {
    /// Loads and fills a template.
    fn compile_template(&self, opts: ShaderTemplateOpts) -> anyhow::Result<String>;
}

/// When an error happens while compiling a shader.
#[derive(Debug, Clone, Error)]
pub enum ShaderTemplateError {
    #[error("No such shader template file")]
    NoSuchFile,
}

fn make_language() -> Arc<Language> {
    Arc::new(default_language().build())
}
