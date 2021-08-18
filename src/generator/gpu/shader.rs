use crate::generator::FractalOpts;
use naga::{
    back, front,
    valid::{ValidationFlags, Validator},
};
use std::borrow::Cow;
use tokio::{fs::File, io::AsyncWriteExt};
use wgpu::ShaderSource;

const TEMPLATE_SOURCE: &str = include_str!("template.wgsl");

pub async fn load_shaders(_opts: FractalOpts) -> ShaderSource<'static> {
    info!("Loading utility functions...");
    let module = front::wgsl::parse_str(TEMPLATE_SOURCE).unwrap();

    info!("Validating module...");
    let mut validator = Validator::new(ValidationFlags::all(), Default::default());
    let module_info = validator.validate(&module).unwrap();

    info!("Writing module as txt...");
    let mut file = File::create("debug.txt").await.unwrap();
    file.write_all(format!("{:#?}", &module).as_bytes())
        .await
        .unwrap();

    info!("Compiling WGSL...");
    let mut wgsl_str = String::new();
    let mut writer = back::wgsl::Writer::new(&mut wgsl_str);
    writer.write(&module, &module_info).unwrap();
    writer.finish();

    info!("Writing WGSL...");
    let mut wgsl_file = File::create("debug.wgsl").await.unwrap();
    wgsl_file.write_all(wgsl_str.as_bytes()).await.unwrap();

    ShaderSource::Wgsl(Cow::Owned(wgsl_str))
}
