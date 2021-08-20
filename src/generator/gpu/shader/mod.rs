pub mod opts;

use crate::generator::{gpu::shader::opts::GpuFractalOpts, FractalOpts};
use naga::{
    back, front,
    valid::{ValidationFlags, Validator},
};
use std::borrow::Cow;
use tokio::{fs::File, io::AsyncWriteExt};
use wgpu::ShaderSource;

const TEMPLATE_SOURCE: &str = include_str!("template.wgsl");

pub async fn load_shaders(opts: FractalOpts) -> anyhow::Result<ShaderSource<'static>> {
    info!("Loading utility functions...");
    let mut module = front::wgsl::parse_str(TEMPLATE_SOURCE).unwrap();

    opts.install(&mut module)?;

    info!("Writing module as txt...");
    let mut file = File::create("debug.txt").await.unwrap();
    file.write_all(format!("{:#?}", &module).as_bytes())
        .await
        .unwrap();

    info!("Validating module...");
    let mut validator = Validator::new(ValidationFlags::all(), Default::default());
    let module_info = validator.validate(&module).unwrap();

    info!("Compiling WGSL...");
    let mut wgsl_str = String::new();
    let mut writer = back::wgsl::Writer::new(&mut wgsl_str);
    writer.write(&module, &module_info).unwrap();
    writer.finish();

    info!("Writing WGSL...");
    let mut wgsl_file = File::create("debug.wgsl").await.unwrap();
    wgsl_file.write_all(wgsl_str.as_bytes()).await.unwrap();

    Ok(ShaderSource::Wgsl(Cow::Owned(wgsl_str)))
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
