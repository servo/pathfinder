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

/// Errors that can occur when parsing OpenType fonts.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FontError {
    /// A miscellaneous error occurred.
    Failed,
    /// The file ended unexpectedly.
    UnexpectedEof,
    /// There is no font with this index in this font collection.
    FontIndexOutOfBounds,
    /// The file declared that it was in a version of the format we don't support.
    UnsupportedVersion,
    /// The file was of a format we don't support.
    UnknownFormat,
    /// The font had a glyph format we don't support.
    UnsupportedGlyphFormat,
    /// We don't support the declared version of the font's CFF outlines.
    UnsupportedCffVersion,
    /// We don't support the declared version of the font's character map.
    UnsupportedCmapVersion,
    /// The font character map has an unsupported platform/encoding ID.
    UnsupportedCmapEncoding,
    /// The font character map has an unsupported format.
    UnsupportedCmapFormat,
    /// We don't support the declared version of the font header.
    UnsupportedHeadVersion,
    /// We don't support the declared version of the font's horizontal metrics.
    UnsupportedHheaVersion,
    /// We don't support the declared version of the font's OS/2 and Windows table.
    UnsupportedOs2Version,
    /// A required table is missing.
    RequiredTableMissing,
    /// An integer in a CFF DICT was not found.
    CffIntegerNotFound,
    /// The CFF Top DICT was not found.
    CffTopDictNotFound,
    /// A CFF `Offset` value was formatted incorrectly.
    CffBadOffset,
    /// The CFF evaluation stack overflowed.
    CffStackOverflow,
    /// An unimplemented CFF CharString operator was encountered.
    CffUnimplementedOperator,
}

impl FontError {
    #[doc(hidden)]
    #[inline]
    pub fn eof<T>(_: T) -> FontError {
        FontError::UnexpectedEof
    }
}


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

/// An error in glyph store creation. See `Typesetter::create_glyph_store()`.
#[derive(Debug)]
pub enum GlyphStoreCreationError {
    /// An error occurred when looking up a glyph ID for a character in the font.
    FontError(FontError),
    /// An error occurred when uploading the outlines to the GPU.
    GlError(GlError),
}

/// An error in construction of a hinter.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HinterCreationError {
    /// A miscellaneous error occurred.
    Failed,
    /// An error was encountered while analyzing the font program.
    FontProgramAnalysisError(HintingAnalysisError),
    /// An error was encountered while analyzing the control value program.
    ControlValueProgramAnalysisError(HintingAnalysisError),
    /// An error was encountered during execution of the font program.
    FontProgramExecutionError(HintingExecutionError),
}

/// An error encountered during parsing of the TrueType hinting bytecode.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum HintingParseError {
    /// The instruction stream terminated normally.
    Eof,
    /// The instruction stream terminated abnormally.
    UnexpectedEof,
    /// An unexpected opcode was encountered.
    UnknownOpcode(u8),
    /// An unexpected value was encountered for `DistanceType`.
    InvalidDistanceType,
}

/// An error encountered during semantic analysis of the TrueType hinting bytecode.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HintingAnalysisError {
    /// An error occurred while parsing the instruction stream.
    ParseError(HintingParseError),
    /// A branch target (e.g. `Eif`) was found without a corresponding branch instruction.
    BranchTargetMissingBranch,
    /// A branch target was (e.g. `If`) was found without a corresponding branch target (e.g.
    /// `Eif`).
    BranchMissingBranchTarget,
    /// A branch target was mismatched with its branch instruction (`Eif` vs. `If`, etc.)
    MismatchedBranchInstruction,
}

/// An error encountered during execution of the TrueType hinting bytecode.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HintingExecutionError {
    /// An error occurred while parsing the instruction stream.
    ParseError(HintingParseError),
    /// An instruction expected more values than were on the stack.
    StackUnderflow,
    /// An operation tried to read out of bounds of the control value table.
    CvtReadOutOfBounds,
    /// An undefined function ID was called.
    CallToUndefinedFunction,
}

