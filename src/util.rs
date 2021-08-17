use crate::config::Config;
use std::path::PathBuf;

pub fn find_filename(config: &Config) -> PathBuf {
    let mut retry = 0;
    let mut path = PathBuf::from(filename(config, retry));

    while path.exists() {
        retry += 1;
        path = PathBuf::from(filename(config, retry));
    }

    return path;
}

fn filename(config: &Config, retry: u32) -> String {
    let coords = if config.mandelbrot {
        format!("mandelbrot-({},{})", config.center_x, config.center_y)
    } else {
        format!("julia-({}+{}i)", config.c.re, config.c.im)
    };

    if retry > 0 {
        format!(
            "{}-{}x{}-{}-{}.png",
            &config.file_prefix, config.view.image_width, config.view.image_height, coords, retry
        )
    } else {
        format!(
            "{}-{}x{}-{}.png",
            &config.file_prefix, config.view.image_width, config.view.image_height, coords
        )
    }
}
