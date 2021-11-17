// TODO: Rewrite this.
// Note: https://doc.rust-lang.org/std/collections/struct.BinaryHeap.html
// Note: https://docs.rs/futures/0.3.12/futures/future/fn.join_all.html

use crate::generator::{view::View, FractalGenerator, FractalGeneratorInstance, PixelBlock};
use futures::{future::BoxFuture, FutureExt};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use wgpu::{Device, Queue, Texture, TextureView};

pub struct CompositeFractalGenerator {
    generators: Vec<Box<dyn FractalGenerator + Send + Sync>>,
}

impl FractalGenerator for CompositeFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'static, anyhow::Result<usize>> {
        let futs: Vec<_> = self
            .generators
            .iter()
            .map(|gen| gen.min_views_hint())
            .collect();

        async move {
            let mut sum = 0;
            for g in futs {
                sum += g.await?;
            }
            Ok(sum)
        }
        .boxed()
    }

    fn start_generation_to_cpu(
        &self,
        _views: &[View],
        _sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>
    {
        unimplemented!()
    }

    fn start_generation_to_gpu(
        &self,
        _views: &[View],
        _device: Arc<Device>,
        _queue: Arc<Queue>,
        _texture: Arc<Texture>,
        _texture_view: Arc<TextureView>,
    ) -> BoxFuture<'static, anyhow::Result<Box<dyn FractalGeneratorInstance + Send + 'static>>>
    {
        unimplemented!()
    }
}
