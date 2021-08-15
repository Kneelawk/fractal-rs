use crate::generator::{
    error::GenError, view::View, FractalGenerator,
    FractalGeneratorInstance,
};
use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};

pub mod opts;

// TODO: Rewrite this.

pub struct CpuFractalGenerator {
    thread_count: usize,
}

impl FractalGenerator for CpuFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'_, usize> {
        ready(self.thread_count).boxed()
    }

    fn start_generation(
        &self,
        views: &[View],
    ) -> BoxFuture<Result<Box<dyn FractalGeneratorInstance>, GenError>> {
        unimplemented!()
    }
}
