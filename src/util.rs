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
    if retry > 0 {
        format!(
            "{}-{}x{}-({}+{}i)-{}-{}.png",
            &config.file_prefix,
            config.view.image_width,
            config.view.image_height,
            config.c.re,
            config.c.im,
            if config.mandelbrot {
                "mandelbrot"
            } else {
                "julia"
            },
            retry
        )
    } else {
        format!(
            "{}-{}x{}-({}+{}i)-{}.png",
            &config.file_prefix,
            config.view.image_width,
            config.view.image_height,
            config.c.re,
            config.c.im,
            if config.mandelbrot {
                "mandelbrot"
            } else {
                "julia"
            }
        )
    }
}
