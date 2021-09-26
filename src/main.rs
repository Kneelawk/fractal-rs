//! main.rs - This file contains the `main()` function. This method delegates to
//! `gui` module for gui-based core application logic.
//!
//! This system will hopefully eventually determine based on application
//! arguments whether to start a gui or to perform some other function.

#![feature(never_type)]

#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;

mod generator;
mod gui;
mod logging;

fn main() {
    logging::init();
    info!("Hello from fractal-rs-2");

    gui::start_gui_application();
}
