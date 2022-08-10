use crate::{
    generator::gpu::shader::source::{
        ShaderTemplateError, ShaderTemplateLoader, ShaderTemplateOpts, LIQUID_LANGUAGE,
    },
    liquid::partials::CompositePartialStore,
};
use anyhow::Context;
use include_dir::{Dir, DirEntry};
use liquid::partials::{LazyCompiler, PartialSource};
use liquid_core::{
    partials::PartialCompiler,
    runtime::{PartialStore, RuntimeBuilder},
};
use std::{borrow::Cow, fmt::Debug, sync::Arc};

/// Shader source files include into this rust binary.
static SHADER_TEMPLATE_DIR: Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/res/shader/generator");

lazy_static! {
    static ref DIR_CONTENTS: Vec<String> = make_dir_contents();
    static ref STATIC_STORE: Arc<dyn PartialStore + Send + Sync> = make_store();
}

fn make_dir_contents() -> Vec<String> {
    let mut contents = vec![];
    walk_dir_contents(&SHADER_TEMPLATE_DIR, &mut contents);
    contents
}

fn walk_dir_contents(dir: &Dir, into: &mut Vec<String>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(dir) => {
                walk_dir_contents(dir, into);
            },
            DirEntry::File(file) => {
                into.push(file.path().to_string_lossy().to_string());
            },
        }
    }
}

fn make_store() -> Arc<dyn PartialStore + Send + Sync> {
    LazyCompiler::<ShaderTemplateDirSource>::new(ShaderTemplateDirSource)
        .compile(LIQUID_LANGUAGE.clone())
        .expect("Error compiling shader template `liquid` static partial store")
        .into()
}

/// `PartialSource` representing the contents of the shader template dir.
#[derive(Debug)]
struct ShaderTemplateDirSource;

impl PartialSource for ShaderTemplateDirSource {
    fn contains(&self, name: &str) -> bool {
        SHADER_TEMPLATE_DIR.get_file(name).is_some()
    }

    fn names(&self) -> Vec<&str> {
        DIR_CONTENTS.iter().map(|s| s.as_str()).collect()
    }

    fn try_get(&self, name: &str) -> Option<Cow<str>> {
        SHADER_TEMPLATE_DIR
            .get_file(name)
            .map(|f| String::from_utf8_lossy(f.contents()))
    }
}

pub struct StaticShaderTemplateLoader;

impl ShaderTemplateLoader for StaticShaderTemplateLoader {
    fn compile_template(&self, opts: ShaderTemplateOpts) -> anyhow::Result<String> {
        // Build the composite store. This will eventually be able to contain
        // naga-generated partials as well, allowing templates to incorporate
        // generated code. Though naga-generated partials will likely also be
        // template-based.
        let store = CompositePartialStore::new(vec![STATIC_STORE.clone()]);

        // Build the runtime.
        let runtime = RuntimeBuilder::new()
            .set_globals(opts.globals)
            .set_partials(&store)
            .build();

        // Get the cached template.
        let template = store
            .try_get(&opts.path)
            .ok_or(ShaderTemplateError::NoSuchFile)?;

        // Render the template.
        Ok(template
            .render(&runtime)
            .context("Error parsing or rendering shader template")?)
    }
}
