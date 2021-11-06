# fractal-rs
Simple fractal viewer written in Rust

## Linux-Mesa-Tests
This branch of `fractal-rs` is devoted to trying to get the fractal generator
running on Linux using Mesa graphics drivers. There is currently a bug in Mesa
that causes WGPU to hang if command buffers are submitted to the queue more than
once per frame, or more than once at all if running headless. This makes it
quite difficult to run the fractal generator as it is often either headless or
submitting draw commands unrelated to any swapchain. However, it is possible to
compile a fixed version of the Mesa drivers from source and use them instead.

```bash
# First, you want to obtain the fixed version of the Mesa drivers. In a
# directory you have chosen for the Mesa source code, clone the fixed branch:
git clone https://gitlab.freedesktop.org/llandwerlin/mesa.git -b 'review/anv-fix-multiple-wait-signal-semaphore' --single-branch

# Next, you want to install all of Mesa's build dependencies:
# for debian based systems
sudo apt install meson
sudo apt build-dep mesa

# Next, you want to decide on a local Mesa installation directory and store its
# path in `MESA_INSTALLDIR`.
export MESA_INSTALLDIR="<mesa-install-dir-path>"

# Next, you'll want to build and install Mesa to your local installation
# location:
cd mesa
meson builddir/ -Dprefix="$MESA_INSTALLDIR"
ninja -C builddir/ install

# Now you can run the fractal generator with environment variables pointing to
# the local Mesa installation. In the `fractal-rs` project directory, run:
export LD_LIBRARY_PATH="$MESA_INSTALLDIR/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH"
export VK_ICD_FILENAMES="$MESA_INSTALLDIR/share/vulkan/icd.d/intel_icd.x86_64.json"
cargo run
```
