use crate::generator::{
    color::RGBAColor,
    cpu::opts::CpuFractalOpts,
    view::View,
    FractalGenerationMessage,
    FractalGenerationStartError,
    FractalGenerator,
};
use std::{
    sync::{
        mpsc::{channel, Sender, SyncSender},
        Arc,
        Mutex,
        RwLock,
    },
    thread,
};

pub mod opts;

/// Fractal Generator implementation that uses multiple threads running on the
/// CPU to generate fractals.
pub struct CpuFractalGenerator<Opts>
where
    Opts: CpuFractalOpts + Send + Sync + Clone + 'static,
{
    threads: Vec<Arc<FractalThread<Opts>>>,
    current_chunk: Arc<RwLock<usize>>,
    state: Arc<RwLock<CpuFractalGeneratorState>>,
}

#[derive(Debug, Copy, Clone)]
enum CpuFractalGeneratorState {
    Running { chunk_count: usize },
    NotRunning,
}

struct FractalThreadMessage {
    index: usize,
    color: RGBAColor,
}

struct FractalThread<Opts>
where
    Opts: CpuFractalOpts + Send + Sync + 'static,
{
    opts: Opts,
    progress: RwLock<f32>,
    running: Mutex<bool>,
}

impl<Opts> CpuFractalGenerator<Opts>
where
    Opts: CpuFractalOpts + Send + Sync + Clone + 'static,
{
    /// Constructs a new fractal generator running on the CPU, utilising
    /// `num_threads` threads.
    pub fn new(opts: Opts, num_threads: usize) -> CpuFractalGenerator<Opts> {
        let mut threads = vec![];
        for _i in 0..num_threads {
            threads.push(FractalThread::new(opts.clone()));
        }

        CpuFractalGenerator {
            threads,
            current_chunk: Arc::new(RwLock::new(0)),
            state: Arc::new(RwLock::new(CpuFractalGeneratorState::NotRunning)),
        }
    }

    fn generate(
        threads: Vec<Arc<FractalThread<Opts>>>,
        current_chunk: Arc<RwLock<usize>>,
        state: Arc<RwLock<CpuFractalGeneratorState>>,
        views: Vec<View>,
        result: SyncSender<FractalGenerationMessage>,
    ) {
        for (index, view) in views.into_iter().enumerate() {
            *current_chunk.write().unwrap() = index;

            // start all the fractal threads
            let rx = {
                let num_threads = threads.len();
                let (tx, rx) = channel();

                // how many of the threads should have an extra pixel
                let left_over =
                    view.image_width as usize * view.image_height as usize % num_threads;

                for (index, thread) in threads.iter().enumerate() {
                    // the number of pixels to generate
                    let count = view.image_width as usize * view.image_height as usize
                        / num_threads
                        + if index < left_over { 1 } else { 0 };
                    thread
                        .start_generation(view, count, index, num_threads, tx.clone())
                        .unwrap();
                }

                rx
            };

            let mut image = vec![0u8; view.image_width as usize * view.image_height as usize * 4]
                .into_boxed_slice();

            // receive all the pixels from each of the threads
            for message in rx {
                image[(message.index * 4)..(message.index * 4 + 4)]
                    .copy_from_slice(&Into::<[u8; 4]>::into(message.color));
            }

            result
                .send(FractalGenerationMessage { view, image })
                .unwrap()
        }

        *state.write().unwrap() = CpuFractalGeneratorState::NotRunning;
    }
}

impl<Opts> FractalGenerator for CpuFractalGenerator<Opts>
where
    Opts: CpuFractalOpts + Send + Sync + Clone + 'static,
{
    fn min_views_hint(&self) -> usize {
        self.threads.len()
    }

    fn start_generation(
        &self,
        views: Vec<View>,
        result: SyncSender<FractalGenerationMessage>,
    ) -> Result<(), FractalGenerationStartError> {
        let mut state = self.state.write().unwrap();
        if !state.running() {
            *state = CpuFractalGeneratorState::Running {
                chunk_count: views.len(),
            };

            let threads = self.threads.clone();
            let current_chunk = self.current_chunk.clone();
            let state_lock = self.state.clone();

            thread::spawn(move || {
                CpuFractalGenerator::generate(threads, current_chunk, state_lock, views, result);
            });

            Ok(())
        } else {
            Err(FractalGenerationStartError::AlreadyRunning)
        }
    }

    fn get_progress(&self) -> f32 {
        let state = self.state.read().unwrap();
        if let CpuFractalGeneratorState::Running { chunk_count } = *state {
            let mut progress = 0f32;
            for thread in self.threads.iter() {
                progress += thread.get_progress();
            }

            (progress / self.threads.len() as f32 + *self.current_chunk.read().unwrap() as f32)
                / chunk_count as f32
        } else {
            0f32
        }
    }

    fn running(&self) -> bool {
        self.state.read().unwrap().running()
    }
}

impl CpuFractalGeneratorState {
    fn running(&self) -> bool {
        match self {
            CpuFractalGeneratorState::Running { .. } => true,
            CpuFractalGeneratorState::NotRunning => false,
        }
    }
}

impl<Opts> FractalThread<Opts>
where
    Opts: CpuFractalOpts + Send + Sync + 'static,
{
    fn new(opts: Opts) -> Arc<FractalThread<Opts>> {
        Arc::new(FractalThread {
            opts,
            progress: RwLock::new(0f32),
            running: Mutex::new(false),
        })
    }

    fn start_generation(
        self: &Arc<Self>,
        view: View,
        count: usize,
        offset: usize,
        skip: usize,
        img_data: Sender<FractalThreadMessage>,
    ) -> Result<(), FractalThreadStartError> {
        let mut running = self.running.lock().unwrap();
        if !*running {
            *running = true;

            *self.progress.write().unwrap() = 0f32;

            let clone = self.clone();

            thread::spawn(move || {
                clone.generate(view, count, offset, skip, img_data);
            });

            Ok(())
        } else {
            Err(FractalThreadStartError::AlreadyRunning)
        }
    }

    fn generate(
        &self,
        view: View,
        count: usize,
        offset: usize,
        skip: usize,
        img_data: Sender<FractalThreadMessage>,
    ) {
        for i in 0usize..count {
            let index = i * skip + offset;

            let x = index % view.image_width;
            let y = index / view.image_width;

            let color = self.opts.gen_pixel(view, x, y);

            img_data
                .send(FractalThreadMessage { index, color })
                .unwrap();

            *self.progress.write().unwrap() = (i + 1) as f32 / count as f32;
        }

        *self.running.lock().unwrap() = false;
    }

    fn get_progress(&self) -> f32 {
        *self.progress.read().unwrap()
    }
}

#[derive(Debug, Copy, Clone)]
enum FractalThreadStartError {
    AlreadyRunning,
}
