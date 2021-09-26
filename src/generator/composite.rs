// TODO: Rewrite this.
// Note: https://doc.rust-lang.org/std/collections/struct.BinaryHeap.html
// Note: https://docs.rs/futures/0.3.12/futures/future/fn.join_all.html

use crate::generator::{view::View, FractalGenerator, FractalGeneratorInstance, PixelBlock};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use wgpu::{Texture, TextureView};

pub struct CompositeFractalGenerator {
    generators: Vec<Box<dyn FractalGenerator + Send + Sync>>,
}

#[async_trait]
impl FractalGenerator for CompositeFractalGenerator {
    async fn min_views_hint(&self) -> anyhow::Result<usize> {
        let mut sum = 0;
        for g in self.generators.iter() {
            sum += g.min_views_hint().await?;
        }
        Ok(sum)
    }

    async fn start_generation_to_cpu(
        &self,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> anyhow::Result<Box<dyn FractalGeneratorInstance>> {
        unimplemented!()
    }

    async fn start_generation_to_gpu(
        &self,
        views: &[View],
        texture: Arc<Texture>,
        texture_view: Arc<TextureView>,
    ) -> anyhow::Result<Box<dyn FractalGeneratorInstance>> {
        unimplemented!()
    }
}
