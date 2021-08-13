use crate::generator::{
    view::View, FractalGenerationMessage, FractalGenerationStartError, FractalGenerator,
};
use futures::{
    future::{ready, BoxFuture},
    FutureExt,
};
use tokio::sync::mpsc::Sender;

pub mod opts;

// TODO: Rewrite this.

pub struct CpuFractalGenerator {
    thread_count: usize
}

impl FractalGenerator for CpuFractalGenerator {
    fn min_views_hint(&self) -> BoxFuture<'_, usize> {
        ready(self.thread_count).boxed()
    }

    fn start_generation(
        &self,
        views: &dyn Iterator<Item = View>,
        result: Sender<FractalGenerationMessage>,
    ) -> BoxFuture<'_, Result<(), FractalGenerationStartError>> {
        unimplemented!()
    }

    fn get_progress(&self) -> BoxFuture<'_, f32> {
        unimplemented!()
    }

    fn running(&self) -> BoxFuture<'_, bool> {
        unimplemented!()
    }
}
