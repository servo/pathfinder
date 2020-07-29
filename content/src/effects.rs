// pathfinder/content/src/effects.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Special effects that can be applied to layers.

use pathfinder_color::{ColorF, matrix::ColorMatrix};
use pathfinder_geometry::line_segment::LineSegment2F;
use pathfinder_geometry::vector::Vector2F;
use pathfinder_simd::default::F32x2;

/// A defringing kernel for LCD screens that approximates the macOS/iOS look.
///
/// This intentionally does not precisely match what Core Graphics does (a Lanczos function),
/// because we don't want any ringing artefacts.
pub static DEFRINGING_KERNEL_CORE_GRAPHICS: DefringingKernel =
    DefringingKernel([0.033165660, 0.102074051, 0.221434336, 0.286651906]);

/// A defringing kernel for LCD screens that approximates the FreeType look.
pub static DEFRINGING_KERNEL_FREETYPE: DefringingKernel =
    DefringingKernel([0.0, 0.031372549, 0.301960784, 0.337254902]);

/// Stem darkening factors that approximate the macOS look.
///
/// Should match macOS 10.13 High Sierra.
pub static STEM_DARKENING_FACTORS: [f32; 2] = [0.0121, 0.0121 * 1.25];

/// The maximum number of pixels we are willing to expand outlines by to match the macOS look.
///
/// Should match macOS 10.13 High Sierra.
pub const MAX_STEM_DARKENING_AMOUNT: [f32; 2] = [0.3, 0.3];

/// A subjective cutoff. Above this ppem value, no stem darkening is performed.
pub const MAX_STEM_DARKENING_PIXELS_PER_EM: f32 = 72.0;

/// The shader that should be used when compositing this layer onto its destination.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Filter {
    /// No special filter.
    None,

    /// Converts a linear gradient to a radial one.
    RadialGradient {
        /// The line that the circles lie along.
        line: LineSegment2F,
        /// The radii of the circles at the two endpoints.
        radii: F32x2,
        /// The origin of the linearized gradient in the texture.
        uv_origin: Vector2F,
    },

    /// One of the `PatternFilter` filters.
    PatternFilter(PatternFilter),
}

/// Shaders applicable to patterns.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PatternFilter {
    /// Performs postprocessing operations useful for monochrome text.
    Text {
        /// The foreground color of the text.
        fg_color: ColorF,
        /// The background color of the text.
        bg_color: ColorF,
        /// The kernel used for defringing, if subpixel AA is enabled.
        defringing_kernel: Option<DefringingKernel>,
        /// Whether gamma correction is used when compositing.
        ///
        /// If this is enabled, stem darkening is advised.
        gamma_correction: bool,
    },

    /// A blur operation in one direction, either horizontal or vertical.
    ///
    /// To produce a full Gaussian blur, perform two successive blur operations, one in each
    /// direction.
    Blur {
        /// The axis of the blur: horizontal or vertical.
        direction: BlurDirection,
        /// Half the blur radius.
        sigma: f32,
    },

    /// A color matrix multiplication.
    /// 
    /// The matrix is stored in 5 columns of `F32x4`. See the `feColorMatrix` element in the SVG
    /// specification.
    ColorMatrix(ColorMatrix),
}

/// Blend modes that can be applied to individual paths.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BlendMode {
    // Porter-Duff, supported by GPU blender
    /// No regions are enabled.
    Clear,
    /// Only the source will be present.
    Copy,
    /// The source that overlaps the destination, replaces the destination.
    SrcIn,
    /// Source is placed, where it falls outside of the destination.
    SrcOut,
    /// Source is placed over the destination.
    SrcOver,
    /// Source which overlaps the destination, replaces the destination. Destination is placed
    /// elsewhere.
    SrcAtop,
    /// Destination which overlaps the source, replaces the source.
    DestIn,
    /// Destination is placed, where it falls outside of the source.
    DestOut,
    /// Destination is placed over the source.
    DestOver,
    /// Destination which overlaps the source replaces the source. Source is placed elsewhere.
    DestAtop,
    /// The non-overlapping regions of source and destination are combined.
    Xor,
    /// Display the sum of the source image and destination image. It is defined in the Porter-Duff
    /// paper as the plus operator.
    Lighter,

    // Others, unsupported by GPU blender
    /// Selects the darker of the backdrop and source colors.
    Darken,
    /// Selects the lighter of the backdrop and source colors.
    Lighten,
    /// The source color is multiplied by the destination color and replaces the destination.
    Multiply,
    /// Multiplies the complements of the backdrop and source color values, then complements the
    /// result.
    Screen,
    /// Multiplies or screens the colors, depending on the source color value. The effect is
    /// similar to shining a harsh spotlight on the backdrop.
    HardLight,
    /// Multiplies or screens the colors, depending on the backdrop color value.
    Overlay,
    /// Brightens the backdrop color to reflect the source color.
    ColorDodge,
    /// Darkens the backdrop color to reflect the source color.
    ColorBurn,
    /// Darkens or lightens the colors, depending on the source color value. The effect is similar
    /// to shining a diffused spotlight on the backdrop.
    SoftLight,
    /// Subtracts the darker of the two constituent colors from the lighter color.
    Difference,
    /// Produces an effect similar to that of the Difference mode but lower in contrast.
    Exclusion,
    /// Creates a color with the hue of the source color and the saturation and luminosity of the
    /// backdrop color.
    Hue,
    /// Creates a color with the saturation of the source color and the hue and luminosity of the
    /// backdrop color.
    Saturation,
    /// Creates a color with the hue and saturation of the source color and the luminosity of the
    /// backdrop color.
    Color,
    /// Creates a color with the luminosity of the source color and the hue and saturation of the
    /// backdrop color. This produces an inverse effect to that of the Color mode.
    Luminosity,
}

/// The convolution kernel that will be applied horizontally to reduce color fringes when
/// performing subpixel antialiasing. This kernel is automatically mirrored horizontally. The
/// fourth element of this kernel is applied to the center of the pixel, the third element is
/// applied one pixel to the left, and so on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DefringingKernel(pub [f32; 4]);

/// The axis a Gaussian blur is applied to.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BlurDirection {
    /// The horizontal axis.
    X,
    /// The vertical axis.
    Y,
}

impl Default for BlendMode {
    #[inline]
    fn default() -> BlendMode {
        BlendMode::SrcOver
    }
}

impl Default for Filter {
    #[inline]
    fn default() -> Filter {
        Filter::None
    }
}

impl BlendMode {
    /// Whether the backdrop is irrelevant when applying this blend mode (i.e. destination blend
    /// factor is zero when source alpha is one).
    #[inline]
    pub fn occludes_backdrop(self) -> bool {
        match self {
            BlendMode::SrcOver | BlendMode::Clear => true,
            BlendMode::DestOver |
            BlendMode::DestOut |
            BlendMode::SrcAtop |
            BlendMode::Xor |
            BlendMode::Lighter |
            BlendMode::Lighten |
            BlendMode::Darken |
            BlendMode::Copy |
            BlendMode::SrcIn |
            BlendMode::DestIn |
            BlendMode::SrcOut |
            BlendMode::DestAtop |
            BlendMode::Multiply |
            BlendMode::Screen |
            BlendMode::HardLight |
            BlendMode::Overlay |
            BlendMode::ColorDodge |
            BlendMode::ColorBurn |
            BlendMode::SoftLight |
            BlendMode::Difference |
            BlendMode::Exclusion |
            BlendMode::Hue |
            BlendMode::Saturation |
            BlendMode::Color |
            BlendMode::Luminosity => false,
        }
    }

    /// True if this blend mode does not preserve destination areas outside the source.
    pub fn is_destructive(self) -> bool {
        match self {
            BlendMode::Clear |
            BlendMode::Copy |
            BlendMode::SrcIn |
            BlendMode::DestIn |
            BlendMode::SrcOut |
            BlendMode::DestAtop => true,
            BlendMode::SrcOver |
            BlendMode::DestOver |
            BlendMode::DestOut |
            BlendMode::SrcAtop |
            BlendMode::Xor |
            BlendMode::Lighter |
            BlendMode::Lighten |
            BlendMode::Darken |
            BlendMode::Multiply |
            BlendMode::Screen |
            BlendMode::HardLight |
            BlendMode::Overlay |
            BlendMode::ColorDodge |
            BlendMode::ColorBurn |
            BlendMode::SoftLight |
            BlendMode::Difference |
            BlendMode::Exclusion |
            BlendMode::Hue |
            BlendMode::Saturation |
            BlendMode::Color |
            BlendMode::Luminosity => false,
        }
    }
}
