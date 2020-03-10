use crate::generator::{
    view::View,
    FractalGenerationMessage,
    FractalGenerationStartError,
    FractalGenerator,
};
use std::sync::{mpsc::SyncSender, Arc, Mutex};

/// Fractal generator implementation that simply delegates generation from views
/// to multiple sub fractal generators.
pub struct CompositeFractalGenerator {
    generators: Vec<Box<dyn FractalGenerator>>,
}

impl FractalGenerator for CompositeFractalGenerator {
    fn min_views_hint(&self) -> usize {
        self.generators.iter().map(|g| g.min_views_hint()).sum()
    }

    fn start_generation<Views>(
        self: &Arc<Self>,
        views: Arc<Mutex<Views>>,
        result: SyncSender<FractalGenerationMessage>,
    ) -> Result<(), FractalGenerationStartError>
    where
        Views: Iterator<Item = View> + Send + 'static,
    {
        unimplemented!()
    }

    fn get_progress(&self) -> f32 {
        unimplemented!()
    }

    fn running(&self) -> bool {
        unimplemented!()
    }
}
