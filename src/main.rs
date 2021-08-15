#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

mod generator;
mod logging;

fn main() {
    logging::init();
    info!("Hello from fractal-rs-2");
}
