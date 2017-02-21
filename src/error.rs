// Copyright 2017 The Servo Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Errors.

use compute_shader;
use gl::types::GLenum;
use std::io;

/// An OpenGL error with the given code.
///
/// You cannot depend on these being reliably returned. Pathfinder does not call `glGetError()`
/// unless necessary, to avoid driver stalls.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GlError(pub GLenum);

/// An initialization error. This could be an OpenGL error or a shader compilation/link error.
#[derive(Debug)]
pub enum InitError {
    /// An OpenGL error occurred.
    GlError(GlError),

    /// A shader could not be loaded.
    ShaderUnreadable(io::Error),

    /// Shader compilation failed.
    ///
    /// The first string specifies the type of shader (vertex, fragment, etc.); the second holds
    /// the error message that the driver returned.
    CompileFailed(&'static str, String),

    /// Shader linking failed.
    ///
    /// The string holds the error message that the driver returned.
    LinkFailed(String),

    /// An error occurred setting up GPU compute.
    ComputeError(compute_shader::error::Error),

    /// One of the rasterization options had an invalid syntax.
    InvalidSetting,
}

/// A rasterization error. This could be an OpenGL error or a compute error.
#[derive(Debug)]
pub enum RasterError {
    /// No glyphs were supplied.
    NoGlyphsToDraw,
    /// An OpenGL error occurred.
    GlError(GlError),
    /// An error occurred during GPU compute.
    ComputeError(compute_shader::error::Error),
    /// An destination image with an unsupported format was supplied.
    ///
    /// Currently supported formats are R8 and RGBA8.
    UnsupportedImageFormat,
}

