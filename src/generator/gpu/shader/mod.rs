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
use liquid_core::object;
use naga::{
    back, front,
    valid::{ValidationFlags, Validator},
};
use std::borrow::Cow;
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
    let template_source = loader
        .compile_template(ShaderTemplateOpts {
            path: Cow::Borrowed(FRAGMENT_SHADER_PATH),
            globals: &object!({
                "opts": object!({
                    "c_real": opts.c.re,
                    "c_imag": opts.c.im,
                    "iterations": opts.iterations,
                    "mandelbrot": opts.mandelbrot
                })
            }),
        })
        .context("Error loading fragment shader template")?;

    info!("Writing filled template...");
    let path = debug_dir().join("debug_fragment_template.wgsl");
    let mut file = File::create(&path)
        .await
        .with_context(|| format!("Error opening {:?} for writing", &path))?;
    file.write_all(template_source.as_bytes())
        .await
        .with_context(|| format!("Error writing to {:?}", &path))?;

    info!("Loading utility functions...");
    let mut module = front::wgsl::parse_str(&template_source)
        .map_err(|e| {
            anyhow!(
                "Error in template file: {:?}\n{}",
                &path,
                e.emit_to_string(&template_source)
            )
        })
        .context("Error parsing filled WGSL template")?;

    opts.install(&mut module)
        .context("Error installing fractal options")?;

    info!("Writing module as txt...");
    let path = debug_dir().join("debug_fragment.txt");
    let mut file = File::create(&path)
        .await
        .with_context(|| format!("Error opening {:?} for writing", &path))?;
    file.write_all(format!("{:#?}", &module).as_bytes())
        .await
        .with_context(|| format!("Error writing to {:?}", &path))?;

    info!("Validating module...");
    let mut validator = Validator::new(ValidationFlags::all(), Default::default());
    let module_info = validator
        .validate(&module)
        .context("Error while validating filled WGSL template")?;

    info!("Compiling WGSL...");
    let mut frag_str = String::new();
    let mut writer = back::wgsl::Writer::new(&mut frag_str);
    writer
        .write(&module, &module_info)
        .context("Error writing validated WGSL to string")?;
    writer.finish();

    info!("Writing WGSL...");
    let path = debug_dir().join("debug_fragment.wgsl");
    let mut wgsl_file = File::create(&path)
        .await
        .with_context(|| format!("Error opening {:?} for writing", &path))?;
    wgsl_file
        .write_all(frag_str.as_bytes())
        .await
        .with_context(|| format!("Error writing to {:?}", &path))?;

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

    info!("Writing Vertex Shader WGSL...");
    let path = debug_dir().join("debug_vertex.wgsl");
    let mut wgsl_file = File::create(&path)
        .await
        .with_context(|| format!("Error opening {:?} for writing", &path))?;
    wgsl_file
        .write_all(vert_str.as_bytes())
        .await
        .with_context(|| format!("Error writing to {:?}", &path))?;

    Ok(LoadedShaders {
        vertex: ShaderSource::Wgsl(Cow::Owned(vert_str)),
        fragment: ShaderSource::Wgsl(Cow::Owned(frag_str)),
    })
}

#[derive(Error, Debug)]
pub enum ShaderError {
    #[error("Missing template function '{}'", .0)]
    MissingTemplateFunction(String),
    #[error("Missing template constant '{}'", .0)]
    MissingTemplateConstant(String),
    #[error("Missing template type '{}'", .0)]
    MissingTemplateType(String),
}
