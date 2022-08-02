use crate::{
    generator::gpu::shader::source::{ShaderTemplateLoader, ShaderTemplateOpts, LIQUID_LANGUAGE},
    liquid::partials::{CompositePartialStore, FilePartialSource},
};
use anyhow::Context;
use liquid_core::{
    partials::{LazyCompiler, PartialCompiler},
    runtime::{PartialStore, RuntimeBuilder},
};
use std::{path::Path, sync::Arc};

pub struct DynamicShaderTemplateLoader {
    store: Arc<dyn PartialStore + Send + Sync>,
}

impl DynamicShaderTemplateLoader {
    pub fn new(root: impl AsRef<Path>) -> anyhow::Result<DynamicShaderTemplateLoader> {
        let source = FilePartialSource::new(root).context("Error creating file source")?;
        let store = LazyCompiler::<FilePartialSource>::new(source)
            .compile(LIQUID_LANGUAGE.clone())
            .context("Error compiling partial store")?
            .into();

        Ok(DynamicShaderTemplateLoader { store })
    }
}

impl ShaderTemplateLoader for DynamicShaderTemplateLoader {
    fn compile_template(&self, opts: ShaderTemplateOpts) -> anyhow::Result<String> {
        let store = CompositePartialStore::new(vec![self.store.clone()]);

        let runtime = RuntimeBuilder::new()
            .set_globals(opts.globals)
            .set_partials(&store)
            .build();

        let template = store.get(&opts.path)?;

        Ok(template.render(&runtime)?)
    }
}
