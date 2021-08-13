// TODO: Rewrite this.
// Note: https://doc.rust-lang.org/std/collections/struct.BinaryHeap.html
// Note: https://docs.rs/futures/0.3.12/futures/future/fn.join_all.html

use crate::generator::{
    view::View, FractalGenerationMessage, FractalGenerationStartError, FractalGenerator,
};
use futures::{
    future::{join_all, BoxFuture},
    FutureExt,
};
use tokio::sync::mpsc::Sender;

pub struct CompositeFractalGenerator {
    generators: Vec<Box<dyn FractalGenerator>>,
}

impl FractalGenerator for CompositeFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'_, usize> {
        join_all(self.generators.iter().map(|g| g.min_views_hint()))
            .map(|s| s.into_iter().sum::<usize>())
            .boxed()
    }

    fn start_generation(
        &self,
        views: &dyn Iterator<Item = View>,
        result: Sender<FractalGenerationMessage>,
    ) -> BoxFuture<'_, Result<(), FractalGenerationStartError>> {
        unimplemented!()
    }

    fn get_progress(&self) -> BoxFuture<'_, f32> {
        // TODO: Fix naive implementation
        let generator_count = self.generators.len();
        join_all(self.generators.iter().map(|g| g.get_progress()))
            .map(move |s| s.into_iter().sum::<f32>() / generator_count as f32)
            .boxed()
    }

    fn running(&self) -> BoxFuture<'_, bool> {
        unimplemented!()
    }
}
