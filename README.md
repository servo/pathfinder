# Pathfinder

Pathfinder is a fast, practical GPU-based rasterizer for OpenType fonts using OpenGL 4.3. It
features:

* Very low setup time. Glyph outlines can go from the `.otf` file to the GPU in a form ready for
  rasterization in less than a microsecond. There is no expensive tessellation or preprocessing
  step.

* High quality antialiasing. Unlike techniques that rely on multisample antialiasing, Pathfinder
  computes exact fractional trapezoidal area coverage on a per-pixel basis.

* Fast rendering, even at small pixel sizes. On typical systems, Pathfinder should easily exceed
  the performance of the best CPU rasterizers.

* Low memory consumption. The only memory overhead over the glyph and outline storage itself is
  that of a coverage buffer which typically consumes somewhere between 4MB-16MB and can be
  discarded under memory pressure. Outlines are stored on-GPU in a compressed format and usually
  take up only a few dozen kilobytes.

* Portability to most GPUs manufactured in the last few years, including integrated GPUs.

## Authors

The primary author is Patrick Walton (@pcwalton), with contributions from the Servo development
community.

The code is owned by the Mozilla Foundation.

## License

Licensed under the same terms as Rust itself. See `LICENSE-APACHE` and `LICENSE-MIT`.

