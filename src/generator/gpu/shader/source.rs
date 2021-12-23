use crate::liquid::{default_language, partials::CompositePartialStore};
use cached::proc_macro::cached;
use include_dir::{Dir, DirEntry};
use liquid::partials::{LazyCompiler, PartialSource};
use liquid_core::{
    parser,
    partials::PartialCompiler,
    runtime::{PartialStore, RuntimeBuilder},
    Language, Renderable, Template,
};
use std::{borrow::Cow, fmt::Debug, sync::Arc};

/// Shader source files include into this rust binary.
static SHADER_TEMPLATE_DIR: Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/res/shader/generator");

lazy_static! {
    static ref DIR_CONTENTS: Vec<String> = make_dir_contents();
    static ref LIQUID_LANGUAGE: Arc<Language> = make_language();
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

fn make_language() -> Arc<Language> {
    Arc::new(default_language().build())
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

#[cached(result = true)]
fn get_template(path: String) -> Result<Arc<Template>, ShaderTemplateError> {
    let source = SHADER_TEMPLATE_DIR
        .get_file(&path)
        .ok_or(ShaderTemplateError::NoSuchFile)?
        .contents_utf8()
        .ok_or(ShaderTemplateError::NotUTF8)?;
    Ok(Arc::new(Template::new(parser::parse(
        source,
        LIQUID_LANGUAGE.as_ref(),
    )?)))
}

pub fn compile_template(path: String) -> Result<String, ShaderTemplateError> {
    // Build the composite store. This will eventually be able to contain
    // naga-generated partials as well, allowing templates to incorporate
    // generated code. Though naga-generated partials will likely also be
    // template-based.
    let store = CompositePartialStore::new(vec![STATIC_STORE.clone()]);

    // Build the runtime.
    let runtime = RuntimeBuilder::new().set_partials(&store).build();

    // Get the cached template.
    let template = get_template(path)?;

    // Render the template.
    Ok(template.render(&runtime)?)
}

#[derive(Debug, Clone, Error)]
pub enum ShaderTemplateError {
    #[error("No such shader template file")]
    NoSuchFile,
    #[error("Shader template file is not in UTF8")]
    NotUTF8,
    #[error("Error parsing or rendering shader template")]
    LiquidError(#[from] liquid::Error),
}
