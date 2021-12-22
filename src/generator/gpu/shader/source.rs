use cached::proc_macro::cached;
use include_dir::{Dir, DirEntry};
use liquid::{
    object,
    partials::{LazyCompiler, PartialSource},
    Parser, ParserBuilder, Template,
};
use serde::__private::from_utf8_lossy;
use std::{borrow::Cow, fmt::Debug, sync::Arc};

/// Shader source files include into this rust binary.
static SHADER_TEMPLATE_DIR: Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/res/shader/generator");

lazy_static! {
    static ref DIR_CONTENTS: Vec<String> = make_dir_contents();
    static ref LIQUID_PARSER: Parser = make_liquid_parser();
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

fn make_compiler() -> LazyCompiler<ShaderTemplateDirSource> {
    LazyCompiler::<ShaderTemplateDirSource>::new(ShaderTemplateDirSource)
}

fn make_liquid_parser() -> Parser {
    ParserBuilder::new()
        .stdlib()
        .partials(make_compiler())
        .build()
        .expect("Error building `liquid` shader template parser")
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
            .map(|f| from_utf8_lossy(f.contents()))
    }
}

#[cached(result = true)]
fn get_template(path: String) -> Result<Arc<Template>, ShaderTemplateError> {
    let source = SHADER_TEMPLATE_DIR
        .get_file(&path)
        .ok_or(ShaderTemplateError::NoSuchFile)?
        .contents_utf8()
        .ok_or(ShaderTemplateError::NotUTF8)?;
    Ok(Arc::new(LIQUID_PARSER.parse(source)?))
}

pub fn compile_template(path: String) -> Result<String, ShaderTemplateError> {
    let template = get_template(path)?;
    let object = object!({});
    Ok(template.render(&object)?)
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
