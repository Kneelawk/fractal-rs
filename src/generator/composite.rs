// TODO: Rewrite this.
// Note: https://doc.rust-lang.org/std/collections/struct.BinaryHeap.html
// Note: https://docs.rs/futures/0.3.12/futures/future/fn.join_all.html

use crate::generator::{view::View, FractalGenerator, FractalGeneratorInstance};
use futures::{
    future::{join_all, BoxFuture},
    FutureExt,
};

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
        views: &[View],
    ) -> BoxFuture<Result<Box<dyn FractalGeneratorInstance>, anyhow::Error>> {
        unimplemented!()
    }
}
