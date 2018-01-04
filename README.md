# Pathfinder 2

Pathfinder 2 is a fast, practical, work in progress GPU-based rasterizer for fonts and vector
graphics using OpenGL and OpenGL ES 2.0+.

Please note that Pathfinder is under heavy development and is incomplete in various areas.

The project features:

* Low setup time. Typical glyph outlines can be prepared for GPU rendering in about 5 microseconds
  each (typically O(n log n) in the number of vertices), making Pathfinder suitable for dynamic
  environments. The setup process is lossless and fully resolution independent; paths need only be
  prepared once and can thereafter be rendered at any zoom level without any loss in quality.
  Pathfinder can also render outlines without any mesh at all, reducing the setup time to nearly
  zero, at the cost of some runtime performance. For static paths such as game assets, the
  resulting meshes can be saved to disk to avoid having to generate them at runtime.

* High quality antialiasing. Pathfinder can compute exact fractional trapezoidal area coverage on a
  per-pixel basis for the highest-quality antialiasing, provided that either OpenGL 3.0+ or a few
  common extensions are available. Supersampling is available as an alternative for 3D scenes
  and/or lower-end hardware.

* Fast rendering, even at small pixel sizes. Even on lower-end GPUs, Pathfinder typically matches
  or exceeds the performance of the best CPU rasterizers. The difference is particularly pronouced
  at large sizes, where Pathfinder regularly achieves multi-factor speedups. All shaders have no
  loops and minimal branching.

* Advanced font rendering. Pathfinder can render fonts with slight hinting and can perform subpixel
  antialiasing on LCD screens. It can do stem darkening/font dilation like macOS and FreeType in
  order to make text easier to read at small sizes. The library also has support for gamma
  correction.

* Support for full vector scenes. Pathfinder 2 is designed to efficiently handle workloads that
  consist of many overlapping vector paths, such as those commonly found in SVG and PDF files. It
  makes heavy use of the hardware Z buffer to perform occlusion culling, which often results in
  dramatic performance wins over typical software renderers that use the painter's algorithm.

* 3D capability. Pathfinder 2 can render fonts and vector paths in 3D environments. Vector meshes
  are rendered just like any other mesh, with a simple shader applied.

* Portability to most GPUs manufactured in the last decade, including integrated and mobile GPUs.
  Geometry, tessellation, and compute shader functionality is not required.

## Building

Pathfinder 2 is a set of modular packages, allowing you to choose which parts of the library you
need. A WebGL demo is included, so you can try Pathfinder right in your browser. (Please note that,
like the rest of Pathfinder, it's under heavy development and has known bugs.)

To run the demo, make sure NPM and nightly Rust are installed, and run the following commands:

    $ cd demo/client
    $ npm install
    $ npm run build
    $ cd ../server
    $ cargo run --release

Then navigate to http://localhost:8000/.

## Testing

Pathfinder contains reference tests that compare its output to that of other rendering libraries,
such as Cairo. To run them, install Cairo and FreeType if necessary, then perform the above steps,
substituting the last line with:

    $ cargo run --release --features reftests

## Authors

The primary author is Patrick Walton (@pcwalton), with contributions from the Servo development
community.

The logo was designed by Jay Vining.

Pathfinder abides by the same Code of Conduct as Rust itself.

## License

Pathfinder is licensed under the same terms as Rust itself. See `LICENSE-APACHE` and `LICENSE-MIT`.

Material Design icons are copyright Google Inc. and licensed under the Apache 2.0 license.
