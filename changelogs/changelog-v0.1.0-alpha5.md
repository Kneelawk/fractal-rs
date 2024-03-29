# Fractal-RS-2 version 0.1.0-alpha5

## Changelog:

* Moved config, logging, and debug files to a `.fractal-rs-2` directory in the
  user's home directory.
* Added ability to select locations based on complex number instead of screen
  location.
* Added keyboard shortcuts for closing and creating new tabs.
* Added ability to generate a Julia/Fatou set from a point on a Mandelbrot set.
* Added keyboard shortcuts for generating a Julia/Fatou set from a point on a
  Mandelbrot set, switching to a source Mandelbrot set, and switching to an
  already generated Julia/Fatou set.
* Added keyboard shortcut for generating a fractal.
* Made the application save settings between runs.
* Added the ability for the user to configure keyboard shortcuts.
* Fixed bugs with generator backends not being selected. (These bugs made it so
  that dedicated GPUs were never actually being used.)
* Increased viewer image max size to be the same as the GPU's max texture size.
