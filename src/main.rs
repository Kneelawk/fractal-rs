//! main.rs - This file contains the `main()` function. This method delegates to
//! `gui` module for gui-based core application logic.
//!
//! This system will hopefully eventually determine based on application
//! arguments whether to start a gui or to perform some other function.

#![feature(never_type)]

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;
#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate thiserror;

use crate::storage::{CfgGeneral, CfgSingleton};

mod generator;
mod gpu;
mod gui;
mod logging;
mod storage;
mod util;

fn main() {
    // initialize the start date variable
    util::get_start_date();

    // setup the logger
    logging::init();
    info!("Hello from fractal-rs-2");

    info!("Loading settings and configs...");
    CfgGeneral::load().expect("Error loading general config");

    gui::start_gui_application();
}
