// TODO: Rewrite this.
// Note: https://doc.rust-lang.org/std/collections/struct.BinaryHeap.html
// Note: https://docs.rs/futures/0.3.12/futures/future/fn.join_all.html

use crate::generator::{view::View, FractalGenerator, FractalGeneratorInstance, PixelBlock};
use futures::{
    future::{join_all, BoxFuture},
    FutureExt,
};
use tokio::sync::mpsc::Sender;

pub struct CompositeFractalGenerator {
    generators: Vec<Box<dyn FractalGenerator>>,
}

impl FractalGenerator for CompositeFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'_, anyhow::Result<usize>> {
        join_all(self.generators.iter().map(|g| g.min_views_hint()))
            .map(|s| {
                let res: Result<Vec<_>, _> = s.into_iter().collect();
                res.map(|v| v.into_iter().sum::<usize>())
            })
            .boxed()
    }

    fn start_generation(
        &self,
        views: &[View],
        sender: Sender<anyhow::Result<PixelBlock>>,
    ) -> BoxFuture<anyhow::Result<Box<dyn FractalGeneratorInstance>>> {
        unimplemented!()
    }
}
