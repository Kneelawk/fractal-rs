pub mod opts;
pub mod source;

use crate::{
    generator::{
        gpu::shader::{opts::GpuFractalOpts, source::ShaderTemplateOpts},
        FractalOpts,
    },
    util::files::debug_dir,
};
use anyhow::Context;
use naga::{
    front,
    valid::{ValidationFlags, Validator},
};
use std::{borrow::Cow, path::Path};
use tokio::{fs::File, io::AsyncWriteExt};
use wgpu::ShaderSource;

const VERTEX_SHADER_PATH: &str = "screen_rect_vertex_shader.wgsl.liquid";
const FRAGMENT_SHADER_PATH: &str = "fragment_shader_main.wgsl.liquid";

/// Both
pub struct LoadedShaders {
    pub vertex: ShaderSource<'static>,
    pub fragment: ShaderSource<'static>,
}

pub async fn load_shaders(opts: FractalOpts) -> anyhow::Result<LoadedShaders> {
    info!("Getting shader loader...");
    let loader = source::obtain_loader().context("Error obtaining shader loader")?;

    //
    // Fragment Shader
    //

    info!("Loading fragment shader template...");
    let frag_str = loader
        .compile_template(ShaderTemplateOpts {
            path: Cow::Borrowed(FRAGMENT_SHADER_PATH),
            globals: &opts
                .globals()
                .context("Error creating globals for fractal options")?,
        })
        .context("Error loading fragment shader template")?;

    info!("Writing fragment shader WGSL to debug file...");
    let frag_path = debug_dir().join("debug_fragment.wgsl");
    let mut file = File::create(&frag_path)
        .await
        .with_context(|| format!("Error opening {:?} for writing", &frag_path))?;
    file.write_all(frag_str.as_bytes())
        .await
        .with_context(|| format!("Error writing to {:?}", &frag_path))?;

    validate(&frag_str, &frag_path, "fragment").await?;

    //
    // Vertex Shader
    //

    info!("Loading vertex shader template...");
    let vert_str = loader
        .compile_template(ShaderTemplateOpts {
            path: Cow::Borrowed(VERTEX_SHADER_PATH),
            ..Default::default()
        })
        .context("Error loading vertex shader template")?;

    info!("Writing vertex shader WGSL to debug file...");
    let vert_path = debug_dir().join("debug_vertex.wgsl");
    let mut wgsl_file = File::create(&vert_path)
        .await
        .with_context(|| format!("Error opening {:?} for writing", &vert_path))?;
    wgsl_file
        .write_all(vert_str.as_bytes())
        .await
        .with_context(|| format!("Error writing to {:?}", &vert_path))?;

    validate(&vert_str, &vert_path, "vertex").await?;

    Ok(LoadedShaders {
        vertex: ShaderSource::Wgsl(Cow::Owned(vert_str)),
        fragment: ShaderSource::Wgsl(Cow::Owned(frag_str)),
    })
}

async fn validate(source: &str, source_file: &Path, shader_name: &str) -> anyhow::Result<()> {
    info!("Validating {} source...", shader_name);
    let module = front::wgsl::parse_str(source)
        .map_err(|e| {
            anyhow!(
                "Error in template file: {:?}\n{}",
                source_file,
                e.emit_to_string(source)
            )
        })
        .with_context(|| format!("Error parsing {} shader", shader_name))?;

    info!("Writing {} module as txt...", shader_name);
    let path = debug_dir().join(format!("debug_{}.txt", shader_name));
    let mut file = File::create(&path)
        .await
        .with_context(|| format!("Error opening {:?} for writing", &path))?;
    file.write_all(format!("{:#?}", &module).as_bytes())
        .await
        .with_context(|| format!("Error writing to {:?}", &path))?;

    info!("Validating {} module...", shader_name);
    let mut validator = Validator::new(ValidationFlags::all(), Default::default());
    let _ = validator
        .validate(&module)
        .with_context(|| format!("Error validating {} shader", shader_name))?;

    Ok(())
}

#[derive(Error, Debug)]
pub enum ShaderError {}
