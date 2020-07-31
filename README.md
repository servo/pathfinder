# Pathfinder 3

![Logo](https://github.com/servo/pathfinder/raw/master/resources/textures/pathfinder-logo.png)

Pathfinder 3 is a fast, practical, GPU-based rasterizer for fonts and vector graphics using OpenGL
3.0+, OpenGL ES 3.0+, WebGL 2, and Metal.

Please note that Pathfinder is under heavy development and is incomplete in various areas.

## Quick start

Pathfinder contains a library that implements a subset of the
[HTML canvas API](https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API). You can quickly add
vector rendering to any Rust or C/C++ app with it. The library is available on `crates.io`. See
`examples/canvas_minimal` for a small example of usage.

### Demos

Demo app sources are available in
[demo/native](https://github.com/servo/pathfinder/tree/master/demo/native). Simply run:

    $ cd demo/native
    $ cargo run --release

A variety of small examples are also available to get you up and running quickly. For example, you
can run the `canvas_nanovg` example like so:

    $ cd examples/canvas_nanovg
    $ cargo run --release

## Features

The project features:

* Rust and C bindings, for easy embedding in your own applications regardless of programming
  language. (Note that the C bindings are currently less complete; pull requests are welcome!)

* GPU compute-based rendering, where available. Pathfinder has two rendering modes: D3D11, which is
  based on compute, and D3D9, which is based on hardware rasterization. (Note that these names are
  purely convenient ways to refer to hardware levels: the project doesn't have a proper Direct3D
  backend yet.) In the D3D11 mode, Pathfinder uses compute shaders to achieve large reductions in
  CPU usage and overall better performance than what the built-in GPU rasterization hardware can
  provide.

* Fast CPU setup if needed, making full use of parallelism. If the D3D9 backend is in use,
  Pathfinder performs the tiling step using SIMD and Rayon in order to get as much parallelism out
  of the CPU as possible. (In the D3D11 backend, these steps are done on GPU instead.) The CPU step
  can be pipelined with the GPU to hide its latency.

* Fast GPU rendering, even at small pixel sizes. Even on lower-end GPUs, Pathfinder often matches
  or exceeds the performance of the best CPU rasterizers. The difference is particularly pronounced
  at large sizes, where Pathfinder regularly achieves multi-factor speedups.

* High quality antialiasing. Pathfinder can compute exact fractional trapezoidal area coverage on a
  per-pixel basis for the highest-quality antialiasing possible (effectively 256xAA).

* Advanced font rendering. Pathfinder can render fonts with slight hinting and can perform subpixel
  antialiasing on LCD screens. It can do stem darkening/font dilation like macOS and FreeType in
  order to make text easier to read at small sizes. The library also has support for gamma
  correction.

* Support for SVG. Pathfinder 3 is designed to efficiently handle workloads that consist of many
  overlapping vector paths, such as those commonly found in complex SVG and PDF files. It performs
  tile-based occlusion culling, which often results in dramatic performance wins over typical
  software renderers that use the painter's algorithm. A simple loader that leverages the `resvg`
  library to render a subset of SVG is included, so it's easy to get started.

* 3D capability. Pathfinder can render fonts and vector paths in 3D environments without any loss
  in quality. This is intended to be useful for vector-graphics-based user interfaces in VR, for
  example.

* Lightweight. Pathfinder is designed first and foremost for simplicity and generality instead of
  a large number of specialized fast paths. It consists of a set of modular crates, so applications can pick and choose only the components that are necessary to minimize dependencies.

* Portability to most GPUs manufactured in the last decade, including integrated and mobile GPUs.
  Any GPU capable of Direct3D 9/OpenGL 3.0/WebGL 2.0 should be able to run Pathfinder. Currently,
  backends are available for OpenGL, OpenGL ES, Metal, and WebGL.

## Building

Pathfinder can be used from either Rust or C/C++. See the appropriate section below.

### Rust

Simply run `cargo build --release` at top level to build all the crates. Pathfinder is a set of
modular crates, allowing you to select only the parts of the library you need and omit the rest.
The libraries are available on `crates.io` with the `pathfinder_` prefix (e.g.
`pathfinder_canvas`), but you may wish to use the `master` branch for the latest features and bug
fixes.

### C

The C bindings use [cargo-c](https://github.com/lu-zero/cargo-c). Install `cargo-c` with
`cargo install cargo-c`, and then use a command like:

    $ cargo cinstall --destdir=/tmp/pathfinder-destdir --manifest-path c/Cargo.toml
    $ sudo cp -a /tmp/pathfinder-destdir/* /

The resulting library is usable via `pkg-config` as `pathfinder`. For examples of use, see the
examples in the `examples/` directory beginning with `c_`.

`cargo-c` has a variety of other options such as `--prefix`, which may be useful for packagers.

## Community

There's a Matrix chat room available at
[`#pathfinder:mozilla.org`](https://matrix.to/#/!XiDASQfNTTMrJbXHTw:mozilla.org?via=mozilla.org).
If you're on the Mozilla Matrix server, you can search for Pathfinder to find it. For more
information on connecting to the Matrix network, see
[this `wiki.mozilla.org` page](https://wiki.mozilla.org/Matrix).

The entire Pathfinder community, including the chat room and GitHub project, is expected to abide
by the same Code of Conduct that the Rust project itself follows. (At the moment, the authors will
handle violations.)

## Build status

[![Build Status](https://travis-ci.org/servo/pathfinder.svg?branch=master)](https://travis-ci.org/servo/pathfinder)

## Authors

The primary author is Patrick Walton (@pcwalton), with contributions from the Servo development
community.

The logo was designed by Jay Vining.

## License

Pathfinder is licensed under the same terms as Rust itself. See `LICENSE-APACHE` and `LICENSE-MIT`.

Material Design icons are copyright Google Inc. and licensed under the Apache 2.0 license.
