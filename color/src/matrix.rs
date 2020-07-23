// pathfinder/color/src/lib.rs
//
// Copyright © 2020 The Pathfinder Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use pathfinder_simd::default::F32x4;
use std::ops::{Add, Mul, Deref};

/// ColorMatrix filter/transformation
///
/// The entries are stored in 5 columns of F32x4, each containing a row.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ColorMatrix(pub [F32x4; 5]);

impl ColorMatrix {
    #[inline]
    pub fn from_rows(rows: [[f32; 5]; 4]) -> ColorMatrix {
        ColorMatrix([
            F32x4::new(rows[0][0], rows[1][0], rows[2][0], rows[3][0]),
            F32x4::new(rows[0][1], rows[1][1], rows[2][1], rows[3][1]),
            F32x4::new(rows[0][2], rows[1][2], rows[2][2], rows[3][2]),
            F32x4::new(rows[0][3], rows[1][3], rows[2][3], rows[3][3]),
            F32x4::new(rows[0][4], rows[1][4], rows[2][4], rows[3][4]),
        ])
    }

    /// Creates a hue-rotate color matrix filter from the given angle in radians.
    ///
    /// See the `hueRotate` attribute of the `feColorMatrix` element in the SVG specification.
    pub fn hue_rotate(angle: f32) -> ColorMatrix {
        let a = ColorMatrix::from_rows([
            [ 0.213,  0.715,  0.072, 0.0, 0.0],
            [ 0.213,  0.715,  0.072, 0.0, 0.0],
            [ 0.213,  0.715,  0.072, 0.0, 0.0],
            [ 0.0,    0.0,    0.0,   1.0, 0.0],
        ]);
        let b = ColorMatrix::from_rows([
            [ 0.787, -0.715, -0.072, 0.0, 0.0],
            [-0.213,  0.285, -0.072, 0.0, 0.0],
            [-0.213, -0.715,  0.928, 0.0, 0.0],
            [ 0.0,    0.0,    0.0,   0.0, 0.0],
        ]);
        let c = ColorMatrix::from_rows([
            [-0.213, -0.715,  0.928, 0.0, 0.0],
            [ 0.143,  0.140, -0.283, 0.0, 0.0],
            [-0.787,  0.715,  0.072, 0.0, 0.0],
            [ 0.0,    0.0,    0.0,   0.0, 0.0],
        ]);
        a + b * angle.cos() + c * angle.sin()
    }

    /// Creates a saturate color matrix filter with the given factor between 0 and 1.
    ///
    /// See the `saturate` attribute of the `feColorMatrix` element in the SVG specification.
    pub fn saturate(saturation: f32) -> ColorMatrix {
        let a = ColorMatrix::from_rows([
            [ 0.213,  0.715,  0.072, 0.0, 0.0],
            [ 0.213,  0.715,  0.072, 0.0, 0.0],
            [ 0.213,  0.715,  0.072, 0.0, 0.0],
            [ 0.0,    0.0,    0.0,   1.0, 0.0],
        ]);
        let b = ColorMatrix::from_rows([
            [ 0.787, -0.715, -0.072, 0.0, 0.0],
            [-0.213,  0.285, -0.072, 0.0, 0.0],
            [-0.213, -0.715,  0.928, 0.0, 0.0],
            [ 0.0,    0.0,    0.0,   0.0, 0.0],
        ]);
        a + b * saturation
    }

    /// Creates a luminance-to-alpha color matrix filter.
    ///
    /// See the `luminanceToAlpha` attribute of the `feColorMatrix` element in the SVG
    /// specification.
    pub fn luminance_to_alpha() -> ColorMatrix {
        ColorMatrix::from_rows([
            [ 0.0,    0.0,    0.0,    0.0, 0.0],
            [ 0.0,    0.0,    0.0,    0.0, 0.0],
            [ 0.0,    0.0,    0.0,    0.0, 0.0],
            [ 0.2125, 0.7154, 0.0721, 0.0, 0.0],
        ])
    }
}
impl Deref for ColorMatrix {
    type Target = [F32x4; 5];

    #[inline]
    fn deref(&self) -> &[F32x4; 5] {
        &self.0
    }
}
impl Add for ColorMatrix {
    type Output = ColorMatrix;

    #[inline]
    fn add(self, rhs: ColorMatrix) -> ColorMatrix {
        ColorMatrix([
            self[0] + rhs[0],
            self[1] + rhs[1],
            self[2] + rhs[2],
            self[3] + rhs[3],
            self[4] + rhs[4],
        ])
    }
}

impl Mul<f32> for ColorMatrix {
    type Output = ColorMatrix;

    #[inline]
    fn mul(self, rhs: f32) -> ColorMatrix {
        let rhs = F32x4::splat(rhs);
        ColorMatrix([
            self[0] * rhs,
            self[1] * rhs,
            self[2] * rhs,
            self[3] * rhs,
            self[4] * rhs,
        ])
    }
}
